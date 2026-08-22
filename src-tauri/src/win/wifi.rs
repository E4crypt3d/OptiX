//! Wireless association details via the Native WiFi API (`wlanapi.dll`) —
//! locale-independent, unlike parsing `netsh wlan show interfaces`.

use crate::models::network::WifiInfo;

#[cfg(not(windows))]
pub fn connected_info(_interface_guid: &str) -> Option<WifiInfo> {
    None
}

#[cfg(windows)]
pub fn connected_info(interface_guid: &str) -> Option<WifiInfo> {
    imp::connected_info(interface_guid)
}

#[cfg(windows)]
mod imp {
    use super::WifiInfo;
    use crate::win::network::{
        auth_name, cipher_name, mac_to_string, parse_guid, phy_type_name,
        signal_quality_to_rssi,
    };
    use std::slice;
    use windows_sys::Win32::Foundation::{HANDLE, ERROR_SUCCESS};
    use windows_sys::Win32::NetworkManagement::WiFi::{
        WlanCloseHandle, WlanEnumInterfaces, WlanFreeMemory, WlanOpenHandle,
        WlanQueryInterface, WLAN_CONNECTION_ATTRIBUTES, WLAN_INTERFACE_INFO_LIST,
        WLAN_INTF_OPCODE, wlan_intf_opcode_channel_number, wlan_intf_opcode_current_connection,
        wlan_interface_state_connected,
    };

    fn query_u32(handle: HANDLE, guid: &windows_sys::core::GUID, opcode: WLAN_INTF_OPCODE) -> Option<u32> {
        let mut size = 0u32;
        let mut data = std::ptr::null_mut();
        let mut value_type = 0i32;
        unsafe {
            if WlanQueryInterface(handle, guid, opcode, std::ptr::null(), &mut size, &mut data, &mut value_type)
                != ERROR_SUCCESS
                || data.is_null()
            {
                return None;
            }
            let value = *(data as *const u32);
            WlanFreeMemory(data.cast());
            Some(value)
        }
    }

    pub(super) fn connected_info(interface_guid: &str) -> Option<WifiInfo> {
        let target = parse_guid(interface_guid)?;
        unsafe {
            let mut negotiated = 0u32;
            let mut handle: HANDLE = std::ptr::null_mut();
            // Client version 2 = Vista+ interface set; fine on Win10/11.
            if WlanOpenHandle(2, std::ptr::null(), &mut negotiated, &mut handle) != ERROR_SUCCESS {
                return None;
            }
            let result = query_connection(handle, &target);
            WlanCloseHandle(handle, std::ptr::null());
            result
        }
    }

    unsafe fn query_connection(handle: HANDLE, target: &windows_sys::core::GUID) -> Option<WifiInfo> {
        let mut list: *mut WLAN_INTERFACE_INFO_LIST = std::ptr::null_mut();
        if WlanEnumInterfaces(handle, std::ptr::null(), &mut list) != ERROR_SUCCESS || list.is_null() {
            return None;
        }
        let count = (*list).dwNumberOfItems as usize;
        let items = slice::from_raw_parts((*list).InterfaceInfo.as_ptr(), count);
        let iface = items
            .iter()
            .find(|i| i.InterfaceGuid.data1 == target.data1
                && i.InterfaceGuid.data2 == target.data2
                && i.InterfaceGuid.data3 == target.data3
                && i.InterfaceGuid.data4 == target.data4)
            .map(|i| i.InterfaceGuid);
        WlanFreeMemory(list.cast());
        let iface = iface?;

        let mut size = 0u32;
        let mut data = std::ptr::null_mut();
        let mut value_type = 0i32;
        if WlanQueryInterface(
            handle,
            &iface,
            wlan_intf_opcode_current_connection,
            std::ptr::null(),
            &mut size,
            &mut data,
            &mut value_type,
        ) != ERROR_SUCCESS
            || data.is_null()
        {
            return None;
        }
        let attrs = &*(data as *const WLAN_CONNECTION_ATTRIBUTES);
        let info = build_info(attrs);
        WlanFreeMemory(data.cast());

        let mut info = info?;
        info.channel = query_u32(handle, &iface, wlan_intf_opcode_channel_number);
        Some(info)
    }

    unsafe fn build_info(attrs: &WLAN_CONNECTION_ATTRIBUTES) -> Option<WifiInfo> {
        if attrs.isState != wlan_interface_state_connected {
            return None;
        }
        let assoc = &attrs.wlanAssociationAttributes;
        let ssid_len = assoc.dot11Ssid.uSSIDLength as usize;
        let ssid = (ssid_len > 0 && ssid_len <= 32).then(|| {
            String::from_utf8_lossy(&assoc.dot11Ssid.ucSSID[..ssid_len]).into_owned()
        });
        let bssid = mac_to_string(&assoc.dot11Bssid);
        let signal = assoc.wlanSignalQuality.min(100);
        Some(WifiInfo {
            ssid,
            bssid,
            channel: None,
            signal_percent: Some(signal),
            rssi_dbm: Some(signal_quality_to_rssi(signal)),
            phy_type: phy_type_name(assoc.dot11PhyType),
            rx_rate_mbps: (assoc.ulRxRate > 0).then(|| assoc.ulRxRate as f64 / 1000.0),
            tx_rate_mbps: (assoc.ulTxRate > 0).then(|| assoc.ulTxRate as f64 / 1000.0),
            authentication: auth_name(attrs.wlanSecurityAttributes.dot11AuthAlgorithm),
            cipher: cipher_name(attrs.wlanSecurityAttributes.dot11CipherAlgorithm),
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn rejects_malformed_guid_early() {
            assert!(connected_info("not-a-guid").is_none());
        }
    }
}
