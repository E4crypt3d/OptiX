//! systemd service enumeration and control (Linux).
//!
//! systemd has no single "list everything with description + exec" call that
//! is stable across versions, so enumeration combines two `systemctl`
//! invocations:
//!
//! - `list-unit-files --type=service` → the enabled/disabled/static/masked
//!   state (the "start type" equivalent).
//! - `list-units --type=service --all` → the live state (running/stopped) and
//!   the human-readable description.
//!
//! Start/stop/enable/disable shell out to `systemctl`; user-scope units are
//! queried with `--user` (no root needed). System-scope mutations require
//! root or a polkit agent — failures surface as a normal error to the UI.

#[cfg(target_os = "linux")]
use std::collections::HashMap;
#[cfg(target_os = "linux")]
use std::process::Command;

use crate::error::{OptixError, Result};
use crate::models::services::ServiceInfo;
use crate::models::snapshot::ChangeRecord;

/// Run a `systemctl` command, returning trimmed stdout. `scope` is
/// `Some("--user")` for the user manager, `None` for the system manager.
#[cfg(target_os = "linux")]
fn systemctl(scope: Option<&str>, args: &[&str]) -> Option<String> {
    let mut cmd = Command::new("systemctl");
    if let Some(scope) = scope {
        cmd.arg(scope);
    }
    cmd.args(args).arg("--no-legend").arg("--no-pager").arg("--plain");
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// `systemctl list-unit-files --type=service`: lines are `NAME STATE`.
/// The state column is the boot-enablement ("start type" equivalent):
/// enabled, disabled, static, masked, indirect, generated, alias.
#[cfg(target_os = "linux")]
fn parse_unit_files(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut fields = line.split_whitespace();
        let (Some(name), Some(state)) = (fields.next(), fields.next()) else {
            continue;
        };
        if !name.ends_with(".service") {
            continue;
        }
        out.push((name.to_string(), state.to_string()));
    }
    out
}

/// `systemctl list-units --type=service --all`: columns are
/// `UNIT LOAD ACTIVE SUB DESCRIPTION`. The ACTIVE column is
/// active/inactive/failed/activating/deactivating.
#[cfg(target_os = "linux")]
fn parse_units(text: &str) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut fields = line.split_whitespace();
        let (Some(name), Some(_load), Some(active), Some(_sub)) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        if !name.ends_with(".service") {
            continue;
        }
        let description = fields.collect::<Vec<_>>().join(" ");
        out.push((name.to_string(), active.to_string(), description));
    }
    out
}

/// Enumerate systemd services (system + user scope).
#[cfg(target_os = "linux")]
pub fn list_services() -> Vec<ServiceInfo> {
    let mut out = Vec::new();

    // System scope first.
    if let Some(text) = systemctl(None, &["list-unit-files", "--type=service"]) {
        let states: HashMap<String, String> = parse_unit_files(&text).into_iter().collect();
        if let Some(text) = systemctl(None, &["list-units", "--type=service", "--all"]) {
            for (name, active, description) in parse_units(&text) {
                out.push(from_systemd(&name, &active, &description, states.get(&name).map(String::as_str).unwrap_or("unknown"), "system"));
            }
        }
    }

    // User scope (best-effort; fails cleanly when there's no user manager).
    if let Some(text) = systemctl(Some("--user"), &["list-unit-files", "--type=service"]) {
        let states: HashMap<String, String> = parse_unit_files(&text).into_iter().collect();
        if let Some(text) = systemctl(Some("--user"), &["list-units", "--type=service", "--all"]) {
            for (name, active, description) in parse_units(&text) {
                out.push(from_systemd(&name, &active, &description, states.get(&name).map(String::as_str).unwrap_or("unknown"), "user"));
            }
        }
    }

    out.sort_by(|a, b| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()));
    out
}

/// Map a systemd unit into the shared `ServiceInfo` model. The Windows start
/// types (auto/manual/disabled/boot/system) map to systemd's enablement
/// states as follows: enabled→auto, disabled→disabled, static→manual,
/// masked→masked, everything else→unknown.
#[cfg(target_os = "linux")]
fn from_systemd(name: &str, active: &str, description: &str, unit_file_state: &str, scope: &str) -> ServiceInfo {
    let state = match active {
        "active" => "running",
        "inactive" => "stopped",
        "failed" => "failed",
        "activating" => "starting",
        "deactivating" => "stopping",
        other => other,
    };
    let start_type = match unit_file_state {
        "enabled" => "auto",
        "disabled" => "disabled",
        "static" => "manual",
        "masked" => "masked",
        "indirect" => "manual",
        "generated" => "manual",
        other => other,
    };
    let binary_path = exec_start(name, scope).unwrap_or_default();
    ServiceInfo {
        name: name.to_string(),
        display_name: if description.is_empty() { name.to_string() } else { description.to_string() },
        description: description.to_string(),
        state: state.to_string(),
        start_type: start_type.to_string(),
        binary_path,
        is_driver: false,
        delayed_auto_start: false,
        // The run account: user-scope units run as the invoking user, system
        // units as root (the Linux equivalent of LocalSystem).
        account: if scope == "user" { whoami().unwrap_or_else(|| "user".into()) } else { "root".into() },
        classification: String::new(),
    }
}

/// Read the `ExecStart=` path from a unit file without shelling out per-unit
/// (`systemctl show` is one process per unit — too slow for hundreds).
/// Searches the standard system + user unit directories.
#[cfg(target_os = "linux")]
fn exec_start(name: &str, scope: &str) -> Option<String> {
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    if scope == "user" {
        if let Some(home) = std::env::var_os("HOME") {
            dirs.push(std::path::PathBuf::from(&home).join(".config/systemd/user"));
            dirs.push(std::path::PathBuf::from(&home).join(".local/share/systemd/user"));
        }
        dirs.push(std::path::PathBuf::from("/etc/systemd/user"));
        dirs.push(std::path::PathBuf::from("/usr/lib/systemd/user"));
        dirs.push(std::path::PathBuf::from("/usr/local/lib/systemd/user"));
    } else {
        dirs.push(std::path::PathBuf::from("/etc/systemd/system"));
        dirs.push(std::path::PathBuf::from("/run/systemd/system"));
        dirs.push(std::path::PathBuf::from("/usr/lib/systemd/system"));
        dirs.push(std::path::PathBuf::from("/usr/local/lib/systemd/system"));
    }
    for dir in dirs {
        let path = dir.join(name);
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        if let Some(exec) = parse_exec_start(&text) {
            return Some(exec);
        }
    }
    None
}

/// Pull the executable out of a unit file's `[Service] ExecStart=` line.
/// Handles `-` (ignore errors) and `@` (argv0 override) prefixes and quoted
/// paths. Pure parse — unit-testable.
#[cfg(target_os = "linux")]
fn parse_exec_start(unit: &str) -> Option<String> {
    let mut in_service = false;
    for line in unit.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_service = line == "[Service]";
            continue;
        }
        if !in_service || !line.to_ascii_lowercase().starts_with("execstart=") {
            continue;
        }
        let mut value = line["ExecStart=".len()..].trim().to_string();
        // Strip `-` (ignore failure) and `@` (argv[0] override) prefixes.
        value = value.trim_start_matches('-').trim().to_string();
        if let Some(rest) = value.strip_prefix('@') {
            value = rest.trim().to_string();
        }
        // Quoted path first (e.g. `/usr/bin/foo "bar baz"`).
        if let Some(end) = value.find('"') {
            value = value[..end].trim().to_string();
        }
        // First whitespace-delimited token.
        let first = value.split_whitespace().next().unwrap_or("").trim_matches('"').to_string();
        if !first.is_empty() {
            return Some(first);
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn whoami() -> Option<String> {
    let output = Command::new("id").arg("-un").output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Start a unit (system scope first, user scope fallback).
#[cfg(target_os = "linux")]
pub fn start_service(name: &str) -> Result<()> {
    if run_unit_action(None, "start", name).is_ok() {
        return Ok(());
    }
    run_unit_action(Some("--user"), "start", name)
}

/// Stop a unit.
#[cfg(target_os = "linux")]
pub fn stop_service(name: &str) -> Result<()> {
    if run_unit_action(None, "stop", name).is_ok() {
        return Ok(());
    }
    run_unit_action(Some("--user"), "stop", name)
}

/// Change boot-enablement: `auto` → enable, `disabled` → disable. `manual`
/// (systemd `static`) can't be toggled from here — the engine's guard already
/// classifies those as required.
#[cfg(target_os = "linux")]
pub fn set_start_type(name: &str, start_type: &str) -> Result<()> {
    let action = match start_type {
        "auto" => "enable",
        "disabled" => "disable",
        other => {
            return Err(OptixError::InvalidState(format!(
                "start type {other} is not settable for systemd units"
            )))
        }
    };
    if run_unit_action(None, action, name).is_ok() {
        return Ok(());
    }
    run_unit_action(Some("--user"), action, name)
}

#[cfg(target_os = "linux")]
fn run_unit_action(scope: Option<&str>, action: &str, name: &str) -> Result<()> {
    let mut cmd = Command::new("systemctl");
    if let Some(scope) = scope {
        cmd.arg(scope);
    }
    cmd.arg(action).arg(name).arg("--no-pager");
    let output = cmd.output().map_err(|e| OptixError::Other(format!("systemctl {action}: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(OptixError::Other(format!(
            "systemctl {action} {name}: {stderr}"
        )));
    }
    Ok(())
}

/// Roll back a `service`-domain change recorded by the engine.
#[cfg(target_os = "linux")]
pub fn rollback_service(change: &ChangeRecord) -> Result<()> {
    match change.kind.as_str() {
        "stop" => start_service(&change.location),
        "start" => stop_service(&change.location),
        "set_start_type" => {
            let old = change.old_value.as_deref().ok_or_else(|| {
                OptixError::InvalidState("no previous start type recorded".into())
            })?;
            set_start_type(&change.location, old)
        }
        other => Err(OptixError::InvalidState(format!(
            "unknown service change kind: {other}"
        ))),
    }
}

#[cfg(not(target_os = "linux"))]
pub fn list_services() -> Vec<ServiceInfo> {
    Vec::new()
}

#[cfg(not(target_os = "linux"))]
pub fn start_service(_name: &str) -> Result<()> {
    Err(OptixError::UnsupportedPlatform("services".into()))
}

#[cfg(not(target_os = "linux"))]
pub fn stop_service(_name: &str) -> Result<()> {
    Err(OptixError::UnsupportedPlatform("services".into()))
}

#[cfg(not(target_os = "linux"))]
pub fn set_start_type(_name: &str, _start_type: &str) -> Result<()> {
    Err(OptixError::UnsupportedPlatform("services".into()))
}

#[cfg(not(target_os = "linux"))]
pub fn rollback_service(_change: &ChangeRecord) -> Result<()> {
    Err(OptixError::UnsupportedPlatform("service rollback".into()))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn parses_unit_file_list() {
        let text = "accounts-daemon.service                enabled\nalsa-restore.service                 static\nalsa-utils.service                   masked\nsshd.service                         disabled\n";
        let units = parse_unit_files(text);
        assert_eq!(units.len(), 4);
        assert_eq!(units[0], ("accounts-daemon.service".to_string(), "enabled".to_string()));
        assert_eq!(units[2], ("alsa-utils.service".to_string(), "masked".to_string()));
    }

    #[test]
    fn parses_unit_list() {
        let text = "accounts-daemon.service  loaded  active   running Accounts Service\nalsa-state.service       loaded  inactive dead    Manage Sound Card State\n";
        let units = parse_units(text);
        assert_eq!(units.len(), 2);
        assert_eq!(units[0], ("accounts-daemon.service".to_string(), "active".to_string(), "Accounts Service".to_string()));
        assert_eq!(units[1].1, "inactive");
    }

    #[test]
    fn maps_systemd_states_into_model() {
        let info = from_systemd("sshd.service", "active", "OpenSSH Daemon", "enabled", "system");
        assert_eq!(info.state, "running");
        assert_eq!(info.start_type, "auto");
        assert_eq!(info.account, "root");
        assert_eq!(info.is_driver, false);

        let masked = from_systemd("x.service", "inactive", "", "masked", "system");
        assert_eq!(masked.start_type, "masked");
        assert_eq!(masked.state, "stopped");
    }

    #[test]
    fn extracts_exec_from_unit_files() {
        let unit = "[Unit]\nDescription=Test\n[Service]\nType=simple\nExecStart=/usr/bin/foo --bar \"baz qux\"\n";
        assert_eq!(parse_exec_start(unit), Some("/usr/bin/foo".to_string()));

        // `-` ignore-error prefix and quoted path with args.
        let unit2 = "[Service]\nExecStart=-/opt/app/bin/daemon -c /etc/app.conf\n";
        assert_eq!(parse_exec_start(unit2), Some("/opt/app/bin/daemon".to_string()));

        // `@` argv0 override.
        let unit3 = "[Service]\nExecStart=@/usr/lib/exec/foo --flag\n";
        assert_eq!(parse_exec_start(unit3), Some("/usr/lib/exec/foo".to_string()));

        // Exec in a section other than [Service] is ignored.
        let unit4 = "[Unit]\nExecStart=/bad/path\n[Service]\nExecStart=/good/path\n";
        assert_eq!(parse_exec_start(unit4), Some("/good/path".to_string()));

        // No ExecStart at all.
        assert_eq!(parse_exec_start("[Service]\nType=oneshot\n"), None);
    }
}
