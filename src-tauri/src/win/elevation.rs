//! Elevation bootstrap. Privileged operations (HKLM registry, services, power
//! schemes, netsh) require an elevated process, so the app relaunches itself
//! through UAC when needed. On non-Windows hosts (dev mode) these are no-ops.

/// Whether the current process holds administrator privileges.
#[cfg(windows)]
pub fn is_elevated() -> bool {
    use windows_sys::Win32::UI::Shell::IsUserAnAdmin;
    unsafe { IsUserAnAdmin() != 0 }
}

#[cfg(not(windows))]
pub fn is_elevated() -> bool {
    true
}

/// Relaunch the current executable elevated via UAC ("runas").
/// Returns `true` if the relaunch request was submitted.
#[cfg(windows)]
pub fn relaunch_elevated() -> bool {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let exe_wide: Vec<u16> = exe
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let verb: Vec<u16> = "runas".encode_utf16().chain(std::iter::once(0)).collect();

    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            exe_wide.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    // ShellExecuteW returns an HINSTANCE > 32 on success.
    result as isize > 32
}

#[cfg(not(windows))]
pub fn relaunch_elevated() -> bool {
    false
}

/// Ensure the process is elevated. Returns `true` when the caller should
/// continue running; `false` when a relaunch was requested (the current
/// instance should exit).
#[cfg(windows)]
pub fn ensure_elevated() -> bool {
    if is_elevated() {
        return true;
    }
    // Once the UAC relaunch is submitted this instance must exit — returning
    // `true` here would keep BOTH copies alive and show two windows. A
    // declined/failed prompt also exits: unelevated is not a supported mode.
    relaunch_elevated();
    false
}

#[cfg(not(windows))]
pub fn ensure_elevated() -> bool {
    true
}
