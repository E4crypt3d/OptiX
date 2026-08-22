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

/// Bitfield masks for `MIB_IF_ROW2.InterfaceAndOperStatusFlags` (LSB-first per
/// the NDIS docs): hardware, filter, connector-present, not-authenticated,
/// not-media-connected, paused, low-power, endpoint-interface.
#[cfg(windows)]
const FLAG_HARDWARE: u8 = 0x01;
#[cfg(windows)]
const FLAG_CONNECTOR_PRESENT: u8 = 0x04;
#[cfg(windows)]
const FLAG_ENDPOINT_INTERFACE: u8 = 0x80;

#[allow(clippy::too_many_lines)]
#[cfg_attr(not(windows), allow(dead_code))]
fn classify_interface(if_type: u32, media_type: i32, physical_medium: i32, description: &str) -> &'static str {
    const IF_TYPE_SOFTWARE_LOOPBACK: u32 = 24;
    const IF_TYPE_IEEE80211: u32 = 71;
    const IF_TYPE_TUNNEL: u32 = 131;
    // NdisMedium802_3 = 0, NdisMediumNative802_11 = 16, NdisMediumWirelessWAN = 9.
    const NDIS_MEDIUM_NATIVE_80211: i32 = 16;
    const NDIS_MEDIUM_WIRELESS_WAN: i32 = 9;

    if if_type == IF_TYPE_IEEE80211 || media_type == NDIS_MEDIUM_NATIVE_80211 {
        return "wifi";
    }
    if if_type == IF_TYPE_TUNNEL || media_type == NDIS_MEDIUM_WIRELESS_WAN {
        return "vpn";
    }
    let desc = description.to_ascii_lowercase();
    if desc.contains("bluetooth") && desc.contains("personal area") {
        return "bluetooth";
    }
    if if_type != IF_TYPE_SOFTWARE_LOOPBACK && is_virtual_description(&desc) {
        return "virtual";
    }
    if if_type == IF_TYPE_SOFTWARE_LOOPBACK {
        "loopback"
    } else if physical_medium == NDIS_MEDIUM_WIRELESS_WAN {
        "vpn"
    } else {
        "ethernet"
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
fn is_virtual_description(desc_lower: &str) -> bool {
    let desc_lower = desc_lower.to_ascii_lowercase();
    const MARKERS: [&str; 14] = [
        "virtual",
        "hyper-v",
        "vethernet",
        "tap-",
        "tunnel",
        "wireguard",
        "tailscale",
        "zerotier",
        "openvpn",
        "warp ",
        "vmware",
        "virtualbox",
        "wi-fi direct",
        "km-test",
    ];
    MARKERS.iter().any(|m| desc_lower.contains(m))
}

#[cfg_attr(not(windows), allow(dead_code))]
fn include_in_inventory(kind: &str) -> bool {
    kind != "loopback"
}

#[cfg(windows)]
fn utf16_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

/// Format a `windows_sys::core::GUID` as `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}`
/// (uppercase, braced — the registry's representation).
#[cfg(windows)]
fn guid_to_string(g: &windows_sys::core::GUID) -> String {
    format!(
        "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        g.data1,
        g.data2,
        g.data3,
        g.data4[0],
        g.data4[1],
        g.data4[2],
        g.data4[3],
        g.data4[4],
        g.data4[5],
        g.data4[6],
        g.data4[7]
    )
}

#[cfg(windows)]
pub(crate) fn parse_guid(text: &str) -> Option<windows_sys::core::GUID> {
    let hex: String = text
        .trim()
        .trim_start_matches('{')
        .trim_end_matches('}')
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .collect();
    if hex.len() != 32 {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok();
    let word = |i: usize| -> Option<u16> {
        Some((u16::from(byte(i)?) << 8) | u16::from(byte(i + 1)?))
    };
    let mut data4 = [0u8; 8];
    for (i, slot) in data4.iter_mut().enumerate() {
        *slot = byte(8 + i)?;
    }
    Some(windows_sys::core::GUID {
        data1: u32::from_str_radix(&hex[0..8], 16).ok()?,
        data2: word(4)?,
        data3: word(6)?,
        data4,
    })
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn mac_to_string(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 2 || bytes.iter().all(|b| *b == 0) {
        return None;
    }
    Some(
        bytes
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(":"),
    )
}

/// Convert Windows WLAN signal quality (0–100) to approximate RSSI dBm using
/// the linear interpolation from wlanapi.h (-100 dBm at 0, -50 dBm at 100).
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn signal_quality_to_rssi(quality: u32) -> i32 {
    match quality {
        0 => -100,
        q if q >= 100 => -50,
        q => (q / 2) as i32 - 100,
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn phy_type_name(phy: i32) -> String {
    match phy {
        1 => "802.11 (FHSS)",
        2 => "802.11b (DSSS)",
        4 => "802.11a (OFDM)",
        5 => "802.11b (HR-DSSS)",
        6 => "802.11g (ERP)",
        7 => "Wi-Fi 4 (802.11n)",
        8 => "Wi-Fi 5 (802.11ac)",
        9 => "802.11ad (60 GHz)",
        10 => "Wi-Fi 6 (802.11ax)",
        11 => "Wi-Fi 7 (802.11be)",
        _ => "Unknown",
    }
    .to_string()
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn auth_name(auth: i32) -> String {
    match auth {
        1 => "Open".into(),
        3 => "WPA Enterprise".into(),
        4 => "WPA-PSK".into(),
        6 => "WPA2 Enterprise".into(),
        7 => "WPA2-PSK".into(),
        9 => "WPA3-SAE".into(),
        11 => "WPA3 Enterprise".into(),
        12 => "WPA3-SAE (transition)".into(),
        _ => format!("Unknown ({auth})"),
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn cipher_name(cipher: i32) -> String {
    match cipher {
        0x100 => "None".into(),
        1 => "WEP-40".into(),
        2 => "TKIP".into(),
        4 => "AES-CCMP".into(),
        5 => "WEP-104".into(),
        7 => "AES-GCMP".into(),
        8 => "AES-GCMP-256".into(),
        _ => format!("Unknown ({cipher})"),
    }
}

/// Parse a CIM datetime such as `20240115000000.000000+060` into `2024-01-15`.
#[cfg_attr(not(windows), allow(dead_code))]
fn driver_date_from_cim(raw: &str) -> Option<String> {
    let digits: Vec<char> = raw.chars().take_while(char::is_ascii_digit).collect();
    if digits.len() < 8 {
        return None;
    }
    let s: String = digits.into_iter().collect();
    Some(format!("{}-{}-{}", &s[0..4], &s[4..6], &s[6..8]))
}


#[cfg(windows)]
mod imp {
    use super::{
        classify_interface, guid_to_string, include_in_inventory, mac_to_string,
        utf16_to_string, FLAG_CONNECTOR_PRESENT, FLAG_ENDPOINT_INTERFACE, FLAG_HARDWARE,
    };
    use crate::models::network::{AdapterInventory, InterfaceCounters};
    use std::net::{Ipv4Addr, Ipv6Addr};
    use windows_sys::core::PWSTR;
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        FreeMibTable, GetAdaptersAddresses, GetIfTable2, IP_ADAPTER_ADDRESSES_LH,
        MIB_IF_TABLE2,
    };
    use windows_sys::Win32::NetworkManagement::Ndis::{
        MediaConnectStateConnected, NET_IF_OPER_STATUS_UP,
    };
    use windows_sys::Win32::Networking::WinSock::{
        AF_INET, AF_INET6, SOCKADDR, SOCKADDR_IN, SOCKADDR_IN6,
    };

    /// `Dhcpv4Enabled` is the first bit of the `IP_ADAPTER_ADDRESSES_LH`
    /// flags bitfield (LSB-first per the SDK header).
    pub(super) const GAA_DHCPV4_ENABLED: u32 = 0x01;

    fn pwstr_to_string(ptr: PWSTR) -> String {
        if ptr.is_null() {
            return String::new();
        }
        unsafe {
            let len = (0..).position(|i| *ptr.add(i) == 0).unwrap_or(0);
            String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len))
        }
    }

    fn normalize_guid(guid: &str) -> String {
        guid.replace(['{', '}'], "").to_uppercase()
    }

    fn sockaddr_to_ip(sa: *const SOCKADDR) -> Option<String> {
        if sa.is_null() {
            return None;
        }
        unsafe {
            match (*sa).sa_family {
                AF_INET => {
                    let v4 = sa as *const SOCKADDR_IN;
                    Some(Ipv4Addr::from((*v4).sin_addr.S_un.S_addr.to_be_bytes()).to_string())
                }
                AF_INET6 => {
                    let v6 = sa as *const SOCKADDR_IN6;
                    Some(Ipv6Addr::from((*v6).sin6_addr.u.Byte).to_string())
                }
                _ => None,
            }
        }
    }

    /// Base inventory rows from `GetIfTable2` — one entry per interface with
    /// hardware identity, state, link speed and lifetime counters.
    pub(super) fn if_table_rows() -> (Vec<AdapterInventory>, Vec<(u64, String)>) {
        let mut out = Vec::new();
        let mut luids = Vec::new();
        unsafe {
            let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
            if GetIfTable2(&mut table) != 0 || table.is_null() {
                return (out, luids);
            }
            let count = (*table).NumEntries as usize;
            let first = (*table).Table.as_ptr();
            for index in 0..count {
                let row = &*first.add(index);
                let description = utf16_to_string(&row.Description);
                let kind = classify_interface(
                    row.Type,
                    row.MediaType,
                    row.PhysicalMediumType,
                    &description,
                );
                if !include_in_inventory(kind) {
                    continue;
                }
                let flags = row.InterfaceAndOperStatusFlags._bitfield;
                let oper_up = row.OperStatus == NET_IF_OPER_STATUS_UP;
                let connector = flags & FLAG_CONNECTOR_PRESENT != 0;
                let media_connected = row.MediaConnectState == MediaConnectStateConnected;
                luids.push((row.InterfaceLuid.Value, guid_to_string(&row.InterfaceGuid)));
                out.push(AdapterInventory {
                    guid: guid_to_string(&row.InterfaceGuid),
                    name: utf16_to_string(&row.Alias),
                    description,
                    kind: kind.to_string(),
                    // Virtual/tunnel adapters report unknown media state; for
                    // those an UP operational status is enough.
                    is_up: oper_up && (!connector || media_connected),
                    is_virtual: flags & FLAG_HARDWARE == 0
                        || flags & FLAG_ENDPOINT_INTERFACE != 0,
                    mac_address: mac_to_string(&row.PhysicalAddress),
                    mtu: (row.Mtu > 0).then_some(row.Mtu),
                    transmit_link_bps: (row.TransmitLinkSpeed > 0)
                        .then_some(row.TransmitLinkSpeed),
                    receive_link_bps: (row.ReceiveLinkSpeed > 0).then_some(row.ReceiveLinkSpeed),
                    ip_addresses: Vec::new(),
                    gateways: Vec::new(),
                    dns_servers: Vec::new(),
                    dhcp_enabled: None,
                    counters: Some(InterfaceCounters {
                        received_bytes: row.InOctets,
                        sent_bytes: row.OutOctets,
                        receive_errors: row.InErrors,
                        send_errors: row.OutErrors,
                        receive_discards: row.InDiscards,
                        send_discards: row.OutDiscards,
                    }),
                    wifi: None,
                    driver: None,
                });
            }
            FreeMibTable(table.cast());
        }
        (out, luids)
    }

    /// Fill addresses/gateways/DHCP from `GetAdaptersAddresses`, matching by
    /// interface LUID (the GUID is not exposed on the GAA struct).
    pub(super) fn merge_addresses(entries: &mut [AdapterInventory], luids: &[(u64, String)]) {
        const GAA_FLAGS: u32 = 256 | 128; // INCLUDE_ALL_INTERFACES | INCLUDE_GATEWAYS
        let mut size: u32 = 15 * 1024;
        let mut buffer = vec![0u8; size as usize];
        let status = unsafe {
            GetAdaptersAddresses(
                0, // AF_UNSPEC — both families
                GAA_FLAGS,
                std::ptr::null(),
                buffer.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>(),
                &mut size,
            )
        };
        if status != 0 {
            return;
        }
        let mut cursor = buffer.as_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
        while !cursor.is_null() {
            let adapter = unsafe { &*cursor };
            let luid_value = unsafe { adapter.Luid.Value };
            let key = luids
                .iter()
                .find(|(luid, _)| *luid == luid_value)
                .map(|(_, guid)| normalize_guid(guid));
            let Some(key) = key else {
                cursor = adapter.Next;
                continue;
            };
            if let Some(entry) = entries.iter_mut().find(|e| normalize_guid(&e.guid) == key) {
                let friendly = pwstr_to_string(adapter.FriendlyName);
                if !friendly.is_empty() && (entry.name.is_empty() || adapter.IfType == 71) {
                    entry.name = friendly;
                }
                let mut unicast = adapter.FirstUnicastAddress;
                while !unicast.is_null() {
                    let ua = unsafe { &*unicast };
                    let prefix = ua.OnLinkPrefixLength;
                    if let Some(ip) = sockaddr_to_ip(ua.Address.lpSockaddr) {
                        entry.ip_addresses.push(format!("{ip}/{prefix}"));
                    }
                    unicast = ua.Next;
                }
                let mut gateway = adapter.FirstGatewayAddress;
                while !gateway.is_null() {
                    let ga = unsafe { &*gateway };
                    if let Some(ip) = sockaddr_to_ip(ga.Address.lpSockaddr) {
                        entry.gateways.push(ip);
                    }
                    gateway = ga.Next;
                }
                // Anonymous2 unions Flags with the per-protocol bitfield; the
                // Dhcpv4Enabled bit lives at bit 0 of the bitfield view.
                let dhcp_bit =
                    unsafe { adapter.Anonymous2.Anonymous._bitfield & GAA_DHCPV4_ENABLED } != 0;
                entry.dhcp_enabled.get_or_insert(dhcp_bit);
            }
            cursor = adapter.Next;
        }
    }
}

#[cfg(windows)]
pub fn inventory_windows() -> Vec<crate::models::network::AdapterInventory> {
    let (mut entries, luids) = imp::if_table_rows();
    imp::merge_addresses(&mut entries, &luids);

    let dns_map: std::collections::HashMap<String, (Vec<String>, bool)> = list_adapters()
        .into_iter()
        .map(|a| (normalize_guid_key(&a.guid), (a.dns_servers, a.dhcp_enabled)))
        .collect();
    let drivers = query_adapter_drivers();

    for entry in &mut entries {
        let key = normalize_guid_key(&entry.guid);
        if let Some((dns, dhcp)) = dns_map.get(&key) {
            entry.dns_servers.clone_from(dns);
            entry.dhcp_enabled.get_or_insert(*dhcp);
        }
        entry.driver = drivers.get(&key).cloned();
        if entry.kind == "wifi" && entry.is_up {
            entry.wifi = crate::win::wifi::connected_info(&entry.guid);
        }
    }
    entries.sort_by(|a, b| b.is_up.cmp(&a.is_up).then_with(|| a.name.cmp(&b.name)));
    entries
}

/// Full cross-type adapter inventory: physical NICs, Wi-Fi radios, VPN and
/// tunnel adapters, Hyper-V/WSL virtual NICs — everything except loopback.
pub fn inventory() -> Vec<crate::models::network::AdapterInventory> {
    #[cfg(windows)]
    return inventory_windows();

    #[cfg(not(windows))]
    linux_inventory()
}

#[cfg(windows)]
fn normalize_guid_key(guid: &str) -> String {
    guid.replace(['{', '}'], "").to_uppercase()
}

/// Driver metadata from the `MSFT_NetAdapter` WMI class, keyed by normalized
/// interface GUID.
#[cfg(windows)]
fn query_adapter_drivers() -> std::collections::HashMap<String, crate::models::network::AdapterDriver> {
    use crate::models::network::AdapterDriver;
    use std::collections::HashMap;
    use wmi::{Variant, WMIConnection};

    fn get_string(map: &std::collections::HashMap<String, Variant>, key: &str) -> Option<String> {
        match map.get(key) {
            Some(Variant::String(s)) => Some(s.clone()),
            _ => None,
        }
    }

    fn get_u8(map: &std::collections::HashMap<String, Variant>, key: &str) -> Option<u8> {
        match map.get(key) {
            Some(Variant::UI1(v)) => Some(*v),
            _ => None,
        }
    }

    let mut map = HashMap::new();
    let Ok(conn) = WMIConnection::with_namespace_path("ROOT\\StandardCimv2") else {
        return map;
    };
    let Ok(rows) = conn.raw_query(
        "SELECT InterfaceGuid, DriverVersionString, DriverDate, DriverProvider, \
         DriverMajorNdisVersion, DriverMinorNdisVersion, MediaDuplexState \
         FROM MSFT_NetAdapter",
    ) else {
        return map;
    };
    for row in rows {
        let Some(guid) = get_string(&row, "InterfaceGuid").map(|g| normalize_guid_key(&g)) else {
            continue;
        };
        let duplex = match row.get("MediaDuplexState") {
            Some(Variant::UI4(1)) => Some(false),
            Some(Variant::UI4(2)) => Some(true),
            _ => None,
        };
        let ndis = match (get_u8(&row, "DriverMajorNdisVersion"), get_u8(&row, "DriverMinorNdisVersion")) {
            (Some(major), Some(minor)) => Some(format!("{major}.{minor}")),
            _ => None,
        };
        map.insert(
            guid,
            AdapterDriver {
                version: get_string(&row, "DriverVersionString"),
                date: get_string(&row, "DriverDate").and_then(|d| driver_date_from_cim(&d)),
                provider: get_string(&row, "DriverProvider"),
                ndis_version: ndis,
                full_duplex: duplex,
            },
        );
    }
    map
}

/// Best-effort sysfs inventory so the Network tab works during Linux
/// development (`/sys/class/net` + `/proc/net/route`).
#[cfg(not(windows))]
fn linux_inventory() -> Vec<crate::models::network::AdapterInventory> {
    use crate::models::network::{AdapterInventory, InterfaceCounters};
    use std::fs;
    use std::net::Ipv4Addr;

    fn read_trim(path: &std::path::Path) -> Option<String> {
        fs::read_to_string(path).ok().map(|s| s.trim().to_string())
    }

    let mut out = Vec::new();
    let Ok(dir) = fs::read_dir("/sys/class/net") else {
        return out;
    };
    for iface in dir.flatten() {
        let name = iface.file_name().to_string_lossy().to_string();
        if name == "lo"
            || ["tun", "wg", "zt", "vm", "br-", "docker", "veth"]
                .iter()
                .any(|prefix| name.starts_with(prefix))
        {
            continue;
        }
        let base = iface.path();
        let kind = if name.starts_with("wl") {
            "wifi"
        } else if name.starts_with("ww") {
            "vpn"
        } else {
            "ethernet"
        };
        let stat = |file: &str| {
            read_trim(&base.join("statistics").join(file))
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0)
        };
        let speed_mbps = read_trim(&base.join("speed")).and_then(|v| v.parse::<u64>().ok());
        let gateways = fs::read_to_string("/proc/net/route")
            .map(|text| {
                text.lines()
                    .filter(|l| l.split_whitespace().nth(1) == Some("00000000"))
                    .filter_map(|l| l.split_whitespace().nth(2))
                    .filter_map(|hex| u32::from_str_radix(hex, 16).ok())
                    .map(|v| Ipv4Addr::from(v.to_be_bytes()).to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let driver_name = fs::read_link(base.join("device/driver"))
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()));
        let duplex = read_trim(&base.join("duplex"));
        out.push(AdapterInventory {
            guid: name.clone(),
            name: name.clone(),
            description: driver_name.unwrap_or_else(|| kind.to_string()),
            kind: kind.to_string(),
            is_up: read_trim(&base.join("operstate")).as_deref() == Some("up"),
            is_virtual: false,
            mac_address: read_trim(&base.join("address")),
            mtu: read_trim(&base.join("mtu")).and_then(|v| v.parse().ok()),
            transmit_link_bps: speed_mbps.map(|m| m * 1_000_000),
            receive_link_bps: speed_mbps.map(|m| m * 1_000_000),
            ip_addresses: Vec::new(),
            gateways,
            dns_servers: current_dns_servers(),
            dhcp_enabled: None,
            counters: Some(InterfaceCounters {
                received_bytes: stat("rx_bytes"),
                sent_bytes: stat("tx_bytes"),
                receive_errors: stat("rx_errors"),
                send_errors: stat("tx_errors"),
                receive_discards: stat("rx_dropped"),
                send_discards: stat("tx_dropped"),
            }),
            wifi: None,
            driver: Some(crate::models::network::AdapterDriver {
                version: None,
                date: None,
                provider: None,
                ndis_version: None,
                full_duplex: duplex.as_deref().map(|d| d == "full"),
            }),
        });
    }
    out.sort_by(|a, b| b.is_up.cmp(&a.is_up).then_with(|| a.name.cmp(&b.name)));
    out
}

#[cfg(test)]
mod inventory_tests {
    use super::*;

    #[test]
    fn classifies_common_interfaces() {
        assert_eq!(classify_interface(6, 0, -1, "Realtek Gaming GbE"), "ethernet");
        assert_eq!(classify_interface(71, -1, -1, "Intel Wi-Fi 6 AX200"), "wifi");
        assert_eq!(classify_interface(131, -1, -1, "WireGuard Tunnel"), "vpn");
        assert_eq!(classify_interface(6, 16, -1, "any native wifi medium"), "wifi");
        assert_eq!(classify_interface(6, 9, -1, "cellular modem"), "vpn");
        assert_eq!(classify_interface(24, 0, -1, "Loopback Pseudo-Interface"), "loopback");
    }

    #[test]
    fn detects_virtual_adapters_by_description() {
        assert!(is_virtual_description("hyper-v virtual ethernet adapter"));
        assert!(is_virtual_description("TAP-Windows Adapter V9"));
        assert!(is_virtual_description("tailscale tunnel"));
        assert!(!is_virtual_description("realtek gaming 2.5gbe family controller"));
    }

    #[test]
    fn loops_back_excluded_from_inventory() {
        assert!(!include_in_inventory("loopback"));
        assert!(include_in_inventory("ethernet"));
        assert!(include_in_inventory("virtual"));
    }

    #[test]
    fn rssi_interpolation_matches_wlanapi_doc() {
        assert_eq!(signal_quality_to_rssi(0), -100);
        assert_eq!(signal_quality_to_rssi(100), -50);
        assert_eq!(signal_quality_to_rssi(50), -75);
        assert_eq!(signal_quality_to_rssi(99), -51);
    }

    #[test]
    fn phy_and_auth_names_are_readable() {
        assert_eq!(phy_type_name(7), "Wi-Fi 4 (802.11n)");
        assert_eq!(phy_type_name(10), "Wi-Fi 6 (802.11ax)");
        assert_eq!(auth_name(9), "WPA3-SAE");
        assert_eq!(cipher_name(4), "AES-CCMP");
    }

    #[test]
    fn cim_driver_date_parses() {
        assert_eq!(
            driver_date_from_cim("20240115000000.000000+060"),
            Some("2024-01-15".to_string())
        );
        assert_eq!(driver_date_from_cim(""), None);
    }

    #[cfg(windows)]
    #[test]
    fn guid_roundtrip() {
        let text = "{4d36e972-e325-11ce-bfc1-08002be10318}";
        let parsed = parse_guid(text).unwrap();
        assert_eq!(guid_to_string(&parsed), "{4D36E972-E325-11CE-BFC1-08002BE10318}");
        assert!(parse_guid("not a guid").is_none());
        assert!(parse_guid("{}").is_none());
    }

    #[test]
    fn mac_formatting_skips_zero_addresses() {
        assert_eq!(mac_to_string(&[]), None);
        assert_eq!(mac_to_string(&[0, 0, 0, 0, 0, 0]), None);
        assert_eq!(mac_to_string(&[0xA1, 0xB2, 0xC3, 0xD4, 0xE5, 0xF6]), Some("A1:B2:C3:D4:E5:F6".to_string()));
    }
}
