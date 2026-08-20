//! Phase 10 — Benchmark System.
//!
//! PresentMon CSV parsing + frame-time percentile math (the 1%/0.1% lows that
//! PresentMon doesn't compute context-aware), a deterministic config hash for
//! before/after grouping, and the capture orchestration (Windows-gated). The
//! pure parsing/statistics are unit-tested on Linux.

use crate::db::sqlite::data_dir;
use crate::error::{OptixError, Result};
use crate::models::benchmark::BenchmarkResult;
use crate::models::games::GameProfile;
use crate::win;

/// Result of parsing a PresentMon CSV: frame times (ms between displayed
/// frames) plus the total dropped-frame count.
pub struct ParsedCsv {
    pub frame_times_ms: Vec<f64>,
    pub dropped_frames: u64,
}

/// Computed frame statistics.
pub struct FrameStats {
    pub avg_fps: f64,
    pub p1_fps: f64,
    pub p01_fps: f64,
    pub avg_frame_time_ms: f64,
    pub p95_frame_time_ms: f64,
    pub frame_count: usize,
}

/// Parse a PresentMon v2 CSV. Uses `MsBetweenDisplayChange` (user-perceived
/// frame pacing) with `MsBetweenPresents` as a fallback; sums `Dropped`.
pub fn parse_presentmon_csv(text: &str) -> std::result::Result<ParsedCsv, String> {
    let mut lines = text.lines();
    let header = lines.next().ok_or("empty CSV")?;
    let columns = split_csv_line(header);

    let find_col = |names: &[&str]| {
        names
            .iter()
            .find_map(|n| columns.iter().position(|c| c == *n))
    };
    let time_col = find_col(&["MsBetweenDisplayChange"])
        .or_else(|| find_col(&["MsBetweenPresents"]))
        .ok_or("no frame-time column found (expected MsBetweenDisplayChange)")?;
    let dropped_col = find_col(&["Dropped", "Dropped Frames"]);

    let mut frame_times = Vec::new();
    let mut dropped = 0u64;
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let fields = split_csv_line(line);
        let Some(raw) = fields.get(time_col) else {
            continue;
        };
        let Ok(ms) = raw.trim().parse::<f64>() else {
            continue;
        };
        if ms <= 0.0 {
            continue;
        }
        frame_times.push(ms);
        if let Some(dc) = dropped_col {
            if let Some(d) = fields.get(dc) {
                if let Ok(v) = d.trim().parse::<u64>() {
                    dropped += v;
                }
            }
        }
    }

    if frame_times.is_empty() {
        return Err("no frame samples found in CSV".into());
    }
    Ok(ParsedCsv {
        frame_times_ms: frame_times,
        dropped_frames: dropped,
    })
}

/// Compute frame statistics from frame times. 1% low FPS = 1000 / 99th-
/// percentile frame time (nearest-rank), 0.1% low = 1000 / 99.9th percentile.
pub fn analyze(frame_times: &[f64]) -> FrameStats {
    if frame_times.is_empty() {
        return FrameStats {
            avg_fps: 0.0,
            p1_fps: 0.0,
            p01_fps: 0.0,
            avg_frame_time_ms: 0.0,
            p95_frame_time_ms: 0.0,
            frame_count: 0,
        };
    }
    let n = frame_times.len();
    let mut sorted = frame_times.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let avg_frame_time_ms = frame_times.iter().sum::<f64>() / n as f64;
    let p95 = percentile(&sorted, 0.95);
    let p99 = percentile(&sorted, 0.99);
    let p999 = percentile(&sorted, 0.999);

    FrameStats {
        avg_fps: 1000.0 / avg_frame_time_ms,
        p1_fps: 1000.0 / p99,
        p01_fps: 1000.0 / p999,
        avg_frame_time_ms,
        p95_frame_time_ms: p95,
        frame_count: n,
    }
}

/// Nearest-rank percentile of a sorted (ascending) slice.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((p * sorted.len() as f64).ceil() as usize).saturating_sub(1);
    sorted[idx.min(sorted.len() - 1)]
}

/// Minimal CSV line splitter that respects double-quoted fields.
fn split_csv_line(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                current.push('"');
                chars.next();
            }
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                out.push(std::mem::take(&mut current));
            }
            _ => current.push(c),
        }
    }
    out.push(current);
    out
}

/// Deterministic FNV-1a 64 hash over sorted key/value pairs (stable across
/// runs, unlike `DefaultHasher`), for before/after config grouping.
pub fn config_hash(entries: &[(&str, &str)]) -> String {
    let mut items: Vec<(String, String)> = entries
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    items.sort();

    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for (k, v) in &items {
        for b in k.bytes().chain(std::iter::once(0u8)).chain(v.bytes()) {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    format!("{hash:016x}")
}

/// Hash of the optimization-relevant fields of a game profile.
pub fn profile_config_hash(profile: &GameProfile) -> String {
    config_hash(&[
        ("cpu_priority", profile.cpu_priority.as_str()),
        ("affinity_mask", profile.affinity_mask.as_deref().unwrap_or("")),
        ("power_profile", profile.power_profile.as_str()),
        ("network_profile", profile.network_profile.as_str()),
        ("cleanup_bg", if profile.cleanup_bg { "1" } else { "0" }),
        ("gpu_profile", profile.gpu_profile.as_deref().unwrap_or("")),
    ])
}

/// Sample CPU usage and RAM (used MB) averages over `duration_secs`.
fn sample_session(duration_secs: u64) -> (f64, f64) {
    use sysinfo::System;

    let mut sys = System::new();
    sys.refresh_cpu_all();
    sys.refresh_memory();

    let start = std::time::Instant::now();
    let mut cpu_sum = 0.0f64;
    let mut ram_sum = 0.0f64;
    let mut n = 0u64;

    loop {
        std::thread::sleep(std::time::Duration::from_millis(500));
        sys.refresh_cpu_all();
        sys.refresh_memory();
        cpu_sum += sys.global_cpu_usage() as f64;
        ram_sum += (sys.used_memory() / (1024 * 1024)) as f64;
        n += 1;
        if start.elapsed().as_secs() >= duration_secs {
            break;
        }
    }

    if n == 0 {
        (0.0, 0.0)
    } else {
        (cpu_sum / n as f64, ram_sum / n as f64)
    }
}

/// Path for a capture CSV, under `<data_dir>/Benchmarks`.
fn benchmark_csv_path() -> Result<String> {
    let dir = data_dir().join("Benchmarks");
    std::fs::create_dir_all(&dir)?;
    Ok(dir
        .join(format!("capture_{}.csv", super::now_ms()))
        .to_string_lossy()
        .into_owned())
}

/// Run a PresentMon FPS capture for `exe_name` and analyze the result.
/// Blocks for roughly `duration_secs`; call from a blocking thread.
pub fn capture_and_analyze(
    exe_name: &str,
    duration_secs: u64,
    config_hash: Option<String>,
    game_id: Option<i64>,
    game_name: Option<String>,
) -> Result<BenchmarkResult> {
    // Resolve PresentMon before spawning the sampler: a missing binary or a
    // failed capture must not leave the sampler thread sampling for the full
    // duration.
    let presentmon = win::presentmon::find_presentmon().ok_or_else(|| {
        OptixError::InvalidState(
            "PresentMon64.exe not found. Place it next to the Optix executable or add it to PATH."
                .into(),
        )
    })?;

    let csv = benchmark_csv_path()?;
    let started_at = super::now_ms() as i64;

    let sampler = std::thread::spawn(move || sample_session(duration_secs));
    let capture = win::presentmon::run_capture(&presentmon, exe_name, duration_secs, &csv);
    let (cpu_avg, ram_avg) = sampler
        .join()
        .map_err(|_| OptixError::Other("sampler thread panicked".into()))?;
    if let Err(e) = capture {
        // Best-effort cleanup of a partial capture; the error is what matters.
        let _ = std::fs::remove_file(&csv);
        return Err(e);
    }

    let text = std::fs::read_to_string(&csv)?;
    let parsed = parse_presentmon_csv(&text).map_err(OptixError::Other)?;
    let stats = analyze(&parsed.frame_times_ms);

    Ok(BenchmarkResult {
        id: None,
        game_id,
        game_name,
        started_at,
        duration_ms: (duration_secs * 1000) as i64,
        avg_fps: Some(stats.avg_fps),
        p1_fps: Some(stats.p1_fps),
        p01_fps: Some(stats.p01_fps),
        avg_frame_time_ms: Some(stats.avg_frame_time_ms),
        p95_frame_time_ms: Some(stats.p95_frame_time_ms),
        cpu_avg: Some(cpu_avg),
        gpu_avg: None,
        ram_avg_mb: Some(ram_avg),
        latency_ms: None,
        config_hash,
        csv_path: Some(csv),
        frame_times_ms: parsed.frame_times_ms,
        dropped_frames: parsed.dropped_frames,
        frame_count: stats.frame_count,
    })
}

/// Run a system-stress benchmark (CPU/RAM averages only, no PresentMon).
pub fn run_stress(duration_secs: u64) -> BenchmarkResult {
    let started_at = super::now_ms() as i64;
    let (cpu_avg, ram_avg) = sample_session(duration_secs);
    BenchmarkResult {
        id: None,
        game_id: None,
        game_name: None,
        started_at,
        duration_ms: (duration_secs * 1000) as i64,
        avg_fps: None,
        p1_fps: None,
        p01_fps: None,
        avg_frame_time_ms: None,
        p95_frame_time_ms: None,
        cpu_avg: Some(cpu_avg),
        gpu_avg: None,
        ram_avg_mb: Some(ram_avg),
        latency_ms: None,
        config_hash: None,
        csv_path: None,
        frame_times_ms: Vec::new(),
        dropped_frames: 0,
        frame_count: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CSV: &str = "\
Application,ProcessID,MsBetweenPresents,MsBetweenDisplayChange,Dropped
game.exe,1234,16.67,16.67,0
game.exe,1234,16.67,16.67,0
game.exe,1234,16.67,16.67,0
game.exe,1234,16.67,16.67,0
game.exe,1234,16.67,16.67,1
game.exe,1234,33.34,33.34,0
";

    #[test]
    fn parses_presentmon_csv() {
        let parsed = parse_presentmon_csv(CSV).unwrap();
        assert_eq!(parsed.frame_times_ms.len(), 6);
        assert_eq!(parsed.dropped_frames, 1);
    }

    #[test]
    fn falls_back_to_ms_between_presents() {
        let csv = "Application,MsBetweenPresents
x,20
x,20
";
        let parsed = parse_presentmon_csv(csv).unwrap();
        assert_eq!(parsed.frame_times_ms, vec![20.0, 20.0]);
    }

    #[test]
    fn rejects_csv_without_frame_column() {
        assert!(parse_presentmon_csv("A,B
1,2
").is_err());
        assert!(parse_presentmon_csv("").is_err());
    }

    #[test]
    fn computes_frame_stats() {
        let stats = analyze(&[16.67, 16.67, 16.67, 16.67, 33.34]);
        // avg frame time = (4*16.67 + 33.34)/5 = 20.0 → 50 FPS.
        assert!((stats.avg_frame_time_ms - 20.0).abs() < 0.01);
        assert!((stats.avg_fps - 50.0).abs() < 0.01);
        assert_eq!(stats.frame_count, 5);
        // p95 (nearest-rank): idx ceil(0.95*5)=5 → idx 4 → 33.34.
        assert!((stats.p95_frame_time_ms - 33.34).abs() < 0.001);
        assert!(stats.p1_fps < stats.avg_fps);
    }

    #[test]
    fn analyze_empty_is_safe() {
        let stats = analyze(&[]);
        assert_eq!(stats.avg_fps, 0.0);
        assert_eq!(stats.frame_count, 0);
    }

    #[test]
    fn percentile_nearest_rank() {
        let sorted = [10.0, 20.0, 30.0, 40.0];
        assert_eq!(percentile(&sorted, 0.5), 20.0);
        assert_eq!(percentile(&sorted, 0.99), 40.0);
    }

    #[test]
    fn split_csv_line_handles_quotes() {
        assert_eq!(
            split_csv_line(r#"a,"b,c",d"#),
            vec!["a", "b,c", "d"]
        );
        assert_eq!(split_csv_line("a,b"), vec!["a", "b"]);
    }

    #[test]
    fn config_hash_is_deterministic_and_sensitive() {
        let a = config_hash(&[("x", "1"), ("y", "2")]);
        let b = config_hash(&[("y", "2"), ("x", "1")]);
        let c = config_hash(&[("x", "2"), ("y", "2")]);
        assert_eq!(a, b, "hash must not depend on order");
        assert_ne!(a, c, "hash must change with value");
        assert_eq!(a.len(), 16);
    }
}
