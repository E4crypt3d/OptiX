//! ICMP ping measurement (Phase 7).
//!
//! Windows uses `IcmpSendEcho` (iphlpapi) — accurate and unprivileged. On
//! Linux (dev mode) we shell out to the system `ping` command and parse the
//! `time=…ms` values, which keeps the feature usable during development.

use crate::error::{OptixError, Result};

/// Ping `host` `count` times, returning one RTT in milliseconds per reply.
#[cfg(windows)]
pub fn ping_host(host: &str, count: u32, timeout_ms: u32) -> Result<Vec<f64>> {
    use std::net::ToSocketAddrs;
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho, ICMP_ECHO_REPLY, IP_OPTION_INFORMATION,
    };

    let count = count.clamp(1, 64);
    let timeout = timeout_ms.clamp(100, 10_000);

    let ip = host
        .to_socket_addrs()
        .map_err(|e| OptixError::Other(format!("cannot resolve {host}: {e}")))?
        .filter_map(|a| match a {
            std::net::SocketAddr::V4(v4) => Some(*v4.ip()),
            _ => None,
        })
        .next()
        .ok_or_else(|| OptixError::Other(format!("no IPv4 address for {host}")))?;

    let handle = unsafe { IcmpCreateFile() };
    if handle.is_null() {
        return Err(OptixError::Windows("IcmpCreateFile failed".into()));
    }

    let mut out = Vec::with_capacity(count as usize);
    // Network byte order is what IcmpSendEcho expects.
    let dest = u32::from_be_bytes(ip.octets());
    let mut reply = vec![0u8; std::mem::size_of::<ICMP_ECHO_REPLY>() + 72];
    let data = b"optix-icmp-probe";

    for _ in 0..count {
        let sent = unsafe {
            IcmpSendEcho(
                handle,
                dest,
                data.as_ptr() as *const core::ffi::c_void,
                data.len() as u16,
                std::ptr::null::<IP_OPTION_INFORMATION>() as *mut IP_OPTION_INFORMATION,
                reply.as_mut_ptr() as *mut core::ffi::c_void,
                reply.len() as u32,
                timeout,
            )
        };
        if sent > 0 {
            let reply_ptr = reply.as_ptr() as *const ICMP_ECHO_REPLY;
            let status = unsafe { (*reply_ptr).Status };
            if status == 0 {
                // Status 0 = IP_SUCCESS. RoundTripTime is already in ms.
                out.push(unsafe { (*reply_ptr).RoundTripTime } as f64);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    unsafe { IcmpCloseHandle(handle) };
    Ok(out)
}

#[cfg(not(windows))]
pub fn ping_host(host: &str, count: u32, timeout_ms: u32) -> Result<Vec<f64>> {
    use std::process::Command;

    let count = count.clamp(1, 64);
    // `--` stops option parsing so a host starting with `-` is treated as a
    // target, not a flag (the host string comes from the frontend).
    let output = Command::new("ping")
        .args([
            "-c",
            &count.to_string(),
            "-W",
            &timeout_ms.clamp(1, 10).to_string(),
            "-i",
            "0.2",
            "--",
            host,
        ])
        .output()
        .map_err(|e| OptixError::Other(format!("cannot run ping: {e}")))?;

    let text = String::from_utf8_lossy(&output.stdout);
    Ok(parse_ping_output(&text))
}

/// Extract RTT values from a `ping` invocation's stdout. Handles both
/// `time=12.3 ms` (Windows/Linux) and `time<1 ms` (Linux sub-ms) formats;
/// tolerant of extra text.
pub fn parse_ping_output(text: &str) -> Vec<f64> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut rest = line;
        loop {
            let eq = rest.find("time=");
            let lt = rest.find("time<");
            let pos = match (eq, lt) {
                (Some(a), Some(b)) => a.min(b),
                (Some(a), None) => a,
                (None, Some(b)) => b,
                (None, None) => break,
            };
            // "time=" and "time<" are both 5 characters long.
            let sub_ms = rest[pos..].starts_with("time<");
            rest = &rest[pos + 5..];
            if sub_ms {
                // "time<1 ms" — sub-millisecond RTT, count as ~0.5 ms.
                out.push(0.5);
                continue;
            }
            let end = rest
                .find(|c: char| !c.is_ascii_digit() && c != '.')
                .unwrap_or(rest.len());
            let num = &rest[..end];
            if let Ok(v) = num.parse::<f64>() {
                out.push(v);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_linux_ping_output() {
        let sample = "\
PING google.com (142.250.72.14) 56(84) bytes of data.
64 bytes from 142.250.72.14: icmp_seq=1 ttl=115 time=11.4 ms
64 bytes from 142.250.72.14: icmp_seq=2 ttl=115 time=10.9 ms
64 bytes from 142.250.72.14: icmp_seq=3 ttl=115 time<1 ms
--- google.com ping statistics ---
3 packets transmitted, 3 received, 0% packet loss
";
        assert_eq!(parse_ping_output(sample), vec![11.4, 10.9, 0.5]);
    }

    #[test]
    fn parses_windows_style_output() {
        let sample = "Reply from 1.1.1.1: bytes=32 time=12ms TTL=56\nReply from 1.1.1.1: time=9ms";
        assert!(parse_ping_output(sample).len() == 2);
    }

    #[test]
    fn empty_when_no_replies() {
        assert_eq!(parse_ping_output("100% packet loss"), Vec::<f64>::new());
    }
}