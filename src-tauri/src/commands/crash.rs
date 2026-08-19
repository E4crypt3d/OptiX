use crate::engine::crash;
use crate::error::{OptixError, Result};
use crate::models::crash::CrashReport;

/// Scan the event log, WER reports, and minidumps for crashes.
#[tauri::command]
pub async fn scan_crashes() -> Result<Vec<CrashReport>> {
    tauri::async_runtime::spawn_blocking(crash::scan_crashes)
        .await
        .map_err(|e| OptixError::Other(e.to_string()))
}

/// Generate a `CrashReport.zip` for a crash, returning the zip path.
#[tauri::command]
pub async fn generate_crash_report(crash: CrashReport) -> Result<String> {
    tauri::async_runtime::spawn_blocking(move || crash::generate_report_zip(&crash))
        .await
        .map_err(|e| OptixError::Other(e.to_string()))?
}
