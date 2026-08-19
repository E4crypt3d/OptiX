//! Benchmark models (Phase 10): FPS (PresentMon) and system-stress results.

use serde::Serialize;

/// A single benchmark run. FPS runs populate the frame stats; stress runs
/// leave them `None` and report only CPU/RAM averages.
///
/// `frame_times_ms`, `dropped_frames`, and `frame_count` are transient (the
/// frontend renders them for a just-completed or loaded run); only the scalar
/// columns are persisted in the `benchmarks` table.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkResult {
    pub id: Option<i64>,
    pub game_id: Option<i64>,
    pub game_name: Option<String>,
    pub started_at: i64,
    pub duration_ms: i64,
    pub avg_fps: Option<f64>,
    /// 1% low FPS (inverse of the 99th-percentile frame time).
    pub p1_fps: Option<f64>,
    /// 0.1% low FPS (inverse of the 99.9th-percentile frame time).
    pub p01_fps: Option<f64>,
    pub avg_frame_time_ms: Option<f64>,
    pub p95_frame_time_ms: Option<f64>,
    pub cpu_avg: Option<f64>,
    pub gpu_avg: Option<f64>,
    pub ram_avg_mb: Option<f64>,
    pub latency_ms: Option<f64>,
    /// Deterministic hash of the applied optimization config, for meaningful
    /// before/after comparison.
    pub config_hash: Option<String>,
    pub csv_path: Option<String>,
    // --- transient (not persisted) ---
    pub frame_times_ms: Vec<f64>,
    pub dropped_frames: u64,
    pub frame_count: usize,
}
