use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use sysinfo::{Disks, Networks, System};

use crate::db::sqlite::Database;
use crate::error::{OptixError, Result};
use crate::models::app::AppInfo;
use crate::models::hardware::*;
use crate::win;

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Shared state used to compute live telemetry deltas (CPU / network) between
/// successive polls from the frontend.
pub struct MonitorState {
    sys: Mutex<System>,
    networks: Mutex<Networks>,
    /// Instant of the last `Networks::refresh`, shared by every command that
    /// refreshes the network counters, so per-window byte deltas can be
    /// converted to accurate rates regardless of which command refreshed last.
    last_refresh: Mutex<Option<Instant>>,
}

impl MonitorState {
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_all();
        sys.refresh_memory();
        let networks = Networks::new_with_refreshed_list();
        Self {
            sys: Mutex::new(sys),
            networks: Mutex::new(networks),
            last_refresh: Mutex::new(None),
        }
    }
}

impl Default for MonitorState {
    fn default() -> Self {
        Self::new()
    }
}

/// Full one-shot system scan. Runs on a blocking thread because WMI queries
/// can take hundreds of milliseconds.
#[tauri::command]
pub async fn scan_system() -> Result<HardwareInfo> {
    tauri::async_runtime::spawn_blocking(scan_system_blocking)
        .await
        .map_err(|e| OptixError::Other(e.to_string()))?
}

pub(crate) fn scan_system_blocking() -> Result<HardwareInfo> {
    let mut sys = System::new();
    sys.refresh_all();

    let first_cpu = sys.cpus().first();
    let cpu = CpuInfo {
        name: first_cpu.map(|c| c.name().to_string()).unwrap_or_default(),
        brand: first_cpu.map(|c| c.brand().to_string()).unwrap_or_default(),
        vendor: first_cpu
            .map(|c| c.vendor_id().to_string())
            .unwrap_or_default(),
        physical_cores: System::physical_core_count().unwrap_or(0),
        logical_cores: sys.cpus().len(),
        frequency_mhz: first_cpu.map(|c| c.frequency()).unwrap_or(0),
        usage_percent: sys.global_cpu_usage(),
    };

    let total = sys.total_memory();
    let used = sys.used_memory();
    let memory = MemoryInfo {
        total_bytes: total,
        used_bytes: used,
        available_bytes: sys.available_memory(),
        usage_percent: if total > 0 {
            (used as f32 / total as f32) * 100.0
        } else {
            0.0
        },
    };

    let disks = Disks::new_with_refreshed_list();
    let disks_info: Vec<DiskInfo> = disks
        .list()
        .iter()
        .map(|d| DiskInfo {
            name: d.name().to_string_lossy().into_owned(),
            mount_point: d.mount_point().to_string_lossy().into_owned(),
            file_system: d.file_system().to_string_lossy().into_owned(),
            total_bytes: d.total_space(),
            available_bytes: d.available_space(),
            used_bytes: d.total_space().saturating_sub(d.available_space()),
            kind: match d.kind() {
                sysinfo::DiskKind::SSD => "SSD".to_string(),
                sysinfo::DiskKind::HDD => "HDD".to_string(),
                sysinfo::DiskKind::Unknown(_) => "Unknown".to_string(),
            },
            is_removable: d.is_removable(),
        })
        .collect();

    let networks = Networks::new_with_refreshed_list();
    let network_info: Vec<NetworkInterface> = networks
        .iter()
        .map(|(name, data)| NetworkInterface {
            name: name.clone(),
            received_bytes: data.received(),
            transmitted_bytes: data.transmitted(),
            total_received_bytes: data.total_received(),
            total_transmitted_bytes: data.total_transmitted(),
            received_bytes_per_sec: 0.0,
            transmitted_bytes_per_sec: 0.0,
        })
        .collect();

    let processes: Vec<ProcessInfo> = sys
        .processes()
        .iter()
        .map(|(pid, p)| ProcessInfo {
            pid: pid.as_u32(),
            name: p.name().to_string_lossy().into_owned(),
            exe: p
                .exe()
                .map(|e| e.to_string_lossy().into_owned())
                .unwrap_or_default(),
            cpu_usage_percent: p.cpu_usage(),
            memory_bytes: p.memory(),
            disk_read_bytes: p.disk_usage().total_read_bytes,
            disk_written_bytes: p.disk_usage().total_written_bytes,
            start_time: p.start_time(),
        })
        .collect();

    let components = sysinfo::Components::new_with_refreshed_list();
    let temperatures: Vec<TemperatureInfo> = components
        .iter()
        .map(|c| TemperatureInfo {
            label: c.label().to_string(),
            celsius: c.temperature(),
        })
        .collect();

    let os = OsInfo {
        name: System::name().unwrap_or_default(),
        version: System::long_os_version()
            .or_else(System::os_version)
            .unwrap_or_default(),
        kernel_version: System::kernel_version().unwrap_or_default(),
        host_name: System::host_name().unwrap_or_default(),
        uptime_seconds: System::uptime(),
        build_number: None,
        is_windows_11: false,
        edition: None,
    };

    let gpus = win::hardware::detect_gpus();
    let displays: Vec<DisplayInfo> = win::hardware::primary_display().into_iter().collect();
    let startup_apps = win::hardware::detect_startup_apps();

    let info = HardwareInfo {
        cpu,
        gpus,
        memory,
        disks: disks_info,
        physical_disks: Vec::new(),
        network: network_info,
        displays,
        temperatures,
        os,
        motherboard: None,
        bios: None,
        processes,
        startup_apps,
        scanned_at_ms: now_ms(),
    };

    Ok(win::enrich::enrich(info))
}

/// Live telemetry for the dashboard (CPU, per-core, memory, network rates).
#[tauri::command]
pub fn system_stats(state: tauri::State<'_, MonitorState>) -> Result<SystemStats> {
    let mut sys = state.sys.lock().map_err(|e| OptixError::InvalidState(e.to_string()))?;
    sys.refresh_cpu_all();
    sys.refresh_memory();

    let total = sys.total_memory();
    let used = sys.used_memory();
    let memory = MemoryInfo {
        total_bytes: total,
        used_bytes: used,
        available_bytes: sys.available_memory(),
        usage_percent: if total > 0 {
            (used as f32 / total as f32) * 100.0
        } else {
            0.0
        },
    };

    let per_core_usage: Vec<f32> = sys.cpus().iter().map(|c| c.cpu_usage()).collect();

    let mut networks = state
        .networks
        .lock()
        .map_err(|e| OptixError::InvalidState(e.to_string()))?;
    let now = Instant::now();
    let elapsed = {
        let mut last = state
            .last_refresh
            .lock()
            .map_err(|e| OptixError::InvalidState(e.to_string()))?;
        let elapsed = last
            .map(|t| now.duration_since(t).as_secs_f64())
            .unwrap_or(1.0);
        *last = Some(now);
        elapsed
    };
    networks.refresh(true);
    let network: Vec<NetworkInterface> = networks
        .iter()
        .map(|(name, data)| NetworkInterface {
            name: name.clone(),
            received_bytes: data.received(),
            transmitted_bytes: data.transmitted(),
            total_received_bytes: data.total_received(),
            total_transmitted_bytes: data.total_transmitted(),
            received_bytes_per_sec: data.received() as f64 / elapsed,
            transmitted_bytes_per_sec: data.transmitted() as f64 / elapsed,
        })
        .collect();

    Ok(SystemStats {
        cpu_usage_percent: sys.global_cpu_usage(),
        per_core_usage,
        memory,
        network,
        timestamp_ms: now_ms(),
    })
}

/// Persist a telemetry sample to `hardware_history`.
#[tauri::command]
pub fn record_sample(
    state: tauri::State<'_, MonitorState>,
    db: tauri::State<'_, Database>,
) -> Result<()> {
    let mut sys = state.sys.lock().map_err(|e| OptixError::InvalidState(e.to_string()))?;
    sys.refresh_cpu_all();
    sys.refresh_memory();

    let cpu_usage = sys.global_cpu_usage();
    let ram_used_mb = (sys.used_memory() / (1024 * 1024)) as i64;
    let ram_total_mb = (sys.total_memory() / (1024 * 1024)) as i64;

    let disks = Disks::new_with_refreshed_list();
    let mut disk_used = 0u64;
    let mut disk_total = 0u64;
    for d in disks.list() {
        disk_used += d.total_space().saturating_sub(d.available_space());
        disk_total += d.total_space();
    }

    let mut networks = state
        .networks
        .lock()
        .map_err(|e| OptixError::InvalidState(e.to_string()))?;
    let now = Instant::now();
    let elapsed = {
        let mut last = state
            .last_refresh
            .lock()
            .map_err(|e| OptixError::InvalidState(e.to_string()))?;
        let elapsed = last
            .map(|t| now.duration_since(t).as_secs_f64())
            .unwrap_or(1.0);
        *last = Some(now);
        elapsed
    };
    networks.refresh(true);
    let down_bytes = networks.iter().map(|(_, d)| d.received()).sum::<u64>();
    let up_bytes = networks.iter().map(|(_, d)| d.transmitted()).sum::<u64>();

    // Bytes since the last refresh, divided by the actual elapsed time, in
    // bits/sec for the `hardware_history` schema.
    let net_down_bps = (down_bytes as f64 / elapsed * 8.0) as i64;
    let net_up_bps = (up_bytes as f64 / elapsed * 8.0) as i64;

    let sample = HardwareSample {
        id: None,
        ts_ms: now_ms() as i64,
        cpu_usage: Some(cpu_usage),
        cpu_temp: None,
        ram_used_mb: Some(ram_used_mb),
        ram_total_mb: Some(ram_total_mb),
        gpu_usage: None,
        gpu_temp: None,
        gpu_vram_mb: None,
        gpu_power_w: None,
        disk_used_mb: Some((disk_used / (1024 * 1024)) as i64),
        disk_total_mb: Some((disk_total / (1024 * 1024)) as i64),
        net_down_bps: Some(net_down_bps),
        net_up_bps: Some(net_up_bps),
        fps: None,
        frame_time_ms: None,
    };

    db.insert_hardware_sample(&sample)?;
    Ok(())
}

/// Return the most recent telemetry samples, newest first.
#[tauri::command]
pub fn recent_samples(db: tauri::State<'_, Database>) -> Result<Vec<HardwareSample>> {
    db.recent_hardware_samples(200)
}

/// App version and on-disk data locations (Settings page).
#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        data_dir: crate::db::sqlite::data_dir().to_string_lossy().into_owned(),
        snapshots_dir: crate::db::sqlite::snapshots_dir().to_string_lossy().into_owned(),
        snapshot_retention: crate::engine::snapshot::SNAPSHOT_RETENTION,
        log_path: crate::logging::log_path()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default(),
    }
}
