//! PresentMon capture runner (Windows-only). Locates `PresentMon64.exe` and
//! runs a timed per-process capture to CSV. PresentMon is an ETW-based tool
//! (MIT); it is bundled by the release build under `resources/`.

use crate::error::{OptixError, Result};

/// Locate the PresentMon binary: next to our exe, in a `resources` dir, in the
/// app data dir, or on PATH.
#[cfg(windows)]
pub fn find_presentmon() -> Option<String> {
    use std::path::PathBuf;

    let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf()));
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(dir) = &exe_dir {
        candidates.push(dir.join("PresentMon64.exe"));
        candidates.push(dir.join("resources").join("PresentMon64.exe"));
    }
    if let Ok(home) = std::env::var("PROGRAMDATA") {
        candidates.push(
            PathBuf::from(home)
                .join("Optix")
                .join("presentmon")
                .join("PresentMon64.exe"),
        );
    }
    candidates.push(PathBuf::from("PresentMon64.exe")); // PATH lookup via Command

    candidates
        .into_iter()
        .find(|p| p.is_absolute() && p.is_file())
        .map(|p| p.to_string_lossy().into_owned())
}

#[cfg(not(windows))]
pub fn find_presentmon() -> Option<String> {
    None
}

/// Run a timed PresentMon capture for `process_name`, writing CSV to `output`.
/// Blocks for the capture duration.
#[cfg(windows)]
pub fn run_capture(binary: &str, process_name: &str, duration_secs: u64, output: &str) -> Result<()> {
    use std::process::Command;

    let result = Command::new(binary)
        .args([
            "-process_name",
            process_name,
            "-output_file",
            output,
            "-terminate_after_timed",
            &duration_secs.to_string(),
            "-stop_existing_session",
        ])
        .output();

    match result {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            Err(OptixError::Windows(format!(
                "PresentMon capture failed: {}",
                stderr.trim()
            )))
        }
        Err(e) => Err(OptixError::Windows(format!(
            "cannot run {binary}: {e}"
        ))),
    }
}

#[cfg(not(windows))]
pub fn run_capture(
    _binary: &str,
    _process_name: &str,
    _duration_secs: u64,
    _output: &str,
) -> Result<()> {
    Err(OptixError::UnsupportedPlatform("PresentMon capture".into()))
}
