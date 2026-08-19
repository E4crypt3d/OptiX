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

/// Read a process's current CPU-affinity bitmask.
#[cfg(windows)]
pub fn get_affinity(pid: u32) -> Option<u64> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        GetProcessAffinityMask, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let handle = unsafe { open_process(pid, PROCESS_QUERY_LIMITED_INFORMATION) }?;
    let mut process_mask: usize = 0;
    let mut system_mask: usize = 0;
    let ok = unsafe { GetProcessAffinityMask(handle as _, &mut process_mask, &mut system_mask) };
    unsafe { CloseHandle(handle as _) };
    if ok == 0 {
        return None;
    }
    Some(process_mask as u64)
}

#[cfg(not(windows))]
pub fn get_affinity(_pid: u32) -> Option<u64> {
    None
}

/// Change a process's CPU-affinity bitmask. Refuses a zero mask.
#[cfg(windows)]
pub fn set_affinity(pid: u32, mask: u64) -> Result<()> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        SetProcessAffinityMask, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_INFORMATION,
    };

    if mask == 0 {
        return Err(OptixError::InvalidState(
            "CPU affinity mask must be non-zero".into(),
        ));
    }

    let access = PROCESS_SET_INFORMATION | PROCESS_QUERY_LIMITED_INFORMATION;
    let handle = unsafe { open_process(pid, access) }
        .ok_or_else(|| OptixError::Windows(format!("cannot open process {pid}")))?;

    let ok = unsafe { SetProcessAffinityMask(handle as _, mask as usize) };
    unsafe { CloseHandle(handle as _) };
    if ok == 0 {
        return Err(OptixError::Windows(format!(
            "SetProcessAffinityMask failed for pid {pid}"
        )));
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn set_affinity(_pid: u32, _mask: u64) -> Result<()> {
    Err(OptixError::UnsupportedPlatform("CPU affinity".into()))
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
