//! Phase 7 — Network Optimization.
//!
//! A minimal cross-platform UDP DNS client (no external resolver dependency)
//! powers the resolver benchmark. DNS application writes the adapter's
//! `NameServer` registry value snapshot-first and reuses the registry rollback
//! path.

use std::cmp::Ordering;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicU16, Ordering as AtomicOrdering};
use std::time::{Duration, Instant};

use crate::db::sqlite::Database;
use crate::engine::{rollback, snapshot};
use crate::error::{OptixError, Result};
use crate::models::network::{DnsApplyResult, DnsBenchmarkResult, DnsServer, NetworkStatus};
use crate::models::snapshot::ChangeRecord;
use crate::win;

static QUERY_ID: AtomicU16 = AtomicU16::new(0x5144);

fn next_id() -> u16 {
    QUERY_ID.fetch_add(1, AtomicOrdering::Relaxed)
}

/// Build a DNS A-record query for `domain`.
fn build_query(domain: &str, id: u16) -> Vec<u8> {
    let mut pkt = Vec::with_capacity(64);
    pkt.extend_from_slice(&id.to_be_bytes());
    pkt.extend_from_slice(&0x0100u16.to_be_bytes()); // flags: RD=1
    pkt.extend_from_slice(&1u16.to_be_bytes()); // QDCOUNT
    pkt.extend_from_slice(&0u16.to_be_bytes()); // ANCOUNT
    pkt.extend_from_slice(&0u16.to_be_bytes()); // NSCOUNT
    pkt.extend_from_slice(&0u16.to_be_bytes()); // ARCOUNT
    for label in domain.split('.') {
        let bytes = label.as_bytes();
        pkt.push(bytes.len() as u8);
        pkt.extend_from_slice(bytes);
    }
    pkt.push(0); // root label
    pkt.extend_from_slice(&1u16.to_be_bytes()); // QTYPE = A
    pkt.extend_from_slice(&1u16.to_be_bytes()); // QCLASS = IN
    pkt
}

/// Skip a (possibly compressed) DNS name, returning the offset after it.
#[cfg(test)]
fn skip_name(resp: &[u8], mut pos: usize) -> usize {
    loop {
        if pos >= resp.len() {
            return pos;
        }
        let len = resp[pos] as usize;
        if len == 0 {
            return pos + 1;
        }
        if len & 0xC0 == 0xC0 {
            return pos + 2; // compression pointer
        }
        pos += 1 + len;
    }
}

/// Extract A-record IPv4 addresses from a DNS response.
#[cfg(test)]
fn parse_a_records(resp: &[u8]) -> Vec<[u8; 4]> {
    if resp.len() < 12 {
        return Vec::new();
    }
    let ancount = u16::from_be_bytes([resp[6], resp[7]]) as usize;
    let mut pos = 12;
    pos = skip_name(resp, pos);
    pos += 4; // QTYPE + QCLASS
    let mut addrs = Vec::new();
    for _ in 0..ancount {
        pos = skip_name(resp, pos);
        if pos + 10 > resp.len() {
            break;
        }
        let rtype = u16::from_be_bytes([resp[pos], resp[pos + 1]]);
        let rdlen = u16::from_be_bytes([resp[pos + 8], resp[pos + 9]]) as usize;
        pos += 10;
        if rtype == 1 && rdlen == 4 && pos + 4 <= resp.len() {
            addrs.push([resp[pos], resp[pos + 1], resp[pos + 2], resp[pos + 3]]);
        }
        pos += rdlen;
    }
    addrs
}

/// Send one A-record query and measure the round-trip in milliseconds.
fn query_once(server: &str, domain: &str, timeout_ms: u64) -> std::result::Result<f64, ()> {
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|_| ())?;
    socket
        .set_read_timeout(Some(Duration::from_millis(timeout_ms)))
        .map_err(|_| ())?;
    let id = next_id();
    let query = build_query(domain, id);
    let start = Instant::now();
    socket.send_to(&query, (server, 53)).map_err(|_| ())?;
    let mut buf = [0u8; 512];
    let (_n, _addr) = socket.recv_from(&mut buf).map_err(|_| ())?;
    // Only accept responses to our query.
    if buf.len() >= 2 && u16::from_be_bytes([buf[0], buf[1]]) == id {
        Ok(start.elapsed().as_secs_f64() * 1000.0)
    } else {
        Err(())
    }
}

fn median(sorted: &[f64]) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let mid = sorted.len() / 2;
    Some(if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    })
}

fn percentile(sorted: &[f64], p: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let idx = (((sorted.len() - 1) as f64) * p).round() as usize;
    Some(sorted[idx.min(sorted.len() - 1)])
}

/// System network state (interfaces, gateway, current DNS).
pub fn status() -> NetworkStatus {
    NetworkStatus {
        adapters: win::network::list_adapters(),
        gateway: win::network::gateway(),
        current_dns: win::network::current_dns_servers(),
    }
}

/// The curated public resolvers offered for benchmarking/applying.
pub fn default_servers() -> Vec<DnsServer> {
    [
        ("Cloudflare", "1.1.1.1"),
        ("Google", "8.8.8.8"),
        ("Quad9", "9.9.9.9"),
        ("Control D", "76.76.2.0"),
        ("OpenDNS", "208.67.222.222"),
        ("AdGuard", "94.140.14.14"),
    ]
    .iter()
    .map(|(name, ip)| DnsServer {
        name: (*name).to_string(),
        ip: (*ip).to_string(),
        is_current: false,
    })
    .collect()
}

/// Benchmark every resolver against `domains`, measuring median/p95/min
/// latency and packet loss.
pub fn benchmark(
    servers: &[DnsServer],
    domains: &[String],
    queries_per_domain: usize,
) -> Vec<DnsBenchmarkResult> {
    servers
        .iter()
        .map(|s| {
            let mut latencies = Vec::new();
            let mut failures = 0usize;
            let mut queries = 0usize;
            for domain in domains {
                for _ in 0..queries_per_domain {
                    queries += 1;
                    match query_once(&s.ip, domain, 2000) {
                        Ok(ms) => latencies.push(ms),
                        Err(_) => failures += 1,
                    }
                }
            }
            latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
            let loss_percent = if queries == 0 {
                0.0
            } else {
                failures as f64 / queries as f64 * 100.0
            };
            DnsBenchmarkResult {
                name: s.name.clone(),
                ip: s.ip.clone(),
                is_current: s.is_current,
                median_ms: median(&latencies),
                p95_ms: percentile(&latencies, 0.95),
                min_ms: latencies.first().copied(),
                loss_percent,
                queries,
                failures,
            }
        })
        .collect()
}

/// Apply static DNS servers to an adapter (snapshot-first, reversible).
pub fn apply_dns(db: &Database, guid: &str, servers: &[String]) -> Result<DnsApplyResult> {
    let location = format!(
        r"HKLM\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\{guid}\NameServer"
    );
    let old = win::network::name_server(guid);
    let new = servers.join(",");

    let snap = snapshot::create_lightweight(
        db,
        "DNS servers",
        Some(&format!("set DNS on adapter {guid}")),
    )?;

    win::registry::set_registry_value(&location, &new)?;

    // Verify the write landed; on failure restore the previous value.
    if win::network::name_server(guid).as_deref() != Some(new.as_str()) {
        match &old {
            Some(prev) => {
                let _ = win::registry::set_registry_value(&location, prev);
            }
            None => {
                let _ = win::registry::delete_registry_value(&location);
            }
        }
        return Err(OptixError::Windows(
            "DNS apply verification failed; reverted".into(),
        ));
    }

    rollback::record_change(
        db,
        &snap.id,
        ChangeRecord {
            id: None,
            snapshot_id: String::new(),
            domain: "registry".to_string(),
            location,
            kind: "set".to_string(),
            old_value: old.clone(),
            new_value: Some(new),
            old_json: None,
            new_json: None,
            applied_at_ms: None,
            verified: true,
            rolled_back: false,
        },
    )?;

    win::network::flush_dns();
    Ok(DnsApplyResult {
        snapshot_id: snap.id,
        changes: 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_has_valid_header_and_question() {
        let q = build_query("example.com", 0x1234);
        assert_eq!(&q[0..2], &[0x12, 0x34]); // ID
        assert_eq!(&q[2..4], &[0x01, 0x00]); // flags
        assert_eq!(u16::from_be_bytes([q[4], q[5]]), 1); // QDCOUNT
        // Name: 7 example 3 com 0 (13 bytes)
        assert_eq!(&q[12..25], b"\x07example\x03com\x00");
        assert_eq!(u16::from_be_bytes([q[25], q[26]]), 1); // QTYPE A
        assert_eq!(u16::from_be_bytes([q[27], q[28]]), 1); // QCLASS IN
    }

    #[test]
    fn parses_a_records_with_compression() {
        // Header: ID=0, flags=0x8180, QD=1, AN=2
        let mut r = vec![
            0x00, 0x00, 0x81, 0x80, 0x00, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00,
        ];
        // Question: 3 www 7 example 3 com 0 + type + class
        r.extend_from_slice(b"\x03www\x07example\x03com\x00\x00\x01\x00\x01");
        // Answer 1: pointer to "example.com" (0xC00C), type A, class IN, TTL, rdlen 4, 1.2.3.4
        r.extend_from_slice(&[0xC0, 0x0C, 0x00, 0x01, 0x00, 0x01]);
        r.extend_from_slice(&[0x00, 0x00, 0x00, 0x3C]);
        r.extend_from_slice(&[0x00, 0x04, 1, 2, 3, 4]);
        // Answer 2: pointer to "example.com", type A, rdlen 4, 5.6.7.8
        r.extend_from_slice(&[0xC0, 0x0C, 0x00, 0x01, 0x00, 0x01]);
        r.extend_from_slice(&[0x00, 0x00, 0x00, 0x3C]);
        r.extend_from_slice(&[0x00, 0x04, 5, 6, 7, 8]);

        let addrs = parse_a_records(&r);
        assert_eq!(addrs, vec![[1, 2, 3, 4], [5, 6, 7, 8]]);
    }

    #[test]
    #[ignore = "requires network access"]
    fn live_dns_query_resolves() {
        let ms = query_once("1.1.1.1", "example.com", 3000).expect("query should succeed");
        assert!(ms >= 0.0);
    }

    #[test]
    fn median_and_percentile_math() {
        let sorted = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(median(&sorted), Some(3.0));
        assert_eq!(percentile(&sorted, 0.95), Some(5.0));
        assert_eq!(median(&[1.0, 2.0]), Some(1.5));
        assert_eq!(median(&[]), None);
    }
}
