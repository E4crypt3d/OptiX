//! Windows process control (priority class, termination) via `windows-sys`.
//! These are privileged operations that only apply on Windows; the non-Windows
//! build returns `UnsupportedPlatform` so the rest of the crate compiles on
//! Linux during development.

use crate::error::{OptixError, Result};
use crate::models::process::PriorityClass;

#[cfg(windows)]
use crate::engine::processes::{priority_from_flag, priority_to_flag};

/// Open a process handle with the rights needed to read or change its priority
/// class, or terminate it.
#[cfg(windows)]
unsafe fn open_process(pid: u32, access: u32) -> Option<isize> {
    use windows_sys::Win32::System::Threading::OpenProcess;

    let handle = OpenProcess(access, 0, pid);
    if handle.is_null() {
        None
    } else {
        Some(handle as isize)
    }
}

/// Read a process's current priority class.
#[cfg(windows)]
pub fn get_priority(pid: u32) -> Option<PriorityClass> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        GetPriorityClass, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let handle = unsafe { open_process(pid, PROCESS_QUERY_LIMITED_INFORMATION) }?;
    let flag = unsafe { GetPriorityClass(handle as _) };
    unsafe { CloseHandle(handle as _) };
    if flag == 0 {
        return None;
    }
    Some(priority_from_flag(flag))
}

#[cfg(not(windows))]
pub fn get_priority(_pid: u32) -> Option<PriorityClass> {
    None
}

/// Change a process's priority class. Refuses `Realtime` (never appropriate
/// for gaming — can freeze input/audio).
#[cfg(windows)]
pub fn set_priority(pid: u32, class: PriorityClass) -> Result<()> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        SetPriorityClass, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_INFORMATION,
    };

    if !class.is_settable() {
        return Err(OptixError::NotPermitted(
            "REALTIME priority is disabled by design".into(),
        ));
    }

    let access = PROCESS_SET_INFORMATION | PROCESS_QUERY_LIMITED_INFORMATION;
    let handle = unsafe { open_process(pid, access) }
        .ok_or_else(|| OptixError::Windows(format!("cannot open process {pid}")))?;

    let ok = unsafe { SetPriorityClass(handle as _, priority_to_flag(class)) };
    unsafe { CloseHandle(handle as _) };
    if ok == 0 {
        return Err(OptixError::Windows(format!(
            "SetPriorityClass failed for pid {pid}"
        )));
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn set_priority(_pid: u32, _class: PriorityClass) -> Result<()> {
    Err(OptixError::UnsupportedPlatform("process priority".into()))
}

/// Terminate a process. Only callable on processes the caller already
/// classified as safe/unknown and the user confirmed.
#[cfg(windows)]
pub fn terminate(pid: u32) -> Result<()> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{TerminateProcess, PROCESS_TERMINATE};

    let handle = unsafe { open_process(pid, PROCESS_TERMINATE) }
        .ok_or_else(|| OptixError::Windows(format!("cannot open process {pid}")))?;
    let ok = unsafe { TerminateProcess(handle as _, 1) };
    unsafe { CloseHandle(handle as _) };
    if ok == 0 {
        return Err(OptixError::Windows(format!(
            "TerminateProcess failed for pid {pid}"
        )));
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn terminate(_pid: u32) -> Result<()> {
    Err(OptixError::UnsupportedPlatform("process termination".into()))
}
