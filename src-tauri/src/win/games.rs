//! Launcher discovery (Windows-only): where Steam, Epic, Riot, and Battle.net
//! games live. The actual file/VDF parsing is cross-platform and lives in
//! `engine::games`; this module only supplies the Windows-specific locations
//! (registry + environment paths).

use crate::models::games::DetectedGame;

/// Steam install path from `HKLM\SOFTWARE\WOW6432Node\Valve\Steam\InstallPath`.
#[cfg(windows)]
pub fn steam_install_path() -> Option<String> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SOFTWARE\WOW6432Node\Valve\Steam")
        .ok()?
        .get_value::<String, _>("InstallPath")
        .ok()
}

#[cfg(not(windows))]
pub fn steam_install_path() -> Option<String> {
    None
}

/// Directory containing Epic Games `.item` manifests.
#[cfg(windows)]
pub fn epic_manifests_dir() -> Option<String> {
    let programdata = std::env::var("PROGRAMDATA").ok()?;
    Some(format!(r"{programdata}\Epic\EpicGamesLauncher\Data\Manifests"))
}

#[cfg(not(windows))]
pub fn epic_manifests_dir() -> Option<String> {
    None
}

/// Scan the uninstall registry for products whose name/publisher contains
/// `keyword`, returning best-effort DetectedGames (install path + DisplayIcon
/// as the executable guess).
#[cfg(windows)]
fn uninstall_games(keyword: &str, launcher: &str) -> Vec<DetectedGame> {
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ};
    use winreg::RegKey;

    const ROOTS: &[&str] = &[
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
    ];

    let kw = keyword.to_lowercase();
    let mut out = Vec::new();

    for root in ROOTS {
        let Ok(base) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey_with_flags(root, KEY_READ)
        else {
            continue;
        };
        for sub in base.enum_keys().flatten() {
            let Ok(key) = base.open_subkey(&sub) else {
                continue;
            };
            let display: String = key.get_value("DisplayName").unwrap_or_default();
            if display.is_empty() {
                continue;
            }
            let publisher: String = key.get_value("Publisher").unwrap_or_default();
            if !display.to_lowercase().contains(&kw) && !publisher.to_lowercase().contains(&kw) {
                continue;
            }
            let install_path: String = key.get_value("InstallLocation").unwrap_or_default();
            let icon: String = key.get_value("DisplayIcon").unwrap_or_default();
            out.push(DetectedGame {
                name: display,
                launcher: launcher.to_string(),
                app_id: None,
                install_path,
                executable: icon.trim_matches('"').to_string(),
            });
        }
    }
    out
}

/// Riot games (VALORANT, League of Legends, …) via the uninstall registry.
#[cfg(windows)]
pub fn riot_games() -> Vec<DetectedGame> {
    uninstall_games("riot", "riot")
}

#[cfg(not(windows))]
pub fn riot_games() -> Vec<DetectedGame> {
    Vec::new()
}

/// Battle.net / Blizzard games via the uninstall registry.
#[cfg(windows)]
pub fn battlenet_games() -> Vec<DetectedGame> {
    uninstall_games("blizzard", "battlenet")
}

#[cfg(not(windows))]
pub fn battlenet_games() -> Vec<DetectedGame> {
    Vec::new()
}
