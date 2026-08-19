use tauri::State;

use crate::db::sqlite::Database;
use crate::engine::crash;
use crate::engine::diagnostics::{
    self, BenchmarkSnapshot, CrashSnapshot, DiagnosticInput, ProcessSnapshot,
};
use crate::error::{OptixError, Result};
use crate::models::diagnostics::Diagnostic;

/// Run the rule-based diagnostic engine over the live system.
#[tauri::command]
pub async fn run_diagnostics(db: State<'_, Database>) -> Result<Vec<Diagnostic>> {
    let benchmarks: Vec<BenchmarkSnapshot> = db
        .list_benchmarks()?
        .into_iter()
        .map(|b| BenchmarkSnapshot {
            avg_fps: b.avg_fps,
            cpu_avg: b.cpu_avg,
            gpu_avg: b.gpu_avg,
        })
        .collect();

    tauri::async_runtime::spawn_blocking(move || collect_and_diagnose(benchmarks))
        .await
        .map_err(|e| OptixError::Other(e.to_string()))
}

fn collect_and_diagnose(benchmarks: Vec<BenchmarkSnapshot>) -> Vec<Diagnostic> {
    let input = collect_input(benchmarks);
    diagnostics::diagnose(&input)
}

fn collect_input(benchmarks: Vec<BenchmarkSnapshot>) -> DiagnosticInput {
    use sysinfo::{Components, Disks, ProcessesToUpdate, System};

    let mut sys = System::new();
    sys.refresh_cpu_all();
    sys.refresh_memory();
    // Second sample yields a meaningful CPU delta.
    std::thread::sleep(std::time::Duration::from_millis(250));
    sys.refresh_cpu_all();
    sys.refresh_memory();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let ram_used_mb = (sys.used_memory() / (1024 * 1024)) as i64;
    let ram_total_mb = (sys.total_memory() / (1024 * 1024)) as i64;

    let processes: Vec<ProcessSnapshot> = sys
        .processes()
        .iter()
        .map(|(_, p)| ProcessSnapshot {
            name: p.name().to_string_lossy().into_owned(),
            cpu_usage: p.cpu_usage(),
            memory_bytes: p.memory(),
        })
        .collect();

    let disks = Disks::new_with_refreshed_list();
    let mut used = 0u64;
    let mut total = 0u64;
    for d in disks.list() {
        used += d.total_space().saturating_sub(d.available_space());
        total += d.total_space();
    }
    let disk_free_percent = if total > 0 {
        Some((total.saturating_sub(used)) as f32 / total as f32 * 100.0)
    } else {
        None
    };

    let temperatures: Vec<(String, f32)> = Components::new_with_refreshed_list()
        .iter()
        .filter_map(|c| c.temperature().map(|t| (c.label().to_string(), t)))
        .collect();

    let crashes: Vec<CrashSnapshot> = crash::scan_crashes()
        .into_iter()
        .map(|c| CrashSnapshot {
            event_id: c.event_id,
            module: c.module,
            severity: c.severity,
        })
        .collect();

    DiagnosticInput {
        cpu_usage: sys.global_cpu_usage(),
        gpu_usage: None,
        ram_used_mb,
        ram_total_mb,
        disk_free_percent,
        processes,
        benchmarks,
        crashes,
        temperatures,
    }
}
