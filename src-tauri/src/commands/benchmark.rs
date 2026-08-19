use tauri::State;

use crate::db::sqlite::Database;
use crate::engine::{benchmark, games};
use crate::error::{OptixError, Result};
use crate::models::benchmark::BenchmarkResult;

/// Run a PresentMon FPS capture for `exe_name` and analyze it. Blocks for the
/// capture duration (run on a background thread).
#[tauri::command]
pub async fn run_fps_benchmark(
    db: State<'_, Database>,
    game_id: Option<i64>,
    game_name: Option<String>,
    exe_name: String,
    duration_secs: u64,
) -> Result<BenchmarkResult> {
    let duration = duration_secs.clamp(5, 300);

    // Compute the config hash up front (needs the game profile from the DB).
    let config_hash = match game_id {
        Some(gid) => {
            let profile = db
                .get_game_profile(gid)?
                .unwrap_or_else(|| games::default_profile(gid));
            Some(benchmark::profile_config_hash(&profile))
        }
        None => None,
    };

    let mut result = tauri::async_runtime::spawn_blocking(move || {
        benchmark::capture_and_analyze(&exe_name, duration, config_hash, game_id, game_name)
    })
    .await
    .map_err(|e| OptixError::Other(e.to_string()))??;

    let id = db.insert_benchmark(&result)?;
    result.id = Some(id);
    Ok(result)
}

/// Run a system-stress benchmark (CPU/RAM averages, no PresentMon needed).
#[tauri::command]
pub async fn run_stress_benchmark(
    db: State<'_, Database>,
    duration_secs: u64,
) -> Result<BenchmarkResult> {
    let duration = duration_secs.clamp(5, 300);
    let mut result = tauri::async_runtime::spawn_blocking(move || benchmark::run_stress(duration))
        .await
        .map_err(|e| OptixError::Other(e.to_string()))?;
    let id = db.insert_benchmark(&result)?;
    result.id = Some(id);
    Ok(result)
}

/// List saved benchmark runs (newest first).
#[tauri::command]
pub fn list_benchmarks(db: State<'_, Database>) -> Result<Vec<BenchmarkResult>> {
    db.list_benchmarks()
}

/// Delete a benchmark run.
#[tauri::command]
pub fn delete_benchmark(db: State<'_, Database>, id: i64) -> Result<()> {
    db.delete_benchmark(id)
}

/// Re-load a run's frame-time series from its saved CSV (for charting).
#[tauri::command]
pub fn benchmark_frame_times(db: State<'_, Database>, id: i64) -> Result<Vec<f64>> {
    let run = db
        .get_benchmark(id)?
        .ok_or_else(|| OptixError::InvalidState(format!("benchmark {id} not found")))?;
    let Some(path) = run.csv_path else {
        return Ok(Vec::new());
    };
    let text = std::fs::read_to_string(&path)?;
    let parsed = benchmark::parse_presentmon_csv(&text).map_err(OptixError::Other)?;
    Ok(parsed.frame_times_ms)
}
