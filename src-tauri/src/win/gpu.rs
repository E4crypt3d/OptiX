//! Windows GPU driver registry access (Phase 8). Currently: the AMD shader
//! cache mode (`UMD\ShaderCache` REG_BINARY) with snapshot-first, verified,
//! reversible writes. Gaming toggles (HAGS/GameDVR/VBS/Game Mode/MPO) reuse
//! the generic registry helpers in `win::registry`.

use crate::error::{OptixError, Result};
use crate::models::gpu::AmdShaderCache;
use crate::models::snapshot::ChangeRecord;

/// Live GPU telemetry snapshot (temperature, usage, VRAM used).
pub struct LiveTelemetry {
    pub temperature: Option<f32>,
    pub usage: Option<f32>,
    pub memory_used: Option<u64>,
}

/// Query live GPU telemetry by adapter name and vendor.
#[cfg(windows)]
pub fn live_telemetry(name: &str, vendor: &str) -> LiveTelemetry {
    if vendor.eq_ignore_ascii_case("NVIDIA") {
        return live_telemetry_nvml(name);
    }
    LiveTelemetry {
        temperature: None,
        usage: None,
        memory_used: None,
    }
}

/// Query live GPU telemetry from Linux sysfs (AMD amdgpu, Intel i915/xe).
#[cfg(not(windows))]
pub fn live_telemetry(_name: &str, vendor: &str) -> LiveTelemetry {
    let mut temp = None;
    let mut usage = None;
    let mut mem_used = None;

    if let Ok(entries) = std::fs::read_dir("/sys/class/drm") {
        for entry in entries.flatten() {
            let card = entry.file_name().to_string_lossy().into_owned();
            if !card.starts_with("card") || card.contains('-') {
                continue;
            }
            let device = entry.path().join("device");
            let card_vendor = std::fs::read_to_string(device.join("vendor"))
                .ok()
                .map(|s| s.trim().to_string())
                .unwrap_or_default();

            // Match vendor: 0x1002=AMD, 0x10de=NVIDIA, 0x8086=Intel
            let matches = match vendor.to_ascii_lowercase().as_str() {
                "amd" => card_vendor == "0x1002",
                "nvidia" => card_vendor == "0x10de",
                "intel" => card_vendor == "0x8086",
                _ => false,
            };
            if !matches {
                continue;
            }

            // AMD: gpu_busy_percent, mem_info_vram_used, hwmon temp1_input
            if card_vendor == "0x1002" {
                if usage.is_none() {
                    usage = std::fs::read_to_string(device.join("gpu_busy_percent"))
                        .ok()
                        .and_then(|s| s.trim().parse::<f32>().ok());
                }
                if mem_used.is_none() {
                    mem_used = std::fs::read_to_string(device.join("mem_info_vram_used"))
                        .ok()
                        .and_then(|s| s.trim().parse::<u64>().ok());
                }
                if temp.is_none() {
                    temp = read_hwmon_temp(&device);
                }
            }

            // Intel: hwmon temp1_input (i915/xe)
            if card_vendor == "0x8086" && temp.is_none() {
                temp = read_hwmon_temp(&device);
            }

            if temp.is_some() || usage.is_some() || mem_used.is_some() {
                break;
            }
        }
    }

    LiveTelemetry {
        temperature: temp,
        usage,
        memory_used: mem_used,
    }
}

/// Read the first hwmon temp1_input (millidegrees Celsius) from a GPU device.
#[cfg(not(windows))]
fn read_hwmon_temp(device: &std::path::Path) -> Option<f32> {
    let hwmon_dir = device.join("hwmon");
    let entries = std::fs::read_dir(&hwmon_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path().join("temp1_input");
        if path.exists() {
            let raw: u32 = std::fs::read_to_string(&path)
                .ok()?
                .trim()
                .parse()
                .ok()?;
            return Some(raw as f32 / 1000.0);
        }
    }
    None
}

/// NVIDIA GPU telemetry via NVML (loaded dynamically).
#[cfg(windows)]
fn live_telemetry_nvml(name: &str) -> LiveTelemetry {
    use std::ffi::CStr;

    let mut temp = None;
    let mut usage = None;
    let mut mem_used = None;

    // Try to load NVML dynamically — fails gracefully if not installed.
    let nvml = unsafe {
        // Narrow entry point on purpose: building the wide name would need
        // real UTF-16, not a byte-pointer reinterpretation.
        let lib = windows_sys::Win32::System::LibraryLoader::LoadLibraryA(
            c"nvml.dll".as_ptr().cast(),
        );
        if lib.is_null() {
            return LiveTelemetry { temperature: None, usage: None, memory_used: None };
        }
        let init_fn: Option<unsafe extern "C" fn() -> u32> = {
            let sym = windows_sys::Win32::System::LibraryLoader::GetProcAddress(
                lib,
                c"nvmlInit_v2".as_ptr().cast(),
            );
            sym.map(|f| std::mem::transmute(f))
        };
        if let Some(init) = init_fn {
            let status = init();
            if status != 0 {
                return LiveTelemetry { temperature: None, usage: None, memory_used: None };
            }
        } else {
            return LiveTelemetry { temperature: None, usage: None, memory_used: None };
        }
        lib
    };

    // Get device count, find the matching adapter.
    unsafe {
        let mut count = 0u32;
        let get_count: Option<unsafe extern "C" fn(*mut u32) -> u32> = {
            let sym = windows_sys::Win32::System::LibraryLoader::GetProcAddress(
                nvml,
                c"nvmlDeviceGetCount_v2".as_ptr().cast(),
            );
            sym.map(|f| std::mem::transmute(f))
        };
        if let Some(f) = get_count {
            let _ = f(&mut count);
        }

        let get_name: Option<unsafe extern "C" fn(u32, *mut i8, u32) -> u32> = {
            let sym = windows_sys::Win32::System::LibraryLoader::GetProcAddress(
                nvml,
                c"nvmlDeviceGetHandleByIndex_v2".as_ptr().cast(),
            );
            sym.map(|f| std::mem::transmute(f))
        };
        let get_temp: Option<unsafe extern "C" fn(usize, u32, *mut u32) -> u32> = {
            let sym = windows_sys::Win32::System::LibraryLoader::GetProcAddress(
                nvml,
                c"nvmlDeviceGetTemperature".as_ptr().cast(),
            );
            sym.map(|f| std::mem::transmute(f))
        };
        let get_util: Option<unsafe extern "C" fn(usize, *mut NvmlUtilization) -> u32> = {
            let sym = windows_sys::Win32::System::LibraryLoader::GetProcAddress(
                nvml,
                c"nvmlDeviceGetUtilizationRates".as_ptr().cast(),
            );
            sym.map(|f| std::mem::transmute(f))
        };
        let get_mem: Option<unsafe extern "C" fn(usize, *mut NvmlMemory) -> u32> = {
            let sym = windows_sys::Win32::System::LibraryLoader::GetProcAddress(
                nvml,
                c"nvmlDeviceGetMemoryInfo".as_ptr().cast(),
            );
            sym.map(|f| std::mem::transmute(f))
        };

        for i in 0..count {
            let mut dev: usize = 0;
            if let Some(f) = get_name {
                if f(i, &mut dev as *mut usize as *mut i8, 64) != 0 || dev == 0 {
                    continue;
                }
            } else {
                continue;
            }

            // Check if this device name matches.
            let mut buf = [0i8; 96];
            let get_dev_name: Option<unsafe extern "C" fn(usize, *mut i8, u32) -> u32> = {
                let sym = windows_sys::Win32::System::LibraryLoader::GetProcAddress(
                    nvml,
                    c"nvmlDeviceGetName".as_ptr().cast(),
                );
                sym.map(|f| std::mem::transmute(f))
            };
            if let Some(f) = get_dev_name {
                let _ = f(dev, buf.as_mut_ptr(), 96);
                let device_name = CStr::from_ptr(buf.as_ptr())
                    .to_string_lossy()
                    .into_owned();
                if !name.eq_ignore_ascii_case(&device_name)
                    && !device_name.to_ascii_lowercase().contains(&name.to_ascii_lowercase())
                {
                    continue;
                }
            }

            // Temperature (sensor 0 = GPU).
            if let Some(f) = get_temp {
                let mut t = 0u32;
                if f(dev, 0, &mut t) == 0 {
                    temp = Some(t as f32);
                }
            }

            // Utilization.
            if let Some(f) = get_util {
                let mut util: NvmlUtilization = std::mem::zeroed();
                if f(dev, &mut util) == 0 {
                    usage = Some(util.gpu as f32);
                }
            }

            // Memory.
            if let Some(f) = get_mem {
                let mut mem: NvmlMemory = std::mem::zeroed();
                if f(dev, &mut mem) == 0 {
                    mem_used = Some(mem.used);
                }
            }

            break;
        }

        // Shutdown NVML.
        let shutdown: Option<unsafe extern "C" fn() -> u32> = {
            let sym = windows_sys::Win32::System::LibraryLoader::GetProcAddress(
                nvml,
                c"nvmlShutdown".as_ptr().cast(),
            );
            sym.map(|f| std::mem::transmute(f))
        };
        if let Some(f) = shutdown {
            let _ = f();
        }
        // windows-sys 0.61 declares FreeLibrary under Foundation, not
        // LibraryLoader.
        windows_sys::Win32::Foundation::FreeLibrary(nvml);
    }

    LiveTelemetry {
        temperature: temp,
        usage,
        memory_used: mem_used,
    }
}

#[cfg(windows)]
#[repr(C)]
struct NvmlUtilization {
    gpu: u32,
    memory: u32,
}

#[cfg(windows)]
#[repr(C)]
struct NvmlMemory {
    total: u64,
    free: u64,
    used: u64,
}

#[cfg(windows)]
const DISPLAY_CLASS: &str =
    r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";

/// Find the first adapter subkey that has a `UMD` key (AMD-specific).
#[cfg(windows)]
fn find_amd_umd() -> Option<(String, String)> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let class = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(DISPLAY_CLASS).ok()?;
    for sub in class.enum_keys().flatten() {
        let Ok(key) = class.open_subkey(&sub) else {
            continue;
        };
        if key.open_subkey("UMD").is_err() {
            continue;
        }
        let name: String = key.get_value("DriverDesc").unwrap_or_default();
        if !name.is_empty() {
            return Some((sub, name));
        }
    }
    None
}

/// Full registry location of the AMD `ShaderCache` value
/// (`HKLM\SYSTEM\...\Class\{...}\<adapter>\UMD\ShaderCache`).
#[cfg(windows)]
pub fn amd_shader_cache_location() -> Option<String> {
    let (sub, _name) = find_amd_umd()?;
    Some(format!(r"HKLM\{DISPLAY_CLASS}\{sub}\UMD\ShaderCache"))
}

#[cfg(not(windows))]
pub fn amd_shader_cache_location() -> Option<String> {
    None
}

/// Read the raw `ShaderCache` REG_BINARY bytes (the driver mode).
#[cfg(windows)]
pub fn amd_shader_cache_bytes() -> Option<Vec<u8>> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let (sub, _name) = find_amd_umd()?;
    RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(format!(r"{DISPLAY_CLASS}\{sub}\UMD"))
        .ok()?
        .get_raw_value("ShaderCache")
        .ok()
        .map(|raw| raw.bytes.to_vec())
}

#[cfg(not(windows))]
pub fn amd_shader_cache_bytes() -> Option<Vec<u8>> {
    None
}

/// Write raw REG_BINARY bytes to an `HKLM\<subkey>\<value>` location.
#[cfg(windows)]
pub fn write_shader_cache_bytes(location: &str, bytes: &[u8]) -> Result<()> {
    use std::borrow::Cow;
    use winreg::enums::{KEY_SET_VALUE, REG_BINARY};
    use winreg::{RegKey, RegValue};

    let (hive, subkey, value_name) = crate::win::registry::parse_location(location)?;
    let key = RegKey::predef(hive)
        .open_subkey_with_flags(&subkey, KEY_SET_VALUE)
        .map_err(|e| OptixError::Windows(format!("cannot open {subkey}: {e}")))?;
    key.set_raw_value(
        &value_name,
        &RegValue {
            bytes: Cow::Owned(bytes.to_vec()),
            vtype: REG_BINARY,
        },
    )
    .map_err(|e| OptixError::Windows(format!("cannot write {value_name}: {e}")))?;
    Ok(())
}

#[cfg(not(windows))]
pub fn write_shader_cache_bytes(_location: &str, _bytes: &[u8]) -> Result<()> {
    Err(OptixError::UnsupportedPlatform("AMD shader cache".into()))
}

/// Read the AMD shader cache mode from `UMD\ShaderCache`.
#[cfg(windows)]
pub fn amd_shader_cache_status() -> AmdShaderCache {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let (sub, name) = match find_amd_umd() {
        Some(v) => v,
        None => {
            return AmdShaderCache {
                adapter: String::new(),
                mode: "unknown".to_string(),
            }
        }
    };
    let mode = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(format!(r"{DISPLAY_CLASS}\{sub}\UMD"))
        .ok()
        .and_then(|k| k.get_raw_value("ShaderCache").ok())
        .and_then(|raw| raw.bytes.first().copied())
        .map(|b| match b {
            0x32 => "always_on",
            0x31 => "optimized",
            _ => "unknown",
        })
        .unwrap_or("unknown");
    AmdShaderCache {
        adapter: name,
        mode: mode.to_string(),
    }
}

#[cfg(not(windows))]
pub fn amd_shader_cache_status() -> AmdShaderCache {
    AmdShaderCache {
        adapter: String::new(),
        mode: "unknown".to_string(),
    }
}

/// Set the AMD shader cache mode (`always_on` = `32 00`, else `31 00`).
#[cfg(windows)]
pub fn set_amd_shader_cache(always_on: bool) -> Result<()> {
    let location = amd_shader_cache_location()
        .ok_or_else(|| OptixError::Windows("no AMD adapter with a UMD key found".into()))?;
    let bytes: Vec<u8> = if always_on { vec![0x32, 0x00] } else { vec![0x31, 0x00] };
    write_shader_cache_bytes(&location, &bytes)
}

#[cfg(not(windows))]
pub fn set_amd_shader_cache(_always_on: bool) -> Result<()> {
    Err(OptixError::UnsupportedPlatform("AMD shader cache".into()))
}

/// Roll back a `gpu`-domain change recorded by the engine: AMD shader cache
/// (REG_BINARY, which the generic registry rollback cannot restore — this
/// path re-writes the raw bytes captured before the change) and NVIDIA DRS
/// per-game profiles (removed by name).
#[cfg(windows)]
pub fn rollback_gpu(change: &ChangeRecord) -> Result<()> {
    match change.kind.as_str() {
        "set_amd_shader_cache" => {
            let old = change.old_value.as_deref().ok_or_else(|| {
                OptixError::InvalidState("no previous AMD shader cache value recorded".into())
            })?;
            let bytes = hex_bytes(old).ok_or_else(|| {
                OptixError::InvalidState(format!("bad AMD shader cache value: {old}"))
            })?;
            write_shader_cache_bytes(&change.location, &bytes)
        }
        "nvapi_profile" => crate::win::nvapi::remove_profile(&change.location),
        other => Err(OptixError::InvalidState(format!(
            "unknown GPU change kind: {other}"
        ))),
    }
}

#[cfg(not(windows))]
pub fn rollback_gpu(_change: &ChangeRecord) -> Result<()> {
    Err(OptixError::UnsupportedPlatform("GPU rollback".into()))
}

/// Decode a lowercase hex string (e.g. `"3200"`) into raw bytes.
#[cfg(any(windows, test))]
fn hex_bytes(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_bytes_decodes_even_length_hex() {
        assert_eq!(hex_bytes("3200"), Some(vec![0x32, 0x00]));
        assert_eq!(hex_bytes("31"), Some(vec![0x31]));
        assert_eq!(hex_bytes(""), Some(Vec::new()));
        assert_eq!(hex_bytes("zz"), None);
        assert_eq!(hex_bytes("123"), None); // odd length
    }
}
