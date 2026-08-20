//! Service & startup manager models (Phase 6).

use serde::Serialize;

/// A Windows service, enriched for the Startup & Service manager.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceInfo {
    pub name: String,
    pub display_name: String,
    pub description: String,
    /// "running" | "stopped" | "starting" | "stopping" | "paused" | "…".
    pub state: String,
    /// "auto" | "manual" | "disabled" | "boot" | "system".
    pub start_type: String,
    pub binary_path: String,
    pub is_driver: bool,
    pub delayed_auto_start: bool,
    /// Run account (e.g. "LocalSystem", "NT AUTHORITY\\NetworkService").
    pub account: String,
    /// "required" | "safe" | "unknown".
    pub classification: String,
}

/// A startup application entry (registry Run keys and startup folders).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupEntry {
    /// Stable identifier (hive + value name, or folder path).
    pub id: String,
    pub name: String,
    pub command: String,
    /// Full location path (e.g. `HKLM\SOFTWARE\...\Run\Name` or file path).
    pub location: String,
    /// "registry" | "startup_folder".
    pub source: String,
    /// Whether the entry currently starts at boot.
    pub enabled: bool,
    /// Whether Optix can toggle it (registry entries are toggleable; folder
    /// entries are listed read-only).
    pub toggleable: bool,
}

/// Result of a service mutation (stop / start / start-type / WSearch).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceActionResult {
    pub snapshot_id: String,
    pub changes: usize,
}

/// Result of toggling a startup entry.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupActionResult {
    pub snapshot_id: String,
    pub changes: usize,
}

/// A scheduled task enumerated from `schtasks /query /fo CSV /v`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledTask {
    pub name: String,
    /// "Ready" | "Running" | "Disabled" | …
    pub status: String,
    pub next_run: String,
    pub last_run: String,
    /// Task author (often empty — signatures come from the action path).
    pub author: String,
    /// Command the task runs (`Task To Run` column).
    pub action: String,
    pub run_as: String,
    /// Authenticode state of the action executable, when one is resolvable:
    /// "trusted" | "untrusted" | "unsigned" | "unavailable".
    pub signature: String,
}

/// Status of the Windows Search (WSearch) service.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WSearchStatus {
    pub enabled: bool,
    pub running: bool,
    pub start_type: String,
}

/// Map a service `Start` registry DWORD to its display string.
#[cfg(any(windows, test))]
pub fn start_type_str(v: u32) -> &'static str {
    match v {
        0 => "boot",
        1 => "system",
        2 => "auto",
        3 => "manual",
        4 => "disabled",
        _ => "unknown",
    }
}

/// Map a settable start-type string back to its `Start` DWORD.
pub fn start_type_value(s: &str) -> Option<u32> {
    match s {
        "auto" => Some(2),
        "manual" => Some(3),
        "disabled" => Some(4),
        _ => None,
    }
}

/// Map a service current-state DWORD to its display string.
#[cfg(any(windows, test))]
pub fn service_state_str(v: u32) -> &'static str {
    match v {
        1 => "stopped",
        2 => "starting",
        3 => "stopping",
        4 => "running",
        5 => "resuming",
        6 => "pausing",
        7 => "paused",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_type_mapping() {
        assert_eq!(start_type_str(2), "auto");
        assert_eq!(start_type_str(4), "disabled");
        assert_eq!(start_type_value("auto"), Some(2));
        assert_eq!(start_type_value("disabled"), Some(4));
        assert_eq!(start_type_value("bogus"), None);
    }

    #[test]
    fn state_mapping() {
        assert_eq!(service_state_str(4), "running");
        assert_eq!(service_state_str(1), "stopped");
    }
}
