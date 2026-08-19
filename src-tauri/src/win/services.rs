//! Windows Service Control Manager access via `advapi32` (enumerate, start,
//! stop, change start type). Service metadata (start type, binary path,
//! description, delayed-auto-start) is read from the service registry keys;
//! live state comes from `EnumServicesStatusExW`.

use crate::error::{OptixError, Result};
use crate::models::services::ServiceInfo;
use crate::models::snapshot::ChangeRecord;

#[cfg(windows)]
use crate::models::services::{service_state_str, start_type_str};

#[cfg(windows)]
const SERVICES_KEY: &str = r"SYSTEM\CurrentControlSet\Services";

#[cfg(windows)]
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Read a null-terminated UTF-16 string from a pointer.
#[cfg(windows)]
unsafe fn from_wide(ptr: windows_sys::core::PWSTR) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    while *ptr.add(len) != 0 {
        len += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len))
}

/// Enumerate services (name, display name, live state) plus registry metadata.
#[cfg(windows)]
pub fn list_services() -> Vec<ServiceInfo> {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_MORE_DATA};
    use windows_sys::Win32::System::Services::{
        CloseServiceHandle, EnumServicesStatusExW, OpenSCManagerW,
        ENUM_SERVICE_STATUS_PROCESSW, SC_ENUM_PROCESS_INFO, SC_MANAGER_CONNECT,
        SC_MANAGER_ENUMERATE_SERVICE, SERVICE_DRIVER, SERVICE_STATE_ALL, SERVICE_WIN32,
    };

    let scm = unsafe {
        OpenSCManagerW(
            std::ptr::null(),
            std::ptr::null(),
            SC_MANAGER_ENUMERATE_SERVICE | SC_MANAGER_CONNECT,
        )
    };
    if scm.is_null() {
        return Vec::new();
    }

    let mut out = Vec::new();
    let mut buffer: Vec<u8> = Vec::new();
    loop {
        let mut needed = 0u32;
        let mut returned = 0u32;
        let mut resume = 0u32;
        let buf_ptr = if buffer.is_empty() {
            std::ptr::null_mut()
        } else {
            buffer.as_mut_ptr()
        };
        let ok = unsafe {
            EnumServicesStatusExW(
                scm,
                SC_ENUM_PROCESS_INFO,
                SERVICE_WIN32 | SERVICE_DRIVER,
                SERVICE_STATE_ALL,
                buf_ptr,
                buffer.len() as u32,
                &mut needed,
                &mut returned,
                &mut resume,
                std::ptr::null(),
            )
        };
        if ok != 0 {
            if returned > 0 && !buffer.is_empty() {
                let base = buffer.as_ptr() as *const ENUM_SERVICE_STATUS_PROCESSW;
                for i in 0..returned as usize {
                    let entry = unsafe { &*base.add(i) };
                    let name = unsafe { from_wide(entry.lpServiceName) };
                    let display_name = unsafe { from_wide(entry.lpDisplayName) };
                    let state = entry.ServiceStatusProcess.dwCurrentState;
                    out.push(enrich(&name, &display_name, state));
                }
            }
            break;
        }
        let err = unsafe { GetLastError() };
        if err != ERROR_MORE_DATA || needed == 0 {
            break;
        }
        // Grow (with margin, since services can change mid-enumeration) and retry.
        buffer = vec![0u8; needed as usize + 4096];
    }

    unsafe { CloseServiceHandle(scm) };
    out
}

#[cfg(not(windows))]
pub fn list_services() -> Vec<ServiceInfo> {
    Vec::new()
}

/// Fill a `ServiceInfo` from the service registry key.
#[cfg(windows)]
fn enrich(name: &str, display_name: &str, state: u32) -> ServiceInfo {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let mut info = ServiceInfo {
        name: name.to_string(),
        display_name: if display_name.is_empty() {
            name.to_string()
        } else {
            display_name.to_string()
        },
        description: String::new(),
        state: service_state_str(state).to_string(),
        start_type: "unknown".to_string(),
        binary_path: String::new(),
        is_driver: false,
        delayed_auto_start: false,
        account: String::new(),
        classification: String::new(),
    };

    let Ok(key) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(&format!(r"{SERVICES_KEY}\{name}"))
    else {
        return info;
    };
    let start: u32 = key.get_value("Start").unwrap_or(3);
    let service_type: u32 = key.get_value("Type").unwrap_or(0);
    info.start_type = start_type_str(start).to_string();
    info.is_driver = service_type & 0x0F != 0; // driver types occupy the low nibble
    info.delayed_auto_start = key.get_value::<u32, _>("DelayedAutoStart").unwrap_or(0) == 1;
    info.binary_path = key.get_value("ImagePath").unwrap_or_default();
    info.account = key.get_value("ObjectName").unwrap_or_default();
    info.description = key.get_value("Description").unwrap_or_default();
    info
}

/// Start a service by name.
#[cfg(windows)]
pub fn start_service(name: &str) -> Result<()> {
    use windows_sys::Win32::System::Services::{
        CloseServiceHandle, OpenSCManagerW, OpenServiceW, StartServiceW, SC_MANAGER_CONNECT,
        SERVICE_START,
    };

    let scm = unsafe { OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_CONNECT) };
    if scm.is_null() {
        return Err(OptixError::Windows("cannot open service manager".into()));
    }
    let wide = to_wide(name);
    let handle = unsafe { OpenServiceW(scm, wide.as_ptr(), SERVICE_START) };
    if handle.is_null() {
        unsafe { CloseServiceHandle(scm) };
        return Err(OptixError::Windows(format!("cannot open service {name}")));
    }
    let ok = unsafe { StartServiceW(handle, 0, std::ptr::null()) };
    unsafe {
        CloseServiceHandle(handle);
        CloseServiceHandle(scm);
    }
    if ok == 0 {
        return Err(OptixError::Windows(format!("failed to start service {name}")));
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn start_service(_name: &str) -> Result<()> {
    Err(OptixError::UnsupportedPlatform("services".into()))
}

/// Stop a service by name.
#[cfg(windows)]
pub fn stop_service(name: &str) -> Result<()> {
    use windows_sys::Win32::System::Services::{
        CloseServiceHandle, ControlService, OpenSCManagerW, OpenServiceW, SC_MANAGER_CONNECT,
        SERVICE_CONTROL_STOP, SERVICE_STATUS, SERVICE_STOP,
    };

    let scm = unsafe { OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_CONNECT) };
    if scm.is_null() {
        return Err(OptixError::Windows("cannot open service manager".into()));
    }
    let wide = to_wide(name);
    let handle = unsafe { OpenServiceW(scm, wide.as_ptr(), SERVICE_STOP) };
    if handle.is_null() {
        unsafe { CloseServiceHandle(scm) };
        return Err(OptixError::Windows(format!("cannot open service {name}")));
    }
    let mut status: SERVICE_STATUS = unsafe { std::mem::zeroed() };
    let ok = unsafe { ControlService(handle, SERVICE_CONTROL_STOP, &mut status) };
    unsafe {
        CloseServiceHandle(handle);
        CloseServiceHandle(scm);
    }
    if ok == 0 {
        return Err(OptixError::Windows(format!("failed to stop service {name}")));
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn stop_service(_name: &str) -> Result<()> {
    Err(OptixError::UnsupportedPlatform("services".into()))
}

/// Change a service's start type (`SERVICE_AUTO_START`/`DEMAND`/`DISABLED`).
#[cfg(windows)]
pub fn set_start_type(name: &str, start_type: u32) -> Result<()> {
    use windows_sys::Win32::System::Services::{
        ChangeServiceConfigW, CloseServiceHandle, OpenSCManagerW, OpenServiceW,
        SC_MANAGER_CONNECT, SERVICE_CHANGE_CONFIG, SERVICE_NO_CHANGE,
    };

    let scm = unsafe { OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_CONNECT) };
    if scm.is_null() {
        return Err(OptixError::Windows("cannot open service manager".into()));
    }
    let wide = to_wide(name);
    let handle = unsafe { OpenServiceW(scm, wide.as_ptr(), SERVICE_CHANGE_CONFIG) };
    if handle.is_null() {
        unsafe { CloseServiceHandle(scm) };
        return Err(OptixError::Windows(format!("cannot open service {name}")));
    }
    let ok = unsafe {
        ChangeServiceConfigW(
            handle,
            SERVICE_NO_CHANGE,
            start_type,
            SERVICE_NO_CHANGE,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    unsafe {
        CloseServiceHandle(handle);
        CloseServiceHandle(scm);
    }
    if ok == 0 {
        return Err(OptixError::Windows(format!(
            "failed to change start type for {name}"
        )));
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn set_start_type(_name: &str, _start_type: u32) -> Result<()> {
    Err(OptixError::UnsupportedPlatform("services".into()))
}

/// Roll back a `service`-domain change recorded by the engine.
#[cfg(windows)]
pub fn rollback_service(change: &ChangeRecord) -> Result<()> {
    match change.kind.as_str() {
        // We stopped a running service → start it back.
        "stop" => start_service(&change.location),
        // We started a stopped service → stop it back.
        "start" => stop_service(&change.location),
        // We changed the start type → restore the previous value.
        "set_start_type" => {
            let old = change.old_value.as_deref().ok_or_else(|| {
                OptixError::InvalidState("no previous start type recorded".into())
            })?;
            let value = crate::models::services::start_type_value(old)
                .ok_or_else(|| OptixError::InvalidState(format!("bad start type: {old}")))?;
            set_start_type(&change.location, value)
        }
        other => Err(OptixError::InvalidState(format!(
            "unknown service change kind: {other}"
        ))),
    }
}

#[cfg(not(windows))]
pub fn rollback_service(_change: &ChangeRecord) -> Result<()> {
    Err(OptixError::UnsupportedPlatform("service rollback".into()))
}
