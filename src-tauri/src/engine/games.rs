//! Phase 9 — Game Profile System.
//!
//! Cross-platform detection of installed games (Steam VDF, Epic manifests,
//! Riot/Battle.net uninstall registry), the saved library + per-game profiles,
//! and live running-process matching. The auto-apply watcher lives in
//! `engine::game_watcher`.

use std::path::{Path, PathBuf};

use crate::db::sqlite::{Database, GameRow};
use crate::engine::game_watcher::GameWatcher;
use crate::error::{OptixError, Result};
use crate::models::games::{DetectedGame, Game, GameProfile};
use crate::models::process::PriorityClass;
use crate::win;

// ---------------------------------------------------------------------------
// Minimal Valve KeyValues (VDF) parser
// ---------------------------------------------------------------------------

/// A parsed VDF node: either a string or an object (ordered key/value list).
#[derive(Debug, Clone, PartialEq)]
pub enum VdfValue {
    Object(Vec<(String, VdfValue)>),
    String(String),
}

impl VdfValue {
    pub fn as_object(&self) -> Option<&[(String, VdfValue)]> {
        match self {
            VdfValue::Object(o) => Some(o),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            VdfValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn get(&self, key: &str) -> Option<&VdfValue> {
        self.as_object()?
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v)
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Str(String),
    Open,
    Close,
}

/// Parse a Valve KeyValues document into a tree. Handles nested objects, line\n/// comments (`//`), escapes, and an optional UTF-8 BOM.
pub fn parse_vdf(text: &str) -> std::result::Result<VdfValue, String> {
    let text = text.trim_start_matches('\u{feff}');
    let tokens = tokenize(text)?;
    let mut pos = 0usize;
    // A VDF document is an implicit root object of key/value pairs.
    let mut entries = Vec::new();
    while pos < tokens.len() {
        let Token::Str(key) = tokens[pos].clone() else {
            return Err("expected a key at document root".into());
        };
        pos += 1;
        let value = parse_value(&tokens, &mut pos)?;
        entries.push((key, value));
    }
    Ok(VdfValue::Object(entries))
}

fn tokenize(text: &str) -> std::result::Result<Vec<Token>, String> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i] as char;
        match c {
            '{' => {
                tokens.push(Token::Open);
                i += 1;
            }
            '}' => {
                tokens.push(Token::Close);
                i += 1;
            }
            '"' => {
                i += 1;
                let mut s = String::new();
                while i < bytes.len() {
                    let ch = bytes[i] as char;
                    if ch == '\\' && i + 1 < bytes.len() {
                        let next = bytes[i + 1] as char;
                        if next == '"' {
                            s.push('"');
                            i += 2;
                        } else if next == '\\' {
                            s.push('\\');
                            i += 2;
                        } else {
                            s.push(ch);
                            i += 1;
                        }
                    } else if ch == '"' {
                        i += 1;
                        break;
                    } else {
                        s.push(ch);
                        i += 1;
                    }
                }
                tokens.push(Token::Str(s));
            }
            '/' if i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            _ if c.is_whitespace() => {
                i += 1;
            }
            _ => return Err(format!("unexpected character '{c}' at byte {i}")),
        }
    }
    Ok(tokens)
}

fn parse_value(tokens: &[Token], pos: &mut usize) -> std::result::Result<VdfValue, String> {
    let Some(token) = tokens.get(*pos) else {
        return Err("unexpected end of input".into());
    };
    match token {
        Token::Str(s) => {
            let s = s.clone();
            *pos += 1;
            Ok(VdfValue::String(s))
        }
        Token::Open => {
            *pos += 1;
            let mut entries = Vec::new();
            loop {
                let Some(token) = tokens.get(*pos) else {
                    return Err("unclosed object".into());
                };
                if *token == Token::Close {
                    *pos += 1;
                    break;
                }
                let Token::Str(key) = token else {
                    return Err("object key must be a string".into());
                };
                let key = key.clone();
                *pos += 1;
                let value = parse_value(tokens, pos)?;
                entries.push((key, value));
            }
            Ok(VdfValue::Object(entries))
        }
        Token::Close => Err("unexpected '}'".into()),
    }
}

// ---------------------------------------------------------------------------
// Manifest parsers (pure, unit-tested)
// ---------------------------------------------------------------------------

/// Library paths from a Steam `libraryfolders.vdf` document.
pub fn steam_libraries(vdf: &str) -> Vec<String> {
    let Ok(root) = parse_vdf(vdf) else {
        return Vec::new();
    };
    let Some(libs) = root.get("libraryfolders").and_then(VdfValue::as_object) else {
        return Vec::new();
    };
    libs.iter()
        .filter_map(|(_, lib)| lib.get("path").and_then(VdfValue::as_str))
        .map(str::to_string)
        .collect()
}

/// `(app_id, name, install_dir)` from a Steam `appmanifest_*.acf` document.
pub fn appmanifest_info(acf: &str) -> Option<(String, String, String)> {
    let root = parse_vdf(acf).ok()?;
    let state = root.get("AppState")?;
    Some((
        state.get("appid")?.as_str()?.to_string(),
        state.get("name")?.as_str()?.to_string(),
        state.get("installdir")?.as_str()?.to_string(),
    ))
}

/// `(display_name, install_location, launch_executable)` from an Epic `.item`.
pub fn epic_manifest_info(json: &str) -> Option<(String, String, String)> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    Some((
        v.get("DisplayName")?.as_str()?.to_string(),
        v.get("InstallLocation")?.as_str()?.to_string(),
        v.get("LaunchExecutable")?.as_str()?.to_string(),
    ))
}

// ---------------------------------------------------------------------------
// Executable resolution + process matching
// ---------------------------------------------------------------------------

/// Basename of a path, lowercased (Windows process matching is case-insensitive).
pub fn exe_name(path: &str) -> String {
    path.rsplit(['\\', '/'])
        .next()
        .unwrap_or(path)
        .to_ascii_lowercase()
}

/// Find the most plausible game `.exe` under a directory (top level first,
/// then common bin subdirectories), skipping obvious non-game binaries.
pub fn find_executable(dir: &Path) -> Option<PathBuf> {
    const SUBDIRS: &[&str] = &["", "bin", "game", "win64", "win32", "x64", "Binaries"];
    let dirname = dir
        .file_name()
        .map(|s| s.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();

    for sub in SUBDIRS {
        let d = if sub.is_empty() {
            dir.to_path_buf()
        } else {
            dir.join(sub)
        };
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        let mut candidates = Vec::new();
        for entry in rd.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) != Some("exe") {
                continue;
            }
            let stem = p
                .file_stem()
                .map(|s| s.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            if stem.contains("unins")
                || stem.contains("crash")
                || stem.contains("setup")
                || stem.contains("redist")
                || stem.contains("vcredist")
                || stem.contains("dxsetup")
            {
                continue;
            }
            candidates.push(p);
        }
        if !candidates.is_empty() {
            return candidates.into_iter().max_by_key(|p| {
                let stem = p
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_ascii_lowercase())
                    .unwrap_or_default();
                if stem == dirname {
                    1
                } else {
                    0
                }
            });
        }
    }
    None
}

/// Match a game's executable name against running process names.
pub fn running_pids(exe_name: &str, processes: &[(String, u32)]) -> Vec<u32> {
    if exe_name.is_empty() {
        return Vec::new();
    }
    processes
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case(exe_name))
        .map(|(_, pid)| *pid)
        .collect()
}

/// Snapshot of running processes as `(name, pid)` pairs.
pub fn running_process_names() -> Vec<(String, u32)> {
    use sysinfo::{ProcessesToUpdate, System};

    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    sys.processes()
        .iter()
        .map(|(pid, p)| (p.name().to_string_lossy().into_owned(), pid.as_u32()))
        .collect()
}

// ---------------------------------------------------------------------------
// Profile helpers
// ---------------------------------------------------------------------------

/// A safe default profile for a newly-added game.
pub fn default_profile(game_id: i64) -> GameProfile {
    GameProfile {
        game_id,
        cpu_priority: "above_normal".into(),
        affinity_mask: None,
        power_profile: "none".into(),
        network_profile: "none".into(),
        cleanup_bg: true,
        gpu_profile: None,
        enabled: true,
    }
}

/// Validate a profile against the allowed values (REALTIME is rejected here).
pub fn validate_profile(p: &GameProfile) -> Result<()> {
    if !["normal", "above_normal", "high"].contains(&p.cpu_priority.as_str()) {
        return Err(OptixError::InvalidState(format!(
            "invalid cpu_priority: {}",
            p.cpu_priority
        )));
    }
    if ![
        "none",
        "balanced_gaming",
        "competitive_gaming",
        "maximum_performance",
    ]
    .contains(&p.power_profile.as_str())
    {
        return Err(OptixError::InvalidState(format!(
            "invalid power_profile: {}",
            p.power_profile
        )));
    }
    if !["none", "dns", "tcp_experimental"].contains(&p.network_profile.as_str()) {
        return Err(OptixError::InvalidState(format!(
            "invalid network_profile: {}",
            p.network_profile
        )));
    }
    if let Some(mask) = &p.affinity_mask {
        if parse_affinity(Some(mask)).is_none() {
            return Err(OptixError::InvalidState(format!(
                "invalid affinity_mask: {mask}"
            )));
        }
    }
    Ok(())
}

/// Parse a hex affinity mask (with optional `0x` prefix). `None` for empty or
/// zero masks.
pub fn parse_affinity(mask: Option<&str>) -> Option<u64> {
    let s = mask?.trim();
    let s = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    let v = u64::from_str_radix(s, 16).ok()?;
    if v == 0 {
        None
    } else {
        Some(v)
    }
}

/// Map a profile's priority string to a `PriorityClass` (never REALTIME).
pub fn priority_class(s: &str) -> PriorityClass {
    match s {
        "high" => PriorityClass::High,
        "above_normal" => PriorityClass::AboveNormal,
        _ => PriorityClass::Normal,
    }
}

// ---------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------

/// Detect games across all supported launchers (deduplicated).
pub fn detect_all() -> Vec<DetectedGame> {
    let mut out = Vec::new();
    out.extend(detect_steam());
    out.extend(detect_epic());
    out.extend(win::games::riot_games());
    out.extend(win::games::battlenet_games());
    dedup(out)
}

fn detect_steam() -> Vec<DetectedGame> {
    let Some(steam) = win::games::steam_install_path() else {
        return Vec::new();
    };
    let vdf_path = Path::new(&steam).join("steamapps").join("libraryfolders.vdf");
    let Ok(text) = std::fs::read_to_string(&vdf_path) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for lib in steam_libraries(&text) {
        let apps = Path::new(&lib).join("steamapps");
        let Ok(rd) = std::fs::read_dir(&apps) else {
            continue;
        };
        for entry in rd.flatten() {
            let p = entry.path();
            let fname = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if !fname.starts_with("appmanifest_") || !fname.ends_with(".acf") {
                continue;
            }
            let Ok(acf) = std::fs::read_to_string(&p) else {
                continue;
            };
            let Some((app_id, name, install_dir)) = appmanifest_info(&acf) else {
                continue;
            };
            let install_path = Path::new(&lib)
                .join("steamapps")
                .join("common")
                .join(&install_dir);
            let executable = find_executable(&install_path)
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            out.push(DetectedGame {
                name,
                launcher: "steam".into(),
                app_id: Some(app_id),
                install_path: install_path.to_string_lossy().into_owned(),
                executable,
            });
        }
    }
    out
}

fn detect_epic() -> Vec<DetectedGame> {
    let Some(dir) = win::games::epic_manifests_dir() else {
        return Vec::new();
    };
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for entry in rd.flatten() {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) != Some("item") {
            continue;
        }
        let Ok(json) = std::fs::read_to_string(&p) else {
            continue;
        };
        let Some((name, install_location, launch_exe)) = epic_manifest_info(&json) else {
            continue;
        };
        let executable = if !launch_exe.is_empty() {
            Path::new(&install_location)
                .join(&launch_exe)
                .to_string_lossy()
                .into_owned()
        } else {
            find_executable(Path::new(&install_location))
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        out.push(DetectedGame {
            name,
            launcher: "epic".into(),
            app_id: None,
            install_path: install_location,
            executable,
        });
    }
    out
}

/// Remove duplicates (same executable name, else same lowercase name).
fn dedup(mut games: Vec<DetectedGame>) -> Vec<DetectedGame> {
    let key_of = |g: &DetectedGame| {
        if g.executable.is_empty() {
            g.name.to_lowercase()
        } else {
            exe_name(&g.executable)
        }
    };
    let mut out: Vec<DetectedGame> = Vec::new();
    for g in games.drain(..) {
        let key = key_of(&g);
        if out.iter().any(|e| key_of(e) == key) {
            continue;
        }
        out.push(g);
    }
    out
}

// ---------------------------------------------------------------------------
// Library
// ---------------------------------------------------------------------------

/// Map a DB row to a `Game` (without live running state).
pub fn row_to_game(row: &GameRow) -> Game {
    Game {
        id: row.id,
        name: row.name.clone(),
        launcher: row.launcher.clone(),
        app_id: row.app_id.clone(),
        install_path: row.install_path.clone(),
        executable: row.executable.clone(),
        exe_name: exe_name(&row.executable),
        last_played: row.last_played,
        detected_at: row.detected_at,
        running: false,
        pids: Vec::new(),
        boosted: false,
    }
}

/// List saved games, annotated with live running/boosted state.
pub fn list_games(db: &Database, watcher: Option<&GameWatcher>) -> Result<Vec<Game>> {
    let rows = db.list_games()?;
    let processes = running_process_names();
    let mut out = Vec::new();
    for row in rows {
        let mut g = row_to_game(&row);
        let pids = running_pids(&g.exe_name, &processes);
        g.running = !pids.is_empty();
        g.pids = pids;
        if let Some(w) = watcher {
            g.boosted = w.is_active(row.id);
        }
        out.push(g);
    }
    Ok(out)
}

/// Add a game to the library (deduplicating by launcher+app_id or executable),
/// creating a default profile if none exists.
pub fn add_game(
    db: &Database,
    launcher: &str,
    app_id: Option<&str>,
    name: &str,
    install_path: &str,
    executable: &str,
) -> Result<Game> {
    let exe = exe_name(executable);
    let rows = db.list_games()?;
    let existing = rows.iter().find(|r| {
        (launcher == r.launcher && app_id.is_some() && r.app_id.as_deref() == app_id)
            || (!exe.is_empty() && exe_name(&r.executable) == exe)
    });

    let id = match existing {
        Some(e) => e.id,
        None => db.insert_game(
            name,
            launcher,
            app_id,
            install_path,
            executable,
            super::now_ms() as i64,
        )?,
    };

    if db.get_game_profile(id)?.is_none() {
        db.save_game_profile(&default_profile(id))?;
    }

    let row = db
        .get_game(id)?
        .ok_or_else(|| OptixError::InvalidState("game vanished after insert".into()))?;
    Ok(row_to_game(&row))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_vdf() {
        let vdf = r#"
"libraryfolders"
{
    "0"
    {
        "path" "D:\\SteamLibrary"
        "apps"
        {
            "730" "123"
        }
    }
}
"#;
        let root = parse_vdf(vdf).unwrap();
        let path = root
            .get("libraryfolders")
            .unwrap()
            .get("0")
            .unwrap()
            .get("path")
            .unwrap()
            .as_str()
            .unwrap();
        assert_eq!(path, "D:\\SteamLibrary");
    }

    #[test]
    fn handles_escapes_and_comments() {
        let vdf = "// leading comment\n\"key\" \"va\\\"lue\"\n\"other\" \"plain\"";
        let root = parse_vdf(vdf).unwrap();
        assert_eq!(root.get("key").unwrap().as_str().unwrap(), "va\"lue");
        assert_eq!(root.get("other").unwrap().as_str().unwrap(), "plain");
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_vdf("hello world").is_err());
        assert!(parse_vdf("\"unclosed").is_err());
    }

    #[test]
    fn extracts_steam_libraries() {
        let vdf = r#""libraryfolders"
{
    "0" { "path" "C:\\Steam" }
    "1" { "path" "D:\\Games" }
}"#;
        assert_eq!(steam_libraries(vdf), vec!["C:\\Steam", "D:\\Games"]);
    }

    #[test]
    fn parses_appmanifest() {
        let acf = r#""AppState"
{
    "appid" "730"
    "name" "Counter-Strike 2"
    "installdir" "Counter-Strike 2"
}"#;
        assert_eq!(
            appmanifest_info(acf),
            Some(("730".into(), "Counter-Strike 2".into(), "Counter-Strike 2".into()))
        );
    }

    #[test]
    fn parses_epic_manifest() {
        let json = r#"{"DisplayName":"Fortnite","InstallLocation":"C:\\Epic\\Fortnite","LaunchExecutable":"FortniteGame\\Binaries\\Win64\\FortniteClient-Win64-Shipping.exe"}"#;
        assert_eq!(
            epic_manifest_info(json).map(|(n, l, e)| (n, l, e.contains("FortniteClient"))),
            Some(("Fortnite".into(), "C:\\Epic\\Fortnite".into(), true))
        );
    }

    #[test]
    fn exe_name_is_basename_lowercased() {
        assert_eq!(exe_name(r"C:\Games\CS2\game\bin\win64\cs2.exe"), "cs2.exe");
        assert_eq!(exe_name("/usr/bin/Game.EXE"), "game.exe");
        assert_eq!(exe_name(""), "");
    }

    #[test]
    fn matches_running_processes_case_insensitively() {
        let processes = vec![
            ("cs2.exe".to_string(), 100),
            ("steam.exe".to_string(), 200),
            ("CS2.EXE".to_string(), 300),
        ];
        assert_eq!(running_pids("cs2.exe", &processes), vec![100, 300]);
        assert!(running_pids("", &processes).is_empty());
        assert!(running_pids("fortnite.exe", &processes).is_empty());
    }

    #[test]
    fn parses_affinity_masks() {
        assert_eq!(parse_affinity(Some("0x5555")), Some(0x5555));
        assert_eq!(parse_affinity(Some("F")), Some(15));
        assert_eq!(parse_affinity(Some("0x0")), None);
        assert_eq!(parse_affinity(None), None);
        assert_eq!(parse_affinity(Some("zz")), None);
    }

    #[test]
    fn priority_never_realtime() {
        assert_eq!(priority_class("high"), PriorityClass::High);
        assert_eq!(priority_class("above_normal"), PriorityClass::AboveNormal);
        assert_eq!(priority_class("bogus"), PriorityClass::Normal);
        assert_eq!(priority_class("realtime"), PriorityClass::Normal);
    }

    #[test]
    fn validates_profiles() {
        let mut p = default_profile(1);
        assert!(validate_profile(&p).is_ok());
        p.cpu_priority = "realtime".into();
        assert!(validate_profile(&p).is_err());
        p.cpu_priority = "above_normal".into();
        p.affinity_mask = Some("not-hex".into());
        assert!(validate_profile(&p).is_err());
        p.affinity_mask = Some("0xF0F".into());
        assert!(validate_profile(&p).is_ok());
    }

    #[test]
    fn dedups_by_executable_or_name() {
        let mut games = vec![
            DetectedGame {
                name: "CS2 (Steam)".into(),
                launcher: "steam".into(),
                app_id: Some("730".into()),
                install_path: "C:\\A".into(),
                executable: "C:\\A\\cs2.exe".into(),
            },
            DetectedGame {
                name: "CS2 (Epic)".into(),
                launcher: "epic".into(),
                app_id: None,
                install_path: "C:\\B".into(),
                executable: "C:\\B\\cs2.exe".into(),
            },
            DetectedGame {
                name: "Unique Game".into(),
                launcher: "steam".into(),
                app_id: Some("1".into()),
                install_path: "C:\\C".into(),
                executable: "".into(),
            },
        ];
        let kept = dedup(std::mem::take(&mut games));
        assert_eq!(kept.len(), 2);
    }
}
