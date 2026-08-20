//! Registry access for snapshot capture and rollback (Windows-only), plus
//! generic read/set/delete helpers used by the gaming toggles and startup
//! manager.

use serde::Serialize;

use crate::error::{OptixError, Result};
use crate::models::snapshot::ChangeRecord;

/// A captured registry value, serialized into `registry.json`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryEntry {
    pub name: String,
    /// Full path including hive and value name, e.g.
    /// `HKLM\\SYSTEM\\...\\GraphicsDrivers\\HwSchMode`.
    pub path: String,
    pub value_name: String,
    pub value: Option<String>,
}

/// Capture the gaming-related registry keys Optix tracks. Read-only here; the
/// toggle/write path lives in `engine::gpu`.
#[cfg(windows)]
pub fn capture_gaming_toggles() -> Vec<RegistryEntry> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    let keys = [
        (
            "HAGS",
            "HKLM",
            HKEY_LOCAL_MACHINE,
            r"SYSTEM\CurrentControlSet\Control\GraphicsDrivers",
            "HwSchMode",
        ),
        (
            "GameDVR",
            "HKCU",
            HKEY_CURRENT_USER,
            r"System\GameConfigStore",
            "GameDVR_Enabled",
        ),
        (
            "GameBarCapture",
            "HKCU",
            HKEY_CURRENT_USER,
            r"Software\Microsoft\Windows\CurrentVersion\GameDVR",
            "AppCaptureEnabled",
        ),
        (
            "MemoryIntegrity",
            "HKLM",
            HKEY_LOCAL_MACHINE,
            r"SYSTEM\CurrentControlSet\Control\DeviceGuard\Scenarios\HypervisorEnforcedCodeIntegrity",
            "Enabled",
        ),
        (
            "GameMode",
            "HKCU",
            HKEY_CURRENT_USER,
            r"Software\Microsoft\GameBar",
            "AutoGameModeEnabled",
        ),
    ];

    keys.iter()
        .map(|(name, hive, root, subkey, value_name)| {
            let value = RegKey::predef(*root)
                .open_subkey(subkey)
                .ok()
                .and_then(|k| k.get_value::<u32, _>(value_name).ok())
                .map(|v| v.to_string());
            RegistryEntry {
                name: (*name).to_string(),
                path: format!("{hive}\\\\{subkey}\\\\{value_name}"),
                value_name: (*value_name).to_string(),
                value,
            }
        })
        .collect()
}

#[cfg(not(windows))]
pub fn capture_gaming_toggles() -> Vec<RegistryEntry> {
    Vec::new()
}

/// Split a `location` of the form `<HIVE>\<subkey>\<value_name>` into its parts.
#[cfg(windows)]
pub(crate) fn parse_location(location: &str) -> Result<(winreg::HKEY, String, String)> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

    let parts: Vec<&str> = location.splitn(3, '\\').collect();
    if parts.len() != 3 {
        return Err(OptixError::InvalidState(format!(
            "malformed registry location: {location}"
        )));
    }
    let hive = match parts[0] {
        "HKLM" => HKEY_LOCAL_MACHINE,
        "HKCU" => HKEY_CURRENT_USER,
        other => {
            return Err(OptixError::InvalidState(format!(
                "unknown registry hive: {other}"
            )))
        }
    };
    Ok((hive, parts[1].to_string(), parts[2].to_string()))
}

/// Read a value at `location` as a string (DWORD decimal, else REG_SZ).
#[cfg(windows)]
pub fn read_registry_value(location: &str) -> Option<String> {
    use winreg::RegKey;

    let (hive, subkey, value_name) = parse_location(location).ok()?;
    let key = RegKey::predef(hive).open_subkey(&subkey).ok()?;
    key.get_value::<u32, _>(&value_name)
        .ok()
        .map(|v| v.to_string())
        .or_else(|| key.get_value::<String, _>(&value_name).ok())
}

#[cfg(not(windows))]
pub fn read_registry_value(_location: &str) -> Option<String> {
    None
}

/// Write a value (DWORD when numeric, otherwise REG_SZ) at `location`.
#[cfg(windows)]
pub fn set_registry_value(location: &str, value: &str) -> Result<()> {
    use winreg::RegKey;

    let (hive, subkey, value_name) = parse_location(location)?;
    let (key, _disposition) = RegKey::predef(hive).create_subkey(&subkey)?;
    if let Ok(v) = value.parse::<u32>() {
        key.set_value(&value_name, &v)?;
    } else {
        key.set_value(&value_name, &value)?;
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn set_registry_value(_location: &str, _value: &str) -> Result<()> {
    Err(OptixError::UnsupportedPlatform("registry".into()))
}

/// Delete the value at `location` (missing value is not an error).
#[cfg(windows)]
pub fn delete_registry_value(location: &str) -> Result<()> {
    use winreg::RegKey;

    let (hive, subkey, value_name) = parse_location(location)?;
    let (key, _disposition) = RegKey::predef(hive).create_subkey(&subkey)?;
    let _ = key.delete_value(&value_name);
    Ok(())
}

#[cfg(not(windows))]
pub fn delete_registry_value(_location: &str) -> Result<()> {
    Err(OptixError::UnsupportedPlatform("registry".into()))
}

/// Restore a registry change. `Some(old)` re-writes the previous value; `None`
/// means the value did not exist before, so it is deleted.
#[cfg(windows)]
pub fn rollback_registry(change: &ChangeRecord) -> Result<()> {
    match change.old_value.as_deref() {
        Some(old) => set_registry_value(&change.location, old),
        None => delete_registry_value(&change.location),
    }
}

#[cfg(not(windows))]
pub fn rollback_registry(_change: &ChangeRecord) -> Result<()> {
    Err(OptixError::UnsupportedPlatform("registry rollback".into()))
}
