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

/// System network state (interfaces, gateway).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStatus {
    pub adapters: Vec<NetworkAdapter>,
    pub gateway: Option<String>,
    pub current_dns: Vec<String>,
}

/// A single TCP/IP tuning parameter read from the registry.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TcpParameter {
    pub name: String,
    /// Current DWORD value, or `None` when the value is absent (driver default).
    pub value: Option<u32>,
}

/// Result of applying DNS servers to an adapter.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsApplyResult {
    pub snapshot_id: String,
    pub changes: usize,
}
