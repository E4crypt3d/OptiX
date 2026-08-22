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

/// One ACPI thermal zone reading from `MSAcpi_ThermalZoneTemperature`.
#[cfg(windows)]
pub struct ThermalZoneReading {
    pub label: String,
    /// Degrees Celsius; `None` when the zone reports no valid reading.
    pub celsius: Option<f32>,
}

/// Convert `CurrentTemperature` (tenths of a degree Kelvin) to Celsius,
/// returning `None` for the sentinel values some zones report when no sensor
/// is present (0 K, `-1`, or out-of-range garbage).
#[cfg_attr(not(windows), allow(dead_code))]
fn thermal_zone_celsius(deci_kelvin: u32) -> Option<f32> {
    let celsius = deci_kelvin as f32 / 10.0 - 273.15;
    (celsius > -100.0 && celsius < 150.0).then_some(celsius)
}

/// Human-readable label for a thermal zone `InstanceName` such as
/// `ACPI\ThermalZone\TZ00_0`.
#[cfg_attr(not(windows), allow(dead_code))]
fn thermal_zone_label(instance_name: &str) -> String {
    instance_name
        .rsplit('\\')
        .next()
        .filter(|tail| !tail.is_empty())
        .map(|tail| format!("Thermal zone {tail}"))
        .unwrap_or_else(|| "Thermal zone".to_string())
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

    /// ACPI thermal zones from the `root\WMI` namespace. sysinfo's
    /// `Components` returns nothing on Windows (no driver-backed sensor
    /// access), so this is the built-in source for the Temperatures card.
    pub fn thermal_zone_temperatures() -> Vec<ThermalZoneReading> {
        let Ok(conn) = WMIConnection::with_namespace_path("ROOT\\WMI") else {
            return Vec::new();
        };
        let Ok(rows) = conn.raw_query(
            "SELECT InstanceName, CurrentTemperature FROM MSAcpi_ThermalZoneTemperature",
        ) else {
            return Vec::new();
        };
        rows.into_iter()
            .filter_map(|m| {
                let label = get_string(&m, "InstanceName")
                    .map(|name| thermal_zone_label(&name))
                    .unwrap_or_else(|| "Thermal zone".to_string());
                let celsius = get_u32(&m, "CurrentTemperature").and_then(thermal_zone_celsius);
                celsius.map(|celsius| ThermalZoneReading {
                    label,
                    celsius: Some(celsius),
                })
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
pub use imp::{physical_disks, system_hardware, thermal_zone_temperatures, video_controllers};

#[cfg(test)]
mod tests {
    use super::{thermal_zone_celsius, thermal_zone_label};

    #[test]
    fn thermal_zone_celsius_conversion() {
        // 3012 deciKelvin = 301.2 K = 28.05 °C (f32 rounding-tolerant).
        let celsius = thermal_zone_celsius(3012).expect("valid reading");
        assert!((celsius - 28.05).abs() < 0.001);
        // Sentinel / unavailable values are dropped.
        assert_eq!(thermal_zone_celsius(0), None);
        assert_eq!(thermal_zone_celsius(0xFFFF_FFFF), None);
    }

    #[test]
    fn thermal_zone_labels() {
        assert_eq!(
            thermal_zone_label(r"ACPI\ThermalZone\TZ00_0"),
            "Thermal zone TZ00_0"
        );
        assert_eq!(thermal_zone_label(""), "Thermal zone");
    }
}
