// Forward-declared for later phases (profiles, games, benchmarks, crash reports).
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// A reusable optimization profile (power, network, gaming, or composite).
/// Mirrors the `profiles` table; the full payload lives in `json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptimizationProfile {
    pub id: Option<i64>,
    pub name: String,
    pub kind: String,
    pub json: serde_json::Value,
}

/// A detected or manually-added game. Mirrors the `games` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: Option<i64>,
    pub name: String,
    /// steam | epic | battlenet | riot | xbox | gog | manual
    pub launcher: Option<String>,
    pub app_id: Option<String>,
    pub install_path: Option<String>,
    pub executable: Option<String>,
    pub last_played_ms: Option<i64>,
    pub detected_at_ms: Option<i64>,
}

/// Per-game optimization settings. Mirrors the `game_profiles` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameProfile {
    pub game_id: i64,
    /// normal | above_normal | high (never realtime)
    pub cpu_priority: Option<String>,
    /// Hex affinity mask, or null for system default.
    pub affinity_mask: Option<String>,
    pub power_profile: Option<String>,
    pub network_profile: Option<String>,
    pub cleanup_bg: bool,
    pub gpu_profile: Option<String>,
    pub enabled: bool,
}

/// One benchmark run. Mirrors the `benchmarks` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkResult {
    pub id: Option<i64>,
    pub game_id: Option<i64>,
    pub game_name: Option<String>,
    pub started_at_ms: Option<i64>,
    pub duration_ms: Option<i64>,
    pub avg_fps: Option<f64>,
    pub p1_fps: Option<f64>,
    pub p01_fps: Option<f64>,
    pub avg_frame_time_ms: Option<f64>,
    pub p95_frame_time_ms: Option<f64>,
    pub cpu_avg: Option<f64>,
    pub gpu_avg: Option<f64>,
    pub ram_avg_mb: Option<f64>,
    pub latency_ms: Option<f64>,
    pub config_hash: Option<String>,
    pub csv_path: Option<String>,
}

/// A captured crash event. Mirrors the `crash_reports` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashReport {
    pub id: Option<i64>,
    pub detected_at_ms: Option<i64>,
    pub app: Option<String>,
    pub pid: Option<i64>,
    pub event_id: Option<i64>,
    pub module: Option<String>,
    pub exception_code: Option<String>,
    pub wer_report_path: Option<String>,
    pub minidump_path: Option<String>,
    pub report_zip_path: Option<String>,
}
