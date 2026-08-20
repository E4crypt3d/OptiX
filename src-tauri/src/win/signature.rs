//! Authenticode signature verification (Phase 6) via `WinVerifyTrust`.
//! Used by the scheduled-task checks to flag unsigned tools.

use crate::error::Result;

/// Signature state of a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureState {
    /// Signature exists and chains to a trusted root.
    Trusted,
    /// Signature exists but the chain is invalid/expired/untrusted.
    Untrusted,
    /// No Authenticode signature.
    Unsigned,
    /// Verification is unavailable on this platform.
    Unavailable,
}

impl SignatureState {
    pub fn as_str(&self) -> &'static str {
        match self {
            SignatureState::Trusted => "trusted",
            SignatureState::Untrusted => "untrusted",
            SignatureState::Unsigned => "unsigned",
            SignatureState::Unavailable => "unavailable",
        }
    }
}

/// Verify the Authenticode signature of a file. `Unsigned` when the file
/// doesn't exist or carries no signature; `Unavailable` on Linux dev.
#[cfg(windows)]
pub fn verify_file_signature(path: &str) -> Result<SignatureState> {
    use windows_sys::core::GUID;
    use windows_sys::Win32::Security::WinTrust::{
        WinVerifyTrust, WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_DATA_0,
        WINTRUST_FILE_INFO, WTD_CHOICE_FILE, WTD_REVOKE_NONE, WTD_STATEACTION_IGNORE, WTD_UI_NONE,
    };

    if !std::path::Path::new(path).is_file() {
        return Ok(SignatureState::Unsigned);
    }

    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let mut file_info: WINTRUST_FILE_INFO = unsafe { std::mem::zeroed() };
    file_info.cbStruct = std::mem::size_of::<WINTRUST_FILE_INFO>() as u32;
    file_info.pcwszFilePath = wide.as_ptr();

    let mut data: WINTRUST_DATA = unsafe { std::mem::zeroed() };
    data.cbStruct = std::mem::size_of::<WINTRUST_DATA>() as u32;
    data.dwUIChoice = WTD_UI_NONE;
    data.fdwRevocationChecks = WTD_REVOKE_NONE;
    data.dwUnionChoice = WTD_CHOICE_FILE;
    data.Anonymous = WINTRUST_DATA_0 { pFile: &mut file_info };
    data.dwStateAction = WTD_STATEACTION_IGNORE;
    data.pwszURLReference = std::ptr::null_mut();

    let mut action: GUID = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    let status = unsafe {
        WinVerifyTrust(
            std::ptr::null_mut(),
            &mut action,
            &mut data as *mut WINTRUST_DATA as *mut core::ffi::c_void,
        )
    };

    if status == 0 {
        return Ok(SignatureState::Trusted);
    }
    // TRUST_E_NOSIGNATURE = 0x800B0100 → unsigned; everything else is a
    // chain/validation failure, i.e. untrusted.
    if status as u32 == 0x800B_0100 {
        Ok(SignatureState::Unsigned)
    } else {
        Ok(SignatureState::Untrusted)
    }
}

#[cfg(not(windows))]
pub fn verify_file_signature(_path: &str) -> Result<SignatureState> {
    Ok(SignatureState::Unavailable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_strings_are_stable() {
        assert_eq!(SignatureState::Trusted.as_str(), "trusted");
        assert_eq!(SignatureState::Unsigned.as_str(), "unsigned");
        assert_eq!(SignatureState::Untrusted.as_str(), "untrusted");
        assert_eq!(SignatureState::Unavailable.as_str(), "unavailable");
    }
}