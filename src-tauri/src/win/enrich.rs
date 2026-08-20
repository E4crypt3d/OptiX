//! Merges Windows-only telemetry (OS build/edition) into a scanner result.
//!
//! Edition is fetched by `win::hardware::system_hardware` during the scan
//! (one WMI connection for motherboard + BIOS + edition); this pass only adds
//! the Windows build number.

use crate::models::hardware::HardwareInfo;

#[cfg(windows)]
pub fn enrich(mut info: HardwareInfo) -> HardwareInfo {
    if let Some(build) = super::os::current_build() {
        info.os.build_number = Some(build);
        info.os.is_windows_11 = super::os::build_is_windows_11(build);
    }
    info
}

#[cfg(not(windows))]
pub fn enrich(info: HardwareInfo) -> HardwareInfo {
    info
}
