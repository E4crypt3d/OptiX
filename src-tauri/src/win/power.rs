//! Windows power-scheme management via `powrprof` (PowerGetActiveScheme,
//! PowerSetActiveScheme, PowerDuplicateScheme, PowerWriteACValueIndex,
//! PowerReadACValueIndex, PowerEnumerate, PowerDeleteScheme). GUIDs cross the
//! module boundary as lowercase strings (`xxxxxxxx-xxxx-...`).

use crate::error::{OptixError, Result};
use crate::models::power::PowerScheme;
use crate::models::snapshot::ChangeRecord;

#[cfg(windows)]
use windows_sys::core::GUID;
#[cfg(windows)]
use windows_sys::Win32::Foundation::{LocalFree, ERROR_SUCCESS};
#[cfg(windows)]
use windows_sys::Win32::System::Power as powrprof;

/// Null power root key (defaults to the system power settings).
#[cfg(windows)]
fn root_key() -> windows_sys::Win32::System::Registry::HKEY {
    std::ptr::null_mut()
}

#[cfg(windows)]
fn guid_to_string(g: &GUID) -> String {
    format!(
        "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        g.data1,
        g.data2,
        g.data3,
        g.data4[0],
        g.data4[1],
        g.data4[2],
        g.data4[3],
        g.data4[4],
        g.data4[5],
        g.data4[6],
        g.data4[7],
    )
}

/// Parse a GUID string (with or without braces/dashes) into a `GUID`.
#[cfg(windows)]
fn parse_guid(s: &str) -> Option<GUID> {
    let hex: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() != 32 {
        return None;
    }
    let value = u128::from_str_radix(&hex, 16).ok()?;
    Some(GUID::from_u128(value))
}

/// Well-known scheme names, used when the localized friendly name cannot be
/// read (or as a stable label).
#[cfg(windows)]
fn known_scheme_name(guid: &str) -> Option<&'static str> {
    match guid {
        "381b4222-f694-41f0-9685-ff5bb260df2e" => Some("Balanced"),
        "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c" => Some("High performance"),
        "a1841308-3541-4fab-bc81-f71556f20b4a" => Some("Power saver"),
        "e9a42b02-d5df-432d-aa00-6a11a9fd3e6e" => Some("Ultimate Performance"),
        _ => None,
    }
}

/// Resolve a scheme GUID's display name: localized friendly name, then the
/// well-known English name, then `None`.
#[cfg(windows)]
pub fn scheme_name(guid: &str) -> Option<String> {
    let parsed = parse_guid(guid)?;
    read_friendly_name(&parsed).or_else(|| known_scheme_name(guid).map(str::to_string))
}

#[cfg(not(windows))]
pub fn scheme_name(_guid: &str) -> Option<String> {
    None
}

/// Whether the system is running on AC power (`GetSystemPowerStatus`).
/// `None` when the status is not reported (ACLineStatus 255).
#[cfg(windows)]
pub fn on_ac_power() -> Option<bool> {
    use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    let mut status: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
    let ok = unsafe { GetSystemPowerStatus(&mut status) };
    if ok == 0 {
        return None;
    }
    match status.ACLineStatus {
        0 => Some(false), // offline (battery)
        1 => Some(true),  // online (AC)
        _ => None,        // 255 = unknown
    }
}

#[cfg(not(windows))]
pub fn on_ac_power() -> Option<bool> {
    None
}

/// Read a scheme's localized friendly name (UTF-16LE) via `PowerReadFriendlyName`.
#[cfg(windows)]
fn read_friendly_name(guid: &GUID) -> Option<String> {
    use windows_sys::Win32::Foundation::ERROR_MORE_DATA;

    // First call: NULL buffer to learn the required size (in bytes).
    let mut size: u32 = 0;
    let result = unsafe {
        powrprof::PowerReadFriendlyName(
            root_key(),
            guid,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null_mut(),
            &mut size,
        )
    };
    if result != ERROR_SUCCESS && result != ERROR_MORE_DATA || size == 0 {
        return None;
    }

    let mut buffer = vec![0u16; size as usize / 2 + 1];
    let result = unsafe {
        powrprof::PowerReadFriendlyName(
            root_key(),
            guid,
            std::ptr::null(),
            std::ptr::null(),
            buffer.as_mut_ptr() as *mut u8,
            &mut size,
        )
    };
    if result != ERROR_SUCCESS {
        return None;
    }
    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    Some(String::from_utf16_lossy(&buffer[..len]))
}

/// Current active power scheme GUID.
#[cfg(windows)]
pub fn active_scheme() -> Option<String> {
    let mut guid_ptr: *mut GUID = std::ptr::null_mut();
    let result = unsafe { powrprof::PowerGetActiveScheme(root_key(), &mut guid_ptr) };
    if result != ERROR_SUCCESS || guid_ptr.is_null() {
        return None;
    }
    let s = guid_to_string(unsafe { &*guid_ptr });
    unsafe { LocalFree(guid_ptr as _) };
    Some(s)
}

#[cfg(not(windows))]
pub fn active_scheme() -> Option<String> {
    None
}

/// Enumerate all power schemes, newest/active-flagged.
#[cfg(windows)]
pub fn list_schemes() -> Vec<PowerScheme> {
    use windows_sys::Win32::System::Power::ACCESS_SCHEME;

    let active = active_scheme();
    let mut out = Vec::new();
    let mut index = 0u32;
    loop {
        let mut buffer = [0u8; 16];
        let mut size = buffer.len() as u32;
        let result = unsafe {
            powrprof::PowerEnumerate(
                root_key(),
                std::ptr::null(),
                std::ptr::null(),
                ACCESS_SCHEME,
                index,
                buffer.as_mut_ptr(),
                &mut size,
            )
        };
        if result != ERROR_SUCCESS {
            break;
        }
        let guid = unsafe { std::ptr::read_unaligned(buffer.as_ptr() as *const GUID) };
        let guid_str = guid_to_string(&guid);
        let name = scheme_name(&guid_str).unwrap_or_else(|| "Custom power scheme".to_string());
        out.push(PowerScheme {
            guid: guid_str.clone(),
            name,
            is_active: active.as_deref() == Some(guid_str.as_str()),
        });
        index += 1;
    }
    out
}

#[cfg(not(windows))]
pub fn list_schemes() -> Vec<PowerScheme> {
    Vec::new()
}

/// Clone `base_guid` into a new scheme, returning the new scheme's GUID.
#[cfg(windows)]
pub fn duplicate_scheme(base_guid: &str) -> Result<String> {
    let base = parse_guid(base_guid)
        .ok_or_else(|| OptixError::InvalidState(format!("invalid scheme GUID: {base_guid}")))?;
    let mut new_ptr: *mut GUID = std::ptr::null_mut();
    let result = unsafe { powrprof::PowerDuplicateScheme(root_key(), &base, &mut new_ptr) };
    if result != ERROR_SUCCESS || new_ptr.is_null() {
        return Err(OptixError::Windows(format!(
            "PowerDuplicateScheme failed for {base_guid} (error {result})"
        )));
    }
    let guid = guid_to_string(unsafe { &*new_ptr });
    unsafe { LocalFree(new_ptr as _) };
    Ok(guid)
}

#[cfg(not(windows))]
pub fn duplicate_scheme(_base_guid: &str) -> Result<String> {
    Err(OptixError::UnsupportedPlatform("power schemes".into()))
}

/// Set a scheme's localized friendly name.
#[cfg(windows)]
pub fn write_friendly_name(guid: &str, name: &str) -> Result<()> {
    let g = parse_guid(guid)
        .ok_or_else(|| OptixError::InvalidState(format!("invalid scheme GUID: {guid}")))?;
    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let result = unsafe {
        powrprof::PowerWriteFriendlyName(
            root_key(),
            &g,
            std::ptr::null(),
            std::ptr::null(),
            wide.as_ptr() as *const u8,
            (wide.len() * 2) as u32,
        )
    };
    if result != ERROR_SUCCESS {
        return Err(OptixError::Windows(format!(
            "PowerWriteFriendlyName failed (error {result})"
        )));
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn write_friendly_name(_guid: &str, _name: &str) -> Result<()> {
    Err(OptixError::UnsupportedPlatform("power schemes".into()))
}

/// Make `guid` the active power scheme.
#[cfg(windows)]
pub fn set_active_scheme(guid: &str) -> Result<()> {
    let g = parse_guid(guid)
        .ok_or_else(|| OptixError::InvalidState(format!("invalid scheme GUID: {guid}")))?;
    let result = unsafe { powrprof::PowerSetActiveScheme(root_key(), &g) };
    if result != ERROR_SUCCESS {
        return Err(OptixError::Windows(format!(
            "PowerSetActiveScheme failed for {guid} (error {result})"
        )));
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn set_active_scheme(_guid: &str) -> Result<()> {
    Err(OptixError::UnsupportedPlatform("power schemes".into()))
}

/// Write an AC value index into a scheme.
#[cfg(windows)]
pub fn write_ac_index(scheme: &str, subgroup: &str, setting: &str, value: u32) -> Result<()> {
    let (s, sg, st) = parse_triple(scheme, subgroup, setting)?;
    let result = unsafe { powrprof::PowerWriteACValueIndex(root_key(), &s, &sg, &st, value) };
    if result != ERROR_SUCCESS {
        return Err(OptixError::Windows(format!(
            "PowerWriteACValueIndex failed for {setting} (error {result})"
        )));
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn write_ac_index(_scheme: &str, _subgroup: &str, _setting: &str, _value: u32) -> Result<()> {
    Err(OptixError::UnsupportedPlatform("power schemes".into()))
}

/// Write a DC value index into a scheme.
#[cfg(windows)]
pub fn write_dc_index(scheme: &str, subgroup: &str, setting: &str, value: u32) -> Result<()> {
    let (s, sg, st) = parse_triple(scheme, subgroup, setting)?;
    let result = unsafe { powrprof::PowerWriteDCValueIndex(root_key(), &s, &sg, &st, value) };
    if result != ERROR_SUCCESS {
        return Err(OptixError::Windows(format!(
            "PowerWriteDCValueIndex failed for {setting} (error {result})"
        )));
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn write_dc_index(_scheme: &str, _subgroup: &str, _setting: &str, _value: u32) -> Result<()> {
    Err(OptixError::UnsupportedPlatform("power schemes".into()))
}

/// Read an AC value index from a scheme.
#[cfg(windows)]
pub fn read_ac_index(scheme: &str, subgroup: &str, setting: &str) -> Option<u32> {
    let (s, sg, st) = parse_triple(scheme, subgroup, setting).ok()?;
    let mut value = 0u32;
    let result =
        unsafe { powrprof::PowerReadACValueIndex(root_key(), &s, &sg, &st, &mut value) };
    if result == ERROR_SUCCESS {
        Some(value)
    } else {
        None
    }
}

#[cfg(not(windows))]
pub fn read_ac_index(_scheme: &str, _subgroup: &str, _setting: &str) -> Option<u32> {
    None
}

/// Delete a power scheme. A scheme that is already gone is treated as success:
/// rollback re-runs (e.g. restoring an older snapshot after a re-apply) must
/// not fail because a previous rollback already removed the clone.
#[cfg(windows)]
pub fn delete_scheme(guid: &str) -> Result<()> {
    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_NOT_FOUND};
    let g = parse_guid(guid)
        .ok_or_else(|| OptixError::InvalidState(format!("invalid scheme GUID: {guid}")))?;
    let result = unsafe { powrprof::PowerDeleteScheme(root_key(), &g) };
    if result == ERROR_NOT_FOUND || result == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    if result != ERROR_SUCCESS {
        return Err(OptixError::Windows(format!(
            "PowerDeleteScheme failed for {guid} (error {result})"
        )));
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn delete_scheme(_guid: &str) -> Result<()> {
    Err(OptixError::UnsupportedPlatform("power schemes".into()))
}

#[cfg(windows)]
fn parse_triple(scheme: &str, subgroup: &str, setting: &str) -> Result<(GUID, GUID, GUID)> {
    let s = parse_guid(scheme)
        .ok_or_else(|| OptixError::InvalidState(format!("invalid scheme GUID: {scheme}")))?;
    let sg = parse_guid(subgroup)
        .ok_or_else(|| OptixError::InvalidState(format!("invalid subgroup GUID: {subgroup}")))?;
    let st = parse_guid(setting)
        .ok_or_else(|| OptixError::InvalidState(format!("invalid setting GUID: {setting}")))?;
    Ok((s, sg, st))
}

/// Roll back a `power`-domain change recorded by the engine. `scheme:active`
/// restores the previous active scheme; `scheme:create` deletes the clone.
#[cfg(windows)]
pub fn rollback_power(change: &ChangeRecord) -> Result<()> {
    match change.location.as_str() {
        "scheme:active" => {
            let old = change.old_value.as_deref().ok_or_else(|| {
                OptixError::InvalidState("no previous power scheme recorded".into())
            })?;
            set_active_scheme(old)
        }
        "scheme:create" => {
            let new = change.new_value.as_deref().ok_or_else(|| {
                OptixError::InvalidState("no cloned power scheme recorded".into())
            })?;
            delete_scheme(new)
        }
        other => Err(OptixError::InvalidState(format!(
            "unknown power change location: {other}"
        ))),
    }
}

#[cfg(not(windows))]
pub fn rollback_power(_change: &ChangeRecord) -> Result<()> {
    Err(OptixError::UnsupportedPlatform("power rollback".into()))
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    #[test]
    fn guid_string_round_trips() {
        use super::{guid_to_string, parse_guid};
        for s in [
            "381b4222-f694-41f0-9685-ff5bb260df2e",
            "{8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c}",
        ] {
            let g = parse_guid(s).unwrap();
            let canonical = guid_to_string(&g);
            assert_eq!(canonical, s.trim_start_matches('{').trim_end_matches('}').to_lowercase());
        }
    }

    #[cfg(windows)]
    #[test]
    fn rejects_malformed_guid() {
        assert!(super::parse_guid("not-a-guid").is_none());
        assert!(super::parse_guid("381b4222").is_none());
    }
}
