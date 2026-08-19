//! Merges Windows-only telemetry (WMI + version) into a scanner result.

use crate::models::hardware::HardwareInfo;

#[cfg(windows)]
pub fn enrich(mut info: HardwareInfo) -> HardwareInfo {
    // GPU VRAM from WMI. AdapterRAM is a 32-bit field, so cards above 4 GiB are
    // under-reported; DXGI (added in a later phase) is the accurate source.
    let controllers = super::wmi::video_controllers();
    for (i, gpu) in info.gpus.iter_mut().enumerate() {
        if let Some(c) = controllers.get(i) {
            if c.adapter_ram_bytes > 0 {
                gpu.memory_bytes = c.adapter_ram_bytes;
            }
        }
    }

    info.physical_disks = super::wmi::physical_disks();
    info.motherboard = super::wmi::motherboard();
    info.bios = super::wmi::bios();

    if let Some(build) = super::os::current_build() {
        info.os.build_number = Some(build);
        info.os.is_windows_11 = super::os::build_is_windows_11(build);
    }
    info.os.edition = super::wmi::os_edition();

    info
}

#[cfg(not(windows))]
pub fn enrich(info: HardwareInfo) -> HardwareInfo {
    info
}
