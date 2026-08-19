//! Startup application enumeration (Windows-only): registry Run keys (with
//! Task Manager's `StartupApproved` disabled state) and the user/common
//! Startup folders.

use crate::models::services::StartupEntry;

/// Registry Run-key sources: (hive, label, subkey, approved-scope).
#[cfg(windows)]
const RUN_SOURCES: &[(&str, &str, &str)] = &[
    ("HKCU", r"Software\Microsoft\Windows\CurrentVersion\Run", "Run"),
    ("HKLM", r"Software\Microsoft\Windows\CurrentVersion\Run", "Run"),
    (
        "HKLM",
        r"Software\Wow6432Node\Microsoft\Windows\CurrentVersion\Run",
        "Run32",
    ),
];

/// Read Task Manager's disabled/enabled state for a Run entry. Absent (or
/// malformed) values default to enabled.
#[cfg(windows)]
fn startup_approved(hive: winreg::HKEY, scope: &str, value_name: &str) -> bool {
    use winreg::RegKey;

    let approved_key = format!(
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\{scope}"
    );
    let Ok(key) = RegKey::predef(hive).open_subkey(&approved_key) else {
        return true;
    };
    let Ok(raw) = key.get_raw_value(value_name) else {
        return true;
    };
    // 12-byte value: u32 enabled flag (2 = enabled) + FILETIME.
    if raw.bytes.len() < 4 {
        return true;
    }
    let flag = u32::from_le_bytes([raw.bytes[0], raw.bytes[1], raw.bytes[2], raw.bytes[3]]);
    flag == 2
}

/// Enumerate startup applications (registry Run keys + startup folders).
#[cfg(windows)]
pub fn list_entries() -> Vec<StartupEntry> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    let mut out = Vec::new();

    for (label, subkey, scope) in RUN_SOURCES {
        let hive = if *label == "HKCU" {
            HKEY_CURRENT_USER
        } else {
            HKEY_LOCAL_MACHINE
        };
        let Ok(key) = RegKey::predef(hive).open_subkey(subkey) else {
            continue;
        };
        for value_name in key.enum_values().flatten().map(|(n, _)| n) {
            let command: String = key.get_value(&value_name).unwrap_or_default();
            let location = format!(r"{label}\{subkey}\{value_name}");
            let enabled = startup_approved(hive, scope, &value_name);
            out.push(StartupEntry {
                id: location.clone(),
                name: value_name.clone(),
                command,
                location,
                source: "registry".to_string(),
                enabled,
                toggleable: true,
            });
        }
    }

    // Startup folders (listed read-only; the .lnk/exe contents aren't modified).
    let folders = [
        std::env::var("APPDATA").map(|p| {
            format!(r"{p}\Microsoft\Windows\Start Menu\Programs\Startup")
        }),
        std::env::var("PROGRAMDATA").map(|p| {
            format!(r"{p}\Microsoft\Windows\Start Menu\Programs\Startup")
        }),
    ];
    for folder in folders.into_iter().flatten() {
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            out.push(StartupEntry {
                id: path.to_string_lossy().into_owned(),
                name,
                command: path.to_string_lossy().into_owned(),
                location: path.to_string_lossy().into_owned(),
                source: "startup_folder".to_string(),
                enabled: true,
                toggleable: false,
            });
        }
    }

    out
}

#[cfg(not(windows))]
pub fn list_entries() -> Vec<StartupEntry> {
    Vec::new()
}
