//! Application-level info surfaced on the Settings page.

use serde::Serialize;

/// Read-only app metadata: version and the on-disk data locations Optix uses.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub data_dir: String,
    pub snapshots_dir: String,
    pub snapshot_retention: usize,
    /// Full path to `logs.txt` (console + file logging).
    pub log_path: String,
}
