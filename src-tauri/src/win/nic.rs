//! Network-adapter power-saving state (Windows-only). Reads and writes the
//! power-management registry values under the network class key, snapshot-first
//! via the rollback engine (each write is recorded as a `registry` change).

use crate::error::{OptixError, Result};
use crate::models::power::NicAdapter;

/// Network adapter class GUID.
#[cfg(windows)]
const NET_CLASS: &str = r"SYSTEM\CurrentControlSet\Control\Class\{4d36e972-e325-11ce-bfc1-08002be10318}";

/// Enumerate network adapters (by driver description) and their power-saving
/// registry values.
#[cfg(windows)]
pub fn list_adapters() -> Vec<NicAdapter> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let mut out = Vec::new();
    let Ok(class) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(NET_CLASS) else {
        return out;
    };
    for subkey in class.enum_keys().flatten() {
        let Ok(key) = class.open_subkey(&subkey) else {
            continue;
        };
        let name: String = key.get_value("DriverDesc").unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        out.push(NicAdapter {
            key: subkey,
            name,
            eee: read_dword(&key, "*EEE"),
            green_ethernet: read_dword(&key, "EnableGreenEthernet"),
            pnp_capabilities: read_dword(&key, "PnPCapabilities"),
            power_management: read_dword(&key, "EnablePowerManagement"),
        });
    }
    out
}

#[cfg(not(windows))]
pub fn list_adapters() -> Vec<NicAdapter> {
    Vec::new()
}

#[cfg(windows)]
fn read_dword(key: &winreg::RegKey, value_name: &str) -> Option<u32> {
    key.get_value::<u32, _>(value_name).ok()
}

/// Read a DWORD value from an adapter's class subkey.
#[cfg(windows)]
pub fn get_dword(adapter_key: &str, value_name: &str) -> Option<u32> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;
    let class = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(NET_CLASS).ok()?;
    let key = class.open_subkey(adapter_key).ok()?;
    read_dword(&key, value_name)
}

#[cfg(not(windows))]
pub fn get_dword(_adapter_key: &str, _value_name: &str) -> Option<u32> {
    None
}

/// Write a DWORD value into an adapter's class subkey.
#[cfg(windows)]
pub fn set_dword(adapter_key: &str, value_name: &str, value: u32) -> Result<()> {
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    let key = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(&format!(r"{NET_CLASS}\{adapter_key}"), KEY_READ | KEY_WRITE)
        .map_err(|e| OptixError::Windows(format!("cannot open adapter {adapter_key}: {e}")))?;
    key.set_value(value_name, &value)
        .map_err(|e| OptixError::Windows(format!("cannot write {value_name}: {e}")))?;
    Ok(())
}

#[cfg(not(windows))]
pub fn set_dword(_adapter_key: &str, _value_name: &str, _value: u32) -> Result<()> {
    Err(OptixError::UnsupportedPlatform("network adapter power".into()))
}
