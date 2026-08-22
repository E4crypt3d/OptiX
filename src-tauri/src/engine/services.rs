//! Phase 6 — Startup & Service Manager.
//!
//! Classifies services into REQUIRED / SAFE / UNKNOWN and applies reversible
//! mutations (stop/start/start-type, the Windows Search toggle, and startup
//! entry enable/disable) via the snapshot + rollback engine.

use crate::db::sqlite::Database;
use crate::engine::{rollback, snapshot};
use crate::error::{OptixError, Result};
use crate::models::services::{
    ScheduledTask, ServiceActionResult, ServiceInfo, StartupActionResult, StartupEntry,
    WSearchStatus,
};
use crate::models::snapshot::ChangeRecord;
#[cfg(windows)]
use crate::win;
#[cfg(target_os = "linux")]
use crate::linux;

/// Services Optix must never stop or disable (drivers, boot/system-start and
/// system-critical services are additionally protected in `classify`).
/// Windows-specific names; the Linux list lives below.
#[cfg(windows)]
const NEVER_FLAG: &[&str] = &[
    "windefend",
    "wscsvc",
    "wuauserv",
    "securityhealthservice",
    "sppsvc",
    "appidsvc",
    "rpcss",
    "dcomlaunch",
    "rpcendpointmapper",
    "lsass",
    "samss",
    "netlogon",
    "keyiso",
    "vaultsvc",
    "cryptsvc",
    "gpsvc",
    "clipsvc",
    "appmodel",
    "lsm",
    "coremessagingregistrar",
    "brokerinfrastructure",
    "systemeventsbroker",
    "staterepository",
    "tiledatamodelsvc",
    "power",
    "plugplay",
    "schedule",
    "dhcp",
    "dnscache",
    "nlasvc",
    "netprofm",
    "bfe",
    "mpssvc",
    "eventlog",
    "audiosrv",
    "audioendpointbuilder",
    "mmcss",
    "wlansvc",
];

/// Critical systemd units that must never be stopped or disabled. These are
/// the systemd-resident core (PID-1's own services, D-Bus, login, and the
/// network stack) — stopping any of them bricks the session.
#[cfg(target_os = "linux")]
const LINUX_NEVER_FLAG: &[&str] = &[
    "systemd",
    "systemd-journald",
    "systemd-logind",
    "systemd-udevd",
    "systemd-networkd",
    "systemd-resolved",
    "systemd-timesyncd",
    "systemd-tmpfiles-setup",
    "systemd-tmpfiles-setup-dev",
    "systemd-user-sessions",
    "systemd-remount-fs",
    "systemd-sysctl",
    "systemd-modules-load",
    "dbus",
    "dbus-broker",
    "dbus-daemon",
    "polkit",
    "getty",
    "login",
    "user",
];

/// Services commonly safe to disable for gaming (user confirmation still
/// required in the UI).
#[cfg(windows)]
const SAFE: &[&str] = &[
    "sysmain", // Superfetch — often flagged as a RAM hog
    "diagtrack", // Connected User Experiences and Telemetry
    "dosvc",     // Delivery Optimization
    "xblauthmanager",
    "xboxgipsvc",
    "xblgamesave",
    "xboxnetapisvc",
    "wsearch", // Windows Search (dedicated toggle)
];

/// Classify a service as "required" | "safe" | "unknown".
pub fn classify(info: &ServiceInfo) -> &'static str {
    let name = info.name.to_ascii_lowercase();
    #[cfg(windows)]
    {
        if info.is_driver
            || info.start_type == "boot"
            || info.start_type == "system"
            || NEVER_FLAG.contains(&name.as_str())
        {
            return "required";
        }
        if SAFE.contains(&name.as_str()) {
            return "safe";
        }
        return "unknown";
    }
    #[cfg(target_os = "linux")]
    {
        // Masked units can't be re-enabled from the UI; systemd-resident and
        // core session units are never touchable.
        if info.start_type == "masked"
            || LINUX_NEVER_FLAG.contains(&name.as_str())
            || name.starts_with("systemd-")
            && !matches!(info.state.as_str(), "stopped")
        {
            return "required";
        }
        // Nothing is auto-classified "safe" on Linux — the SAFE list is
        // Windows-specific and a wrong guess is worse than an unknown.
        "unknown"
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = name;
        "unknown"
    }
}

/// Enumerate services with classification applied (platform-dispatched).
#[cfg(windows)]
pub fn list_services() -> Vec<ServiceInfo> {
    let mut out = win::services::list_services();
    for s in &mut out {
        s.classification = classify(s).to_string();
    }
    out.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
    });
    out
}

#[cfg(target_os = "linux")]
pub fn list_services() -> Vec<ServiceInfo> {
    let mut out = linux::services::list_services();
    for s in &mut out {
        s.classification = classify(s).to_string();
    }
    out.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
    });
    out
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn list_services() -> Vec<ServiceInfo> {
    Vec::new()
}

fn find_service(name: &str) -> Option<ServiceInfo> {
    list_services().into_iter().find(|s| s.name == name)
}

/// Stop a running service (snapshot-first, reversible).
#[cfg(windows)]
pub fn stop_service(db: &Database, name: &str) -> Result<ServiceActionResult> {
    stop_service_impl(db, name, |n| win::services::stop_service(n))
}

#[cfg(target_os = "linux")]
pub fn stop_service(db: &Database, name: &str) -> Result<ServiceActionResult> {
    stop_service_impl(db, name, |n| linux::services::stop_service(n))
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn stop_service(_db: &Database, _name: &str) -> Result<ServiceActionResult> {
    Err(OptixError::UnsupportedPlatform("services".into()))
}

fn stop_service_impl(
    db: &Database,
    name: &str,
    stop: impl FnOnce(&str) -> Result<()>,
) -> Result<ServiceActionResult> {
    let info = find_service(name)
        .ok_or_else(|| OptixError::InvalidState(format!("service not found: {name}")))?;
    guard_mutable(&info)?;
    if info.state != "running" {
        return Ok(ServiceActionResult {
            snapshot_id: String::new(),
            changes: 0,
        });
    }
    let snap = snapshot::create_lightweight(db, &format!("Stop service: {name}"), None)?;
    stop(name)?;
    record(
        db,
        &snap.id,
        "service",
        "stop",
        name,
        Some("running"),
        Some("stopped"),
    )?;
    Ok(ServiceActionResult {
        snapshot_id: snap.id,
        changes: 1,
    })
}

/// Start a stopped service (snapshot-first, reversible).
#[cfg(windows)]
pub fn start_service(db: &Database, name: &str) -> Result<ServiceActionResult> {
    start_service_impl(db, name, |n| win::services::start_service(n))
}

#[cfg(target_os = "linux")]
pub fn start_service(db: &Database, name: &str) -> Result<ServiceActionResult> {
    start_service_impl(db, name, |n| linux::services::start_service(n))
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn start_service(_db: &Database, _name: &str) -> Result<ServiceActionResult> {
    Err(OptixError::UnsupportedPlatform("services".into()))
}

fn start_service_impl(
    db: &Database,
    name: &str,
    start: impl FnOnce(&str) -> Result<()>,
) -> Result<ServiceActionResult> {
    let info = find_service(name)
        .ok_or_else(|| OptixError::InvalidState(format!("service not found: {name}")))?;
    if info.is_driver {
        return Err(OptixError::NotPermitted(format!(
            "service {name} is a driver"
        )));
    }
    if info.state != "stopped" {
        return Ok(ServiceActionResult {
            snapshot_id: String::new(),
            changes: 0,
        });
    }
    let snap = snapshot::create_lightweight(db, &format!("Start service: {name}"), None)?;
    start(name)?;
    record(
        db,
        &snap.id,
        "service",
        "start",
        name,
        Some("stopped"),
        Some("running"),
    )?;
    Ok(ServiceActionResult {
        snapshot_id: snap.id,
        changes: 1,
    })
}

/// Change a service's start type (`auto` | `manual` | `disabled`;
/// `auto`/`disabled` on Linux). Snapshot-first, reversible.
#[cfg(windows)]
pub fn set_start_type(db: &Database, name: &str, start_type: &str) -> Result<ServiceActionResult> {
    let value = crate::models::services::start_type_value(start_type)
        .ok_or_else(|| OptixError::InvalidState(format!("invalid start type: {start_type}")))?;
    set_start_type_impl(db, name, start_type, |n, _| win::services::set_start_type(n, value))
}

#[cfg(target_os = "linux")]
pub fn set_start_type(db: &Database, name: &str, start_type: &str) -> Result<ServiceActionResult> {
    set_start_type_impl(db, name, start_type, |n, v| linux::services::set_start_type(n, v))
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn set_start_type(_db: &Database, _name: &str, _start_type: &str) -> Result<ServiceActionResult> {
    Err(OptixError::UnsupportedPlatform("services".into()))
}

/// Shared start-type change: validate, snapshot, apply, record.
fn set_start_type_impl(
    db: &Database,
    name: &str,
    start_type: &str,
    apply: impl FnOnce(&str, &str) -> Result<()>,
) -> Result<ServiceActionResult> {
    let info = find_service(name)
        .ok_or_else(|| OptixError::InvalidState(format!("service not found: {name}")))?;
    guard_mutable(&info)?;
    if info.start_type == start_type {
        return Ok(ServiceActionResult {
            snapshot_id: String::new(),
            changes: 0,
        });
    }
    let snap = snapshot::create_lightweight(db, &format!("Service start type: {name}"), None)?;
    apply(name, start_type)?;
    record(
        db,
        &snap.id,
        "service",
        "set_start_type",
        name,
        Some(&info.start_type),
        Some(start_type),
    )?;
    Ok(ServiceActionResult {
        snapshot_id: snap.id,
        changes: 1,
    })
}

/// Refuse to mutate REQUIRED services.
fn guard_mutable(info: &ServiceInfo) -> Result<()> {
    if classify(info) == "required" {
        return Err(OptixError::NotPermitted(format!(
            "service {} is required and cannot be modified",
            info.name
        )));
    }
    Ok(())
}

/// Current Windows Search (WSearch) state — a Windows-only service.
#[cfg(windows)]
pub fn wsearch_status() -> WSearchStatus {
    match find_service("WSearch") {
        Some(info) => WSearchStatus {
            enabled: info.start_type != "disabled",
            running: info.state == "running",
            start_type: info.start_type,
        },
        None => WSearchStatus {
            enabled: false,
            running: false,
            start_type: "unknown".to_string(),
        },
    }
}

#[cfg(not(windows))]
pub fn wsearch_status() -> WSearchStatus {
    WSearchStatus {
        enabled: false,
        running: false,
        start_type: "unknown".to_string(),
    }
}

/// Enable/disable Windows Search: set start type and stop/start as needed.
#[cfg(windows)]
pub fn set_wsearch(db: &Database, enabled: bool) -> Result<ServiceActionResult> {
    let info = find_service("WSearch")
        .ok_or_else(|| OptixError::InvalidState("WSearch service not found".into()))?;
    let currently_enabled = info.start_type != "disabled";
    if enabled == currently_enabled {
        return Ok(ServiceActionResult {
            snapshot_id: String::new(),
            changes: 0,
        });
    }

    let snap = snapshot::create_lightweight(
        db,
        if enabled { "Enable Windows Search" } else { "Disable Windows Search" },
        None,
    )?;
    let mut changes = 0usize;

    if enabled {
        win::services::set_start_type("WSearch", 2)?;
        record(
            db,
            &snap.id,
            "service",
            "set_start_type",
            "WSearch",
            Some(&info.start_type),
            Some("auto"),
        )?;
        changes += 1;
        if info.state != "running" {
            win::services::start_service("WSearch")?;
            record(db, &snap.id, "service", "start", "WSearch", Some("stopped"), Some("running"))?;
            changes += 1;
        }
    } else {
        if info.state == "running" {
            win::services::stop_service("WSearch")?;
            record(db, &snap.id, "service", "stop", "WSearch", Some("running"), Some("stopped"))?;
            changes += 1;
        }
        win::services::set_start_type("WSearch", 4)?;
        record(
            db,
            &snap.id,
            "service",
            "set_start_type",
            "WSearch",
            Some(&info.start_type),
            Some("disabled"),
        )?;
        changes += 1;
    }

    Ok(ServiceActionResult {
        snapshot_id: snap.id,
        changes,
    })
}

#[cfg(not(windows))]
pub fn set_wsearch(_db: &Database, _enabled: bool) -> Result<ServiceActionResult> {
    Err(OptixError::UnsupportedPlatform("Windows Search".into()))
}

/// Enumerate scheduled tasks. On Windows the action executable's Authenticode
/// signature state is flagged (trusted/untrusted/unsigned); Linux timers and
/// cron entries have no equivalent and report "unavailable".
#[cfg(windows)]
pub fn list_scheduled_tasks() -> Vec<ScheduledTask> {
    let mut tasks = win::tasks::list_scheduled_tasks();
    for t in &mut tasks {
        t.signature = task_signature(t);
    }
    // Non-Microsoft/system tasks first (the interesting ones).
    tasks.sort_by(|a, b| {
        let a_sys = a.signature == "trusted";
        let b_sys = b.signature == "trusted";
        a_sys.cmp(&b_sys).then_with(|| a.name.cmp(&b.name))
    });
    tasks
}

#[cfg(target_os = "linux")]
pub fn list_scheduled_tasks() -> Vec<ScheduledTask> {
    let mut tasks = linux::tasks::list_scheduled_tasks();
    tasks.sort_by(|a, b| a.name.cmp(&b.name));
    tasks
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn list_scheduled_tasks() -> Vec<ScheduledTask> {
    Vec::new()
}

/// Resolve the executable from a task action and verify its signature.
#[cfg(windows)]
fn task_signature(task: &ScheduledTask) -> String {
    let Some(exe) = action_executable(&task.action) else {
        return "unavailable".to_string();
    };
    // Only verify files that actually exist (task may point at a removed path).
    if !std::path::Path::new(&exe).is_file() {
        return "unavailable".to_string();
    }
    match win::signature::verify_file_signature(&exe) {
        Ok(state) => state.as_str().to_string(),
        Err(_) => "unavailable".to_string(),
    }
}

/// Pull the executable path out of a `schtasks` action string. Handles quoted
/// paths, bare exes, and `cmd.exe /c "..."` wrappers. Pure parse (no disk
/// check) so it is unit-testable; `task_signature` checks existence before
/// verifying.
#[cfg(any(windows, test))]
fn action_executable(action: &str) -> Option<String> {
    let mut s = action.trim().to_string();
    // Strip a leading `cmd.exe /c ...` wrapper.
    if let Some(idx) = s.to_ascii_lowercase().find("cmd.exe") {
        if let Some(rest) = s[idx..].split_once("/c").map(|(_, rest)| rest) {
            s = rest.to_string();
        }
    }
    let s = s.trim().trim_matches('"').to_string();
    // Quoted path (possibly with trailing arguments): first quoted segment.
    if let Some(end) = s.find('"') {
        let p = s[..end].trim().to_string();
        if is_exe_path(&p) {
            return Some(p);
        }
    }
    // Otherwise the first whitespace-delimited token.
    let first = s.split_whitespace().next().unwrap_or("").trim_matches('"').to_string();
    if is_exe_path(&first) {
        Some(first)
    } else {
        None
    }
}

#[cfg(any(windows, test))]
fn is_exe_path(p: &str) -> bool {
    p.to_ascii_lowercase().ends_with(".exe")
}

/// Enumerate startup applications (registry Run keys + folders on Windows;
/// XDG autostart on Linux).
#[cfg(windows)]
pub fn list_startup() -> Vec<StartupEntry> {
    win::startup::list_entries()
}

#[cfg(target_os = "linux")]
pub fn list_startup() -> Vec<StartupEntry> {
    linux::startup::list_entries()
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn list_startup() -> Vec<StartupEntry> {
    Vec::new()
}

/// Enable or disable a startup entry (snapshot-first, reversible).
/// Windows: registry Run value create/delete. Linux: `Hidden=` in the XDG
/// autostart file (shadow copy for system entries).
#[cfg(windows)]
pub fn set_startup_enabled(
    db: &Database,
    location: &str,
    enabled: bool,
    command: &str,
) -> Result<StartupActionResult> {
    if !(location.starts_with("HKLM\\") || location.starts_with("HKCU\\")) {
        return Err(OptixError::InvalidState(format!(
            "not a registry startup location: {location}"
        )));
    }
    let snap = snapshot::create_lightweight(
        db,
        if enabled { "Enable startup" } else { "Disable startup" },
        Some(location),
    )?;

    if enabled {
        win::registry::set_registry_value(location, command)?;
        record(
            db,
            &snap.id,
            "registry",
            "set",
            location,
            None,
            Some(command),
        )?;
    } else {
        win::registry::delete_registry_value(location)?;
        record(
            db,
            &snap.id,
            "registry",
            "delete",
            location,
            Some(command),
            None,
        )?;
    }

    Ok(StartupActionResult {
        snapshot_id: snap.id,
        changes: 1,
    })
}

#[cfg(target_os = "linux")]
pub fn set_startup_enabled(
    db: &Database,
    location: &str,
    enabled: bool,
    _command: &str,
) -> Result<StartupActionResult> {
    if !location.ends_with(".desktop") {
        return Err(OptixError::InvalidState(format!(
            "not an XDG autostart file: {location}"
        )));
    }
    let snap = snapshot::create_lightweight(
        db,
        if enabled { "Enable startup" } else { "Disable startup" },
        Some(location),
    )?;

    let changed = linux::startup::set_enabled(location, enabled)?;
    // Record the *original* file path (the one the user toggled) as the
    // location; the written file is what a rollback would remove.
    record(
        db,
        &snap.id,
        "startup",
        if enabled { "enable" } else { "disable" },
        location,
        Some(if enabled { "disabled" } else { "enabled" }),
        Some(changed.as_str()),
    )?;

    Ok(StartupActionResult {
        snapshot_id: snap.id,
        changes: 1,
    })
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn set_startup_enabled(
    _db: &Database,
    _location: &str,
    _enabled: bool,
    _command: &str,
) -> Result<StartupActionResult> {
    Err(OptixError::UnsupportedPlatform("startup apps".into()))
}

#[allow(clippy::too_many_arguments)]
fn record(
    db: &Database,
    snapshot_id: &str,
    domain: &str,
    kind: &str,
    location: &str,
    old_value: Option<&str>,
    new_value: Option<&str>,
) -> Result<()> {
    rollback::record_change(
        db,
        snapshot_id,
        ChangeRecord {
            id: None,
            snapshot_id: String::new(),
            domain: domain.to_string(),
            location: location.to_string(),
            kind: kind.to_string(),
            old_value: old_value.map(str::to_string),
            new_value: new_value.map(str::to_string),
            old_json: None,
            new_json: None,
            applied_at_ms: None,
            verified: true,
            rolled_back: false,
        },
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::services::ServiceInfo;

    fn svc(name: &str, is_driver: bool, start_type: &str) -> ServiceInfo {
        ServiceInfo {
            name: name.to_string(),
            display_name: String::new(),
            description: String::new(),
            state: "stopped".to_string(),
            start_type: start_type.to_string(),
            binary_path: String::new(),
            is_driver,
            delayed_auto_start: false,
            account: String::new(),
            classification: String::new(),
        }
    }

    #[cfg(windows)]
    #[test]
    fn drivers_and_boot_start_are_required() {
        assert_eq!(classify(&svc("somefilter", true, "auto")), "required");
        assert_eq!(classify(&svc("mydriver", false, "boot")), "required");
        assert_eq!(classify(&svc("mydriver", false, "system")), "required");
    }

    #[cfg(windows)]
    #[test]
    fn never_flag_list_is_required() {
        for name in ["WinDefend", "RpcSs", "Dhcp", "Power", "AudioSrv"] {
            assert_eq!(classify(&svc(name, false, "auto")), "required", "{name}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn safe_list_is_safe() {
        for name in ["SysMain", "DiagTrack", "WSearch", "XblAuthManager"] {
            assert_eq!(classify(&svc(name, false, "auto")), "safe", "{name}");
        }
    }

    #[test]
    fn unknown_defaults_to_unknown() {
        assert_eq!(classify(&svc("mygameservice", false, "auto")), "unknown");
        assert_eq!(classify(&svc("randomtool", false, "manual")), "unknown");
    }

    #[test]
    fn extracts_exe_from_task_actions() {
        // Quoted path with trailing arguments.
        assert_eq!(
            action_executable(r#""C:\Tools\helper.exe" --flag"#),
            Some("C:\\Tools\\helper.exe".to_string())
        );
        // cmd.exe wrapper.
        assert_eq!(
            action_executable(r#"cmd.exe /c "C:\Tools\x.exe" arg"#),
            Some("C:\\Tools\\x.exe".to_string())
        );
        // Bare token with arguments.
        assert_eq!(
            action_executable(r"C:\tools.exe -v"),
            Some("C:\\tools.exe".to_string())
        );
        // Missing/bogus actions resolve to nothing.
        assert_eq!(action_executable(""), None);
        assert_eq!(action_executable("0x9"), None);
    }
}
