//! Windows network detection (adapters, DNS servers, default gateway, TCP/IP
//! parameters) from the TCP/IP registry keys, plus a DNS cache flush.

use crate::models::network::NetworkAdapter;

#[cfg(windows)]
const TCPIP_INTERFACES: &str = r"SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces";

#[cfg(windows)]
fn read_string(key: &winreg::RegKey, name: &str) -> Option<String> {
    key.get_value::<String, _>(name).ok()
}

#[cfg(windows)]
fn read_multi(key: &winreg::RegKey, name: &str) -> Option<Vec<String>> {
    key.get_value::<Vec<String>, _>(name).ok()
}

#[cfg(windows)]
fn split_servers(s: &str) -> Vec<String> {
    s.split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(windows)]
fn connection_name(guid: &str) -> Option<String> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;
    let path = format!(
        r"SYSTEM\CurrentControlSet\Control\Network\{{4d36e972-e325-11ce-bfc1-08002be10318}}\{guid}\Connection"
    );
    RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(path)
        .ok()?
        .get_value::<String, _>("Name")
        .ok()
}

#[cfg(windows)]
fn gateway_of(key: &winreg::RegKey) -> Option<String> {
    let mut values = Vec::new();
    if let Some(s) = read_string(key, "DefaultGateway") {
        values.push(s);
    }
    if let Some(v) = read_multi(key, "DefaultGateway") {
        values.extend(v);
    }
    if let Some(s) = read_string(key, "DhcpDefaultGateway") {
        values.push(s);
    }
    if let Some(v) = read_multi(key, "DhcpDefaultGateway") {
        values.extend(v);
    }
    values
        .into_iter()
        .map(|v| v.trim().to_string())
        .find(|v| !v.is_empty() && v != "0.0.0.0")
}

/// Enumerate network adapters with DNS configuration and active (gateway) flag.
#[cfg(windows)]
pub fn list_adapters() -> Vec<NetworkAdapter> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let mut out = Vec::new();
    let Ok(ifaces) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(TCPIP_INTERFACES) else {
        return out;
    };
    for guid in ifaces.enum_keys().flatten() {
        let Ok(key) = ifaces.open_subkey(&guid) else {
            continue;
        };
        let name = connection_name(&guid).unwrap_or_else(|| guid.clone());
        let name_server: String = key.get_value("NameServer").unwrap_or_default();
        let dhcp_name_server: String = key.get_value("DhcpNameServer").unwrap_or_default();
        let dns = if !name_server.is_empty() {
            split_servers(&name_server)
        } else {
            split_servers(&dhcp_name_server)
        };
        let enable_dhcp: u32 = key.get_value("EnableDHCP").unwrap_or(1);
        let is_active = gateway_of(&key).is_some();
        out.push(NetworkAdapter {
            name,
            guid,
            dns_servers: dns,
            is_active,
            dhcp_enabled: enable_dhcp != 0,
        });
    }
    out.sort_by(|a, b| {
        b.is_active
            .cmp(&a.is_active)
            .then_with(|| a.name.cmp(&b.name))
    });
    out
}

#[cfg(not(windows))]
pub fn list_adapters() -> Vec<NetworkAdapter> {
    Vec::new()
}

/// The raw `NameServer` registry string for an adapter, if present.
#[cfg(windows)]
pub fn name_server(guid: &str) -> Option<String> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;
    let key = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(format!(r"{TCPIP_INTERFACES}\{guid}"))
        .ok()?;
    key.get_value::<String, _>("NameServer").ok()
}

#[cfg(not(windows))]
pub fn name_server(_guid: &str) -> Option<String> {
    None
}

/// The first default gateway found across interfaces.
#[cfg(windows)]
pub fn gateway() -> Option<String> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;
    let Ok(ifaces) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(TCPIP_INTERFACES) else {
        return None;
    };
    for guid in ifaces.enum_keys().flatten() {
        if let Ok(key) = ifaces.open_subkey(&guid) {
            if let Some(g) = gateway_of(&key) {
                return Some(g);
            }
        }
    }
    None
}

#[cfg(not(windows))]
pub fn gateway() -> Option<String> {
    None
}

/// Currently-configured DNS servers (active adapter preferred).
#[cfg(windows)]
pub fn current_dns_servers() -> Vec<String> {
    let adapters = list_adapters();
    adapters
        .iter()
        .find(|a| a.is_active)
        .map(|a| a.dns_servers.clone())
        .or_else(|| {
            adapters
                .iter()
                .find(|a| !a.dns_servers.is_empty())
                .map(|a| a.dns_servers.clone())
        })
        .unwrap_or_default()
}

#[cfg(not(windows))]
pub fn current_dns_servers() -> Vec<String> {
    // Best-effort for Linux dev: parse /etc/resolv.conf.
    let Ok(text) = std::fs::read_to_string("/etc/resolv.conf") else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|l| l.trim().strip_prefix("nameserver").map(|r| r.trim().to_string()))
        .collect()
}

/// Read a single TCP/IP parameter DWORD from the registry (`None` when absent
/// or non-numeric, meaning the driver default applies).
#[cfg(windows)]
pub fn tcp_value(name: &str) -> Option<u32> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;
    RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SYSTEM\CurrentControlSet\Services\Tcpip\Parameters")
        .ok()?
        .get_value::<u32, _>(name)
        .ok()
}

#[cfg(not(windows))]
pub fn tcp_value(_name: &str) -> Option<u32> {
    None
}

/// Flush the DNS resolver cache (best-effort; no-op off Windows).
#[cfg(windows)]
pub fn flush_dns() {
    use std::os::windows::process::CommandExt;
    // Spawn without a console window — a GUI app must not flash one.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    if let Err(e) = std::process::Command::new("ipconfig")
        .arg("/flushdns")
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        crate::logging::warn(&format!("ipconfig /flushdns failed: {e}"));
    }
}

#[cfg(not(windows))]
pub fn flush_dns() {}
