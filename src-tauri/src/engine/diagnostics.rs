//! Phase 12 — AI Diagnostics (rule-based).\n//!\n//! No random changes: a pure, deterministic rule engine scores telemetry,\n//! process, benchmark, and crash evidence into ranked findings with confidence\n//! levels. Every finding carries the specific evidence and a recommendation;\n//! nothing is applied automatically.

use crate::engine::processes::classify;
use crate::models::diagnostics::Diagnostic;
use crate::models::process::ProcessClass;

/// A minimal process snapshot used as rule evidence.
#[derive(Debug, Clone)]
pub struct ProcessSnapshot {
    pub name: String,
    pub cpu_usage: f32,
    pub memory_bytes: u64,
    pub disk_read_bytes: u64,
    pub disk_written_bytes: u64,
}

/// A benchmark run's headline numbers (FPS / CPU / GPU averages).
#[derive(Debug, Clone)]
pub struct BenchmarkSnapshot {
    pub avg_fps: Option<f64>,
    pub cpu_avg: Option<f64>,
    pub gpu_avg: Option<f64>,
}

/// A crash's salient fields.
#[derive(Debug, Clone)]
pub struct CrashSnapshot {
    pub event_id: Option<i64>,
    pub module: Option<String>,
    pub severity: String,
}

/// All evidence available to the diagnostic rules.
#[derive(Debug, Clone, Default)]
pub struct DiagnosticInput {
    pub cpu_usage: f32,
    pub gpu_usage: Option<f32>,
    pub ram_used_mb: i64,
    pub ram_total_mb: i64,
    pub disk_free_percent: Option<f32>,
    pub processes: Vec<ProcessSnapshot>,
    pub benchmarks: Vec<BenchmarkSnapshot>,
    pub crashes: Vec<CrashSnapshot>,
    pub temperatures: Vec<(String, f32)>,
}

fn conf(v: f32) -> u8 {
    v.clamp(0.0, 100.0).round() as u8
}

fn severity_rank(s: &str) -> u8 {
    match s {
        "critical" => 3,
        "warning" => 2,
        _ => 1,
    }
}

fn diag(
    id: &str,
    severity: &str,
    category: &str,
    title: &str,
    detail: String,
    recommendation: &str,
    confidence: u8,
) -> Diagnostic {
    Diagnostic {
        id: id.to_string(),
        severity: severity.to_string(),
        category: category.to_string(),
        title: title.to_string(),
        detail,
        recommendation: recommendation.to_string(),
        confidence,
    }
}

/// Run every rule and return findings ranked by severity then confidence.
pub fn diagnose(input: &DiagnosticInput) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if let Some(d) = memory_pressure(input) {
        out.push(d);
    }
    if let Some(d) = disk_full(input) {
        out.push(d);
    }
    if let Some(d) = cloud_sync(input) {
        out.push(d);
    }
    if let Some(d) = updater(input) {
        out.push(d);
    }
    if let Some(d) = cpu_bottleneck(input) {
        out.push(d);
    }
    if let Some(d) = gpu_bottleneck(input) {
        out.push(d);
    }
    if let Some(d) = driver_crash(input) {
        out.push(d);
    }
    if let Some(d) = thermal(input) {
        out.push(d);
    }
    if let Some(d) = background_apps(input) {
        out.push(d);
    }
    out.sort_by(|a, b| {
        severity_rank(&b.severity)
            .cmp(&severity_rank(&a.severity))
            .then(b.confidence.cmp(&a.confidence))
    });
    out
}

fn memory_pressure(input: &DiagnosticInput) -> Option<Diagnostic> {
    if input.ram_total_mb <= 0 {
        return None;
    }
    let used_pct = input.ram_used_mb as f32 / input.ram_total_mb as f32 * 100.0;
    if used_pct < 90.0 {
        return None;
    }
    Some(diag(
        "memory_pressure",
        if used_pct > 95.0 { "critical" } else { "warning" },
        "memory",
        "High memory usage",
        format!(
            "RAM is {used_pct:.0}% used ({} / {} MB).",
            input.ram_used_mb, input.ram_total_mb
        ),
        "Close background apps or add more RAM.",
        conf((used_pct - 90.0) / 10.0 * 100.0),
    ))
}

fn disk_full(input: &DiagnosticInput) -> Option<Diagnostic> {
    let free = input.disk_free_percent?;
    if free >= 15.0 {
        return None;
    }
    Some(diag(
        "disk_full",
        if free < 8.0 { "critical" } else { "warning" },
        "storage",
        "Disk nearly full",
        format!("Only {free:.0}% free space remains."),
        "Run Cleanup or move files — SSDs slow down below ~15% free.",
        conf((15.0 - free) / 15.0 * 100.0),
    ))
}

fn cloud_sync(input: &DiagnosticInput) -> Option<Diagnostic> {
    const SYNC_NAMES: &[&str] = &[
        "onedrive.exe",
        "dropbox.exe",
        "googledrivesync.exe",
        "msedge.exe",
        "chrome.exe",
        "firefox.exe",
        "discord.exe",
    ];
    let worst = input
        .processes
        .iter()
        .filter(|p| SYNC_NAMES.iter().any(|n| p.name.eq_ignore_ascii_case(n)))
        .max_by(|a, b| a.cpu_usage.total_cmp(&b.cpu_usage))?;
    if worst.cpu_usage < 15.0 {
        return None;
    }
    Some(diag(
        "cloud_sync",
        "warning",
        "background",
        "Background sync / I/O activity",
        format!("{} is using {:.0}% CPU.", worst.name, worst.cpu_usage),
        "Pause cloud sync (OneDrive/Dropbox) while gaming.",
        conf(worst.cpu_usage),
    ))
}

fn updater(input: &DiagnosticInput) -> Option<Diagnostic> {
    let worst = input
        .processes
        .iter()
        .filter(|p| {
            let n = p.name.to_ascii_lowercase();
            (n.contains("update") || n.contains("setup") || n.contains("install"))
                && p.cpu_usage > 10.0
        })
        .max_by(|a, b| a.cpu_usage.total_cmp(&b.cpu_usage))?;
    Some(diag(
        "updater",
        "warning",
        "update",
        "Update process running",
        format!("{} is using {:.0}% CPU.", worst.name, worst.cpu_usage),
        "Defer Windows/app updates while gaming.",
        conf(worst.cpu_usage),
    ))
}

fn cpu_bottleneck(input: &DiagnosticInput) -> Option<Diagnostic> {
    let b = input.benchmarks.last()?;
    let (fps, cpu) = (b.avg_fps?, b.cpu_avg?);
    if fps >= 60.0 || cpu < 85.0 {
        return None;
    }
    // A CPU bottleneck only when the GPU isn't the limiter (if known).
    if b.gpu_avg.map(|g| g >= 70.0).unwrap_or(false) {
        return None;
    }
    Some(diag(
        "cpu_bottleneck",
        "critical",
        "cpu",
        "CPU bottleneck detected",
        format!("{fps:.0} FPS average with CPU at {cpu:.0}%."),
        "Close background apps or apply a high-performance power profile.",
        conf((60.0 - fps.max(0.0)) / 60.0 * 100.0),
    ))
}

fn gpu_bottleneck(input: &DiagnosticInput) -> Option<Diagnostic> {
    let b = input.benchmarks.last()?;
    let (fps, cpu, gpu) = (b.avg_fps?, b.cpu_avg?, b.gpu_avg?);
    if fps >= 60.0 || gpu < 90.0 || cpu > 70.0 {
        return None;
    }
    Some(diag(
        "gpu_bottleneck",
        "warning",
        "gpu",
        "GPU-bound",
        format!("GPU at {gpu:.0}% while CPU at {cpu:.0}% ({fps:.0} FPS)."),
        "Lower graphics settings or resolution.",
        conf(gpu.min(100.0)),
    ))
}

fn driver_crash(input: &DiagnosticInput) -> Option<Diagnostic> {
    let driver_crashes: Vec<&CrashSnapshot> = input
        .crashes
        .iter()
        .filter(|c| c.severity == "high" && (c.event_id == Some(4101) || c.module.is_some()))
        .collect();
    if driver_crashes.is_empty() {
        return None;
    }
    let n = driver_crashes.len();
    Some(diag(
        "driver_crash",
        "critical",
        "driver",
        "GPU driver crash detected",
        format!("{n} driver-related crash(es) (TDR / display driver)."),
        "Update or clean-install your GPU drivers; check thermals.",
        conf(60.0 + n as f32 * 10.0),
    ))
}

fn thermal(input: &DiagnosticInput) -> Option<Diagnostic> {
    let hot = input
        .temperatures
        .iter()
        .filter(|(_, c)| *c > 85.0)
        .max_by(|a, b| a.1.total_cmp(&b.1))?;
    let (label, temp) = hot;
    Some(diag(
        "thermal",
        "critical",
        "thermal",
        "High temperature",
        format!("{label} is {temp:.0}°C."),
        "Check cooling — clean fans, improve airflow, or repaste.",
        conf((temp - 85.0) / 15.0 * 100.0),
    ))
}

fn background_apps(input: &DiagnosticInput) -> Option<Diagnostic> {
    let safe_count = input
        .processes
        .iter()
        .filter(|p| classify(&p.name) == ProcessClass::Safe)
        .count();
    if safe_count < 6 {
        return None;
    }
    Some(diag(
        "background_processes",
        "info",
        "background",
        "Many background apps",
        format!("{safe_count} background apps are running."),
        "Close apps you aren't using from the Processes page.",
        conf(safe_count as f32 * 5.0),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> DiagnosticInput {
        DiagnosticInput {
            cpu_usage: 10.0,
            gpu_usage: None,
            ram_used_mb: 8000,
            ram_total_mb: 16000,
            disk_free_percent: Some(40.0),
            processes: Vec::new(),
            benchmarks: Vec::new(),
            crashes: Vec::new(),
            temperatures: Vec::new(),
        }
    }

    #[test]
    fn flags_memory_pressure() {
        let mut i = input();
        i.ram_used_mb = 15500; // ~97%
        let d = diagnose(&i);
        assert!(d.iter().any(|x| x.id == "memory_pressure" && x.severity == "critical"));
    }

    #[test]
    fn flags_disk_nearly_full() {
        let mut i = input();
        i.disk_free_percent = Some(5.0);
        let d = diagnose(&i);
        assert!(d.iter().any(|x| x.id == "disk_full"));
    }

    #[test]
    fn flags_cloud_sync_cpu() {
        let mut i = input();
        i.processes.push(ProcessSnapshot {
            name: "OneDrive.exe".into(),
            cpu_usage: 40.0,
            memory_bytes: 0,
            disk_read_bytes: 0,
            disk_written_bytes: 0,
        });
        let d = diagnose(&i);
        assert!(d.iter().any(|x| x.id == "cloud_sync"));
    }

    #[test]
    fn flags_updater_process() {
        let mut i = input();
        i.processes.push(ProcessSnapshot {
            name: "msedgeupdate.exe".into(),
            cpu_usage: 55.0,
            memory_bytes: 0,
            disk_read_bytes: 0,
            disk_written_bytes: 0,
        });
        let d = diagnose(&i);
        assert!(d.iter().any(|x| x.id == "updater"));
    }

    #[test]
    fn flags_cpu_bottleneck_from_benchmark() {
        let mut i = input();
        i.benchmarks.push(BenchmarkSnapshot {
            avg_fps: Some(45.0),
            cpu_avg: Some(95.0),
            gpu_avg: Some(50.0),
        });
        let d = diagnose(&i);
        assert!(d.iter().any(|x| x.id == "cpu_bottleneck"));
    }

    #[test]
    fn flags_gpu_bottleneck_from_benchmark() {
        let mut i = input();
        i.benchmarks.push(BenchmarkSnapshot {
            avg_fps: Some(45.0),
            cpu_avg: Some(50.0),
            gpu_avg: Some(98.0),
        });
        let d = diagnose(&i);
        assert!(d.iter().any(|x| x.id == "gpu_bottleneck"));
    }

    #[test]
    fn flags_driver_crash() {
        let mut i = input();
        i.crashes.push(CrashSnapshot {
            event_id: Some(4101),
            module: Some("nvlddmkm.sys".into()),
            severity: "high".into(),
        });
        let d = diagnose(&i);
        assert!(d.iter().any(|x| x.id == "driver_crash"));
    }

    #[test]
    fn flags_thermal() {
        let mut i = input();
        i.temperatures.push(("GPU".into(), 92.0));
        let d = diagnose(&i);
        assert!(d.iter().any(|x| x.id == "thermal"));
    }

    #[test]
    fn ranks_critical_above_info() {
        let mut i = input();
        i.ram_used_mb = 15800; // critical memory
        i.processes.push(ProcessSnapshot {
            name: "chrome.exe".into(),
            cpu_usage: 0.0,
            memory_bytes: 0,
            disk_read_bytes: 0,
            disk_written_bytes: 0,
        });
        i.processes.push(ProcessSnapshot {
            name: "steam.exe".into(),
            cpu_usage: 0.0,
            memory_bytes: 0,
            disk_read_bytes: 0,
            disk_written_bytes: 0,
        });
        i.processes.push(ProcessSnapshot {
            name: "discord.exe".into(),
            cpu_usage: 0.0,
            memory_bytes: 0,
            disk_read_bytes: 0,
            disk_written_bytes: 0,
        });
        i.processes.push(ProcessSnapshot {
            name: "spotify.exe".into(),
            cpu_usage: 0.0,
            memory_bytes: 0,
            disk_read_bytes: 0,
            disk_written_bytes: 0,
        });
        i.processes.push(ProcessSnapshot {
            name: "onedrive.exe".into(),
            cpu_usage: 0.0,
            memory_bytes: 0,
            disk_read_bytes: 0,
            disk_written_bytes: 0,
        });
        i.processes.push(ProcessSnapshot {
            name: "epicgameslauncher.exe".into(),
            cpu_usage: 0.0,
            memory_bytes: 0,
            disk_read_bytes: 0,
            disk_written_bytes: 0,
        });
        let d = diagnose(&i);
        assert!(!d.is_empty());
        assert_eq!(d[0].severity, "critical");
    }
}
