//! Process classification and priority mapping (pure, cross-platform logic).

use crate::models::process::ProcessClass;
#[cfg(any(windows, test))]
use crate::models::process::PriorityClass;

/// Core Windows/system process names (lowercased). Never kill or deprioritize.
const REQUIRED_NAMES: &[&str] = &[
    "system",
    "system idle process",
    "registry",
    "memory compression",
    "smss.exe",
    "csrss.exe",
    "wininit.exe",
    "winlogon.exe",
    "services.exe",
    "lsass.exe",
    "svchost.exe",
    "explorer.exe",
    "dwm.exe",
    "fontdrvhost.exe",
    "msmpeng.exe", // Windows Defender
    "nissrv.exe",
    "securityhealthservice.exe",
    "searchindexer.exe", // Windows Search (handled as dedicated toggle in Phase 6)
    "winlogon.exe",
];

/// User apps Optix considers safe to close/limit (browsers, launchers, cloud
/// sync, updaters). User confirmation is still required before acting.
const SAFE_NAMES: &[&str] = &[
    "chrome.exe",
    "msedge.exe",
    "firefox.exe",
    "opera.exe",
    "brave.exe",
    "steam.exe",
    "steamwebhelper.exe",
    "epicgameslauncher.exe",
    "battle.net.exe",
    "discord.exe",
    "spotify.exe",
    "onedrive.exe",
    "dropbox.exe",
    "googledrivesync.exe",
    "googleupdate.exe",
    "onedriveupdater.exe",
];

/// Classify a process by its image name.
pub fn classify(name: &str) -> ProcessClass {
    let lower = name.to_ascii_lowercase();
    if REQUIRED_NAMES.contains(&lower.as_str()) {
        ProcessClass::Required
    } else if SAFE_NAMES.contains(&lower.as_str()) {
        ProcessClass::Safe
    } else {
        ProcessClass::Unknown
    }
}

/// Whether a process is a system/service process. On Windows, session 0 hosts
/// services and SYSTEM-owned processes; the name check catches the rest.
pub fn is_system_process(name: &str, session_id: Option<u32>) -> bool {
    let lower = name.to_ascii_lowercase();
    session_id == Some(0) || REQUIRED_NAMES.contains(&lower.as_str())
}

/// Memory-pressure level: how full RAM is, combined with swap (pagefile) use.
/// Pure so it is unit-testable without a live system.
///
/// - `critical`: almost no free memory (available < 4%) or RAM ≥ 92% used
///   while the swap is in use.
/// - `elevated`: RAM ≥ 80% used or ≥ 50% of the swap is consumed.
/// - `normal`: otherwise.
pub fn memory_pressure(
    used_bytes: u64,
    total_bytes: u64,
    swap_used_bytes: u64,
    swap_total_bytes: u64,
) -> &'static str {
    if total_bytes == 0 {
        return "normal";
    }
    let used_pct = used_bytes as f64 / total_bytes as f64;
    let available_pct = 1.0 - used_pct;
    let swap_pct = if swap_total_bytes > 0 {
        swap_used_bytes as f64 / swap_total_bytes as f64
    } else {
        0.0
    };
    if available_pct < 0.04 || (used_pct >= 0.92 && swap_used_bytes > 0) {
        "critical"
    } else if used_pct >= 0.80 || swap_pct >= 0.50 {
        "elevated"
    } else {
        "normal"
    }
}

/// Map a Windows `GetPriorityClass`/`SetPriorityClass` flag to our enum.
///
/// The Win32 priority-class flags are: Idle=0x40, BelowNormal=0x4000,
/// Normal=0x20, AboveNormal=0x8000, High=0x80, Realtime=0x100.
#[cfg(windows)]
pub fn priority_from_flag(flag: u32) -> PriorityClass {
    match flag {
        0x40 => PriorityClass::Idle,
        0x4000 => PriorityClass::BelowNormal,
        0x20 => PriorityClass::Normal,
        0x8000 => PriorityClass::AboveNormal,
        0x80 => PriorityClass::High,
        0x100 => PriorityClass::Realtime,
        _ => PriorityClass::Normal,
    }
}

/// Map our enum to the Win32 priority-class flag used by `SetPriorityClass`.
#[cfg(windows)]
pub fn priority_to_flag(class: PriorityClass) -> u32 {
    match class {
        PriorityClass::Idle => 0x40,
        PriorityClass::BelowNormal => 0x4000,
        PriorityClass::Normal => 0x20,
        PriorityClass::AboveNormal => 0x8000,
        PriorityClass::High => 0x80,
        PriorityClass::Realtime => 0x100,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_core_system_processes_as_required() {
        for name in [
            "System",
            "svchost.exe",
            "explorer.exe",
            "MsMpEng.exe",
            "lsass.exe",
            "dwm.exe",
        ] {
            assert_eq!(classify(name), ProcessClass::Required, "{name}");
        }
    }

    #[test]
    fn classifies_allowlisted_apps_as_safe() {
        for name in ["chrome.exe", "steam.exe", "discord.exe", "OneDrive.exe"] {
            assert_eq!(classify(name), ProcessClass::Safe, "{name}");
        }
    }

    #[test]
    fn classifies_unknown_processes_as_unknown() {
        assert_eq!(classify("mygame.exe"), ProcessClass::Unknown);
        assert_eq!(classify("randomtool.exe"), ProcessClass::Unknown);
    }

    #[test]
    fn system_processes_are_flagged() {
        assert!(is_system_process("svchost.exe", Some(0)));
        assert!(is_system_process("svchost.exe", Some(1)));
        assert!(is_system_process("System", None));
        assert!(!is_system_process("chrome.exe", Some(1)));
        // Session 0 services are always system-owned regardless of name.
        assert!(is_system_process("myservice.exe", Some(0)));
    }

    #[test]
    fn memory_pressure_thresholds() {
        let total = 16u64 * 1024 * 1024 * 1024;
        // Normal: 60% used, no swap.
        assert_eq!(
            memory_pressure(total / 10 * 6, total, 0, 0),
            "normal"
        );
        // Elevated: 85% used.
        assert_eq!(
            memory_pressure(total / 100 * 85, total, 0, 8 * 1024 * 1024 * 1024),
            "elevated"
        );
        // Elevated: half the swap consumed even at moderate RAM use.
        assert_eq!(
            memory_pressure(total / 2, total, 4 * 1024 * 1024 * 1024, 8 * 1024 * 1024 * 1024),
            "elevated"
        );
        // Critical: RAM ≥ 92% used while swap is in use.
        assert_eq!(
            memory_pressure(total / 100 * 95, total, 1024 * 1024, 8 * 1024 * 1024 * 1024),
            "critical"
        );
        // Critical: available below 4%.
        assert_eq!(
            memory_pressure(total - 100 * 1024 * 1024, total, 0, 0),
            "critical"
        );
        // Degenerate inputs never panic.
        assert_eq!(memory_pressure(0, 0, 0, 0), "normal");
    }

    #[test]
    fn realtime_is_never_settable() {
        assert!(PriorityClass::Normal.is_settable());
        assert!(PriorityClass::High.is_settable());
        assert!(!PriorityClass::Realtime.is_settable());
    }

    #[test]
    fn priority_round_trips_through_str() {
        for (s, expected) in [
            ("idle", PriorityClass::Idle),
            ("below_normal", PriorityClass::BelowNormal),
            ("normal", PriorityClass::Normal),
            ("above_normal", PriorityClass::AboveNormal),
            ("high", PriorityClass::High),
            ("realtime", PriorityClass::Realtime),
        ] {
            assert_eq!(PriorityClass::from_str(s), Some(expected));
            assert_eq!(expected.as_str(), s);
        }
        assert_eq!(PriorityClass::from_str("bogus"), None);
    }

    #[cfg(windows)]
    #[test]
    fn priority_flags_match_win32_constants() {
        use windows_sys::Win32::System::Threading::*;
        assert_eq!(priority_to_flag(PriorityClass::Idle), IDLE_PRIORITY_CLASS);
        assert_eq!(
            priority_to_flag(PriorityClass::BelowNormal),
            BELOW_NORMAL_PRIORITY_CLASS
        );
        assert_eq!(priority_to_flag(PriorityClass::Normal), NORMAL_PRIORITY_CLASS);
        assert_eq!(
            priority_to_flag(PriorityClass::AboveNormal),
            ABOVE_NORMAL_PRIORITY_CLASS
        );
        assert_eq!(priority_to_flag(PriorityClass::High), HIGH_PRIORITY_CLASS);
        assert_eq!(
            priority_to_flag(PriorityClass::Realtime),
            REALTIME_PRIORITY_CLASS
        );
    }
}
