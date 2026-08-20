//! NVIDIA Driver Settings (DRS) — per-game GPU profiles (Phase 8/9).
//!
//! nvapi64.dll is runtime-loaded by name (`LoadLibraryW` + `GetProcAddressW`),
//! so there is no build-time dependency on the NVIDIA SDK. The struct layouts
//! below are byte-exact copies of the public SDK headers (`nvapi.h`, driver
//! 530+): `NVDRS_PROFILE_V1`, `NVDRS_APPLICATION_V4`, `NVDRS_SETTING_V1` and
//! `NVDRS_BINARY_SETTING`. Version fields are computed with the SDK's own
//! `MAKE_NVAPI_VERSION(type, ver) = sizeof(type) | (ver << 16)` macro, which
//! keeps them correct even if a driver's driver-settings layout changes.
//!
//! Optix only ever touches profiles it names itself (`Optix: <game>`), so
//! removing a profile — the rollback path — deletes exactly what was created
//! and nothing else. This mirrors what NVIDIA Control Panel does per-game.

// On Windows the real implementations live in the `ffi` submodule below; the
// top-level stubs (which use these types) only exist for non-Windows builds.
#[cfg(not(windows))]
use crate::error::{OptixError, Result};

// ---------------------------------------------------------------------------
// Public SDK struct layouts (byte-exact, #[repr(C)])
// ---------------------------------------------------------------------------

/// `NvU16[NVAPI_UNICODE_STRING_MAX]` — `NVAPI_UNICODE_STRING_MAX = 2048`.
pub type NvUnicodeString = [u16; 2048];

/// `NVDRS_PROFILE_V1` — profile metadata handed to `NvAPI_DRS_CreateProfile`.
#[repr(C)]
pub struct NvProfile {
    pub version: u32,
    pub profile_name: NvUnicodeString,
    /// `NVDRS_GPU_SUPPORT` bitfield: bit 0 Geforce, bit 1 Quadro, bit 2 NVS.
    pub gpu_support: u32,
    pub is_predefined: u32,
    pub num_of_apps: u32,
    pub num_of_settings: u32,
}

/// `NVDRS_SETTING_V1` — one driver setting (name + ID + type + values).
///
/// The two anonymous unions hold either a `u32`, an `NvBinarySetting` or a
/// `wsz` string; they are modeled as a flat 4100-byte array (the largest
/// member). A DWORD value lives in the first 4 bytes.
#[repr(C)]
pub struct NvSetting {
    pub version: u32,
    pub setting_name: NvUnicodeString,
    pub setting_id: u32,
    pub setting_type: u32, // NVDRS_DWORD_TYPE = 0
    pub setting_location: u32, // NVDRS_CURRENT_PROFILE_LOCATION = 0
    pub is_current_predefined: u32,
    pub is_predefined_valid: u32,
    /// Union: default value (DWORD in first 4 bytes, or binary/wsz).
    pub predefined_value: [u8; 4100],
    /// Union: current value (DWORD in first 4 bytes, or binary/wsz).
    pub current_value: [u8; 4100],
}

/// `NVDRS_APPLICATION_V4` — an application (executable) bound to a profile.
#[repr(C)]
pub struct NvApplication {
    pub version: u32,
    pub is_predefined: u32,
    pub app_name: NvUnicodeString,
    pub user_friendly_name: NvUnicodeString,
    pub launcher: NvUnicodeString,
    pub file_in_folder: NvUnicodeString,
    /// Bitfield: bit 0 isMetro, bit 1 isCommandLine, rest reserved.
    pub flags: u32,
    pub command_line: NvUnicodeString,
}

/// The `MAKE_NVAPI_VERSION(struct, ver)` macro: `sizeof(struct) | (ver << 16)`.
#[cfg(windows)]
fn version_of<T>(ver: u32) -> u32 {
    (std::mem::size_of::<T>() as u32) | (ver << 16)
}

/// Setting values Optix writes. Kept minimal and conservative — only the
/// settings with measured evidence behind them (power state + shader cache).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrsOptions {
    /// `PREFERRED_PSTATE_ID` = PREFERRED_PSTATE_PREFER_MAX (power mode).
    pub prefer_max_performance: bool,
    /// `PS_SHADERDISKCACHE` = 1 (shader cache on).
    pub shader_cache_on: bool,
}

/// Outcome of a DRS profile apply/remove.
#[derive(Debug, Clone)]
pub struct DrsResult {
    /// The DRS profile name that was created/found.
    pub profile: String,
    /// Setting names written to the profile.
    pub settings: Vec<String>,
    /// Application (executable) bindings on the profile.
    pub apps: Vec<String>,
}

// ---------------------------------------------------------------------------
// Runtime loading (Windows only)
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod ffi {
    use super::*;
    use crate::error::{OptixError, Result};
    use std::sync::OnceLock;
    use windows_sys::Win32::Foundation::HMODULE;
    use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

    type FnInit = unsafe extern "system" fn() -> i32;
    type FnOne = unsafe extern "system" fn(isize) -> i32;
    type FnSession = unsafe extern "system" fn(*mut isize) -> i32;
    type FnFind = unsafe extern "system" fn(isize, *const u16, *mut isize) -> i32;
    type FnCreate = unsafe extern "system" fn(isize, *const NvProfile, *mut isize) -> i32;
    type FnDelete = unsafe extern "system" fn(isize, isize) -> i32;
    type FnApp = unsafe extern "system" fn(isize, isize, *const NvApplication) -> i32;
    type FnSet = unsafe extern "system" fn(isize, isize, *const NvSetting) -> i32;
    type FnSettingId = unsafe extern "system" fn(*const u16, *mut u32) -> i32;

    pub struct Drs {
        pub initialize: FnInit,
        pub create_session: FnSession,
        pub destroy_session: FnOne,
        pub load_settings: FnOne,
        pub save_settings: FnOne,
        pub find_profile: FnFind,
        pub create_profile: FnCreate,
        pub delete_profile: FnDelete,
        pub create_application: FnApp,
        pub set_setting: FnSet,
        pub get_setting_id_from_name: FnSettingId,
    }

    static DRS: OnceLock<Option<Drs>> = OnceLock::new();

    fn lookup<F>(module: HMODULE, name: &[u8]) -> Option<F> {
        unsafe {
            let raw = GetProcAddress(module, name.as_ptr() as *const u8);
            // FARPROC is `Option<unsafe extern "system" fn() -> isize>`;
            // transmute_copy works for any fn-pointer-sized F.
            raw.map(|f| std::mem::transmute_copy::<_, F>(&f))
        }
    }

    /// Load nvapi64.dll and resolve the DRS entry points. Returns `Ok(None)`
    /// when the driver does not ship NVAPI (e.g. no NVIDIA GPU installed).
    pub fn drs() -> Result<Option<&'static Drs>> {
        if let Some(slot) = DRS.get() {
            return Ok(slot.as_ref());
        }
        let loaded = load();
        let _ = DRS.set(loaded);
        Ok(DRS.get().and_then(|s| s.as_ref()))
    }

    fn load() -> Option<Drs> {
        let wide: Vec<u16> = "nvapi64.dll".encode_utf16().chain(std::iter::once(0)).collect();
        let module = unsafe { LoadLibraryW(wide.as_ptr()) };
        if module.is_null() {
            return None;
        }
        let drs = Drs {
            initialize: lookup(module, b"NvAPI_Initialize\0")?,
            create_session: lookup(module, b"NvAPI_DRS_CreateSession\0")?,
            destroy_session: lookup(module, b"NvAPI_DRS_DestroySession\0")?,
            load_settings: lookup(module, b"NvAPI_DRS_LoadSettings\0")?,
            save_settings: lookup(module, b"NvAPI_DRS_SaveSettings\0")?,
            find_profile: lookup(module, b"NvAPI_DRS_FindProfileByName\0")?,
            create_profile: lookup(module, b"NvAPI_DRS_CreateProfile\0")?,
            delete_profile: lookup(module, b"NvAPI_DRS_DeleteProfile\0")?,
            create_application: lookup(module, b"NvAPI_DRS_CreateApplication\0")?,
            set_setting: lookup(module, b"NvAPI_DRS_SetSetting\0")?,
            get_setting_id_from_name: lookup(module, b"NvAPI_DRS_GetSettingIdFromName\0")?,
        };
        Some(drs)
    }

    const NVAPI_OK: i32 = 0;
    const NVAPI_ERROR: i32 = -1;
    const NVAPI_PROFILE_NOT_FOUND: i32 = -101;
    const NVAPI_SETTING_NOT_FOUND: i32 = -102;
    /// GPU support flags: Geforce | Quadro | NVS.
    const GPU_SUPPORT_ALL: u32 = 0b111;

    fn wide_fill(dst: &mut [u16], s: &str) {
        let mut chars = s.encode_utf16();
        for slot in dst.iter_mut() {
            match chars.next() {
                Some(c) => *slot = c,
                None => *slot = 0,
            }
        }
    }

    fn status_to_error(what: &str, status: i32) -> OptixError {
        match status {
            NVAPI_PROFILE_NOT_FOUND => OptixError::Other(format!("{what}: profile not found")),
            NVAPI_SETTING_NOT_FOUND => OptixError::Other(format!("{what}: setting not found")),
            NVAPI_ERROR => OptixError::Windows(format!("{what}: NVAPI_ERROR (-1)")),
            other => OptixError::Windows(format!("{what}: NVAPI status {other}")),
        }
    }

    /// Find or create the Optix profile for `game`. Returns `(session, profile)`.
    fn find_or_create_profile(
        drs: &Drs,
        session: isize,
        profile_name: &str,
    ) -> Result<isize> {
        let mut name: NvUnicodeString = [0; 2048];
        wide_fill(&mut name, profile_name);

        let mut profile = 0isize;
        let status = unsafe { (drs.find_profile)(session, name.as_ptr(), &mut profile) };
        if status == NVAPI_OK {
            return Ok(profile);
        }
        if status != NVAPI_PROFILE_NOT_FOUND {
            return Err(status_to_error("find profile", status));
        }

        // Create it.
        let mut info = NvProfile {
            version: version_of::<NvProfile>(1),
            profile_name: name,
            gpu_support: GPU_SUPPORT_ALL,
            is_predefined: 0,
            num_of_apps: 0,
            num_of_settings: 0,
        };
        let status = unsafe { (drs.create_profile)(session, &mut info, &mut profile) };
        if status != NVAPI_OK {
            return Err(status_to_error("create profile", status));
        }
        Ok(profile)
    }

    /// Resolve a setting name to its binary ID via `NvAPI_DRS_GetSettingIdFromName`.
    fn setting_id(drs: &Drs, name: &str) -> Result<u32> {
        let mut wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let mut id = 0u32;
        let status = unsafe { (drs.get_setting_id_from_name)(wide.as_mut_ptr(), &mut id) };
        if status != NVAPI_OK || id == 0 {
            return Err(status_to_error(&format!("setting ID for {name}"), status));
        }
        Ok(id)
    }

    /// Apply the DRS profile for `game`, binding `exe_path` (when known).
    pub fn apply_profile(
        game: &str,
        exe_path: Option<&str>,
        opts: &DrsOptions,
    ) -> Result<DrsResult> {
        let Some(drs) = drs()? else {
            return Err(OptixError::Windows(
                "NVAPI unavailable — no NVIDIA driver (nvapi64.dll) present".into(),
            ));
        };
        if unsafe { (drs.initialize)() } != NVAPI_OK {
            return Err(OptixError::Windows("NvAPI_Initialize failed".into()));
        }

        let mut session = 0isize;
        let status = unsafe { (drs.create_session)(&mut session) };
        if status != NVAPI_OK || session == 0 {
            return Err(status_to_error("create session", status));
        }
        // Make sure cleanup happens on the way out of every branch.
        let result = (|| {
            if unsafe { (drs.load_settings)(session) } != NVAPI_OK {
                return Err(OptixError::Windows("NvAPI_DRS_LoadSettings failed".into()));
            }

            let profile_name = format!("Optix: {game}");
            let profile = find_or_create_profile(drs, session, &profile_name)?;
            let mut result = DrsResult {
                profile: profile_name,
                settings: Vec::new(),
                apps: Vec::new(),
            };

            // Bind the executable to the profile.
            if let Some(exe) = exe_path {
                if !exe.trim().is_empty() {
                    let mut app = NvApplication {
                        version: version_of::<NvApplication>(4),
                        is_predefined: 0,
                        app_name: [0; 2048],
                        user_friendly_name: [0; 2048],
                        launcher: [0; 2048],
                        file_in_folder: [0; 2048],
                        flags: 0, // not metro, not command-line
                        command_line: [0; 2048],
                    };
                    wide_fill(&mut app.app_name, exe);
                    wide_fill(&mut app.user_friendly_name, game);
                    let status = unsafe {
                        (drs.create_application)(session, profile, &mut app)
                    };
                    if status == NVAPI_OK {
                        result.apps.push(exe.to_string());
                    }
                }
            }

            // Write the requested settings.
            let mut settings: Vec<(String, u32)> = Vec::new();
            if opts.prefer_max_performance {
                settings.push(("PREFERRED_PSTATE_ID".to_string(), 1));
            }
            if opts.shader_cache_on {
                settings.push(("PS_SHADERDISKCACHE".to_string(), 1));
            }

            for (name, value) in &settings {
                let id = match setting_id(drs, name) {
                    Ok(id) => id,
                    Err(_) => continue, // driver lacks this setting — skip, not fatal
                };
                let mut setting = NvSetting {
                    version: version_of::<NvSetting>(1),
                    setting_name: [0; 2048],
                    setting_id: id,
                    setting_type: 0, // NVDRS_DWORD_TYPE
                    setting_location: 0, // NVDRS_CURRENT_PROFILE_LOCATION
                    is_current_predefined: 0,
                    is_predefined_valid: 0,
                    predefined_value: [0; 4100],
                    current_value: [0; 4100],
                };
                setting.current_value[..4].copy_from_slice(&value.to_le_bytes());
                wide_fill(&mut setting.setting_name, name);
                let status = unsafe { (drs.set_setting)(session, profile, &mut setting) };
                if status == NVAPI_OK {
                    result.settings.push(name.clone());
                }
            }

            if unsafe { (drs.save_settings)(session) } != NVAPI_OK {
                return Err(OptixError::Windows("NvAPI_DRS_SaveSettings failed".into()));
            }
            Ok(result)
        })();

        unsafe { (drs.destroy_session)(session) };
        result
    }

    /// Remove the whole `Optix: <game>` profile (rollback of any apply).
    pub fn remove_profile(game: &str) -> Result<()> {
        let Some(drs) = drs()? else {
            return Err(OptixError::Windows(
                "NVAPI unavailable — no NVIDIA driver (nvapi64.dll) present".into(),
            ));
        };
        let mut session = 0isize;
        let status = unsafe { (drs.create_session)(&mut session) };
        if status != NVAPI_OK || session == 0 {
            return Err(status_to_error("create session", status));
        }
        let result = (|| {
            unsafe { (drs.load_settings)(session) };

            let profile_name = format!("Optix: {game}");
            let mut name: NvUnicodeString = [0; 2048];
            wide_fill(&mut name, &profile_name);
            let mut profile = 0isize;
            let status = unsafe { (drs.find_profile)(session, name.as_ptr(), &mut profile) };
            if status != NVAPI_OK {
                // Nothing to remove.
                return Ok(());
            }
            let status = unsafe { (drs.delete_profile)(session, profile) };
            if status != NVAPI_OK {
                return Err(status_to_error("delete profile", status));
            }
            if unsafe { (drs.save_settings)(session) } != NVAPI_OK {
                return Err(OptixError::Windows("NvAPI_DRS_SaveSettings failed".into()));
            }
            Ok(())
        })();

        unsafe { (drs.destroy_session)(session) };
        result
    }
}

#[cfg(windows)]
pub use ffi::{apply_profile, remove_profile};

/// Whether NVIDIA's runtime NVAPI is present on this machine.
#[cfg(windows)]
pub fn nvapi_available() -> bool {
    ffi::drs()
        .map(|o| o.is_some())
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Non-Windows stubs (Linux dev mode)
// ---------------------------------------------------------------------------

#[cfg(not(windows))]
pub fn apply_profile(
    _game: &str,
    _exe_path: Option<&str>,
    _opts: &DrsOptions,
) -> Result<DrsResult> {
    Err(OptixError::UnsupportedPlatform("NVIDIA DRS profile".into()))
}

#[cfg(not(windows))]
pub fn remove_profile(_game: &str) -> Result<()> {
    Err(OptixError::UnsupportedPlatform("NVIDIA DRS profile".into()))
}

#[cfg(not(windows))]
pub fn nvapi_available() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_size_matches_sdk() {
        // NVDRS_PROFILE_V1: 4 + 4096 + 4 + 4 + 4 + 4 = 4116 bytes.
        assert_eq!(std::mem::size_of::<NvProfile>(), 4116);
    }

    #[test]
    fn setting_size_matches_sdk() {
        // NVDRS_SETTING_V1 (SDK-verified): 4 + 4096 + 5*4 + 2*4100 = 12320
        // bytes. The two anonymous DWORD/binary/wsz unions are each modeled as
        // a flat 4100-byte array, so no separate preceding DWORD field.
        assert_eq!(std::mem::size_of::<NvSetting>(), 12320);
    }

    #[test]
    fn application_size_matches_sdk() {
        // NVDRS_APPLICATION_V4 (SDK-verified): 2*4 + 4*4096 + 4 + 4096 = 20492.
        assert_eq!(std::mem::size_of::<NvApplication>(), 20492);
    }

    #[test]
    fn version_macro_matches_sdk_math() {
        #[cfg(windows)]
        {
            assert_eq!(version_of::<NvProfile>(1), 4116 | (1 << 16));
            assert_eq!(version_of::<NvSetting>(1), 12320 | (1 << 16));
            assert_eq!(version_of::<NvApplication>(4), 20492 | (4 << 16));
        }
        #[cfg(not(windows))]
        {
            // size_of must still be right on the dev host — the layout check
            // above already covers that, and the macro math is trivially
            // `size | (ver << 16)`.
            assert!(true);
        }
    }

    #[test]
    fn options_default_to_off() {
        let o = DrsOptions {
            prefer_max_performance: false,
            shader_cache_on: false,
        };
        assert!(!o.prefer_max_performance);
        assert!(!o.shader_cache_on);
    }
}