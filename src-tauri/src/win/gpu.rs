//! Windows GPU driver registry access (Phase 8). Currently: the AMD shader
//! cache mode (`UMD\ShaderCache` REG_BINARY) with snapshot-first, verified,
//! reversible writes. Gaming toggles (HAGS/GameDVR/VBS/Game Mode/MPO) reuse
//! the generic registry helpers in `win::registry`.

use crate::error::{OptixError, Result};
use crate::models::gpu::AmdShaderCache;
use crate::models::snapshot::ChangeRecord;

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
        .open_subkey(&format!(r"{DISPLAY_CLASS}\{sub}\UMD"))
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
    if s.len() % 2 != 0 {
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
