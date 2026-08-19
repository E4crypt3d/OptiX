//! Windows-specific hardware/software detection: GPUs and display settings
//! (registry + GDI) and startup applications (registry Run keys).

use crate::models::hardware::{DisplayInfo, GpuInfo, StartupApp};

/// Display adapter class GUID from the Windows registry.
#[cfg(windows)]
const DISPLAY_CLASS: &str =
    r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";

/// Enumerate installed GPUs (name, vendor, driver version) from the registry.
#[cfg(windows)]
pub fn detect_gpus() -> Vec<GpuInfo> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let mut gpus = Vec::new();
    let class_key = match RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(DISPLAY_CLASS) {
        Ok(k) => k,
        Err(_) => return gpus,
    };

    for subkey_name in class_key.enum_keys().flatten() {
        let Ok(subkey) = class_key.open_subkey(&subkey_name) else {
            continue;
        };
        let name: String = subkey.get_value("DriverDesc").unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        let vendor: String = subkey.get_value("ProviderName").unwrap_or_default();
        let driver_version: String = subkey.get_value("DriverVersion").unwrap_or_default();
        gpus.push(GpuInfo {
            name,
            vendor,
            driver_version,
            memory_bytes: 0,
            usage_percent: 0.0,
        });
    }
    gpus
}

#[cfg(not(windows))]
pub fn detect_gpus() -> Vec<GpuInfo> {
    Vec::new()
}

/// Current primary display resolution and refresh rate.
#[cfg(windows)]
pub fn primary_display() -> Option<DisplayInfo> {
    use windows_sys::Win32::Graphics::Gdi::{EnumDisplaySettingsW, DEVMODEW, ENUM_CURRENT_SETTINGS};

    let mut dm: DEVMODEW = unsafe { std::mem::zeroed() };
    dm.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
    // A null device name queries the current (primary) display device.
    let result = unsafe { EnumDisplaySettingsW(std::ptr::null(), ENUM_CURRENT_SETTINGS, &mut dm) };
    if result == 0 {
        return None;
    }
    Some(DisplayInfo {
        width: dm.dmPelsWidth,
        height: dm.dmPelsHeight,
        refresh_rate: dm.dmDisplayFrequency,
    })
}

#[cfg(not(windows))]
pub fn primary_display() -> Option<DisplayInfo> {
    None
}

/// Enumerate startup applications from the HKLM/HKCU `Run` keys.
#[cfg(windows)]
pub fn detect_startup_apps() -> Vec<StartupApp> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    const RUN_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run";

    let mut apps = Vec::new();
    let sources = [(HKEY_LOCAL_MACHINE, "HKLM"), (HKEY_CURRENT_USER, "HKCU")];

    for (hive, label) in sources {
        let Ok(key) = RegKey::predef(hive).open_subkey(RUN_KEY) else {
            continue;
        };
        for name in key.enum_keys().flatten() {
            if let Ok(command) = key.get_value::<String, _>(&name) {
                apps.push(StartupApp {
                    name,
                    command,
                    location: format!("{label}\\{RUN_KEY}"),
                });
            }
        }
    }
    apps
}

#[cfg(not(windows))]
pub fn detect_startup_apps() -> Vec<StartupApp> {
    Vec::new()
}
