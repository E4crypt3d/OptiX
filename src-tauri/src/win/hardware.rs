//! Windows-specific hardware/software detection: GPUs and display settings
//! (registry + GDI) and startup applications (registry Run keys).

use crate::models::hardware::{DisplayInfo, GpuInfo, StartupApp};

/// Display adapter class GUID from the Windows registry.
#[cfg(windows)]
const DISPLAY_CLASS: &str =
    r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";

#[cfg(windows)]
fn utf16_text(value: &[u16]) -> String {
    let end = value.iter().position(|c| *c == 0).unwrap_or(value.len());
    String::from_utf16_lossy(&value[..end])
}

/// Enumerate installed GPUs (name, vendor, driver version) from the registry.
#[cfg(windows)]
pub fn detect_gpus() -> Vec<GpuInfo> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let mut gpus = Vec::new();
    if let Ok(class_key) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(DISPLAY_CLASS) {
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
    }

    // Some systems expose the integrated adapter through WMI but not through
    // the display-driver registry class. Merge both sources so a laptop's
    // built-in GPU is not hidden when a discrete adapter is also present.
    for controller in crate::win::wmi::video_controllers() {
        if controller.name.trim().is_empty() {
            continue;
        }
        let controller_name = controller.name.to_ascii_lowercase();
        let already_present = gpus.iter().any(|gpu| {
            let gpu_name = gpu.name.to_ascii_lowercase();
            gpu_name == controller_name
                || (!gpu_name.is_empty()
                    && !controller_name.is_empty()
                    && (gpu_name.contains(&controller_name)
                        || controller_name.contains(&gpu_name)))
        });
        if !already_present {
            gpus.push(GpuInfo {
                name: controller.name,
                vendor: String::new(),
                driver_version: String::new(),
                memory_bytes: controller.adapter_ram_bytes,
                usage_percent: 0.0,
            });
        }
    }

    // EnumDisplayDevices is a final local fallback when WMI is unavailable.
    // It reports integrated adapters even on systems where the registry class
    // is incomplete or the WMI provider is disabled.
    use windows_sys::Win32::Graphics::Gdi::{EnumDisplayDevicesW, DISPLAY_DEVICEW};
    let mut index = 0;
    loop {
        let mut device: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
        device.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
        let present = unsafe { EnumDisplayDevicesW(std::ptr::null(), index, &mut device, 0) };
        if present == 0 {
            break;
        }
        index += 1;
        let name = utf16_text(&device.DeviceString);
        if name.is_empty() {
            continue;
        }
        let name_lower = name.to_ascii_lowercase();
        let already_present = gpus.iter().any(|gpu| {
            let gpu_name = gpu.name.to_ascii_lowercase();
            gpu_name == name_lower
                || (!gpu_name.is_empty()
                    && (gpu_name.contains(&name_lower) || name_lower.contains(&gpu_name)))
        });
        if !already_present {
            gpus.push(GpuInfo {
                name,
                vendor: String::new(),
                driver_version: String::new(),
                memory_bytes: 0,
                usage_percent: 0.0,
            });
        }
    }

    gpus
}

#[cfg(not(windows))]
pub fn detect_gpus() -> Vec<GpuInfo> {
    use std::collections::HashSet;
    use std::process::Command;

    let mut gpus = Vec::new();
    let mut seen = HashSet::new();

    // pciutils is present on most desktop Linux installations and provides the
    // actual adapter model, including integrated Intel/AMD graphics.
    if let Ok(output) = Command::new("lspci").arg("-nn").output() {
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            let lower = line.to_ascii_lowercase();
            if !lower.contains("vga compatible controller")
                && !lower.contains("3d controller")
                && !lower.contains("display controller")
            {
                continue;
            }
            let Some((_, description)) = line.split_once(": ") else {
                continue;
            };
            let name = description
                .split(" [")
                .next()
                .unwrap_or(description)
                .split(" (rev")
                .next()
                .unwrap_or(description)
                .trim()
                .to_string();
            if name.is_empty() || !seen.insert(name.to_ascii_lowercase()) {
                continue;
            }
            let name_lower = name.to_ascii_lowercase();
            let vendor = if name_lower.contains("nvidia") {
                "NVIDIA"
            } else if name_lower.contains("amd") || name_lower.contains("ati") {
                "AMD"
            } else if name_lower.contains("intel") {
                "Intel"
            } else {
                ""
            };
            gpus.push(GpuInfo {
                name,
                vendor: vendor.to_string(),
                driver_version: String::new(),
                memory_bytes: 0,
                usage_percent: 0.0,
            });
        }
    }

    // Sysfs still identifies the GPU vendor when lspci/pciutils is missing.
    if gpus.is_empty() {
        if let Ok(entries) = std::fs::read_dir("/sys/class/drm") {
            for entry in entries.flatten() {
                let card = entry.file_name().to_string_lossy().into_owned();
                if !card.starts_with("card") || card.contains('-') {
                    continue;
                }
                let vendor_id = std::fs::read_to_string(format!(
                    "/sys/class/drm/{card}/device/vendor"
                ))
                .unwrap_or_default();
                let vendor = match vendor_id.trim().to_ascii_lowercase().as_str() {
                    "0x8086" => "Intel",
                    "0x1002" => "AMD",
                    "0x10de" => "NVIDIA",
                    _ => "Unknown",
                };
                gpus.push(GpuInfo {
                    name: format!("{vendor} graphics adapter"),
                    vendor: vendor.to_string(),
                    driver_version: String::new(),
                    memory_bytes: 0,
                    usage_percent: 0.0,
                });
            }
        }
    }

    gpus
}

/// Enumerate active Windows displays with their current mode.
#[cfg(windows)]
pub fn displays() -> Vec<DisplayInfo> {
    use windows_sys::Win32::Graphics::Gdi::{
        EnumDisplayDevicesW, EnumDisplaySettingsW, DEVMODEW, DISPLAY_DEVICEW, ENUM_CURRENT_SETTINGS,
    };

    const DISPLAY_DEVICE_ACTIVE: u32 = 0x00000001;
    const DISPLAY_DEVICE_PRIMARY: u32 = 0x00000004;

    let mut found = Vec::new();
    let mut index = 0;
    loop {
        let mut device: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
        device.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
        let present = unsafe { EnumDisplayDevicesW(std::ptr::null(), index, &mut device, 0) };
        if present == 0 {
            break;
        }
        index += 1;
        if device.StateFlags & DISPLAY_DEVICE_ACTIVE == 0 {
            continue;
        }

        let mut mode: DEVMODEW = unsafe { std::mem::zeroed() };
        mode.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
        let current = unsafe {
            EnumDisplaySettingsW(device.DeviceName.as_ptr(), ENUM_CURRENT_SETTINGS, &mut mode)
        };
        if current != 0 {
            found.push(DisplayInfo {
                name: utf16_text(&device.DeviceString),
                width: mode.dmPelsWidth,
                height: mode.dmPelsHeight,
                refresh_rate: mode.dmDisplayFrequency,
                is_primary: device.StateFlags & DISPLAY_DEVICE_PRIMARY != 0,
            });
        }
    }

    // Keep a useful primary-display fallback for unusual drivers that do not
    // enumerate a device but still answer the current-settings query.
    if found.is_empty() {
        let mut mode: DEVMODEW = unsafe { std::mem::zeroed() };
        mode.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
        let current =
            unsafe { EnumDisplaySettingsW(std::ptr::null(), ENUM_CURRENT_SETTINGS, &mut mode) };
        if current != 0 {
            found.push(DisplayInfo {
                name: "Primary display".to_string(),
                width: mode.dmPelsWidth,
                height: mode.dmPelsHeight,
                refresh_rate: mode.dmDisplayFrequency,
                is_primary: true,
            });
        }
    }

    // GetSystemMetrics still returns the laptop panel's logical resolution
    // when display-device enumeration is blocked by a graphics driver.
    if found.is_empty() {
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
        let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        if width > 0 && height > 0 {
            found.push(DisplayInfo {
                name: "Primary display".to_string(),
                width: width as u32,
                height: height as u32,
                refresh_rate: 0,
                is_primary: true,
            });
        }
    }

    found
}

#[cfg(not(windows))]
pub fn displays() -> Vec<DisplayInfo> {
    let Ok(entries) = std::fs::read_dir("/sys/class/drm") else {
        return Vec::new();
    };

    let mut displays = Vec::new();
    for entry in entries.flatten() {
        let connector = entry.file_name().to_string_lossy().into_owned();
        if !connector.starts_with("card") || !connector.contains('-') {
            continue;
        }
        let status = std::fs::read_to_string(entry.path().join("status")).unwrap_or_default();
        if status.trim() != "connected" {
            continue;
        }
        let mode = std::fs::read_to_string(entry.path().join("modes"))
            .ok()
            .and_then(|modes| modes.lines().find_map(parse_display_mode));
        let Some((width, height)) = mode else {
            continue;
        };
        let name = connector
            .split_once('-')
            .map(|(_, output)| output.to_string())
            .unwrap_or(connector);
        displays.push(DisplayInfo {
            name,
            width,
            height,
            refresh_rate: 0,
            is_primary: false,
        });
    }

    // Prefer the internal panel when multiple connectors are present; the
    // first connected output is otherwise the best available primary hint.
    displays.sort_by_key(|display| {
        if display.name.starts_with("eDP") || display.name.starts_with("LVDS") {
            0
        } else {
            1
        }
    });
    if let Some(primary) = displays.first_mut() {
        primary.is_primary = true;
    }
    displays
}

#[cfg(not(windows))]
fn parse_display_mode(mode: &str) -> Option<(u32, u32)> {
    let (width, height) = mode.trim().split_once('x')?;
    Some((width.parse().ok()?, height.parse().ok()?))
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
