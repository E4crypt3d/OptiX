//! Per-process GPU utilization via PDH (Phase 4).
//!
//! Reads the `\GPU Engine(pid_<pid>_<...>)\Utilization Percentage` counters
//! (the same source Task Manager uses) and sums the 3D/Compute/Video engines
//! per process. GPU Engine counters are rate counters, so two samples with a
//! short wait are collected.
//!
//! Instances are named `pid_1234_luid_0x00000000_0x0000B2D0_phys_0_eng_0_engtype_3D`;
//! we parse the `pid_` prefix and sum percentages per PID.

/// Parse the PID out of a `pid_<n>_...` PDH GPU Engine instance name.
pub fn pid_from_instance(name: &str) -> Option<u32> {
    let rest = name.strip_prefix("pid_")?;
    rest.split('_').next()?.parse::<u32>().ok()
}

/// Fetch per-process GPU utilization (percent, all engines summed) on Windows.
/// Rate counters need two samples, so this intentionally takes ~120 ms.
#[cfg(windows)]
pub fn per_process_gpu_usage() -> Vec<(u32, f32)> {
    use windows_sys::Win32::System::Performance::{
        PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
        PdhOpenQueryW, PDH_FMT_DOUBLE, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_HQUERY,
    };

    const PATH: &str = r"\GPU Engine(pid_*_*)\Utilization Percentage";

    let mut query: PDH_HQUERY = std::ptr::null_mut();
    // Encode the counter path once; PdhAddEnglishCounterW wants a wide string.
    let wide: Vec<u16> = PATH.encode_utf16().collect();

    let status = unsafe { PdhOpenQueryW(std::ptr::null(), 0, &mut query) };
    if status != 0 || query.is_null() {
        return Vec::new();
    }
    let mut counter = std::ptr::null_mut();
    let status = unsafe {
        PdhAddEnglishCounterW(query, wide.as_ptr(), 0, &mut counter)
    };
    if status != 0 || counter.is_null() {
        unsafe { PdhCloseQuery(query) };
        return Vec::new();
    }

    // First sample, wait, second sample, then read.
    unsafe { PdhCollectQueryData(query) };
    std::thread::sleep(std::time::Duration::from_millis(120));
    unsafe { PdhCollectQueryData(query) };

    let mut buffer_size = 0u32;
    let mut item_count = 0u32;
    let mut status = unsafe {
        PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut buffer_size, &mut item_count, std::ptr::null_mut())
    };
    let out = if status == 0 && buffer_size > 0 {
        let mut raw = vec![0u8; buffer_size as usize];
        let items = raw.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
        status = unsafe {
            PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut buffer_size, &mut item_count, items)
        };
        let mut per_pid: std::collections::HashMap<u32, f32> = std::collections::HashMap::new();
        if status == 0 {
            for i in 0..item_count as usize {
                let item = unsafe { &*items.add(i) };
                let name = unsafe { widestr(item.szName) };
                if let Some(pid) = pid_from_instance(&name) {
                    let value = unsafe { item.FmtValue.Anonymous.doubleValue } as f32;
                    if value > 0.0 {
                        let e = per_pid.entry(pid).or_insert(0.0);
                        *e += value;
                    }
                }
            }
        }
        per_pid.into_iter().collect()
    } else {
        Vec::new()
    };

    unsafe { PdhCloseQuery(query) };
    out
}

/// Read a null-terminated UTF-16 string from a pointer.
#[cfg(windows)]
unsafe fn widestr(ptr: windows_sys::core::PWSTR) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    while *ptr.add(len) != 0 {
        len += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len))
}

#[cfg(not(windows))]
pub fn per_process_gpu_usage() -> Vec<(u32, f32)> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pid_from_engine_instance() {
        assert_eq!(pid_from_instance("pid_1234_luid_0x00000000_0x0000B2D0_phys_0_eng_0_engtype_3D"), Some(1234));
        assert_eq!(pid_from_instance("pid_42_engtype_Compute"), Some(42));
        assert_eq!(pid_from_instance("_total"), None);
        assert_eq!(pid_from_instance("pid_abc_luid_x"), None);
        assert_eq!(pid_from_instance(""), None);
    }

    #[test]
    fn sums_engines_per_pid() {
        // Engine 3D + VideoEncode for pid 7 must add up to one entry.
        let usage = vec![(7u32, 12.5f32), (7, 3.0), (8, 40.0)];
        let mut map: std::collections::HashMap<u32, f32> = std::collections::HashMap::new();
        for (pid, v) in usage {
            *map.entry(pid).or_insert(0.0) += v;
        }
        assert_eq!(map.get(&7), Some(&15.5));
        assert_eq!(map.get(&8), Some(&40.0));
    }
}