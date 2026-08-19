//! Crash-recovery models (Phase 11): a detected crash, with derived severity
//! and recommendation computed from the exception code and faulting module.

use serde::{Deserialize, Serialize};

/// A detected application/driver crash. Derived fields (`exception_name`,
/// `severity`, `recommendation`) are computed by the engine when the report is
/// built; the remaining fields come from the event log / WER / minidump.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashReport {
    pub detected_at: i64,
    pub app: String,
    pub pid: Option<i64>,
    pub event_id: Option<i64>,
    /// Faulting module (e.g. `nvwgf2umx.dll`).
    pub module: Option<String>,
    pub exception_code: Option<String>,
    /// Human-readable exception name (e.g. "Access violation").
    pub exception_name: Option<String>,
    /// low | medium | high
    pub severity: String,
    pub recommendation: String,
    pub wer_report_path: Option<String>,
    pub minidump_path: Option<String>,
    /// Set after `generate_crash_report` produces a zip.
    pub report_zip_path: Option<String>,
    /// event_log | wer | minidump
    pub source: String,
}
