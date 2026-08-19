//! Windows GPU driver registry access (Phase 8). Currently: the AMD shader
//! cache mode (`UMD\ShaderCache` REG_BINARY). Gaming toggles (HAGS/GameDVR/VBS/
//! Game Mode/MPO) reuse the generic registry helpers in `win::registry`.

use crate::error::{OptixError, Result};
use crate::models::gpu::AmdShaderCache;

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
        .open_subkey(&format!(r"{DISPLAY_CLASS}\{sub}\UMD"))
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
    use std::borrow::Cow;
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_SET_VALUE, REG_BINARY};
    use winreg::{RegKey, RegValue};

    let (sub, _name) = find_amd_umd()
        .ok_or_else(|| OptixError::Windows("no AMD adapter with a UMD key found".into()))?;
    let key = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(&format!(r"{DISPLAY_CLASS}\{sub}\UMD"), KEY_SET_VALUE)
        .map_err(|e| OptixError::Windows(format!("cannot open AMD UMD key: {e}")))?;
    let bytes: Vec<u8> = if always_on { vec![0x32, 0x00] } else { vec![0x31, 0x00] };
    key.set_raw_value(
        "ShaderCache",
        &RegValue {
            bytes: Cow::Owned(bytes),
            vtype: REG_BINARY,
        },
    )
    .map_err(|e| OptixError::Windows(format!("cannot write ShaderCache: {e}")))?;
    Ok(())
}

#[cfg(not(windows))]
pub fn set_amd_shader_cache(_always_on: bool) -> Result<()> {
    Err(OptixError::UnsupportedPlatform("AMD shader cache".into()))
}
