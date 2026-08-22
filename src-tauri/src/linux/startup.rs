//! XDG autostart application enumeration and toggling (Linux).
//!
//! Startup apps live as `.desktop` files in two autostart directories:
//!
//! - user: `$XDG_CONFIG_HOME/autostart` (`~/.config/autostart`)
//! - system: `$XDG_CONFIG_DIRS/autostart` (`/etc/xdg/autostart`)
//!
//! A user file with the same name shadows the system one. Per the spec,
//! `Hidden=true` disables an entry; to disable a *system-wide* entry, a user
//! override with the same name + `Hidden=true` is created. Optix follows that
//! mechanism for both scopes: toggling writes/removes the `Hidden` key in the
//! user file (creating a shadow copy for system entries), which is fully
//! reversible and never deletes the original.

#[cfg(target_os = "linux")]
use std::path::PathBuf;

use crate::error::{OptixError, Result};
use crate::models::services::StartupEntry;

#[cfg(target_os = "linux")]
fn autostart_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    // User dir (most important, shadows system entries).
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")));
    if let Some(dir) = config_home {
        out.push(dir.join("autostart"));
    }
    // System dir(s).
    if let Some(dirs) = std::env::var_os("XDG_CONFIG_DIRS") {
        for dir in std::env::split_paths(&dirs) {
            out.push(dir.join("autostart"));
        }
    }
    out.push(PathBuf::from("/etc/xdg/autostart"));
    out
}

/// Parse a `.desktop` file's `[Desktop Entry]` section into its keys.
#[cfg(target_os = "linux")]
fn parse_desktop(text: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let mut in_desktop_entry = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_desktop_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_desktop_entry || line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            map.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    map
}

/// Whether an autostart entry should actually start, honoring `Hidden=` and
/// the `X-GNOME-Autostart-enabled` override GNOME writes.
#[cfg(target_os = "linux")]
fn is_enabled(keys: &std::collections::HashMap<String, String>) -> bool {
    if keys.get("Hidden").map(|v| v.eq_ignore_ascii_case("true")).unwrap_or(false) {
        return false;
    }
    if let Some(v) = keys.get("X-GNOME-Autostart-enabled") {
        return v.eq_ignore_ascii_case("true");
    }
    true
}

/// Enumerate XDG autostart entries (user + system). User entries shadow
/// system entries with the same filename; both are listed, with the shadowed
/// system copy marked non-toggleable.
#[cfg(target_os = "linux")]
pub fn list_entries() -> Vec<StartupEntry> {
    let mut out = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    // The first autostart dir is always the user-scope one (shadows the rest).
    let user_dir = autostart_dirs().first().cloned().unwrap_or_default();

    for dir in autostart_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let is_user_dir = dir == user_dir;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let keys = parse_desktop(&text);
            if keys.get("Type").map(|t| t != "Application").unwrap_or(false) {
                continue;
            }
            let name = keys.get("Name").cloned().unwrap_or_else(|| {
                path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
            });
            let command = keys.get("Exec").cloned().unwrap_or_default();
            let enabled = is_enabled(&keys);

            // Same filename in a more important dir already handled it.
            let toggleable = if seen.contains(&file_name) {
                false
            } else {
                seen.insert(file_name.clone());
                true
            };

            out.push(StartupEntry {
                id: path.to_string_lossy().into_owned(),
                name,
                command,
                location: path.to_string_lossy().into_owned(),
                source: if is_user_dir { "xdg_user" } else { "xdg_system" }.to_string(),
                enabled,
                toggleable,
            });
        }
    }
    out
}

/// Toggle an XDG autostart entry. Returns the path that was changed.
///
/// - System entries (`/etc/xdg/...`): never edit the original. Per the XDG
///   autostart spec, a user-shadow copy with the same filename containing
///   `Hidden=true` disables it; re-enabling removes that shadow.
/// - User entries: edit the file in place, flipping only the `Hidden` key and
///   preserving every other line (Icon, Comment, TryExec, …).
#[cfg(target_os = "linux")]
pub fn set_enabled(location: &str, enabled: bool) -> Result<String> {
    let path = PathBuf::from(location);

    // System scope: write/remove the user shadow copy.
    if location.starts_with("/etc/xdg/") {
        let mut user_dir = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config"))
            .join("autostart");
        let file_name = path.file_name().ok_or_else(|| {
            OptixError::InvalidState(format!("bad autostart path: {location}"))
        })?;
        user_dir.push(file_name);

        if enabled {
            // Re-enable: remove the shadow copy, the system entry takes over.
            if user_dir.is_file() {
                std::fs::remove_file(&user_dir)?;
            }
            return Ok(user_dir.to_string_lossy().into_owned());
        }
        // Disable: write a minimal shadow that overrides to Hidden=true. The
        // original system file is never modified.
        std::fs::create_dir_all(user_dir.parent().ok_or_else(|| {
            OptixError::InvalidState(format!("bad autostart path: {location}"))
        })?)?;
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Err(OptixError::InvalidState(format!(
                "cannot read autostart file: {location}"
            )));
        };
        let keys = parse_desktop(&text);
        let mut out = String::from("[Desktop Entry]\nType=Application\n");
        if let Some(name) = keys.get("Name") {
            out.push_str(&format!("Name={name}\n"));
        }
        if let Some(exec) = keys.get("Exec") {
            out.push_str(&format!("Exec={exec}\n"));
        }
        if let Some(only) = keys.get("OnlyShowIn") {
            out.push_str(&format!("OnlyShowIn={only}\n"));
        }
        out.push_str("Hidden=true\n");
        std::fs::write(&user_dir, out)?;
        return Ok(user_dir.to_string_lossy().into_owned());
    }

    // User scope: flip the Hidden key in place, keeping everything else.
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Err(OptixError::InvalidState(format!(
            "cannot read autostart file: {location}"
        )));
    };
    let rewritten = set_hidden(&text, enabled);
    std::fs::write(&path, rewritten)?;
    Ok(location.to_string())
}

/// Set the `Hidden` key in a desktop file to `true`/`false`, preserving all
/// other lines and their order. Pure function — unit-testable.
///
/// Enabling also strips an `X-GNOME-Autostart-enabled=false` override: GNOME
/// removes that key when re-enabling, and leaving it would defeat the toggle
/// (`is_enabled` treats it as authoritative).
#[cfg(target_os = "linux")]
fn set_hidden(text: &str, hidden: bool) -> String {
    let value = if hidden { "true" } else { "false" };
    let mut in_desktop_entry = false;
    let mut replaced = false;
    let mut out = String::with_capacity(text.len() + 16);
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_desktop_entry = trimmed == "[Desktop Entry]";
        } else if in_desktop_entry && trimmed.starts_with("Hidden=") {
            // Reuse the original indentation, swap the value.
            let indent = &line[..line.len() - line.trim_start().len()];
            out.push_str(&format!("{indent}Hidden={value}\n"));
            replaced = true;
            continue;
        } else if in_desktop_entry && !hidden && trimmed.starts_with("X-GNOME-Autostart-enabled=") {
            // Re-enabling: drop the GNOME override so the entry starts again.
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    if !replaced && in_desktop_entry {
        // No Hidden key yet — append one.
        out.push_str(&format!("Hidden={value}\n"));
    }
    out
}

#[cfg(not(target_os = "linux"))]
pub fn list_entries() -> Vec<StartupEntry> {
    Vec::new()
}

#[cfg(not(target_os = "linux"))]
pub fn set_enabled(_location: &str, _enabled: bool) -> Result<String> {
    Err(OptixError::UnsupportedPlatform("startup apps".into()))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn parses_desktop_entry_keys() {
        let text = "[Desktop Entry]\nType=Application\nName=Foo Bar\nExec=/usr/bin/foo --flag\nHidden=false\n";
        let keys = parse_desktop(text);
        assert_eq!(keys.get("Name").map(String::as_str), Some("Foo Bar"));
        assert_eq!(keys.get("Exec").map(String::as_str), Some("/usr/bin/foo --flag"));
    }

    #[test]
    fn honors_hidden_and_gnome_override() {
        let mut keys = std::collections::HashMap::new();
        keys.insert("Hidden".to_string(), "true".to_string());
        assert!(!is_enabled(&keys));

        keys.clear();
        keys.insert("Hidden".to_string(), "false".to_string());
        assert!(is_enabled(&keys));

        // GNOME writes X-GNOME-Autostart-enabled=false when the user disables
        // via GNOME Tweaks; treat it as authoritative.
        keys.clear();
        keys.insert("X-GNOME-Autostart-enabled".to_string(), "false".to_string());
        assert!(!is_enabled(&keys));

        keys.clear();
        keys.insert("X-GNOME-Autostart-enabled".to_string(), "true".to_string());
        assert!(is_enabled(&keys));
    }

    #[test]
    fn ignores_non_application_types() {
        let text = "[Desktop Entry]\nType=Link\nName=Shortcut\n";
        let keys = parse_desktop(&text);
        assert_eq!(keys.get("Type").map(String::as_str), Some("Link"));
    }

    #[test]
    fn enabling_strips_gnome_disable_override() {
        let text = "[Desktop Entry]\nType=Application\nName=Dropbox\nExec=/opt/dropbox\nX-GNOME-Autostart-enabled=false\n";
        let rewritten = set_hidden(text, false);
        assert!(!rewritten.contains("X-GNOME-Autostart-enabled"));
        assert!(rewritten.contains("Hidden=false"));
        // The rewritten file must actually read back as enabled.
        assert!(is_enabled(&parse_desktop(&rewritten)));
    }

    #[test]
    fn disabling_adds_hidden_and_keeps_gnome_override() {
        let text = "[Desktop Entry]\nType=Application\nName=Dropbox\nExec=/opt/dropbox\nX-GNOME-Autostart-enabled=false\n";
        let rewritten = set_hidden(text, true);
        assert!(rewritten.contains("X-GNOME-Autostart-enabled=false"));
        assert!(rewritten.contains("Hidden=true"));
        assert!(!is_enabled(&parse_desktop(&rewritten)));
    }
}
