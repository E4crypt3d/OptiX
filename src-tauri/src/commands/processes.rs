use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sysinfo::{ProcessesToUpdate, System};
use tauri::State;

use crate::engine::optimizer::OptimizerState;
use crate::engine::processes::{classify, is_system_process, memory_pressure};
use crate::error::{OptixError, Result};
use crate::models::process::{AffinityInfo, GamingModeResult, MemoryState, PriorityClass, ProcessDetail};
use crate::win;

/// Minimum CPU-sample window. Shorter windows produce noisy percentages.
const MIN_SAMPLE_WINDOW: Duration = Duration::from_millis(100);

/// Shared process-sampling state: one `System` reused across refreshes so CPU
/// percentages are measured over the real interval since the previous call
/// (no fixed sleep on every refresh) and memory is never re-read twice.
#[derive(Clone)]
pub struct ProcessMonitorState {
    inner: Arc<ProcessMonitorInner>,
}

struct ProcessMonitorInner {
    sys: Mutex<System>,
    last: Mutex<Option<Instant>>,
}

impl ProcessMonitorState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(ProcessMonitorInner {
                sys: Mutex::new(System::new()),
                last: Mutex::new(None),
            }),
        }
    }

    /// Two-sample process refresh + row extraction, done while holding the
    /// sample lock (the whole point is that the System instance is shared).
    /// The slow per-process GPU pass (PDH) runs after the lock is released.
    fn collect(&self) -> Vec<ProcessDetail> {
        let mut sys = self.inner.sys.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        let elapsed = {
            let mut last = self.inner.last.lock().unwrap_or_else(|e| e.into_inner());
            let elapsed = last.map(|t| now.duration_since(t)).unwrap_or(Duration::ZERO);
            *last = Some(now);
            elapsed
        };
        sys.refresh_processes(ProcessesToUpdate::All, true);
        if elapsed < MIN_SAMPLE_WINDOW {
            std::thread::sleep(MIN_SAMPLE_WINDOW - elapsed);
        }
        sys.refresh_processes(ProcessesToUpdate::All, true);
        sys.processes()
            .iter()
            .map(|(pid, p)| {
                let pid_u32 = pid.as_u32();
                let name = p.name().to_string_lossy().into_owned();
                let status = format!("{:?}", p.status()).to_lowercase();
                let class = classify(&name);
                let is_system = is_system_process(&name, p.session_id().map(|s| s.as_u32()));
                let priority = win::process::get_priority(pid_u32).map(|c| c.as_str().to_string());
                ProcessDetail {
                    pid: pid_u32,
                    name,
                    exe: p.exe().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default(),
                    cpu_usage_percent: p.cpu_usage(),
                    memory_bytes: p.memory(),
                    disk_read_bytes: p.disk_usage().total_read_bytes,
                    disk_written_bytes: p.disk_usage().total_written_bytes,
                    start_time: p.start_time(),
                    parent_pid: p.parent().map(|pp| pp.as_u32()),
                    threads: p.tasks().map(|t| t.len()).unwrap_or(0),
                    // uid on Linux (0 = root); sysinfo does not expose a user
                    // id on Windows (its Uid there is a SID that fails the
                    // numeric parse, so it safely becomes None).
                    user_id: p.user_id().and_then(|u| u.to_string().parse::<u32>().ok()),
                    status,
                    classification: class.as_str().to_string(),
                    is_system,
                    priority,
                    gpu_usage_percent: 0.0,
                }
            })
            .collect()
    }
}

impl Default for ProcessMonitorState {
    fn default() -> Self {
        Self::new()
    }
}

/// Enumerate running processes with classification and (on Windows) priority.
/// Runs off the main thread; CPU values come from the shared sampling window.
#[tauri::command]
pub async fn list_processes(state: State<'_, ProcessMonitorState>) -> Result<Vec<ProcessDetail>> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut out = state.collect();
        // Per-process GPU utilization from PDH (Windows; empty elsewhere). Sums
        // all GPU engines per PID, matching what Task Manager reports.
        let gpu: std::collections::HashMap<u32, f32> =
            crate::win::pdh::per_process_gpu_usage().into_iter().collect();
        for detail in &mut out {
            if let Some(&usage) = gpu.get(&detail.pid) {
                detail.gpu_usage_percent = usage;
            }
        }
        // Highest resource consumers first.
        out.sort_by(|a, b| {
            b.memory_bytes
                .cmp(&a.memory_bytes)
                .then(b.cpu_usage_percent.total_cmp(&a.cpu_usage_percent))
        });
        Ok(out)
    })
    .await
    .map_err(|e| OptixError::Other(e.to_string()))?
}

/// System-wide memory state for the Processes & RAM page: physical RAM, cached
/// memory (Linux), commit charge (Windows), swap, and a pressure level.
#[tauri::command]
pub fn memory_state() -> MemoryState {
    let mut sys = System::new();
    sys.refresh_memory();

    let total = sys.total_memory();
    let used = sys.used_memory();
    let available = sys.available_memory();
    let swap_total = sys.total_swap();
    let swap_used = sys.used_swap();
    let (cached_bytes, committed_bytes, committed_limit_bytes) = platform_memory_details();

    let usage_percent = if total > 0 {
        (used as f32 / total as f32) * 100.0
    } else {
        0.0
    };
    MemoryState {
        total_bytes: total,
        used_bytes: used,
        available_bytes: available,
        cached_bytes,
        committed_bytes,
        committed_limit_bytes,
        swap_total_bytes: swap_total,
        swap_used_bytes: swap_used,
        usage_percent,
        pressure: memory_pressure(used, total, swap_used, swap_total).to_string(),
    }
}

/// Commit charge on Windows (physical + pagefile, via `GlobalMemoryStatusEx`);
/// buffers+cache on Linux (from `/proc/meminfo`). The other platform's field
/// stays `None`.
#[cfg(windows)]
fn platform_memory_details() -> (Option<u64>, Option<u64>, Option<u64>) {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut status: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
    if ok == 0 {
        return (None, None, None);
    }
    let committed = status.ullTotalPageFile.saturating_sub(status.ullAvailPageFile);
    (
        None,
        Some(committed),
        Some(status.ullTotalPageFile),
    )
}

#[cfg(not(windows))]
fn platform_memory_details() -> (Option<u64>, Option<u64>, Option<u64>) {
    (linux_buffers_cache(), None, None)
}

/// Sum of `Buffers` + `Cached` + `SReclaimable` from `/proc/meminfo` — the
/// same components `free` reports as "buff/cache". `None` when unreadable.
#[cfg(not(windows))]
fn linux_buffers_cache() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    parse_meminfo_cache(&text)
}

/// Pure parser for the buffers+cache total. Returns `None` if the file is
/// missing any of the three fields.
#[cfg(not(windows))]
fn parse_meminfo_cache(text: &str) -> Option<u64> {
    let mut total = 0u64;
    let mut found = 0u32;
    for line in text.lines() {
        let (key, rest) = line.split_once(':')?;
        let Some(value) = rest
            .split_whitespace()
            .next()
            .and_then(|v| v.parse::<u64>().ok())
        else {
            continue;
        };
        if matches!(key.trim(), "Buffers" | "Cached" | "SReclaimable") {
            total = total.saturating_add(value);
            found += 1;
        }
    }
    (found == 3).then_some(total)
}

/// Kill a process. Refuses system/required processes and the current app.
#[tauri::command]
pub fn kill_process(pid: u32) -> Result<()> {
    if pid == std::process::id() {
        return Err(OptixError::NotPermitted(
            "cannot terminate the Optix process itself".into(),
        ));
    }
    if is_protected(pid) {
        return Err(OptixError::NotPermitted(format!(
            "process {pid} is a protected system process"
        )));
    }
    win::process::terminate(pid)
}

/// Suspend a process (freeze it in place). Refuses protected processes and the
/// current app. Works on Windows (NtSuspendProcess) and Linux (SIGSTOP).
#[tauri::command]
pub fn suspend_process(pid: u32) -> Result<()> {
    if pid == std::process::id() {
        return Err(OptixError::NotPermitted(
            "cannot suspend the Optix process itself".into(),
        ));
    }
    if is_protected(pid) {
        return Err(OptixError::NotPermitted(format!(
            "process {pid} is a protected system process"
        )));
    }
    win::process::suspend(pid)
}

/// Resume a suspended process (SIGCONT / NtResumeProcess).
#[tauri::command]
pub fn resume_process(pid: u32) -> Result<()> {
    if is_protected(pid) {
        return Err(OptixError::NotPermitted(format!(
            "process {pid} is a protected system process"
        )));
    }
    win::process::resume(pid)
}

/// Current CPU-affinity mask of a process plus the system core mask
/// (Windows only).
#[tauri::command]
pub fn get_process_affinity(pid: u32) -> Option<AffinityInfo> {
    win::process::get_affinity(pid).map(|(process_mask, system_mask)| AffinityInfo {
        process_mask,
        system_mask,
    })
}

/// Pin a process to a set of CPUs via an affinity bitmask (Windows only).
/// Refuses protected processes and a zero mask.
#[tauri::command]
pub fn set_process_affinity(pid: u32, mask: u64) -> Result<()> {
    if is_protected(pid) {
        return Err(OptixError::NotPermitted(format!(
            "process {pid} is a protected system process"
        )));
    }
    win::process::set_affinity(pid, mask)
}

/// PID owning the foreground window (Windows `GetForegroundWindow`; Linux via
/// `xdotool` when installed). Optix's own window is never reported (it is the
/// foreground window while the button is clicked) — `None` tells the user to
/// focus the target window first.
#[tauri::command]
pub fn foreground_pid() -> Option<u32> {
    let pid = win::process::foreground_pid()?;
    (pid != std::process::id()).then_some(pid)
}

/// Set a process's priority class (Windows only). Refuses REALTIME and
/// protected system processes.
#[tauri::command]
pub fn set_process_priority(pid: u32, priority: String) -> Result<()> {
    let class = PriorityClass::from_str(&priority)
        .ok_or_else(|| OptixError::InvalidState(format!("unknown priority class: {priority}")))?;
    if !class.is_settable() {
        return Err(OptixError::NotPermitted(
            "REALTIME priority is disabled by design".into(),
        ));
    }
    if is_protected(pid) {
        return Err(OptixError::NotPermitted(format!(
            "process {pid} is a protected system process"
        )));
    }
    win::process::set_priority(pid, class)
}

/// Boost game processes and lower background processes. Records originals so
/// `restore_gaming_mode` can revert them.
#[tauri::command]
pub fn apply_gaming_mode(
    state: State<'_, OptimizerState>,
    mut game_pids: Vec<u32>,
    mut background_pids: Vec<u32>,
) -> Result<GamingModeResult> {
    game_pids.sort_unstable();
    game_pids.dedup();
    background_pids.sort_unstable();
    background_pids.dedup();
    validate_gaming_pids(&game_pids, &background_pids)?;

    use sysinfo::System;
    let sys = System::new_all();
    let name_of = move |pid: u32| -> String {
        use sysinfo::Pid;
        sys.process(Pid::from_u32(pid))
            .map(|p| p.name().to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    Ok(crate::engine::optimizer::apply(
        &state,
        &game_pids,
        &background_pids,
        name_of,
    ))
}

/// Restore all priorities changed by gaming mode.
#[tauri::command]
pub fn restore_gaming_mode(state: State<'_, OptimizerState>) -> Result<usize> {
    Ok(crate::engine::optimizer::restore(&state))
}

/// Validate gaming-mode targets at the backend boundary. A process cannot be
/// both boosted and lowered, and required/system processes are never touched.
fn validate_gaming_pids(game_pids: &[u32], background_pids: &[u32]) -> Result<()> {
    use std::collections::HashSet;
    use sysinfo::{Pid, ProcessesToUpdate, System};

    let game_set: HashSet<u32> = game_pids.iter().copied().collect();
    if background_pids.iter().any(|pid| game_set.contains(pid)) {
        return Err(OptixError::InvalidState(
            "a process cannot be both game and background".into(),
        ));
    }

    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    for pid in game_pids.iter().chain(background_pids) {
        let Some(process) = sys.process(Pid::from_u32(*pid)) else {
            return Err(OptixError::InvalidState(format!(
                "process {pid} is no longer running"
            )));
        };
        let name = process.name().to_string_lossy().into_owned();
        if is_system_process(&name, process.session_id().map(|s| s.as_u32()))
            || classify(&name) == crate::models::process::ProcessClass::Required
        {
            return Err(OptixError::NotPermitted(format!(
                "protected system process cannot be used in gaming mode: {name}"
            )));
        }
    }
    Ok(())
}

/// Protect against killing/deprioritizing system processes and Optix itself.
fn is_protected(pid: u32) -> bool {
    use sysinfo::{Pid, System};
    let sys = System::new_all();
    let Some(p) = sys.process(Pid::from_u32(pid)) else {
        return false;
    };
    let name = p.name().to_string_lossy().into_owned();
    classify(&name) == crate::models::process::ProcessClass::Required
        || is_system_process(&name, p.session_id().map(|s| s.as_u32()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn meminfo_cache_parses_standard_file() {
        let text = "MemTotal:       16384000 kB\nMemFree:         4096000 kB\nBuffers:          524288 kB\nCached:          4194304 kB\nSReclaimable:     262144 kB\nSwapTotal:       8388608 kB\nSwapFree:        4194304 kB\n";
        assert_eq!(parse_meminfo_cache(text), Some(524288 + 4194304 + 262144));
    }

    #[cfg(not(windows))]
    #[test]
    fn meminfo_cache_missing_fields_returns_none() {
        assert_eq!(parse_meminfo_cache("MemTotal: 1 kB\nMemFree: 1 kB\n"), None);
        assert_eq!(parse_meminfo_cache(""), None);
    }
}
