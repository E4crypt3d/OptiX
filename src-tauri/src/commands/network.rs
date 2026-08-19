use tauri::State;

use crate::db::sqlite::Database;
use crate::engine::network;
use crate::error::{OptixError, Result};
use crate::models::network::{
    DnsApplyResult, DnsBenchmarkResult, DnsServer, NetworkStatus, TcpParameter,
};
use crate::win;

/// Curated resolvers plus the currently-configured DNS (marked `is_current`).
fn servers_with_current() -> Vec<DnsServer> {
    let current = win::network::current_dns_servers();
    let mut curated = network::default_servers();
    for s in &mut curated {
        if current.contains(&s.ip) {
            s.is_current = true;
        }
    }
    let extra: Vec<DnsServer> = current
        .iter()
        .filter(|ip| !curated.iter().any(|s| s.ip == **ip))
        .map(|ip| DnsServer {
            name: format!("Current ({ip})"),
            ip: ip.clone(),
            is_current: true,
        })
        .collect();
    let mut out = extra;
    out.extend(curated);
    out
}

fn default_domains() -> Vec<String> {
    ["google.com", "cloudflare.com", "microsoft.com", "github.com"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

/// System network state (adapters, gateway, current DNS).
#[tauri::command]
pub fn network_status() -> NetworkStatus {
    network::status()
}

/// Resolvers offered for benchmarking/application.
#[tauri::command]
pub fn list_dns_servers() -> Vec<DnsServer> {
    servers_with_current()
}

/// Benchmark resolvers (UDP A-record queries) on a blocking thread.
#[tauri::command]
pub async fn benchmark_dns(
    domains: Vec<String>,
    queries_per_domain: usize,
) -> Result<Vec<DnsBenchmarkResult>> {
    let domains = if domains.is_empty() {
        default_domains()
    } else {
        domains
    };
    let qpd = queries_per_domain.clamp(1, 20);
    let servers = servers_with_current();
    tauri::async_runtime::spawn_blocking(move || network::benchmark(&servers, &domains, qpd))
        .await
        .map_err(|e| OptixError::Other(e.to_string()))
}

/// Apply static DNS servers to an adapter (snapshot-first, reversible).
#[tauri::command]
pub fn apply_dns(
    db: State<'_, Database>,
    guid: String,
    servers: Vec<String>,
) -> Result<DnsApplyResult> {
    network::apply_dns(db.inner(), &guid, &servers)
}

/// Current TCP/IP tuning parameters (read-only; experimental).
#[tauri::command]
pub fn tcp_parameters() -> Vec<TcpParameter> {
    win::network::tcp_parameters()
}
