use serde::Serialize;

/// CPU information detected by the system scanner.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuInfo {
    pub name: String,
    pub brand: String,
    pub vendor: String,
    pub physical_cores: usize,
    pub logical_cores: usize,
    pub frequency_mhz: u64,
    pub usage_percent: f32,
}

/// A single GPU / display adapter.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuInfo {
    pub name: String,
    pub vendor: String,
    pub driver_version: String,
    pub memory_bytes: u64,
    pub usage_percent: f32,
}

/// Current memory (RAM) state.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryInfo {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub usage_percent: f32,
}

/// A single mounted (logical) storage volume.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskInfo {
    pub name: String,
    pub mount_point: String,
    pub file_system: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
    /// "SSD", "HDD", or "Unknown" (sysinfo best effort).
    pub kind: String,
    pub is_removable: bool,
}

/// A physical storage device reported by WMI `MSFT_PhysicalDisk`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicalDiskInfo {
    pub friendly_name: String,
    /// "SSD" | "HDD" | "SCM" | "Unspecified".
    pub media_type: String,
    /// "Healthy" | "Warning" | "Unhealthy" | "Unknown".
    pub health_status: String,
    pub bus_type: String,
    pub firmware_version: Option<String>,
    pub size_bytes: u64,
}

/// Per-interface network counters.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkInterface {
    pub name: String,
    /// Bytes received since the last refresh window.
    pub received_bytes: u64,
    /// Bytes transmitted since the last refresh window.
    pub transmitted_bytes: u64,
    pub total_received_bytes: u64,
    pub total_transmitted_bytes: u64,
}

/// A connected monitor / display device.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayInfo {
    pub width: u32,
    pub height: u32,
    /// Refresh rate in Hz.
    pub refresh_rate: u32,
}

/// A temperature sensor reading.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemperatureInfo {
    pub label: String,
    pub celsius: Option<f32>,
}

/// Operating system information.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OsInfo {
    pub name: String,
    pub version: String,
    pub kernel_version: String,
    pub host_name: String,
    pub uptime_seconds: u64,
    /// Windows build number (e.g. 22631); `None` off Windows.
    pub build_number: Option<u32>,
    /// Windows 11 uses build numbers >= 22000.
    pub is_windows_11: bool,
    /// Edition caption (e.g. "Microsoft Windows 11 Pro"); `None` off Windows.
    pub edition: Option<String>,
}

/// Motherboard information from WMI `Win32_BaseBoard`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MotherboardInfo {
    pub manufacturer: String,
    pub product: String,
}

/// BIOS information from WMI `Win32_BIOS`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BiosInfo {
    pub vendor: String,
    pub version: String,
}

/// A running process.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub exe: String,
    pub cpu_usage_percent: f32,
    pub memory_bytes: u64,
    pub disk_read_bytes: u64,
    pub disk_written_bytes: u64,
    pub start_time: u64,
}

/// A startup application entry (registry Run keys / Startup folder).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupApp {
    pub name: String,
    pub command: String,
    pub location: String,
}

/// A full one-shot system scan result (Phase 1: System Scanner).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareInfo {
    pub cpu: CpuInfo,
    pub gpus: Vec<GpuInfo>,
    pub memory: MemoryInfo,
    pub disks: Vec<DiskInfo>,
    pub physical_disks: Vec<PhysicalDiskInfo>,
    pub network: Vec<NetworkInterface>,
    pub displays: Vec<DisplayInfo>,
    pub temperatures: Vec<TemperatureInfo>,
    pub os: OsInfo,
    pub motherboard: Option<MotherboardInfo>,
    pub bios: Option<BiosInfo>,
    pub processes: Vec<ProcessInfo>,
    pub startup_apps: Vec<StartupApp>,
    /// Unix timestamp (milliseconds) when the scan completed.
    pub scanned_at_ms: u64,
}

/// Live telemetry polled by the dashboard.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemStats {
    pub cpu_usage_percent: f32,
    pub per_core_usage: Vec<f32>,
    pub memory: MemoryInfo,
    pub network: Vec<NetworkInterface>,
    pub timestamp_ms: u64,
}

/// One row of `hardware_history` — a telemetry sample for dashboard trend
/// lines. Nullable fields are filled in by the phases that own that sensor
/// (GPU/temp in later phases).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareSample {
    pub id: Option<i64>,
    pub ts_ms: i64,
    pub cpu_usage: Option<f32>,
    pub cpu_temp: Option<f32>,
    pub ram_used_mb: Option<i64>,
    pub ram_total_mb: Option<i64>,
    pub gpu_usage: Option<f32>,
    pub gpu_temp: Option<f32>,
    pub gpu_vram_mb: Option<i64>,
    pub gpu_power_w: Option<f32>,
    pub disk_used_mb: Option<i64>,
    pub disk_total_mb: Option<i64>,
    pub net_down_bps: Option<i64>,
    pub net_up_bps: Option<i64>,
    pub fps: Option<f32>,
    pub frame_time_ms: Option<f32>,
}
