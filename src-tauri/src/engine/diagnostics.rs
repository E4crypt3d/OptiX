//! Phase 12 — Diagnostics (rule-based).
//!
//! No random changes: a pure, deterministic rule engine scores telemetry,
//! process, benchmark, and crash evidence into ranked findings with confidence
//! levels plus an overall health score. Every finding carries the specific
//! evidence and a recommendation; nothing is applied automatically.

use std::collections::HashMap;

use crate::engine::processes::classify;
use crate::models::diagnostics::{Diagnostic, DiagnosticsReport};
use crate::models::process::ProcessClass;

/// A minimal process snapshot used as rule evidence.
#[derive(Debug, Clone)]
pub struct ProcessSnapshot {
    pub name: String,
    pub cpu_usage: f32,
    pub memory_bytes: u64,
}

/// A benchmark run's headline numbers (averages plus the stutter-relevant
/// tail metrics).
#[derive(Debug, Clone, Default)]
pub struct BenchmarkSnapshot {
    pub avg_fps: Option<f64>,
    pub cpu_avg: Option<f64>,
    pub gpu_avg: Option<f64>,
    /// 1% low FPS — the stutter signal when far below the average.
    pub p1_fps: Option<f64>,
    /// 95th-percentile frame time in ms.
    pub p95_frame_time_ms: Option<f64>,
    pub dropped_frames: Option<i64>,
}

/// A crash's salient fields.
#[derive(Debug, Clone)]
pub struct CrashSnapshot {
    pub event_id: Option<i64>,
    pub app: Option<String>,
    pub module: Option<String>,
    pub severity: String,
    pub detected_at_ms: i64,
}

/// One `/proc/pressure/<resource>` reading: stall percentages over the last
/// 10 s window ("some" = any task stalled, "full" = every non-idle task
/// stalled). Kernel ≥ 4.20 only; absent elsewhere.
#[derive(Debug, Clone, Copy, Default)]
pub struct PsiWindow {
    pub some_avg10: f32,
    pub full_avg10: f32,
}

/// PSI readings for the two resources Optix can act on.
#[derive(Debug, Clone, Copy, Default)]
pub struct PsiSnapshot {
    pub memory: PsiWindow,
    pub io: PsiWindow,
}

/// All evidence available to the diagnostic rules.
#[derive(Debug, Clone, Default)]
pub struct DiagnosticInput {
    pub cpu_usage: f32,
    pub gpu_usage: Option<f32>,
    pub ram_used_mb: i64,
    pub ram_total_mb: i64,
    /// Swap/pagefile usage. On Windows sysinfo maps this to the page file
    /// (effectively commit charge), so this rule doubles as the Microsoft
    /// "% Committed Bytes In Use" heuristic.
    pub swap_used_mb: i64,
    pub swap_total_mb: i64,
    pub uptime_secs: u64,
    pub disk_free_percent: Option<f32>,
    pub processes: Vec<ProcessSnapshot>,
    pub benchmarks: Vec<BenchmarkSnapshot>,
    pub crashes: Vec<CrashSnapshot>,
    pub temperatures: Vec<(String, f32)>,
    pub psi: Option<PsiSnapshot>,
}

const GB_MB: i64 = 1024;

/// Number of rules evaluated on every run — reported so a clean result is
/// verifiable rather than an empty list.
pub const CHECK_COUNT: u32 = 18;

const DAY_SECS: u64 = 86_400;
const LONG_UPTIME_DAYS: u64 = 14;
const DRIVER_CRASH_WINDOW_MS: i64 = 7 * 24 * 3_600_000;
const CRASH_STORM_WINDOW_MS: i64 = 24 * 3_600_000;

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
    out.extend(
        [
            memory_pressure(input),
            memory_hog(input),
            swap_thrashing(input),
            long_uptime(input),
            disk_full(input),
            cloud_sync(input),
            updater(input),
            cpu_bottleneck(input),
            frame_instability(input),
            gpu_bottleneck(input),
            driver_crash(input),
            crash_storm(input),
            thermal(input),
            high_cpu(input),
            gpu_saturated(input),
            background_apps(input),
            psi_memory(input),
            psi_io(input),
        ]
        .into_iter()
        .flatten(),
    );
    out.sort_by(|a, b| {
        severity_rank(&b.severity)
            .cmp(&severity_rank(&a.severity))
            .then(b.confidence.cmp(&a.confidence))
    });
    out
}

/// Weighted penalty each severity level costs against the 100-point health
/// score, scaled by the rule's confidence.
fn severity_penalty(finding: &Diagnostic) -> f32 {
    let base = match finding.severity.as_str() {
        "critical" => 28.0,
        "warning" => 12.0,
        _ => 4.0,
    };
    base * finding.confidence as f32 / 100.0
}

fn verdict_for(score: u8) -> &'static str {
    if score >= 90 {
        "Healthy"
    } else if score >= 70 {
        "Minor issues detected"
    } else if score >= 45 {
        "Needs attention"
    } else {
        "Critical issues found"
    }
}

/// Run every rule and fold the findings into an overall health score.
pub fn diagnose_report(input: &DiagnosticInput) -> DiagnosticsReport {
    let findings = diagnose(input);
    let total_penalty: f32 = findings.iter().map(severity_penalty).sum();
    let score = (100.0 - total_penalty).round().clamp(0.0, 100.0) as u8;
    DiagnosticsReport {
        checks_run: CHECK_COUNT,
        score,
        verdict: verdict_for(score).to_string(),
        findings,
    }
}

/// Read Linux Pressure Stall Information. Returns `None` outside Linux or on
/// kernels without CONFIG_PSI so every dependent rule degrades silently.
pub fn read_psi() -> Option<PsiSnapshot> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    Some(PsiSnapshot {
        memory: parse_psi_file("/proc/pressure/memory")?,
        io: parse_psi_file("/proc/pressure/io")?,
    })
}

fn parse_psi_file(path: &str) -> Option<PsiWindow> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut w = PsiWindow::default();
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("some") => {
                if let Some(v) = parts.find_map(|t| t.strip_prefix("avg10=")) {
                    w.some_avg10 = v.parse::<f32>().unwrap_or(0.0);
                }
            }
            Some("full") => {
                if let Some(v) = parts.find_map(|t| t.strip_prefix("avg10=")) {
                    w.full_avg10 = v.parse::<f32>().unwrap_or(0.0);
                }
            }
            _ => {}
        }
    }
    Some(w)
}

fn memory_pressure(input: &DiagnosticInput) -> Option<Diagnostic> {
    if input.ram_total_mb <= 0 {
        return None;
    }
    // Microsoft guidance: healthy means >10% available OR at least 4 GiB free.
    // The absolute branch keeps big-RAM machines from false-positiving while
    // games intentionally fill memory with caches.
    let avail_mb = input.ram_total_mb - input.ram_used_mb;
    let avail_pct = avail_mb as f32 / input.ram_total_mb as f32 * 100.0;
    let avail_gb = avail_mb as f32 / 1024.0;
    if avail_pct >= 10.0 || avail_gb >= 4.0 {
        return None;
    }
    let critical = avail_pct < 5.0 && avail_gb < 2.0;
    let gb = avail_gb;
    Some(diag(
        "memory_pressure",
        if critical { "critical" } else { "warning" },
        "memory",
        "Low available memory",
        format!(
            "Only {:.1} GB of {} GB RAM is available ({:.0}% free).",
            gb,
            input.ram_total_mb / 1024,
            avail_pct
        ),
        "Close background apps or add more RAM.",
        conf((10.0 - avail_pct.min(10.0)) / 10.0 * 80.0 + (4.0 - gb.min(4.0)) / 4.0 * 20.0),
    ))
}

fn memory_hog(input: &DiagnosticInput) -> Option<Diagnostic> {
    // Relative threshold: 4 GiB fixed flags Chrome on an 8 GB machine but
    // misses nothing on a 64 GB rig where a single 5 GB process is normal.
    let threshold_bytes = (input.ram_total_mb.max(2 * GB_MB) / 4).max(2 * GB_MB) as u64
        * 1024
        * 1024;
    let hog = input
        .processes
        .iter()
        .filter(|p| p.memory_bytes > threshold_bytes)
        .max_by_key(|p| p.memory_bytes)?;
    let hog_gb = hog.memory_bytes as f32 / (1024.0 * 1024.0 * 1024.0);
    Some(diag(
        "memory_hog",
        "warning",
        "memory",
        "Memory-hungry process",
        format!("{hog_gb:.1} GB of RAM is held by {}.", hog.name),
        "Close it if you aren't using it.",
        conf(hog_gb / 16.0 * 100.0),
    ))
}

fn swap_thrashing(input: &DiagnosticInput) -> Option<Diagnostic> {
    if input.swap_total_mb <= 0 {
        return None;
    }
    let pct = input.swap_used_mb as f32 / input.swap_total_mb as f32 * 100.0;
    if pct < 60.0 {
        return None;
    }
    Some(diag(
        "swap_thrashing",
        if pct >= 90.0 { "critical" } else { "warning" },
        "memory",
        "Swap / page file under heavy load",
        format!(
            "{:.0}% of the page file is committed ({} / {} MB).",
            pct, input.swap_used_mb, input.swap_total_mb
        ),
        "Close memory-heavy apps — the system is spilling RAM to disk.",
        conf((pct - 60.0) / 40.0 * 100.0),
    ))
}

fn long_uptime(input: &DiagnosticInput) -> Option<Diagnostic> {
    let days = input.uptime_secs / DAY_SECS;
    if days < LONG_UPTIME_DAYS {
        return None;
    }
    Some(diag(
        "long_uptime",
        "info",
        "system",
        "Long uptime",
        format!("This system has been running for {days} days without a reboot."),
        "Reboot to clear leaked resources and finish pending updates.",
        conf((days - LONG_UPTIME_DAYS + 1) as f32 * 15.0),
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
    // `benchmarks` is newest-first (list_benchmarks is ORDER BY id DESC), so
    // the newest run is the first entry.
    let b = input.benchmarks.first()?;
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
        conf(((60.0 - fps.max(0.0)) / 60.0 * 100.0) as f32),
    ))
}

/// Smooth average but collapsing tail frames — classic stutter signature the
/// plain bottleneck rules can't see.
fn frame_instability(input: &DiagnosticInput) -> Option<Diagnostic> {
    let b = input.benchmarks.first()?;
    let fps = b.avg_fps?;
    if fps < 55.0 {
        return None;
    }
    let target_ft_ms = 1000.0 / fps;
    let low_p1 = b
        .p1_fps
        .is_some_and(|p| p < 30.0 && fps - p > 25.0 || p * 3.0 < fps);
    let slow_tail = b
        .p95_frame_time_ms
        .is_some_and(|ft| ft > target_ft_ms * 2.0);
    if !low_p1 && !slow_tail {
        return None;
    }
    let evidence = match (b.p1_fps, b.p95_frame_time_ms) {
        (Some(p1), Some(ft)) => format!("1% lows at {p1:.0} FPS, p95 frame time {ft:.1} ms"),
        (Some(p1), None) => format!("1% lows at {p1:.0} FPS"),
        (None, Some(ft)) => format!("p95 frame time {ft:.1} ms vs {target_ft_ms:.1} ms target"),
        (None, None) => return None,
    };
    let dropped = b.dropped_frames.unwrap_or(0);
    let mut detail = format!("Average {fps:.0} FPS is smooth on paper, but {evidence}.");
    if dropped > 0 {
        detail.push_str(&format!(" {dropped} frames were dropped."));
    }
    Some(diag(
        "frame_instability",
        "warning",
        "frametime",
        "Frame-time instability",
        detail,
        "Check for background interference and thermal throttling; cap FPS or lower settings to stabilize frame pacing.",
        conf(70.0),
    ))
}

fn gpu_bottleneck(input: &DiagnosticInput) -> Option<Diagnostic> {
    // Newest run first (see `cpu_bottleneck`).
    let b = input.benchmarks.first()?;
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
        conf(gpu.min(100.0) as f32),
    ))
}

fn driver_crash(input: &DiagnosticInput) -> Option<Diagnostic> {
    let cutoff = crate::engine::now_ms() as i64 - DRIVER_CRASH_WINDOW_MS;
    let n = input
        .crashes
        .iter()
        .filter(|c| {
            c.severity == "high"
                && c.detected_at_ms >= cutoff
                && (c.event_id == Some(4101) || c.module.is_some())
        })
        .count();
    if n == 0 {
        return None;
    }
    Some(diag(
        "driver_crash",
        "critical",
        "driver",
        "Recent GPU driver crash",
        format!("{n} driver-related crash(es) in the last 7 days (TDR / display driver)."),
        "Update or clean-install your GPU drivers; check thermals.",
        conf(60.0 + n as f32 * 10.0),
    ))
}

/// Same application crashing repeatedly inside a day usually means a broken
/// install, a bad overlay injection, or an incompatible mod — not noise.
fn crash_storm(input: &DiagnosticInput) -> Option<Diagnostic> {
    let cutoff = crate::engine::now_ms() as i64 - CRASH_STORM_WINDOW_MS;
    let mut counts: HashMap<String, (String, u32)> = HashMap::new();
    for c in &input.crashes {
        if c.detected_at_ms < cutoff || c.detected_at_ms == 0 {
            continue;
        }
        let Some(app) = c.app.as_deref().map(str::trim).filter(|a| {
            !a.is_empty() && !a.eq_ignore_ascii_case("unknown application")
        }) else {
            continue;
        };
        let entry = counts
            .entry(app.to_ascii_lowercase())
            .or_insert_with(|| (app.to_string(), 0));
        entry.1 += 1;
    }
    let (name, n) = counts.into_values().max_by_key(|(_, n)| *n)?;
    if n < 3 {
        return None;
    }
    Some(diag(
        "crash_storm",
        if n >= 6 { "critical" } else { "warning" },
        "stability",
        "Repeated application crashes",
        format!("{name} crashed {n} times in the last 24 hours."),
        "Update or reinstall the app; disable overlays/injectors for it and check its logs.",
        conf((n as f32 - 2.0) * 20.0),
    ))
}

/// Sensor-class temperature limits in °C: (warning, critical). A single flat
/// cutoff mislabels NVMe controllers (spec limit ~70–85 °C, throttle much
/// earlier) against CPU packages (TjMax ~95–100 °C) and chipset zones that
/// idle hot by design.
fn thermal_limits(label_lc: &str) -> (f32, f32, &'static str) {
    const CPU_KEYS: &[&str] = &[
        "cpu", "package", "tctl", "tdie", "k10temp", "coretemp", "core ",
    ];
    const STORAGE_KEYS: &[&str] = &[
        "nvme", "ssd", "composite", "disk", "drive", "hdd", "sda", "sdb",
    ];
    const GPU_KEYS: &[&str] = &["gpu", "edge", "junction"];
    if CPU_KEYS.iter().any(|k| label_lc.contains(k)) {
        (85.0, 95.0, "CPU")
    } else if STORAGE_KEYS.iter().any(|k| label_lc.contains(k)) {
        (65.0, 78.0, "storage")
    } else if GPU_KEYS.iter().any(|k| label_lc.contains(k)) {
        (80.0, 90.0, "GPU")
    } else {
        (90.0, 105.0, "sensor")
    }
}

fn thermal(input: &DiagnosticInput) -> Option<Diagnostic> {
    let hottest = input
        .temperatures
        .iter()
        .filter(|(label, temp)| {
            let (warn, _, _) = thermal_limits(&label.to_ascii_lowercase());
            *temp >= warn
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))?;
    let (label, temp) = hottest;
    let (warn, crit, class) = thermal_limits(&label.to_ascii_lowercase());
    let span = (crit - warn).max(1.0);
    Some(diag(
        "thermal",
        if *temp >= crit { "critical" } else { "warning" },
        "thermal",
        &if *temp >= crit {
            format!("{class} temperature critical")
        } else {
            format!("{class} running hot")
        },
        format!("{label} is at {temp:.0}°C (limit band {warn:.0}–{crit:.0}°C)."),
        "Check cooling — clean fans, improve airflow, or repaste.",
        conf(40.0 + (temp - warn) / span * 60.0),
    ))
}

fn high_cpu(input: &DiagnosticInput) -> Option<Diagnostic> {
    if input.cpu_usage < 90.0 {
        return None;
    }
    Some(diag(
        "high_cpu",
        "warning",
        "cpu",
        "Sustained high CPU usage",
        format!("CPU is at {:.0}%.", input.cpu_usage),
        "Check the Processes page for the top consumer.",
        conf((input.cpu_usage - 90.0) / 10.0 * 100.0),
    ))
}

fn gpu_saturated(input: &DiagnosticInput) -> Option<Diagnostic> {
    let gpu = input.gpu_usage?;
    if gpu < 95.0 || input.cpu_usage > 30.0 {
        return None;
    }
    Some(diag(
        "gpu_saturated",
        "info",
        "gpu",
        "GPU at maximum",
        format!("GPU is at {gpu:.0}% while CPU is at {:.0}%.", input.cpu_usage),
        "Expected while gaming — lower settings if FPS is too low.",
        conf(gpu),
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

/// Linux-only: kernel-reported task stalls on memory reclaim. This fires
/// before percentage-based rules on kernels that track PSI.
fn psi_memory(input: &DiagnosticInput) -> Option<Diagnostic> {
    let w = input.psi.as_ref()?.memory;
    if !(w.full_avg10 >= 5.0 || w.some_avg10 >= 25.0) {
        return None;
    }
    Some(diag(
        "psi_memory_pressure",
        "warning",
        "memory",
        "Tasks are stalling on memory",
        format!(
            "Kernel PSI (10s window): some {:.1}%, full {:.1}% stalled.",
            w.some_avg10, w.full_avg10
        ),
        "Close memory-heavy apps — the kernel is spending real time reclaiming pages.",
        conf(50.0 + w.full_avg10 * 6.0),
    ))
}

/// Linux-only: sustained I/O wait reported by the kernel.
fn psi_io(input: &DiagnosticInput) -> Option<Diagnostic> {
    let w = input.psi.as_ref()?.io;
    if w.full_avg10 < 20.0 {
        return None;
    }
    Some(diag(
        "psi_io_pressure",
        "warning",
        "storage",
        "Disk I/O stalls detected",
        format!("Kernel PSI (10s window): {:.1}% of time fully blocked on I/O.", w.full_avg10),
        "Find the busy process on the Processes page; consider moving games off a saturated drive.",
        conf(40.0 + w.full_avg10 * 2.5),
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
            swap_used_mb: 0,
            swap_total_mb: 0,
            uptime_secs: 3600,
            disk_free_percent: Some(40.0),
            processes: Vec::new(),
            benchmarks: Vec::new(),
            crashes: Vec::new(),
            temperatures: Vec::new(),
            psi: None,
        }
    }

    fn crash(app: &str, age_ms: i64, severity: &str) -> CrashSnapshot {
        CrashSnapshot {
            event_id: None,
            app: Some(app.into()),
            module: None,
            severity: severity.into(),
            detected_at_ms: crate::engine::now_ms() as i64 - age_ms,
        }
    }

    #[test]
    fn flags_memory_pressure_when_both_branches_low() {
        let mut i = input();
        i.ram_used_mb = 15500; // ~500 MB available
        let d = diagnose(&i);
        assert!(d.iter().any(|x| x.id == "memory_pressure" && x.severity == "critical"));
    }

    #[test]
    fn big_ram_machine_with_full_cache_is_not_flagged() {
        // 91% used on a 64 GB machine still leaves 5.7 GB — healthy per the
        // Microsoft ">10% OR ≥4 GB" heuristic.
        let mut i = input();
        i.ram_total_mb = 64 * 1024;
        i.ram_used_mb = 58 * 1024;
        assert!(!diagnose(&i).iter().any(|x| x.id == "memory_pressure"));
    }

    #[test]
    fn hog_threshold_scales_with_ram() {
        let mut i = input();
        i.ram_total_mb = 8192; // 8 GB: threshold = max(2 GB, 25%) = 2 GB
        i.processes.push(ProcessSnapshot {
            name: "chrome.exe".into(),
            cpu_usage: 0.0,
            memory_bytes: 3u64 * 1024 * 1024 * 1024,
        });
        assert!(diagnose(&i).iter().any(|x| x.id == "memory_hog"));

        let mut big = input();
        big.ram_total_mb = 64 * 1024; // threshold = max(2 GB, 16 GB) = 16 GB
        big.processes.push(ProcessSnapshot {
            name: "chrome.exe".into(),
            cpu_usage: 0.0,
            memory_bytes: 5u64 * 1024 * 1024 * 1024,
        });
        assert!(!diagnose(&big).iter().any(|x| x.id == "memory_hog"));
    }

    #[test]
    fn flags_swap_thrashing() {
        let mut i = input();
        i.swap_total_mb = 8192;
        i.swap_used_mb = 7900; // 96%
        let d = diagnose(&i);
        assert!(d.iter().any(|x| x.id == "swap_thrashing" && x.severity == "critical"));
    }

    #[test]
    fn ignores_light_swap_usage() {
        let mut i = input();
        i.swap_total_mb = 8192;
        i.swap_used_mb = 800; // ~10%
        assert!(!diagnose(&i).iter().any(|x| x.id == "swap_thrashing"));
    }

    #[test]
    fn flags_long_uptime() {
        let mut i = input();
        i.uptime_secs = 20 * DAY_SECS;
        assert!(diagnose(&i).iter().any(|x| x.id == "long_uptime"));

        i.uptime_secs = 2 * DAY_SECS;
        assert!(!diagnose(&i).iter().any(|x| x.id == "long_uptime"));
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
            ..Default::default()
        });
        let d = diagnose(&i);
        assert!(d.iter().any(|x| x.id == "cpu_bottleneck"));
    }

    #[test]
    fn bottleneck_rules_use_the_newest_benchmark() {
        // list_benchmarks returns newest first, so the rules must analyze the
        // first entry — the second (older) run would trip the rule here.
        let mut i = input();
        i.benchmarks.push(BenchmarkSnapshot {
            avg_fps: Some(120.0),
            cpu_avg: Some(50.0),
            gpu_avg: Some(80.0),
            ..Default::default()
        });
        i.benchmarks.push(BenchmarkSnapshot {
            avg_fps: Some(45.0),
            cpu_avg: Some(95.0),
            gpu_avg: Some(50.0),
            ..Default::default()
        });
        let d = diagnose(&i);
        assert!(!d.iter().any(|x| x.id == "cpu_bottleneck"));
    }

    #[test]
    fn flags_gpu_bottleneck_from_benchmark() {
        let mut i = input();
        i.benchmarks.push(BenchmarkSnapshot {
            avg_fps: Some(45.0),
            cpu_avg: Some(50.0),
            gpu_avg: Some(98.0),
            ..Default::default()
        });
        let d = diagnose(&i);
        assert!(d.iter().any(|x| x.id == "gpu_bottleneck"));
    }

    #[test]
    fn flags_frame_instability_on_low_p1() {
        let mut i = input();
        i.benchmarks.push(BenchmarkSnapshot {
            avg_fps: Some(80.0),
            cpu_avg: Some(50.0),
            gpu_avg: Some(60.0),
            p1_fps: Some(22.0),
            ..Default::default()
        });
        assert!(diagnose(&i).iter().any(|x| x.id == "frame_instability"));
    }

    #[test]
    fn stable_framerate_not_flagged() {
        let mut i = input();
        i.benchmarks.push(BenchmarkSnapshot {
            avg_fps: Some(80.0),
            cpu_avg: Some(50.0),
            gpu_avg: Some(60.0),
            p1_fps: Some(65.0),
            p95_frame_time_ms: Some(14.0),
            ..Default::default()
        });
        assert!(!diagnose(&i).iter().any(|x| x.id == "frame_instability"));
    }

    #[test]
    fn recent_driver_crash_flagged_old_one_ignored() {
        let mut i = input();
        i.crashes.push(CrashSnapshot {
            event_id: Some(4101),
            app: Some("Display driver (TDR)".into()),
            module: Some("nvlddmkm.sys".into()),
            severity: "high".into(),
            detected_at_ms: crate::engine::now_ms() as i64 - 3_600_000,
        });
        assert!(diagnose(&i).iter().any(|x| x.id == "driver_crash"));

        let mut old = input();
        old.crashes.push(CrashSnapshot {
            detected_at_ms: crate::engine::now_ms() as i64 - 8 * DRIVER_CRASH_WINDOW_MS / 7,
            ..crash("old", 0, "high")
        });
        assert!(!diagnose(&old).iter().any(|x| x.id == "driver_crash"));
    }

    #[test]
    fn flags_crash_storm_for_repeated_app() {
        let mut i = input();
        for k in 0..3 {
            i.crashes.push(crash("game.exe", k * 1_800_000, "medium"));
        }
        i.crashes.push(crash("other.exe", 600_000, "medium")); // decoy
        let d = diagnose(&i);
        let storm = d.iter().find(|x| x.id == "crash_storm").expect("storm");
        assert_eq!(storm.severity, "warning");
        assert!(storm.detail.contains("game.exe"));
        assert!(!storm.detail.contains("other.exe"));
    }

    #[test]
    fn scattered_crashes_do_not_storm() {
        let mut i = input();
        i.crashes.push(crash("game.exe", 0, "medium"));
        i.crashes.push(crash("game.exe", 400_000, "medium"));
        assert!(!diagnose(&i).iter().any(|x| x.id == "crash_storm"));
    }

    #[test]
    fn thermal_tiers_match_sensor_class() {
        let (w, c, class) = thermal_limits("nvme composite");
        assert_eq!((w, c, class), (65.0, 78.0, "storage"));
        let (w, c, class) = thermal_limits("package id 0");
        assert_eq!((w, c, class), (85.0, 95.0, "CPU"));
        let (w, c, class) = thermal_limits("amdgpu edge");
        assert_eq!((w, c, class), (80.0, 90.0, "GPU"));
    }

    #[test]
    fn nvme_at_70_flags_but_chipset_zone_does_not() {
        let mut i = input();
        i.temperatures.push(("nvme0 Composite".into(), 70.0));
        assert!(diagnose(&i).iter().any(|x| x.id == "thermal"));

        let mut calm = input();
        calm.temperatures.push(("ACPI Thermal Zone TZ00".into(), 70.0));
        assert!(!diagnose(&calm).iter().any(|x| x.id == "thermal"));
    }

    #[test]
    fn flags_thermal_critical_gpu() {
        let mut i = input();
        i.temperatures.push(("GPU".into(), 92.0));
        let d = diagnose(&i);
        let t = d.iter().find(|x| x.id == "thermal").expect("thermal");
        assert_eq!(t.severity, "critical");
    }

    #[test]
    fn parses_psi_file_lines() {
        let dir = std::env::temp_dir().join(format!("optix-psi-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("pressure");
        std::fs::write(
            &path,
            "some avg10=12.34 avg60=1.00 avg300=0.50 total=100\n\
             full avg10=5.67 avg60=0.00 avg300=0.00 total=9\n",
        )
        .unwrap();
        let w = parse_psi_file(path.to_str().unwrap()).unwrap();
        assert!((w.some_avg10 - 12.34).abs() < 1e-4);
        assert!((w.full_avg10 - 5.67).abs() < 1e-4);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn psi_rule_fires_on_kernel_stall_signal() {
        let mut i = input();
        i.psi = Some(PsiSnapshot {
            memory: PsiWindow {
                some_avg10: 30.0,
                full_avg10: 8.0,
            },
            io: PsiWindow::default(),
        });
        assert!(diagnose(&i).iter().any(|x| x.id == "psi_memory_pressure"));
        assert!(!diagnose(&i).iter().any(|x| x.id == "psi_io_pressure"));
    }

    #[test]
    fn ranks_critical_above_info() {
        let mut i = input();
        i.ram_used_mb = 15800; // critical memory
        for name in ["chrome.exe", "steam.exe", "discord.exe", "spotify.exe", "onedrive.exe", "epicgameslauncher.exe"] {
            i.processes.push(ProcessSnapshot {
                name: name.into(),
                cpu_usage: 0.0,
                memory_bytes: 0,
            });
        }
        let d = diagnose(&i);
        assert!(!d.is_empty());
        assert_eq!(d[0].severity, "critical");
    }

    #[test]
    fn report_score_reflects_findings() {
        let clean = diagnose_report(&input());
        assert_eq!(clean.score, 100);
        assert_eq!(clean.checks_run, CHECK_COUNT);
        assert_eq!(clean.verdict, "Healthy");

        let mut bad_input = input();
        bad_input.ram_used_mb = 15500;
        bad_input.disk_free_percent = Some(3.0);
        let bad = diagnose_report(&bad_input);
        assert!(bad.score < 70);
        assert_ne!(bad.verdict, "Healthy");

        // More severe findings must cost strictly more than milder ones.
        let mut worse = bad_input;
        worse.temperatures.push(("package id 0".into(), 99.0));
        assert!(diagnose_report(&worse).score < bad.score);
    }
}
