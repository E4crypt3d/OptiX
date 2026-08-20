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
        // Bundled via `bundle.resources` (lands next to the exe), the
        // resources/ subdir, or an unpacked copy beside the exe.
        candidates.push(dir.join("PresentMon64.exe"));
        candidates.push(dir.join("PresentMon.exe"));
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
    if let Some(found) = candidates.into_iter().find(|p| p.is_file()) {
        return Some(found.to_string_lossy().into_owned());
    }

    // Bare name resolves through PATH when spawned via `Command::new` — check
    // it explicitly so "or on PATH" in the UI is actually true.
    let on_path = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join("PresentMon64.exe").is_file()))
        .unwrap_or(false);
    on_path.then(|| "PresentMon64.exe".to_string())
}

#[cfg(not(windows))]
pub fn find_presentmon() -> Option<String> {
    None
}

/// Run a timed PresentMon capture for `process_name`, writing CSV to `output`.
/// Blocks for the capture duration.
#[cfg(windows)]
pub fn run_capture(binary: &str, process_name: &str, duration_secs: u64, output: &str) -> Result<()> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    // PresentMon is a console binary; spawn without a window so a capture
    // doesn't flash a console for its whole duration.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
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
        .creation_flags(CREATE_NO_WINDOW)
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
