//! Windows Management Instrumentation queries for hardware detection.

use crate::models::hardware::{BiosInfo, MotherboardInfo};
#[cfg(windows)]
use crate::models::hardware::PhysicalDiskInfo;

/// WMI row for a video controller (GPU).
#[cfg(windows)]
pub struct VideoControllerInfo {
    pub name: String,
    pub adapter_ram_bytes: u64,
}

/// Motherboard / BIOS / OS edition, fetched together because they share one
/// WMI connection (each `WMIConnection::new()` costs tens of milliseconds).
#[derive(Debug, Default)]
pub struct SystemHardware {
    pub motherboard: Option<MotherboardInfo>,
    pub bios: Option<BiosInfo>,
    pub edition: Option<String>,
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::collections::HashMap;
    use wmi::{Variant, WMIConnection};

    fn get_string(map: &HashMap<String, Variant>, key: &str) -> Option<String> {
        match map.get(key) {
            Some(Variant::String(s)) => Some(s.clone()),
            _ => None,
        }
    }

    fn get_u16(map: &HashMap<String, Variant>, key: &str) -> Option<u16> {
        match map.get(key) {
            Some(Variant::UI2(v)) => Some(*v),
            _ => None,
        }
    }

    fn get_u32(map: &HashMap<String, Variant>, key: &str) -> Option<u32> {
        match map.get(key) {
            Some(Variant::UI4(v)) => Some(*v),
            _ => None,
        }
    }

    fn get_u64(map: &HashMap<String, Variant>, key: &str) -> Option<u64> {
        match map.get(key) {
            Some(Variant::UI8(v)) => Some(*v),
            _ => None,
        }
    }

    fn media_type_name(v: u16) -> String {
        match v {
            3 => "HDD",
            4 => "SSD",
            5 => "SCM",
            _ => "Unspecified",
        }
        .to_string()
    }

    fn health_name(v: u16) -> String {
        match v {
            0 => "Healthy",
            1 => "Warning",
            2 => "Unhealthy",
            _ => "Unknown",
        }
        .to_string()
    }

    fn bus_type_name(v: u16) -> String {
        match v {
            0 => "Unknown",
            1 => "SCSI",
            2 => "ATAPI",
            3 => "ATA",
            4 => "IEEE 1394",
            5 => "SSA",
            6 => "Fibre Channel",
            7 => "USB",
            8 => "RAID",
            9 => "iSCSI",
            10 => "SAS",
            11 => "SATA",
            12 => "SD",
            13 => "MMC",
            14 => "Reserved",
            15 => "File Backed Virtual",
            16 => "Storage Spaces",
            17 => "NVMe",
            _ => "Unknown",
        }
        .to_string()
    }

    /// Enumerate video controllers. `AdapterRAM` is a 32-bit field, so VRAM
    /// above 4 GiB is under-reported (DXGI is the accurate source, later).
    pub fn video_controllers() -> Vec<VideoControllerInfo> {
        let Ok(conn) = WMIConnection::new() else {
            return Vec::new();
        };
        let Ok(rows) = conn.raw_query("SELECT Name, AdapterRAM FROM Win32_VideoController") else {
            return Vec::new();
        };
        rows.into_iter()
            .map(|m| VideoControllerInfo {
                name: get_string(&m, "Name").unwrap_or_default(),
                adapter_ram_bytes: get_u32(&m, "AdapterRAM").map(|v| v as u64).unwrap_or(0),
            })
            .collect()
    }

    /// Enumerate physical disks from the Storage namespace.
    pub fn physical_disks() -> Vec<PhysicalDiskInfo> {
        let Ok(conn) = WMIConnection::with_namespace_path("ROOT\\Microsoft\\Windows\\Storage")
        else {
            return Vec::new();
        };
        let Ok(rows) = conn.raw_query(
            "SELECT FriendlyName, MediaType, HealthStatus, BusType, FirmwareVersion, Size \
             FROM MSFT_PhysicalDisk",
        ) else {
            return Vec::new();
        };
        rows.into_iter()
            .map(|m| PhysicalDiskInfo {
                friendly_name: get_string(&m, "FriendlyName").unwrap_or_default(),
                media_type: get_u16(&m, "MediaType")
                    .map(media_type_name)
                    .unwrap_or_else(|| "Unspecified".to_string()),
                health_status: get_u16(&m, "HealthStatus")
                    .map(health_name)
                    .unwrap_or_else(|| "Unknown".to_string()),
                bus_type: get_u16(&m, "BusType")
                    .map(bus_type_name)
                    .unwrap_or_else(|| "Unknown".to_string()),
                firmware_version: get_string(&m, "FirmwareVersion"),
                size_bytes: get_u64(&m, "Size").unwrap_or(0),
            })
            .collect()
    }

    /// Motherboard, BIOS, and OS edition from a single WMI connection.
    pub fn system_hardware() -> SystemHardware {
        let Ok(conn) = WMIConnection::new() else {
            return SystemHardware::default();
        };
        let motherboard = conn
            .raw_query("SELECT Manufacturer, Product FROM Win32_BaseBoard")
            .ok()
            .and_then(|rows| rows.into_iter().next())
            .map(|m| MotherboardInfo {
                manufacturer: get_string(&m, "Manufacturer").unwrap_or_default(),
                product: get_string(&m, "Product").unwrap_or_default(),
            });
        let bios = conn
            .raw_query("SELECT Manufacturer, SMBIOSBIOSVersion FROM Win32_BIOS")
            .ok()
            .and_then(|rows| rows.into_iter().next())
            .map(|m| BiosInfo {
                vendor: get_string(&m, "Manufacturer").unwrap_or_default(),
                version: get_string(&m, "SMBIOSBIOSVersion").unwrap_or_default(),
            });
        let edition = conn
            .raw_query("SELECT Caption FROM Win32_OperatingSystem")
            .ok()
            .and_then(|rows| rows.into_iter().next())
            .and_then(|m| get_string(&m, "Caption"));
        SystemHardware {
            motherboard,
            bios,
            edition,
        }
    }
}

#[cfg(windows)]
pub use imp::{physical_disks, system_hardware, video_controllers};
