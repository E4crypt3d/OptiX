//! Windows-specific hardware/software detection: GPUs and display settings
//! (registry + GDI) and startup applications (registry Run keys).

use crate::models::hardware::{DisplayInfo, GpuInfo, PhysicalDiskInfo, StartupApp, TemperatureInfo};
#[cfg(not(windows))]
use crate::models::hardware::{BiosInfo, MotherboardInfo};

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
            let memory_bytes = vram_bytes(
                subkey.get_value("HardwareInformation.qwMemorySize").ok(),
                subkey.get_value("HardwareInformation.MemorySize").ok(),
            );
            gpus.push(GpuInfo {
                name,
                vendor,
                driver_version,
                memory_bytes,
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

/// Best-effort VRAM from the display-driver registry values. NVIDIA stores
/// `HardwareInformation.qwMemorySize` (REG_QWORD, bytes); AMD/Intel store
/// `HardwareInformation.MemorySize` (REG_DWORD, megabytes). Prefer the byte
/// value, fall back to MB (× 1 MiB). Unlike WMI's 32-bit `AdapterRAM` this is
/// accurate beyond 4 GiB.
#[cfg_attr(not(windows), allow(dead_code))]
fn vram_bytes(qw_bytes: Option<u64>, size_mb: Option<u32>) -> u64 {
    qw_bytes
        .filter(|b| *b > 0)
        .or_else(|| size_mb.filter(|m| *m > 0).map(|m| (m as u64) * 1024 * 1024))
        .unwrap_or(0)
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

    fill_linux_vram(&mut gpus);
    gpus
}

/// Attach best-effort VRAM to the detected GPUs. AMD exposes total VRAM in
/// sysfs (`mem_info_vram_total`, bytes); NVIDIA exposes per-GPU memory in
/// `/proc/driver/nvidia/gpus/<pci-addr>/information`. Intel iGPUs share system
/// memory, so they stay at 0.
#[cfg(not(windows))]
fn fill_linux_vram(gpus: &mut [GpuInfo]) {
    for (vendor, vram) in sysfs_vram_map() {
        if let Some(gpu) = gpus
            .iter_mut()
            .find(|g| g.vendor == vendor && g.memory_bytes == 0)
        {
            gpu.memory_bytes = vram;
        }
    }
}

/// (vendor, VRAM bytes) per GPU vendor, taking the largest value when several
/// cards share a vendor.
#[cfg(not(windows))]
fn sysfs_vram_map() -> Vec<(String, u64)> {
    use std::collections::HashMap;

    let Ok(entries) = std::fs::read_dir("/sys/class/drm") else {
        return Vec::new();
    };
    let mut per_vendor: HashMap<String, u64> = HashMap::new();
    for entry in entries.flatten() {
        let card = entry.file_name().to_string_lossy().into_owned();
        if !card.starts_with("card") || card.contains('-') {
            continue;
        }
        let device = entry.path().join("device");
        let vendor = match std::fs::read_to_string(device.join("vendor"))
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "0x1002" => "AMD",
            "0x10de" => "NVIDIA",
            "0x8086" => "Intel",
            _ => continue,
        };
        let mut vram = std::fs::read_to_string(device.join("mem_info_vram_total"))
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok());
        if vram.is_none() && vendor == "NVIDIA" {
            // The card's PCI address (e.g. 0000:01:00.0) keys the
            // /proc/driver/nvidia/gpus/<addr>/information layout.
            let bdf = std::fs::canonicalize(&device).ok().and_then(|p| {
                p.to_str().and_then(|s| {
                    s.rsplit('/')
                        .find(|seg| seg.len() > 5 && seg.contains(':'))
                        .map(str::to_string)
                })
            });
            if let Some(bdf) = bdf {
                vram = std::fs::read_to_string(format!(
                    "/proc/driver/nvidia/gpus/{bdf}/information"
                ))
                .ok()
                .and_then(|s| {
                    s.lines().find_map(|line| {
                        let rest = line.trim().strip_prefix("Video Memory :")?;
                        let mib: u64 = rest.split_whitespace().next()?.parse().ok()?;
                        Some(mib * 1024 * 1024)
                    })
                });
            }
        }
        if let Some(v) = vram {
            let entry = per_vendor.entry(vendor.to_string()).or_insert(0);
            *entry = (*entry).max(v);
        }
    }
    per_vendor.into_iter().collect()
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
        // The first mode line in the file is the preferred mode and carries
        // the refresh rate: "3840x2160 60.00 3840 4080 4480 5120 2160 2163 2168 2200 193000".
        let mode = std::fs::read_to_string(entry.path().join("modes"))
            .ok()
            .and_then(|modes| modes.lines().find_map(parse_display_mode));
        let Some((width, height, refresh_rate)) = mode else {
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
            refresh_rate,
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
fn parse_display_mode(mode: &str) -> Option<(u32, u32, u32)> {
    // "3840x2160 60.00 3840 4080 4480 5120 2160 2163 2168 2200 193000" →
    // (3840, 2160, 60). Lines without a parseable rate keep refresh 0.
    let mut fields = mode.split_whitespace();
    let (width, height) = fields.next()?.split_once('x')?;
    let refresh_rate = fields
        .next()
        .and_then(|hz| hz.parse::<f32>().ok())
        .map(|hz| hz.round() as u32)
        .unwrap_or(0);
    Some((width.parse().ok()?, height.parse().ok()?, refresh_rate))
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
    use std::collections::HashSet;
    use std::path::Path;

    let mut apps = Vec::new();
    let mut seen = HashSet::new();
    let mut add_dir = |dir: &Path, location: &str| {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Some((name, exec)) = parse_desktop_entry(&text) else {
                continue;
            };
            if seen.insert(exec.clone()) {
                apps.push(StartupApp {
                    name,
                    command: exec,
                    location: location.to_string(),
                });
            }
        }
    };

    if let Some(home) = std::env::var_os("HOME") {
        add_dir(&Path::new(&home).join(".config/autostart"), "~/.config/autostart");
    }
    add_dir(Path::new("/etc/xdg/autostart"), "/etc/xdg/autostart");
    apps
}

/// Parse a freedesktop `.desktop` autostart entry into (Name, Exec). Returns
/// `None` for hidden entries, entries without an Exec line, or non-`[Desktop
/// Entry]` sections.
#[cfg_attr(windows, allow(dead_code))]
fn parse_desktop_entry(text: &str) -> Option<(String, String)> {
    let mut name = String::new();
    let mut exec = String::new();
    let mut hidden = false;
    let mut in_entry = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry || line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(v) = line.strip_prefix("Name=") {
            name = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("Exec=") {
            exec = v.trim().to_string();
        } else if line == "Hidden=true" {
            hidden = true;
        }
    }
    if hidden || exec.is_empty() {
        return None;
    }
    Some((if name.is_empty() { exec.clone() } else { name }, exec))
}

/// Physical storage devices. Windows: WMI `MSFT_PhysicalDisk` (model, media,
/// health, bus). Linux: sysfs under `/sys/block` (model, size, rotational →
/// SSD/HDD, bus heuristic). SMART health requires root on Linux, so it stays
/// "Unknown" rather than guessing.
#[cfg(windows)]
pub fn physical_disks() -> Vec<PhysicalDiskInfo> {
    crate::win::wmi::physical_disks()
}

#[cfg(not(windows))]
pub fn physical_disks() -> Vec<PhysicalDiskInfo> {
    let Ok(entries) = std::fs::read_dir("/sys/block") else {
        return Vec::new();
    };
    let mut disks = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_ignored_block(&name) {
            continue;
        }
        let dir = entry.path();
        let friendly_name = std::fs::read_to_string(dir.join("device/model"))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let size_bytes = std::fs::read_to_string(dir.join("size"))
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .map(sectors_to_bytes)
            .unwrap_or(0);
        let rotational = std::fs::read_to_string(dir.join("queue/rotational"))
            .ok()
            .map(|s| s.trim() == "1")
            .unwrap_or(false);
        let device_path = std::fs::canonicalize(&dir)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        disks.push(PhysicalDiskInfo {
            friendly_name,
            media_type: if rotational { "HDD" } else { "SSD" }.to_string(),
            health_status: "Unknown".to_string(),
            bus_type: classify_bus(&name, &device_path),
            firmware_version: None,
            size_bytes,
        });
    }
    disks.sort_by_key(|d| std::cmp::Reverse(d.size_bytes));
    disks
}

/// Loop, RAM-disk, optical, device-mapper, and other virtual block devices are
/// not physical storage.
#[cfg(not(windows))]
fn is_ignored_block(name: &str) -> bool {
    name.starts_with("loop")
        || name.starts_with("ram")
        || name.starts_with("sr")
        || name.starts_with("dm-")
        || name.starts_with("zram")
        || name.starts_with("md")
        || name.starts_with("fd")
        || name.starts_with("nbd")
}

/// 512-byte sectors as reported by `/sys/block/<name>/size`.
#[cfg(not(windows))]
fn sectors_to_bytes(sectors: u64) -> u64 {
    sectors.saturating_mul(512)
}

/// Heuristic bus classification from the block device name and the resolved
/// sysfs device path (which embeds the transport, e.g. `/usb/`).
#[cfg(not(windows))]
fn classify_bus(name: &str, device_path: &str) -> String {
    let lower = device_path.to_ascii_lowercase();
    if name.starts_with("nvme") {
        "NVMe".to_string()
    } else if lower.contains("/usb") {
        "USB".to_string()
    } else if lower.contains("sas") {
        "SAS".to_string()
    } else if name.starts_with("sd") || name.starts_with("hd") || name.starts_with("mmc") {
        "SATA".to_string()
    } else {
        "Unknown".to_string()
    }
}

/// Temperature sensors. Windows: ACPI thermal zones via WMI
/// (`MSAcpi_ThermalZoneTemperature`) — sysinfo's `Components` has no sensor
/// access on Windows and always returns an empty list there. Linux: sysinfo
/// `Components` (sysfs hwmon) is read by the caller, so this adds nothing.
#[cfg(windows)]
pub fn temperatures() -> Vec<TemperatureInfo> {
    crate::win::wmi::thermal_zone_temperatures()
        .into_iter()
        .map(|t| TemperatureInfo {
            label: t.label,
            celsius: t.celsius,
        })
        .collect()
}

#[cfg(not(windows))]
pub fn temperatures() -> Vec<TemperatureInfo> {
    Vec::new()
}

/// Motherboard, BIOS, and OS edition, detected together. Windows: one WMI
/// connection for all three. Linux: world-readable DMI files under
/// `/sys/class/dmi/id` (returns `None` where the distro restricts them).
#[cfg(windows)]
pub fn system_hardware() -> crate::win::wmi::SystemHardware {
    crate::win::wmi::system_hardware()
}

#[cfg(not(windows))]
pub fn system_hardware() -> crate::win::wmi::SystemHardware {
    crate::win::wmi::SystemHardware {
        motherboard: motherboard(),
        bios: bios(),
        edition: None,
    }
}

#[cfg(not(windows))]
fn motherboard() -> Option<MotherboardInfo> {
    let manufacturer = std::fs::read_to_string("/sys/class/dmi/id/board_vendor")
        .map(|s| s.trim().to_string())
        .ok()?;
    let product = std::fs::read_to_string("/sys/class/dmi/id/board_name")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    if manufacturer.is_empty() && product.is_empty() {
        return None;
    }
    Some(MotherboardInfo {
        manufacturer,
        product,
    })
}

#[cfg(not(windows))]
fn bios() -> Option<BiosInfo> {
    let vendor = std::fs::read_to_string("/sys/class/dmi/id/bios_vendor")
        .map(|s| s.trim().to_string())
        .ok()?;
    let version = std::fs::read_to_string("/sys/class/dmi/id/bios_version")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    if vendor.is_empty() && version.is_empty() {
        return None;
    }
    Some(BiosInfo { vendor, version })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vram_bytes_prefers_qword_and_converts_mb() {
        // qwMemorySize (bytes) wins over the MB value.
        assert_eq!(
            vram_bytes(Some(8 * 1024 * 1024 * 1024), Some(8192)),
            8 * 1024 * 1024 * 1024
        );
        // MB value converts to bytes — including past WMI's 4 GiB AdapterRAM cap.
        assert_eq!(vram_bytes(None, Some(8192)), 8 * 1024 * 1024 * 1024);
        assert_eq!(vram_bytes(None, Some(4096)), 4 * 1024 * 1024 * 1024);
        // Nothing known → 0, and zeroes are treated as unknown.
        assert_eq!(vram_bytes(None, None), 0);
        assert_eq!(vram_bytes(Some(0), Some(0)), 0);
    }

    #[cfg(not(windows))]
    #[test]
    fn sectors_to_bytes_uses_512_byte_sectors() {
        assert_eq!(sectors_to_bytes(1_953_525_168), 1_000_204_886_016);
        assert_eq!(sectors_to_bytes(0), 0);
    }

    #[cfg(not(windows))]
    #[test]
    fn ignored_block_devices_are_excluded() {
        for n in ["loop0", "ram0", "sr0", "dm-0", "zram0", "md0", "fd0", "nbd0"] {
            assert!(is_ignored_block(n), "{n} should be ignored");
        }
        for n in ["sda", "nvme0n1", "mmcblk0", "sdb1"] {
            assert!(!is_ignored_block(n), "{n} should not be ignored");
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn bus_classification_heuristics() {
        assert_eq!(
            classify_bus("nvme0n1", "/sys/devices/pci0000:00/0000:03:00.0/nvme/nvme0"),
            "NVMe"
        );
        assert_eq!(
            classify_bus("sdb", "/sys/devices/pci0000:00/0000:00:14.0/usb1/1-2/1-2:1.0/host6/block/sdb"),
            "USB"
        );
        assert_eq!(
            classify_bus("sda", "/sys/devices/pci0000:00/0000:00:17.0/ata1/host0/target0:0:0/0:0:0:0/block/sda"),
            "SATA"
        );
        assert_eq!(
            classify_bus("sdc", "/sys/devices/pci0000:00/0000:05:00.0/sas_host/host4/port-4:0/block/sdc"),
            "SAS"
        );
        assert_eq!(classify_bus("xyz", "/sys/devices/unknown"), "Unknown");
    }

    #[cfg(not(windows))]
    #[test]
    fn display_mode_parses_resolution_and_refresh() {
        assert_eq!(
            parse_display_mode(
                "3840x2160 60.00 3840 4080 4480 5120 2160 2163 2168 2200 193000"
            ),
            Some((3840, 2160, 60))
        );
        assert_eq!(
            parse_display_mode(
                "2560x1440 143.98 2560 2608 2640 2720 1440 1443 1448 1478 241500"
            ),
            Some((2560, 1440, 144))
        );
        // No rate field → refresh 0, resolution still parsed.
        assert_eq!(parse_display_mode("1920x1080"), Some((1920, 1080, 0)));
        assert_eq!(parse_display_mode("garbage"), None);
    }

    #[test]
    fn desktop_entry_parsing() {
        let text = "[Desktop Entry]\nType=Application\nName=Steam\nExec=/usr/bin/steam %U\nX-GNOME-Autostart-enabled=true\n";
        assert_eq!(
            parse_desktop_entry(text),
            Some(("Steam".to_string(), "/usr/bin/steam %U".to_string()))
        );
        // Hidden entries are skipped.
        assert_eq!(parse_desktop_entry("[Desktop Entry]\nName=X\nExec=/bin/x\nHidden=true\n"), None);
        // No Exec line → skipped.
        assert_eq!(parse_desktop_entry("[Desktop Entry]\nName=X\n"), None);
        // Non-`[Desktop Entry]` sections are ignored, and Name is the first one.
        let multi = "[Desktop Entry]\nName=A\nExec=/bin/a\n[Desktop Action foo]\nName=B\nExec=/bin/b\n";
        assert_eq!(
            parse_desktop_entry(multi),
            Some(("A".to_string(), "/bin/a".to_string()))
        );
    }
}
