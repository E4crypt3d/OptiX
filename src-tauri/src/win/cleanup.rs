//! Windows-only cleanup helpers (Phase 3): Recycle Bin size/emptying and the
//! DISM component cleanup subprocess. The engine keeps the generic directory
//! scanner; these are the special categories that need Win32 APIs.

use crate::error::{OptixError, Result};

#[cfg(windows)]
use std::io::{BufRead, BufReader};

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
/// Blocks until dism exits and returns its full output for the UI. Both stdout
/// and stderr are drained on threads — reading only stdout while the child
/// fills the stderr pipe would deadlock once the pipe buffer fills.
#[cfg(windows)]
pub fn run_dism_component_cleanup() -> Result<String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    // Spawn without a console window — a GUI app must not flash one.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut child = Command::new("dism.exe")
        .args(["/online", "/cleanup-image", "/startcomponentcleanup"])
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| OptixError::Windows(format!("cannot start dism.exe: {e}")))?;

    let mut drains = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        drains.push(std::thread::spawn(move || drain_lines(stdout)));
    }
    if let Some(stderr) = child.stderr.take() {
        drains.push(std::thread::spawn(move || drain_lines(stderr)));
    }

    let mut out = String::new();
    for drain in drains {
        if let Ok(mut lines) = drain.join() {
            for line in lines.drain(..) {
                out.push_str(&line);
                out.push('\n');
            }
        }
    }

    let status = child
        .wait()
        .map_err(|e| OptixError::Windows(format!("dism.exe wait failed: {e}")))?;
    if !status.success() {
        return Ok(out); // dism reports errors in its own output; surface it
    }
    Ok(out)
}

/// Read a child pipe to EOF, returning its trimmed lines.
#[cfg(windows)]
fn drain_lines<R: std::io::Read>(pipe: R) -> Vec<String> {
    let reader = BufReader::new(pipe);
    reader
        .lines()
        .map_while(std::io::Result::ok)
        .map(|line| line.trim().to_string())
        .collect()
}

#[cfg(not(windows))]
pub fn run_dism_component_cleanup() -> Result<String> {
    Err(OptixError::UnsupportedPlatform("DISM component cleanup".into()))
}

/// Whether the Windows Update service (`wuauserv`) is currently running, in
/// which case SoftwareDistribution cleanup should be skipped. A single SCM
/// query — not a full service enumeration (this runs on every cleanup scan).
#[cfg(windows)]
pub fn update_service_busy() -> bool {
    crate::win::services::service_running("wuauserv")
}

#[cfg(not(windows))]
pub fn update_service_busy() -> bool {
    false
}