//! Phase 11 — Crash Recovery.
//!
//! Cross-platform parsing/classification (WER `Report.wer`, rendered Event
//! Log XML, exception codes, faulting modules, minidump filenames) plus the
//! orchestration that merges event-log + WER + minidump sources into a single
//! crash list, and `CrashReport.zip` generation. The Event Log query itself is
//! Windows-gated in `win::crash`.

use std::io::Write;
use std::path::Path;

use crate::db::sqlite::data_dir;
use crate::error::Result;
use crate::models::crash::CrashReport;
use crate::win;

/// A parsed Event Log entry (Application Error / WER / TDR).
#[derive(Debug, Clone)]
pub struct EventInfo {
    pub event_id: i64,
    pub app: String,
    pub pid: Option<i64>,
    pub module: String,
    pub exception_code: String,
    pub detected_at_ms: i64,
}

/// A parsed WER `Report.wer`.
#[derive(Debug, Clone)]
pub struct WerInfo {
    pub app: String,
    pub module: String,
    pub exception_code: String,
    pub event_time_ms: i64,
}

struct WerScan {
    info: WerInfo,
    wer_path: String,
    minidump_path: Option<String>,
}

/// Convert a Windows FILETIME (100 ns ticks since 1601) to unix ms.
pub fn filetime_to_ms(filetime: i64) -> i64 {
    filetime / 10_000 - 11_644_473_600_000
}

/// Parse an ISO-8601 timestamp (`YYYY-MM-DDTHH:MM:SS[.frac][Z]`) to unix ms.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn parse_iso8601_ms(s: &str) -> Option<i64> {
    if s.len() < 19 {
        return None;
    }
    let year = s[0..4].parse::<i32>().ok()?;
    let month = s[5..7].parse::<u32>().ok()?;
    let day = s[8..10].parse::<u32>().ok()?;
    let hour = s[11..13].parse::<u32>().ok()?;
    let min = s[14..16].parse::<u32>().ok()?;
    let sec = s[17..19].parse::<u32>().ok()?;
    let days = days_from_civil(year, month, day);
    Some((days * 86_400 + hour as i64 * 3_600 + min as i64 * 60 + sec as i64) * 1000)
}

/// Days since 1970-01-01 for a civil date (Howard Hinnant's algorithm).
#[cfg_attr(not(windows), allow(dead_code))]
fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y } as i64;
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Extract the text content of the first `<tag>...</tag>`.
#[cfg_attr(not(windows), allow(dead_code))]
fn xml_tag_content(text: &str, tag: &str) -> Option<String> {
    let open = text.find(&format!("<{tag}"))?;
    let after_open = text[open..].find('>')? + open + 1;
    let close = text[after_open..].find(&format!("</{tag}>"))? + after_open;
    Some(text[after_open..close].to_string())
}

/// Extract `<Data Name="name">value</Data>`.
#[cfg_attr(not(windows), allow(dead_code))]
fn xml_data_value(text: &str, name: &str) -> Option<String> {
    let needle = format!("<Data Name=\"{name}\">");
    let start = text.find(&needle)? + needle.len();
    let end = text[start..].find("</Data>")? + start;
    Some(text[start..end].to_string())
}

/// Extract `attr` of the first `<tag ...>`.
#[cfg_attr(not(windows), allow(dead_code))]
fn xml_attr_value(text: &str, tag: &str, attr: &str) -> Option<String> {
    let open = text.find(&format!("<{tag}"))?;
    let end = text[open..].find('>')? + open;
    let tag_text = &text[open..end];
    let needle = format!("{attr}=\"");
    let a = tag_text.find(&needle)? + needle.len();
    let rest = &tag_text[a..];
    let b = rest.find('"')?;
    Some(rest[..b].to_string())
}

/// Parse a rendered Event Log XML document into an `EventInfo`.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn parse_event_xml(xml: &str) -> Option<EventInfo> {
    let event_id = xml_tag_content(xml, "EventID")?.trim().parse::<i64>().ok()?;
    let app = xml_data_value(xml, "AppName").unwrap_or_default();
    let module = xml_data_value(xml, "FaultingModuleName").unwrap_or_default();
    let exception_code = xml_data_value(xml, "ExceptionCode").unwrap_or_default();
    let pid = xml_data_value(xml, "ProcessId")
        .and_then(|s| s.trim().parse::<i64>().ok())
        .or_else(|| xml_data_value(xml, "Pid").and_then(|s| s.trim().parse::<i64>().ok()));
    let detected_at_ms = xml_attr_value(xml, "TimeCreated", "SystemTime")
        .and_then(|s| parse_iso8601_ms(&s))
        .unwrap_or(0);
    Some(EventInfo {
        event_id,
        app,
        pid,
        module,
        exception_code,
        detected_at_ms,
    })
}

/// Parse a WER `Report.wer` into a `WerInfo` (None for non-crash reports).
pub fn parse_wer(text: &str) -> Option<WerInfo> {
    let mut sig_names: Vec<(usize, String)> = Vec::new();
    let mut sig_values: Vec<(usize, String)> = Vec::new();
    let mut event_time: Option<i64> = None;
    let mut is_crash = false;

    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("EventType=") {
            is_crash = line.contains("APPCRASH") || line.contains("BEX");
        }
        if let Some(v) = line.strip_prefix("EventTime=") {
            event_time = v.trim().parse::<i64>().ok().map(filetime_to_ms);
        }
        if let Some(rest) = line.strip_prefix("Sig[") {
            let Some(idx_end) = rest.find(']') else {
                continue;
            };
            let Ok(idx) = rest[..idx_end].parse::<usize>() else {
                continue;
            };
            let after = &rest[idx_end + 1..];
            if let Some(name) = after.strip_prefix(".Name=") {
                sig_names.push((idx, name.to_string()));
            } else if let Some(value) = after.strip_prefix(".Value=") {
                sig_values.push((idx, value.to_string()));
            }
        }
    }

    if !is_crash {
        return None;
    }
    let get = |target: &str| -> Option<String> {
        let idx = sig_names.iter().find(|(_, n)| n == target)?.0;
        sig_values
            .iter()
            .find(|(i, _)| *i == idx)
            .map(|(_, v)| v.clone())
    };
    Some(WerInfo {
        app: get("Application Name")?,
        module: get("Fault Module Name").unwrap_or_default(),
        exception_code: get("Exception Code").unwrap_or_default(),
        event_time_ms: event_time.unwrap_or(0),
    })
}

/// Human-readable name + severity for an exception code.
pub fn classify_exception(code: &str) -> (String, &'static str) {
    match code.trim().trim_start_matches("0x").to_ascii_lowercase().as_str() {
        "c0000005" => ("Access violation".into(), "medium"),
        "c0000374" => ("Heap corruption".into(), "high"),
        "c0000409" => ("Stack buffer overrun".into(), "high"),
        "c00000fd" => ("Stack overflow".into(), "high"),
        "c0000094" => ("Integer divide by zero".into(), "medium"),
        "c0000135" => ("Module not found".into(), "medium"),
        "80000003" => ("Breakpoint".into(), "low"),
        "e0434352" => (".NET exception".into(), "low"),
        _ => ("Unknown exception".into(), "medium"),
    }
}

/// Recommendation for a faulting module.
pub fn classify_module(module: &str) -> &'static str {
    let m = module.to_ascii_lowercase();
    if m.contains("nvwgf") || m.contains("nvlddmkm") || m.contains("nvcuda") || m.contains("nvoglv") {
        "NVIDIA driver fault — update or clean-install your NVIDIA drivers."
    } else if m.contains("atidxx") || m.contains("amdxx") || m.contains("amdvlk") || m.contains("atig6") {
        "AMD driver fault — update or clean-install your AMD drivers."
    } else if m.contains("igd") || m.contains("igx") || m.contains("ig9icd") {
        "Intel graphics driver fault — update your Intel drivers."
    } else if m.contains("dxgkrnl") || m.contains("d3d") || m.contains("directx") || m.contains("gamebar") || m.contains("gameinput") {
        "Windows graphics/DirectX fault — update Windows and GPU drivers."
    } else if m.contains("ntdll") || m.contains("kernelbase") || m.contains("kernel32") {
        "System DLL fault — run `sfc /scannow` and check for OS updates."
    } else {
        "Application fault — update the application or verify its installation."
    }
}

/// Extract the app name from a user minidump filename (`game.exe.1234.dmp`).
pub fn parse_minidump_app(name: &str) -> String {
    let stem = name.strip_suffix(".dmp").unwrap_or(name);
    match stem.rsplit_once('.') {
        Some((prefix, suffix))
            if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) =>
        {
            prefix.to_string()
        }
        _ => stem.to_string(),
    }
}

/// Scan minidump directories for `.dmp` files (app, path, mtime ms).
fn scan_minidumps(dirs: &[String]) -> Vec<(String, String, i64)> {
    let mut out = Vec::new();
    for dir in dirs {
        let Ok(rd) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) != Some("dmp") {
                continue;
            }
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let mtime = p
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            out.push((parse_minidump_app(&name), p.to_string_lossy().into_owned(), mtime));
        }
    }
    out
}

/// Scan WER report directories, parsing each `Report.wer`.
fn scan_wer_dirs(dirs: &[String]) -> Vec<WerScan> {
    let mut out = Vec::new();
    for dir in dirs {
        let Ok(rd) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let sub = entry.path();
            if !sub.is_dir() {
                continue;
            }
            let wer_path = sub.join("Report.wer");
            let Ok(text) = std::fs::read_to_string(&wer_path) else {
                continue;
            };
            let Some(info) = parse_wer(&text) else {
                continue;
            };
            let minidump_path = std::fs::read_dir(&sub).ok().and_then(|rd| {
                rd.flatten()
                    .map(|e| e.path())
                    .find(|p| p.extension().and_then(|e| e.to_str()) == Some("dmp"))
            });
            out.push(WerScan {
                info,
                wer_path: wer_path.to_string_lossy().into_owned(),
                minidump_path: minidump_path.map(|p| p.to_string_lossy().into_owned()),
            });
        }
    }
    out
}

/// Build a `CrashReport`, computing derived severity + recommendation.
// Positional record builder with three call sites; wrapping the optional
// fields in a struct would repeat nine field names at each site for no
// clarity gain.
#[allow(clippy::too_many_arguments)]
fn build_report(
    detected_at: i64,
    app: String,
    pid: Option<i64>,
    event_id: Option<i64>,
    module: Option<String>,
    exception_code: Option<String>,
    wer: Option<String>,
    minidump: Option<String>,
    source: &str,
) -> CrashReport {
    let (exception_name, exc_severity) = match exception_code.as_deref() {
        Some(c) if !c.is_empty() => {
            let (n, s) = classify_exception(c);
            (Some(n), s)
        }
        _ => (None, "medium"),
    };

    let module_lc = module.as_deref().unwrap_or("").to_ascii_lowercase();
    let is_driver = module_lc.contains("nvwgf")
        || module_lc.contains("nvlddmkm")
        || module_lc.contains("atidxx")
        || module_lc.contains("amdxx")
        || module_lc.contains("igd")
        || module_lc.contains("dxgkrnl")
        || module_lc.contains("gameinput")
        || module_lc.contains("gamebar");
    let severity = if event_id == Some(4101) || is_driver {
        "high"
    } else {
        exc_severity
    };

    let recommendation = if event_id == Some(4101) {
        "Display driver stopped responding (TDR) — update or clean-install your GPU drivers."
            .to_string()
    } else if module.as_deref().map(|m| !m.is_empty()).unwrap_or(false) {
        classify_module(module.as_deref().unwrap()).to_string()
    } else {
        "Update the application or verify its installation.".to_string()
    };

    CrashReport {
        detected_at,
        app,
        pid,
        event_id,
        module,
        exception_code,
        exception_name,
        severity: severity.to_string(),
        recommendation,
        wer_report_path: wer,
        minidump_path: minidump,
        report_zip_path: None,
        source: source.to_string(),
    }
}

fn norm_code(s: &str) -> String {
    s.trim().trim_start_matches("0x").trim_start_matches("0X").to_ascii_lowercase()
}

/// Whether a crash (app/exception/module at time `t`) is already represented.
fn matches_existing(
    reports: &[CrashReport],
    app: &str,
    module: &str,
    exception: &str,
    t: i64,
    window_ms: i64,
) -> bool {
    reports.iter().any(|r| {
        r.app.eq_ignore_ascii_case(app)
            && (exception.is_empty()
                || r
                    .exception_code
                    .as_deref()
                    .map(|c| norm_code(c) == norm_code(exception))
                    .unwrap_or(false))
            && (module.is_empty()
                || r
                    .module
                    .as_deref()
                    .map(|m| m.eq_ignore_ascii_case(module))
                    .unwrap_or(false))
            && (r.detected_at - t).abs() < window_ms
    })
}

/// Scan every crash source (Event Log + WER + minidumps) and merge into a
/// deduplicated, newest-first list.
pub fn scan_crashes() -> Vec<CrashReport> {
    let mut reports = Vec::new();

    for ev in win::crash::query_application_events(200) {
        let app = if ev.app.is_empty() {
            if ev.event_id == 4101 {
                "Display driver (TDR)".to_string()
            } else {
                "Unknown application".to_string()
            }
        } else {
            ev.app
        };
        reports.push(build_report(
            ev.detected_at_ms,
            app,
            ev.pid,
            Some(ev.event_id),
            (!ev.module.is_empty()).then_some(ev.module),
            (!ev.exception_code.is_empty()).then_some(ev.exception_code),
            None,
            None,
            "event_log",
        ));
    }

    for ws in scan_wer_dirs(&win::crash::wer_directories()) {
        if matches_existing(
            &reports,
            &ws.info.app,
            &ws.info.module,
            &ws.info.exception_code,
            ws.info.event_time_ms,
            10 * 60 * 1000,
        ) {
            continue;
        }
        reports.push(build_report(
            ws.info.event_time_ms,
            ws.info.app,
            None,
            Some(1001),
            (!ws.info.module.is_empty()).then_some(ws.info.module),
            (!ws.info.exception_code.is_empty()).then_some(ws.info.exception_code),
            Some(ws.wer_path),
            ws.minidump_path,
            "wer",
        ));
    }

    for (app, path, mtime) in scan_minidumps(&win::crash::minidump_directories()) {
        let already = reports.iter().any(|r| {
            r.minidump_path.as_deref() == Some(path.as_str())
                || (r.app.eq_ignore_ascii_case(&app) && (r.detected_at - mtime).abs() < 10 * 60 * 1000)
        });
        if already {
            continue;
        }
        reports.push(build_report(mtime, app, None, None, None, None, None, Some(path), "minidump"));
    }

    reports.sort_by_key(|r| std::cmp::Reverse(r.detected_at));
    reports
}

/// Sanitize a filename fragment for the zip name.
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Serialize a minimal crash summary for the live crash-watch event.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashAlert {
    pub detected_at_ms: i64,
    pub app: String,
    pub pid: Option<i64>,
    pub event_id: Option<i64>,
    pub module: String,
    pub exception_code: String,
}

/// Compute the alert payload for events newer than `last_seen`. Empty before
/// the first poll (`primed == false`), so crashes that pre-date the app's
/// launch are never reported as "new".
fn alerts_for(events: &[EventInfo], last_seen: i64, primed: bool) -> Vec<CrashAlert> {
    if !primed {
        return Vec::new();
    }
    events
        .iter()
        .filter(|e| e.detected_at_ms > last_seen)
        .map(|e| CrashAlert {
            detected_at_ms: e.detected_at_ms,
            app: e.app.clone(),
            pid: e.pid,
            event_id: Some(e.event_id),
            module: e.module.clone(),
            exception_code: e.exception_code.clone(),
        })
        .collect()
}

/// Spawn a background thread that polls the Application event log for crash
/// events every `interval` seconds and emits `optix://crash-detected` when new
/// crashes appear. The frontend subscribes to that event to refresh the Crash
/// Reports page live. Best-effort: event-log reads are cheap and the thread
/// simply sleeps through failures. The first poll only primes the watermark.
pub fn spawn_crash_watch(app: tauri::AppHandle, interval_secs: u64) {
    use tauri::Emitter;

    std::thread::spawn(move || {
        let interval = std::time::Duration::from_secs(interval_secs.max(5));
        let mut last_seen: i64 = 0;
        let mut primed = false;
        loop {
            let events = win::crash::query_application_events(50);
            let newest = events.iter().map(|e| e.detected_at_ms).max().unwrap_or(0);
            let alerts = alerts_for(&events, last_seen, primed);
            if !alerts.is_empty() {
                if let Err(e) = app.emit("optix://crash-detected", &alerts) {
                    crate::logging::warn(&format!("crash event emit failed: {e}"));
                }
            }
            primed = true;
            last_seen = newest;
            std::thread::sleep(interval);
        }
    });
}

/// Generate a `CrashReport.zip` for a crash, returning the zip path.
pub fn generate_report_zip(crash: &CrashReport) -> Result<String> {
    generate_report_zip_to(crash, &data_dir().join("CrashReports"))
}

fn generate_report_zip_to(crash: &CrashReport, out_dir: &Path) -> Result<String> {
    std::fs::create_dir_all(out_dir)?;
    let name = format!(
        "CrashReport_{}_{}.zip",
        sanitize_filename(&crash.app),
        super::now_ms()
    );
    let path = out_dir.join(&name);

    let file = std::fs::File::create(&path)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = || {
        zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
    };

    let summary = serde_json::to_string_pretty(&serde_json::json!({
        "crash": crash,
        "generated_at_ms": super::now_ms(),
        "optix_version": env!("CARGO_PKG_VERSION"),
    }))?;
    zip.start_file("summary.json", opts())?;
    zip.write_all(summary.as_bytes())?;

    if let Some(wer) = &crash.wer_report_path {
        if let Ok(data) = std::fs::read(wer) {
            zip.start_file("Report.wer", opts())?;
            zip.write_all(&data)?;
        }
    }

    if let Some(dmp) = &crash.minidump_path {
        let too_big = std::fs::metadata(dmp)
            .map(|m| m.len() > 200 * 1024 * 1024)
            .unwrap_or(true);
        if too_big {
            zip.start_file("minidump_skipped.txt", opts())?;
            zip.write_all(b"The minidump exceeded the 200 MB cap and was not included.")?;
        } else if let Ok(mut data) = std::fs::File::open(dmp) {
            let fname = Path::new(dmp)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "minidump.dmp".into());
            zip.start_file(fname, opts())?;
            // Stream instead of buffering up to 200 MB in memory.
            std::io::copy(&mut data, &mut zip)?;
        }
    }

    zip.finish()?;
    Ok(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVENT_XML: &str = r#"<Event xmlns="http://schemas.microsoft.com/win/2004/08/events/event">
  <System>
    <Provider Name="Application Error" />
    <EventID Qualifiers="0">1000</EventID>
    <TimeCreated SystemTime="2024-01-15T10:30:00.0000000Z" />
  </System>
  <EventData>
    <Data Name="AppName">cs2.exe</Data>
    <Data Name="FaultingModuleName">nvwgf2umx.dll</Data>
    <Data Name="ExceptionCode">0xc0000005</Data>
    <Data Name="ProcessId">1234</Data>
  </EventData>
</Event>"#;

    const WER: &str = "\
Version=1
EventType=APPCRASH
EventTime=133600000000000000
Sig[0].Name=Application Name
Sig[0].Value=game.exe
Sig[3].Name=Fault Module Name
Sig[3].Value=nvwgf2umx.dll
Sig[5].Name=Exception Code
Sig[5].Value=c0000005
";

    #[test]
    fn parses_event_xml() {
        let info = parse_event_xml(EVENT_XML).unwrap();
        assert_eq!(info.event_id, 1000);
        assert_eq!(info.app, "cs2.exe");
        assert_eq!(info.module, "nvwgf2umx.dll");
        assert_eq!(info.exception_code, "0xc0000005");
        assert_eq!(info.pid, Some(1234));
        assert_eq!(info.detected_at_ms, 1705314600000);
    }

    #[test]
    fn parses_wer() {
        let info = parse_wer(WER).unwrap();
        assert_eq!(info.app, "game.exe");
        assert_eq!(info.module, "nvwgf2umx.dll");
        assert_eq!(info.exception_code, "c0000005");
    }

    #[test]
    fn wer_ignores_non_crash_reports() {
        assert!(parse_wer("EventType=APPHANG\nSig[0].Name=Application Name\nSig[0].Value=x\n").is_none());
    }

    #[test]
    fn classifies_exceptions_and_modules() {
        assert_eq!(classify_exception("0xc0000005").0, "Access violation");
        assert_eq!(classify_exception("c0000005").1, "medium");
        assert_eq!(classify_exception("0xc0000374").1, "high");
        assert!(classify_module("nvwgf2umx.dll").contains("NVIDIA"));
        assert!(classify_module("atidxx64.dll").contains("AMD"));
        assert!(classify_module("igdumdim64.dll").contains("Intel"));
        assert!(classify_module("something_else.dll").contains("Application"));
    }

    #[test]
    fn filetime_conversion() {
        // 133600000000000000 -> 2024-01-15 ~ known value; just check monotonic sanity.
        let ms = filetime_to_ms(133600000000000000);
        assert!(ms > 1_600_000_000_000);
    }

    #[test]
    fn parses_iso8601() {
        assert_eq!(parse_iso8601_ms("2024-01-15T10:30:00.0000000Z"), Some(1705314600000));
        assert_eq!(parse_iso8601_ms("short"), None);
    }

    #[test]
    fn parses_minidump_filenames() {
        assert_eq!(parse_minidump_app("cs2.exe.1234.dmp"), "cs2.exe");
        assert_eq!(parse_minidump_app("game.exe.dmp"), "game.exe");
        assert_eq!(parse_minidump_app("012524-12345-01.dmp"), "012524-12345-01");
    }

    #[test]
    fn crash_watch_primes_before_alerting() {
        let ev = |t: i64| EventInfo {
            event_id: 1000,
            app: "a.exe".into(),
            pid: None,
            module: "m.dll".into(),
            exception_code: "c0000005".into(),
            detected_at_ms: t,
        };
        let events = vec![ev(2000), ev(1000)];
        // First poll only primes the watermark — no false "new crash" burst.
        assert!(alerts_for(&events, 0, false).is_empty());
        // Nothing newer than the watermark → no alerts.
        assert!(alerts_for(&events, 2000, true).is_empty());
        // Only events strictly newer than the watermark are reported.
        let alerts = alerts_for(&events, 1000, true);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].detected_at_ms, 2000);
        assert_eq!(alerts[0].app, "a.exe");
    }

    #[test]
    fn generates_a_valid_zip() {
        let dir = std::env::temp_dir().join(format!("optix_crash_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let wer = dir.join("Report.wer");
        std::fs::write(&wer, "Version=1\nEventType=APPCRASH\n").unwrap();
        let dmp = dir.join("game.exe.1234.dmp");
        std::fs::write(&dmp, vec![0u8; 32]).unwrap();
        let crash = CrashReport {
            detected_at: 0,
            app: "cs2.exe".into(),
            pid: Some(1234),
            event_id: Some(1000),
            module: Some("nvwgf2umx.dll".into()),
            exception_code: Some("0xc0000005".into()),
            exception_name: Some("Access violation".into()),
            severity: "high".into(),
            recommendation: "update drivers".into(),
            wer_report_path: Some(wer.to_string_lossy().into_owned()),
            minidump_path: Some(dmp.to_string_lossy().into_owned()),
            report_zip_path: None,
            source: "event_log".into(),
        };
        let path = generate_report_zip_to(&crash, &dir).unwrap();
        assert!(Path::new(&path).is_file());
        let archive = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
        let names: Vec<&str> = archive.file_names().collect();
        assert!(names.contains(&"summary.json"));
        assert!(names.contains(&"Report.wer"));
        assert!(names.contains(&"game.exe.1234.dmp"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
