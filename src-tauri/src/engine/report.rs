//! System report export: builds a readable, self-contained HTML report from a
//! `HardwareInfo` scan. JSON export is a plain `serde_json` serialization of
//! the same struct — no dedicated logic needed.

use std::fmt::Write as _;

use crate::error::{OptixError, Result};
use crate::models::hardware::HardwareInfo;

/// Render the hardware scan as a standalone HTML page (inline CSS, no external
/// assets), suitable for saving, sharing, or attaching to a support ticket.
pub fn html_report(info: &HardwareInfo) -> String {
    let mut out = String::with_capacity(16 * 1024);
    out.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    out.push_str("<title>Optix System Report</title>\n<style>\n");
    out.push_str(
        "body{font-family:system-ui,-apple-system,'Segoe UI',sans-serif;background:#0a0e17;color:#e2e8f0;margin:0;padding:2rem}\n\
         h1{font-size:1.5rem;margin:0 0 .25rem}\n\
         h2{font-size:1.1rem;margin:2rem 0 .5rem;color:#67e8f9;border-bottom:1px solid #1e293b;padding-bottom:.25rem}\n\
         .sub{color:#64748b;font-size:.85rem;margin:0 0 1.5rem}\n\
         .grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(320px,1fr));gap:1rem}\n\
         .card{background:#0f172a;border:1px solid #1e293b;border-radius:.5rem;padding:1rem}\n\
         .card h3{margin:0 0 .5rem;font-size:.95rem;color:#cbd5e1}\n\
         .kv{display:flex;justify-content:space-between;gap:1rem;font-size:.85rem;padding:.15rem 0}\n\
         .kv .k{color:#64748b}.kv .v{color:#e2e8f0;text-align:right}\n\
         table{width:100%;border-collapse:collapse;font-size:.8rem}\n\
         th,td{text-align:left;padding:.35rem .5rem;border-bottom:1px solid #1e293b}\n\
         th{color:#64748b;font-weight:500}\n\
         code{background:#1e293b;padding:.1rem .3rem;border-radius:.25rem;font-size:.8rem}\n",
    );
    out.push_str("</style>\n</head>\n<body>\n");

    let _ = writeln!(out, "<h1>Optix System Report</h1>");
    let _ = writeln!(
        out,
        "<p class=\"sub\">Generated {} · Optix v{}</p>",
        format_timestamp(info.scanned_at_ms),
        env!("CARGO_PKG_VERSION")
    );

    // OS + core hardware summary.
    let os = &info.os;
    let _ = writeln!(out, "<h2>System</h2><div class=\"grid\"><div class=\"card\">");
    kv(&mut out, "OS", format!("{} {}", os.edition.as_deref().unwrap_or(&os.name), os.version).trim());
    kv(
        &mut out,
        "Build",
        &os.build_number.map(|b| b.to_string()).unwrap_or_else(|| "—".into()),
    );
    kv(&mut out, "Host", &os.host_name);
    kv(&mut out, "Uptime", &format_uptime(os.uptime_seconds));
    out.push_str("</div><div class=\"card\">");
    kv(
        &mut out,
        "CPU",
        &format!("{} ({} phys / {} log)", info.cpu.brand, info.cpu.physical_cores, info.cpu.logical_cores),
    );
    kv(&mut out, "CPU clock", &format_frequency(info.cpu.frequency_mhz));
    kv(
        &mut out,
        "Memory",
        &format!(
            "{} total · {} used",
            format_bytes(info.memory.total_bytes),
            format_bytes(info.memory.used_bytes)
        ),
    );
    if let Some(board) = &info.motherboard {
        kv(&mut out, "Motherboard", format!("{} {}", board.manufacturer, board.product).trim());
    }
    if let Some(bios) = &info.bios {
        kv(&mut out, "BIOS", format!("{} {}", bios.vendor, bios.version).trim());
    }
    out.push_str("</div></div>");

    // GPUs.
    if !info.gpus.is_empty() {
        let _ = writeln!(out, "<h2>Graphics</h2><div class=\"grid\">");
        for gpu in &info.gpus {
            let _ = writeln!(out, "<div class=\"card\"><h3>{}</h3>", escape(gpu.name.as_str()));
            kv(&mut out, "Vendor", &gpu.vendor);
            kv(&mut out, "Driver", &gpu.driver_version);
            kv(&mut out, "VRAM", &format_bytes(gpu.memory_bytes));
            let _ = writeln!(out, "</div>");
        }
        out.push_str("</div>");
    }

    // Displays.
    if !info.displays.is_empty() {
        let _ = writeln!(out, "<h2>Displays</h2><div class=\"grid\">");
        for display in &info.displays {
            let _ = writeln!(
                out,
                "<div class=\"card\"><h3>{}</h3>",
                if display.name.is_empty() { "Display".to_string() } else { escape(display.name.as_str()) }
            );
            kv(
                &mut out,
                "Resolution",
                &format!("{}×{} @ {} Hz{}", display.width, display.height, display.refresh_rate, if display.is_primary { " (primary)" } else { "" }),
            );
            let _ = writeln!(out, "</div>");
        }
        out.push_str("</div>");
    }

    // Storage: logical volumes + physical disks.
    if !info.disks.is_empty() {
        let _ = writeln!(out, "<h2>Storage</h2><table><thead><tr><th>Volume</th><th>FS</th><th>Total</th><th>Free</th><th>Kind</th></tr></thead><tbody>");
        for disk in &info.disks {
            let _ = writeln!(
                out,
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape(disk.mount_point.as_str()),
                escape(disk.file_system.as_str()),
                format_bytes(disk.total_bytes),
                format_bytes(disk.available_bytes),
                disk.kind
            );
        }
        out.push_str("</tbody></table>");
    }
    if !info.physical_disks.is_empty() {
        let _ = writeln!(out, "<h2>Physical Disks</h2><table><thead><tr><th>Device</th><th>Type</th><th>Health</th><th>Bus</th><th>Size</th></tr></thead><tbody>");
        for disk in &info.physical_disks {
            let _ = writeln!(
                out,
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape(disk.friendly_name.as_str()),
                disk.media_type,
                disk.health_status,
                disk.bus_type,
                format_bytes(disk.size_bytes)
            );
        }
        out.push_str("</tbody></table>");
    }

    // Network interfaces.
    if !info.network.is_empty() {
        let _ = writeln!(out, "<h2>Network Interfaces</h2><table><thead><tr><th>Interface</th><th>Received</th><th>Sent</th></tr></thead><tbody>");
        for nic in &info.network {
            let _ = writeln!(
                out,
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape(nic.name.as_str()),
                format_bytes(nic.total_received_bytes),
                format_bytes(nic.total_transmitted_bytes)
            );
        }
        out.push_str("</tbody></table>");
    }

    // Temperatures.
    if !info.temperatures.is_empty() {
        let _ = writeln!(out, "<h2>Temperatures</h2><div class=\"grid\">");
        for temp in &info.temperatures {
            let value = temp.celsius.map(|c| format!("{c:.0} °C")).unwrap_or_else(|| "—".into());
            let _ = writeln!(out, "<div class=\"card\"><h3>{}</h3>", escape(temp.label.as_str()));
            kv(&mut out, "Temperature", &value);
            let _ = writeln!(out, "</div>");
        }
        out.push_str("</div>");
    }

    // Top processes by CPU.
    if !info.processes.is_empty() {
        let mut top: Vec<_> = info.processes.iter().collect();
        top.sort_by(|a, b| b.cpu_usage_percent.total_cmp(&a.cpu_usage_percent));
        top.truncate(20);
        let _ = writeln!(out, "<h2>Top Processes (CPU)</h2><table><thead><tr><th>Name</th><th>PID</th><th>CPU</th><th>Memory</th></tr></thead><tbody>");
        for process in top {
            let _ = writeln!(
                out,
                "<tr><td>{}</td><td>{}</td><td>{:.1}%</td><td>{}</td></tr>",
                escape(process.name.as_str()),
                process.pid,
                process.cpu_usage_percent,
                format_bytes(process.memory_bytes)
            );
        }
        out.push_str("</tbody></table>");
    }

    // Startup apps.
    if !info.startup_apps.is_empty() {
        let _ = writeln!(out, "<h2>Startup Applications</h2><table><thead><tr><th>Name</th><th>Command</th><th>Location</th></tr></thead><tbody>");
        for app in &info.startup_apps {
            let _ = writeln!(
                out,
                "<tr><td>{}</td><td><code>{}</code></td><td>{}</td></tr>",
                escape(app.name.as_str()),
                escape(app.command.as_str()),
                escape(app.location.as_str())
            );
        }
        out.push_str("</tbody></table>");
    }

    out.push_str("\n</body>\n</html>\n");
    out
}

/// Serialize the scan as pretty JSON, or fail loudly (it shouldn't — the struct
/// is fully `Serialize`). Kept as a function so call sites stay uniform.
pub fn json_report(info: &HardwareInfo) -> Result<String> {
    serde_json::to_string_pretty(info).map_err(|e| OptixError::Other(format!("serialize report: {e}")))
}

/// Write `bytes` to `target` atomically: a temp file in the same directory is
/// written, then renamed over the target, so a partial report never masquerades
/// as a complete one. The temp file is removed if the write or rename fails,
/// and the parent directory is created when the path has one — bare filenames
/// (no parent component) are written relative to the current directory.
pub fn write_atomic(target: &std::path::Path, bytes: &[u8]) -> Result<()> {
    // `Path::parent` returns `Some("")` for a bare filename; creating that
    // would fail, so only create a real parent directory.
    if let Some(parent) = target.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }

    let tmp = target.with_extension(format!(
        "{}.tmp",
        target.extension().and_then(|e| e.to_str()).unwrap_or("bin")
    ));
    let result = std::fs::write(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, target));
    if result.is_err() {
        // Best-effort cleanup so a failed export never leaves a stray file.
        let _ = std::fs::remove_file(&tmp);
    }
    result.map_err(|e| OptixError::Other(format!("write report to {}: {e}", target.display())))
}

fn kv(out: &mut String, key: &str, value: &str) {
    let _ = writeln!(out, "<div class=\"kv\"><span class=\"k\">{}</span><span class=\"v\">{}</span></div>", escape(key), escape(value));
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    if bytes == 0 {
        return "0 B".to_string();
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

fn format_frequency(mhz: u64) -> String {
    if mhz >= 1000 {
        format!("{:.2} GHz", mhz as f64 / 1000.0)
    } else {
        format!("{mhz} MHz")
    }
}

fn format_uptime(secs: u64) -> String {
    let d = secs / 86_400;
    let h = (secs % 86_400) / 3600;
    let m = (secs % 3600) / 60;
    if d > 0 {
        format!("{d}d {h}h {m}m")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{m}m")
    }
}

/// `YYYY-MM-DD HH:MM:SS` (UTC) from epoch milliseconds.
fn format_timestamp(ms: u64) -> String {
    let secs = (ms / 1000) as i64;
    let days = secs.div_euclid(86_400);
    let tod = secs.rem_euclid(86_400);
    let (h, m, s) = (tod / 3600, (tod % 3600) / 60, tod % 60);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02}:{s:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::hardware::{
        CpuInfo, DiskInfo, GpuInfo, MemoryInfo, OsInfo, PhysicalDiskInfo, TemperatureInfo,
    };

    fn sample_info() -> HardwareInfo {
        HardwareInfo {
            cpu: CpuInfo {
                name: "cpu".into(),
                brand: "AMD Ryzen 7".into(),
                vendor: "AuthenticAMD".into(),
                physical_cores: 8,
                logical_cores: 16,
                frequency_mhz: 3700,
                usage_percent: 12.0,
            },
            gpus: vec![GpuInfo {
                name: "GeForce RTX 4070".into(),
                vendor: "NVIDIA".into(),
                driver_version: "610.88".into(),
                memory_bytes: 12 * 1024 * 1024 * 1024,
                usage_percent: 4.0,
            }],
            memory: MemoryInfo {
                total_bytes: 32 * 1024 * 1024 * 1024,
                used_bytes: 8 * 1024 * 1024 * 1024,
                available_bytes: 24 * 1024 * 1024 * 1024,
                usage_percent: 25.0,
            },
            disks: vec![DiskInfo {
                name: "C:".into(),
                mount_point: "C:\\".into(),
                file_system: "NTFS".into(),
                total_bytes: 1024 * 1024 * 1024 * 1024,
                available_bytes: 512 * 1024 * 1024 * 1024,
                used_bytes: 512 * 1024 * 1024 * 1024,
                kind: "SSD".into(),
                is_removable: false,
            }],
            physical_disks: vec![PhysicalDiskInfo {
                friendly_name: "NVMe SSD".into(),
                media_type: "SSD".into(),
                health_status: "Healthy".into(),
                bus_type: "NVMe".into(),
                firmware_version: None,
                size_bytes: 1024 * 1024 * 1024 * 1024,
            }],
            network: vec![],
            displays: vec![],
            temperatures: vec![TemperatureInfo { label: "CPU Package".into(), celsius: Some(45.0) }],
            os: OsInfo {
                name: "Windows".into(),
                version: "11".into(),
                kernel_version: "10.0.22631".into(),
                host_name: "DESKTOP-X".into(),
                uptime_seconds: 86_400,
                build_number: Some(22631),
                is_windows_11: true,
                edition: Some("Microsoft Windows 11 Pro".into()),
            },
            motherboard: None,
            bios: None,
            processes: vec![],
            startup_apps: vec![],
            scanned_at_ms: 1_700_000_000_000,
        }
    }

    #[test]
    fn html_report_renders_key_sections() {
        let html = html_report(&sample_info());
        assert!(html.contains("Optix System Report"));
        assert!(html.contains("AMD Ryzen 7"));
        assert!(html.contains("GeForce RTX 4070"));
        assert!(html.contains("NVMe SSD"));
        assert!(html.contains("CPU Package"));
        assert!(html.contains("</html>"));
    }

    #[test]
    fn html_report_escapes_unsafe_input() {
        let mut info = sample_info();
        info.disks[0].mount_point = "C:\\<script>alert(1)</script>".into();
        let html = html_report(&info);
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn json_report_is_valid() {
        let json = json_report(&sample_info()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["cpu"]["brand"], "AMD Ryzen 7");
    }

    #[test]
    fn write_atomic_creates_parent_and_leaves_no_temp() {
        let dir = std::env::temp_dir().join(format!("optix-report-atomic-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let target = dir.join("nested").join("report.html");

        write_atomic(&target, b"<html>hi</html>").unwrap();

        assert_eq!(std::fs::read(&target).unwrap(), b"<html>hi</html>");
        assert!(!target.with_extension("html.tmp").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn write_atomic_cleans_temp_on_rename_failure() {
        let dir = std::env::temp_dir().join(format!("optix-report-fail-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // A directory at the target path makes the final rename fail.
        let target = dir.join("report.html");
        std::fs::create_dir(&target).unwrap();

        assert!(write_atomic(&target, b"x").is_err());
        // The temp file must not linger after the failed export.
        assert!(!target.with_extension("html.tmp").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn write_atomic_accepts_bare_filenames() {
        let dir = std::env::temp_dir().join(format!("optix-report-bare-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // A relative bare filename has no parent component; it must not be
        // rejected by an empty parent-dir create.
        let target = std::path::Path::new("report.json");
        let old_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();
        let result = write_atomic(target, b"{}");
        std::env::set_current_dir(old_cwd).unwrap();

        result.unwrap();
        assert_eq!(std::fs::read(dir.join("report.json")).unwrap(), b"{}");
        assert!(!dir.join("report.json.tmp").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
