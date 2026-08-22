//! Network optimization models (Phase 7): DNS benchmark, adapter state, and
//! the DNS apply result.

use serde::Serialize;

/// A DNS resolver Optix can benchmark and apply.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsServer {
    pub name: String,
    pub ip: String,
    /// True when this resolver is the one currently configured on the system.
    pub is_current: bool,
}

/// Latency/loss measurements for one resolver.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsBenchmarkResult {
    pub name: String,
    pub ip: String,
    pub is_current: bool,
    /// Median latency in milliseconds (across successful queries).
    pub median_ms: Option<f64>,
    /// 95th-percentile latency in milliseconds.
    pub p95_ms: Option<f64>,
    /// Fastest observed latency in milliseconds.
    pub min_ms: Option<f64>,
    /// Percentage of queries that timed out.
    pub loss_percent: f64,
    pub queries: usize,
    pub failures: usize,
}

/// A network interface with its DNS configuration.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkAdapter {
    pub name: String,
    /// Interface GUID from the TCP/IP registry key.
    pub guid: String,
    /// True when the adapter has a default gateway (i.e. it routes internet
    /// traffic).
    pub is_active: bool,
    pub dns_servers: Vec<String>,
    pub dhcp_enabled: bool,
}

/// Hardware counters for one interface (lifetime since boot).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceCounters {
    pub received_bytes: u64,
    pub sent_bytes: u64,
    pub receive_errors: u64,
    pub send_errors: u64,
    pub receive_discards: u64,
    pub send_discards: u64,
}

/// Live wireless association details (Windows Native WiFi API / Linux
/// `/proc/net/wireless`). `None` on wired and virtual interfaces.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WifiInfo {
    pub ssid: Option<String>,
    pub bssid: Option<String>,
    pub channel: Option<u32>,
    /// 0–100 link quality as reported by Windows (`wlanSignalQuality`).
    pub signal_percent: Option<u32>,
    /// Approximate dBm derived from the quality (linear interpolation).
    pub rssi_dbm: Option<i32>,
    /// Radio generation, e.g. `Wi-Fi 6 (802.11ax)`.
    pub phy_type: String,
    pub rx_rate_mbps: Option<f64>,
    pub tx_rate_mbps: Option<f64>,
    pub authentication: String,
    pub cipher: String,
}

/// Driver metadata joined from the network class registry key /
/// `MSFT_NetAdapter` WMI class.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterDriver {
    pub version: Option<String>,
    /// Formatted `YYYY-MM-DD`.
    pub date: Option<String>,
    pub provider: Option<String>,
    pub ndis_version: Option<String>,
    /// `true` = full duplex, `false` = half, `None` = unknown/virtual.
    pub full_duplex: Option<bool>,
}

/// One physical or virtual interface in the full system inventory:
/// Ethernet, Wi-Fi, VPN/tunnel adapters, Hyper-V/WSL virtual NICs —
/// everything except loopback.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterInventory {
    /// Interface GUID; matches the DNS-apply `NetworkAdapter.guid`.
    pub guid: String,
    /// Connection name ("Ethernet 2", "Wi-Fi", "vEthernet (WSL)").
    pub name: String,
    /// Hardware/driver description ("Realtek Gaming GbE", "Tailscale Tunnel").
    pub description: String,
    /// Coarse classification: `ethernet`, `wifi`, `vpn`, `virtual`,
    /// `bluetooth`, `other`.
    pub kind: String,
    /// Interface is operationally up AND media-connected.
    pub is_up: bool,
    pub is_virtual: bool,
    pub mac_address: Option<String>,
    pub mtu: Option<u32>,
    pub transmit_link_bps: Option<u64>,
    pub receive_link_bps: Option<u64>,
    /// Unicast addresses with prefix length, e.g. `192.168.1.10/24`.
    pub ip_addresses: Vec<String>,
    pub gateways: Vec<String>,
    pub dns_servers: Vec<String>,
    pub dhcp_enabled: Option<bool>,
    pub counters: Option<InterfaceCounters>,
    pub wifi: Option<WifiInfo>,
    pub driver: Option<AdapterDriver>,
}

/// System network state (interfaces, gateway).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStatus {
    pub adapters: Vec<NetworkAdapter>,
    pub gateway: Option<String>,
    pub current_dns: Vec<String>,
    /// Full cross-type inventory: Ethernet, Wi-Fi, VPN/tunnel, virtual.
    pub inventory: Vec<AdapterInventory>,
}

/// Result of applying DNS servers to an adapter.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsApplyResult {
    pub snapshot_id: String,
    pub changes: usize,
}

/// A TCP/IP tweak Optix can apply/undo (registry DWORD under
/// `...\Services\Tcpip\Parameters`). All experimental — on modern Windows the
/// measured impact is usually marginal.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TcpTweak {
    /// Registry value name (e.g. `TcpAckFrequency`).
    pub name: String,
    pub description: String,
    /// The recommended Optix value.
    pub recommended: u32,
    /// Current value, or `None` when absent (driver default applies).
    pub current: Option<u32>,
    /// Whether the current value equals the recommended one.
    pub applied: bool,
}

/// Result of applying or resetting TCP tweaks.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TcpTweakResult {
    pub snapshot_id: String,
    pub changes: usize,
}

/// Summary of an ICMP ping test (median/jitter computed from all replies).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PingResult {
    pub host: String,
    pub sent: u32,
    pub received: usize,
    pub loss_percent: f64,
    pub min_ms: Option<f64>,
    pub median_ms: Option<f64>,
    pub max_ms: Option<f64>,
    /// Median absolute deviation between consecutive RTTs — the "jitter".
    pub jitter_ms: Option<f64>,
    /// Every RTT in milliseconds, so the UI can render the series.
    pub samples_ms: Vec<f64>,
}
