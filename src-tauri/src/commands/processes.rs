use tauri::State;

use crate::engine::optimizer::OptimizerState;
use crate::engine::processes::{classify, is_system_process};
use crate::error::{OptixError, Result};
use crate::models::process::{GamingModeResult, PriorityClass, ProcessDetail};
use crate::win;

/// Enumerate running processes with classification and (on Windows) priority.
/// CPU values require a two-sample window, so we refresh twice.
#[tauri::command]
pub async fn list_processes() -> Result<Vec<ProcessDetail>> {
    tauri::async_runtime::spawn_blocking(list_processes_blocking)
        .await
        .map_err(|e| OptixError::Other(e.to_string()))?
}

fn list_processes_blocking() -> Result<Vec<ProcessDetail>> {
    use sysinfo::{ProcessesToUpdate, System};

    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    // Second refresh yields meaningful CPU deltas.
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let mut out: Vec<ProcessDetail> = sys
        .processes()
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
                status,
                classification: class.as_str().to_string(),
                is_system,
                priority,
            }
        })
        .collect();

    // Highest resource consumers first.
    out.sort_by(|a, b| {
        b.memory_bytes
            .cmp(&a.memory_bytes)
            .then(b.cpu_usage_percent.total_cmp(&a.cpu_usage_percent))
    });
    Ok(out)
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
    game_pids: Vec<u32>,
    background_pids: Vec<u32>,
) -> Result<GamingModeResult> {
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
