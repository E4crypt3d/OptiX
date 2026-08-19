//! Registry access for snapshot capture and rollback (Windows-only).

use serde::Serialize;

use crate::error::{OptixError, Result};
use crate::models::snapshot::ChangeRecord;

/// A captured registry value, serialized into `registry.json`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryEntry {
    pub name: String,
    /// Full path including hive and value name, e.g.
    /// `HKLM\SYSTEM\...\GraphicsDrivers\HwSchMode`.
    pub path: String,
    pub value_name: String,
    pub value: Option<String>,
}

/// Capture the gaming-related registry keys Optix tracks. Read-only here; the
/// toggle/write path lands with the GPU & cache panel.
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
                path: format!("{hive}\\{subkey}\\{value_name}"),
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

/// Restore a registry value from a change record. `location` has the form
/// `<HIVE>\<subkey>\<value_name>`.
#[cfg(windows)]
pub fn rollback_registry(change: &ChangeRecord) -> Result<()> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    let Some(old) = change.old_value.as_deref() else {
        return Err(OptixError::InvalidState(
            "no previous value recorded for registry change".into(),
        ));
    };

    let parts: Vec<&str> = change.location.splitn(3, '\\').collect();
    if parts.len() != 3 {
        return Err(OptixError::InvalidState(format!(
            "malformed registry location: {}",
            change.location
        )));
    }
    let (hive, subkey, value_name) = (parts[0], parts[1], parts[2]);

    let (key, _disposition) = match hive {
        "HKLM" => RegKey::predef(HKEY_LOCAL_MACHINE).create_subkey(subkey)?,
        "HKCU" => RegKey::predef(HKEY_CURRENT_USER).create_subkey(subkey)?,
        other => {
            return Err(OptixError::InvalidState(format!(
                "unknown registry hive: {other}"
            )))
        }
    };

    if let Ok(v) = old.parse::<u32>() {
        key.set_value(value_name, &v)?;
    } else {
        key.set_value(value_name, &old)?;
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn rollback_registry(_change: &ChangeRecord) -> Result<()> {
    Err(OptixError::UnsupportedPlatform("registry rollback".into()))
}
