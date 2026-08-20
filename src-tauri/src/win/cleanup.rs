//! Windows-only cleanup helpers (Phase 3): Recycle Bin size/emptying and the
//! DISM component cleanup subprocess. The engine keeps the generic directory
//! scanner; these are the special categories that need Win32 APIs.

use crate::error::{OptixError, Result};

/// Total bytes + item count currently in the Recycle Bin.
#[cfg(windows)]
pub fn recycle_bin_size() -> (u64, u64) {
    use windows_sys::Win32::UI::Shell::{SHQueryRecycleBinW, SHQUERYRBINFO};

    let mut info: SHQUERYRBINFO = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<SHQUERYRBINFO>() as u32;
    let hr = unsafe { SHQueryRecycleBinW(std::ptr::null(), &mut info) };
    if hr != 0 {
        return (0, 0);
    }
    (info.i64Size as u64, info.i64NumItems as u64)
}

#[cfg(not(windows))]
pub fn recycle_bin_size() -> (u64, u64) {
    (0, 0)
}

/// Empty the Recycle Bin (no confirmation dialog; the UI confirms first).
#[cfg(windows)]
pub fn empty_recycle_bin() -> Result<()> {
    use windows_sys::Win32::UI::Shell::{
        SHEmptyRecycleBinW, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND,
    };

    let hr = unsafe {
        SHEmptyRecycleBinW(
            std::ptr::null_mut(),
            std::ptr::null(),
            SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND,
        )
    };
    if hr != 0 {
        return Err(OptixError::Windows(format!(
            "SHEmptyRecycleBin failed: 0x{hr:X}"
        )));
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn empty_recycle_bin() -> Result<()> {
    Err(OptixError::UnsupportedPlatform("Recycle Bin".into()))
}

/// Run `dism /online /cleanup-image /startcomponentcleanup` (admin required).
/// Blocks, streaming output to `on_line`; returns the full output for the UI.
#[cfg(windows)]
pub fn run_dism_component_cleanup(on_line: &mut impl FnMut(&str)) -> Result<String> {
    use std::io::{BufRead, BufReader};
    use std::process::Command;

    let mut child = Command::new("dism.exe")
        .args(["/online", "/cleanup-image", "/startcomponentcleanup"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| OptixError::Windows(format!("cannot start dism.exe: {e}")))?;

    let mut out = String::new();
    let mut drain = |pipe: std::process::ChildStdout| {
        let reader = BufReader::new(pipe);
        for line in reader.lines().map_while(std::io::Result::ok) {
            on_line(line.trim());
            out.push_str(line.trim());
            out.push('\n');
        }
    };
    if let Some(stdout) = child.stdout.take() {
        drain(stdout);
    }
    let status = child
        .wait()
        .map_err(|e| OptixError::Windows(format!("dism.exe wait failed: {e}")))?;
    if !status.success() {
        return Ok(out); // dism reports errors in its own output; surface it
    }
    Ok(out)
}

#[cfg(not(windows))]
pub fn run_dism_component_cleanup(_on_line: &mut impl FnMut(&str)) -> Result<String> {
    Err(OptixError::UnsupportedPlatform("DISM component cleanup".into()))
}

/// Whether the Windows Update service (`wuauserv`) is currently running, in
/// which case SoftwareDistribution cleanup should be skipped.
#[cfg(windows)]
pub fn update_service_busy() -> bool {
    crate::win::services::list_services()
        .iter()
        .any(|s| s.name.eq_ignore_ascii_case("wuauserv") && s.state == "running")
}

#[cfg(not(windows))]
pub fn update_service_busy() -> bool {
    false
}