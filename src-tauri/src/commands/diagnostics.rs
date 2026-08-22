use tauri::State;

use crate::db::sqlite::Database;
use crate::engine::crash;
use crate::engine::diagnostics::{
    self, BenchmarkSnapshot, CrashSnapshot, DiagnosticInput, ProcessSnapshot,
};
use crate::error::{OptixError, Result};
use crate::models::diagnostics::DiagnosticsReport;

/// Run the rule-based diagnostic engine over the live system.
#[tauri::command]
pub async fn run_diagnostics(db: State<'_, Database>) -> Result<DiagnosticsReport> {
    let benchmarks: Vec<BenchmarkSnapshot> = db
        .list_benchmarks()?
        .into_iter()
        .map(|b| BenchmarkSnapshot {
            avg_fps: b.avg_fps,
            cpu_avg: b.cpu_avg,
            gpu_avg: b.gpu_avg,
            p1_fps: b.p1_fps,
            p95_frame_time_ms: b.p95_frame_time_ms,
            dropped_frames: Some(b.dropped_frames as i64),
        })
        .collect();

    tauri::async_runtime::spawn_blocking(move || collect_and_diagnose(benchmarks))
        .await
        .map_err(|e| OptixError::Other(e.to_string()))
}

fn collect_and_diagnose(benchmarks: Vec<BenchmarkSnapshot>) -> DiagnosticsReport {
    let input = collect_input(benchmarks);
    diagnostics::diagnose_report(&input)
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
    let swap_used_mb = (sys.used_swap() / (1024 * 1024)) as i64;
    let swap_total_mb = (sys.total_swap() / (1024 * 1024)) as i64;

    let processes: Vec<ProcessSnapshot> = sys
        .processes()
        .values()
        .map(|p| ProcessSnapshot {
            name: p.name().to_string_lossy().into_owned(),
            cpu_usage: p.cpu_usage(),
            memory_bytes: p.memory(),
        })
        .collect();

    // The relevant signal is the worst fixed disk: removable media (USB
    // sticks) and empty secondary drives would dilute an aggregate across all
    // disks and hide a nearly-full system drive.
    let disks = Disks::new_with_refreshed_list();
    let disk_free_percent = disks
        .list()
        .iter()
        .filter(|d| !d.is_removable())
        .filter_map(|d| {
            let total = d.total_space();
            (total > 0).then(|| {
                (total.saturating_sub(d.available_space())) as f32 / total as f32 * 100.0
            })
        })
        .min_by(|a, b| a.total_cmp(b));

    let mut temperatures: Vec<(String, f32)> = Components::new_with_refreshed_list()
        .iter()
        .filter_map(|c| c.temperature().map(|t| (c.label().to_string(), t)))
        .collect();
    // Windows: sysinfo has no sensor access, so ACPI thermal zones from WMI
    // back the high-temperature diagnostic.
    temperatures.extend(
        crate::win::hardware::temperatures()
            .into_iter()
            .filter_map(|t| t.celsius.map(|c| (t.label, c))),
    );

    let crashes: Vec<CrashSnapshot> = crash::scan_crashes()
        .into_iter()
        .map(|c| CrashSnapshot {
            event_id: c.event_id,
            app: Some(c.app),
            module: c.module,
            severity: c.severity,
            detected_at_ms: c.detected_at,
        })
        .collect();

    DiagnosticInput {
        cpu_usage: sys.global_cpu_usage(),
        gpu_usage: None,
        ram_used_mb,
        ram_total_mb,
        swap_used_mb,
        swap_total_mb,
        uptime_secs: System::uptime(),
        disk_free_percent,
        processes,
        benchmarks,
        crashes,
        temperatures,
        psi: diagnostics::read_psi(),
    }
}
