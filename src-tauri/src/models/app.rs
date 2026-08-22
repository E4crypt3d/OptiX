//! Application-level info surfaced on the Settings page.

use serde::Serialize;

/// Result of exporting a system report to a file.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemReportExport {
    /// Full path the report was written to.
    pub path: String,
    /// Bytes written.
    pub size_bytes: u64,
    /// "html" | "json"
    pub format: String,
}

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
