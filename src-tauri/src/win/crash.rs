//! Windows crash detection: the Application event log (EvtQuery/EvtNext/
//! EvtRender, rendered to XML and parsed by `engine::crash`) plus the WER
//! report and minidump directory locations.

use crate::engine::crash::EventInfo;
#[cfg(windows)]
use crate::engine::crash::parse_event_xml;

/// Encode a UTF-16 string with a trailing NUL.
#[cfg(windows)]
fn encode_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Render an event handle to XML (two-pass buffer pattern).
#[cfg(windows)]
fn render_event_xml(event: isize) -> Option<String> {
    use windows_sys::Win32::System::EventLog as evt;

    let mut size = 0u32;
    let _ = unsafe {
        evt::EvtRender(
            0,
            event,
            evt::EvtRenderEventXml,
            0,
            std::ptr::null_mut(),
            &mut size,
            std::ptr::null_mut(),
        )
    };
    if size == 0 {
        return None;
    }
    let mut buffer = vec![0u16; (size / 2 + 1) as usize];
    let mut used = size;
    let ok = unsafe {
        evt::EvtRender(
            0,
            event,
            evt::EvtRenderEventXml,
            size,
            buffer.as_mut_ptr() as *mut core::ffi::c_void,
            &mut used,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return None;
    }
    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    Some(String::from_utf16_lossy(&buffer[..len]))
}

/// Query the Application channel for the crash event IDs (1000 Application
/// Error, 1001 WER, 4101 display-driver TDR), newest first.
#[cfg(windows)]
pub fn query_application_events(max: usize) -> Vec<EventInfo> {
    use windows_sys::core::PCWSTR;
    use windows_sys::Win32::System::EventLog as evt;

    let channel = encode_wide("Application");
    let query = encode_wide("*[System[(EventID=1000 or EventID=1001 or EventID=4101)]]");

    let mut out = Vec::new();
    let resultset = unsafe {
        evt::EvtQuery(
            0,
            channel.as_ptr() as PCWSTR,
            query.as_ptr() as PCWSTR,
            evt::EvtQueryReverseDirection,
        )
    };
    if resultset == 0 {
        return out;
    }

    const BATCH: usize = 16;
    let mut buffer = [0isize; BATCH];
    loop {
        let mut returned = 0u32;
        let ok = unsafe {
            evt::EvtNext(resultset, BATCH as u32, buffer.as_mut_ptr(), 1000, 0, &mut returned)
        };
        if ok == 0 {
            break;
        }
        for i in 0..returned as usize {
            let event = buffer[i];
            if let Some(xml) = render_event_xml(event) {
                if let Some(info) = parse_event_xml(&xml) {
                    out.push(info);
                    if out.len() >= max {
                        // Close every handle in this batch (including the one
                        // we just consumed) before returning — event handles
                        // are kernel objects and must not be leaked.
                        for handle in &buffer[i..returned as usize] {
                            unsafe { evt::EvtClose(*handle) };
                        }
                        unsafe { evt::EvtClose(resultset) };
                        return out;
                    }
                }
            }
            unsafe { evt::EvtClose(event) };
        }
        if (returned as usize) < BATCH {
            break;
        }
    }
    unsafe { evt::EvtClose(resultset) };
    out
}

#[cfg(not(windows))]
pub fn query_application_events(_max: usize) -> Vec<EventInfo> {
    Vec::new()
}

/// WER report directories (ReportArchive + ReportQueue, per-user + machine).
#[cfg(windows)]
pub fn wer_directories() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        out.push(format!(r"{local}\Microsoft\Windows\WER\ReportArchive"));
        out.push(format!(r"{local}\Microsoft\Windows\WER\ReportQueue"));
    }
    if let Ok(pd) = std::env::var("PROGRAMDATA") {
        out.push(format!(r"{pd}\Microsoft\Windows\WER\ReportArchive"));
        out.push(format!(r"{pd}\Microsoft\Windows\WER\ReportQueue"));
    }
    out
}

#[cfg(not(windows))]
pub fn wer_directories() -> Vec<String> {
    Vec::new()
}

/// Minidump directories (user CrashDumps + system Minidump).
#[cfg(windows)]
pub fn minidump_directories() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        out.push(format!(r"{local}\CrashDumps"));
    }
    if let Ok(windir) = std::env::var("WINDIR") {
        out.push(format!(r"{windir}\Minidump"));
    }
    out
}

#[cfg(not(windows))]
pub fn minidump_directories() -> Vec<String> {
    Vec::new()
}
