use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::error::{OptixError, Result};
use crate::models::cleanup::CleanupCategory;

/// Per-category deletion outcome (internal).
pub struct CategoryOutcome {
    pub id: String,
    pub before_bytes: u64,
    pub freed_bytes: u64,
    pub files_removed: u64,
    pub files_skipped: u64,
}

#[derive(Clone, Copy, PartialEq)]
enum Safety {
    Safe,
    Caution,
}

impl Safety {
    fn as_str(&self) -> &'static str {
        match self {
            Safety::Safe => "safe",
            Safety::Caution => "caution",
        }
    }
}

/// Deletion policy for a category.
#[derive(Clone, Copy)]
enum Policy {
    All,
    /// Keep the N most-recently-modified files.
    KeepNewest(usize),
    /// Only files older than N days.
    OlderThanDays(u64),
}

struct CategoryDef {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    safety: Safety,
    expected_rebuild: bool,
    policy: Policy,
}

const CATEGORIES: &[CategoryDef] = &[
    CategoryDef {
        id: "user_temp",
        name: "User temp",
        description: "Temporary files in the user temp directory.",
        safety: Safety::Safe,
        expected_rebuild: false,
        policy: Policy::All,
    },
    CategoryDef {
        id: "windows_temp",
        name: "Windows temp",
        description: "Temporary files in C:\\Windows\\Temp.",
        safety: Safety::Safe,
        expected_rebuild: false,
        policy: Policy::All,
    },
    CategoryDef {
        id: "browser_cache",
        name: "Browser cache",
        description: "Chrome, Edge, and Firefox web caches.",
        safety: Safety::Safe,
        expected_rebuild: false,
        policy: Policy::All,
    },
    CategoryDef {
        id: "shader_cache",
        name: "GPU shader cache",
        description: "NVIDIA, AMD, and DirectX shader caches. Rebuilt on next game launch.",
        safety: Safety::Safe,
        expected_rebuild: true,
        policy: Policy::All,
    },
    CategoryDef {
        id: "crash_dumps",
        name: "Crash dumps",
        description: "Application and system minidumps (keeps the newest).",
        safety: Safety::Safe,
        expected_rebuild: false,
        policy: Policy::KeepNewest(1),
    },
    CategoryDef {
        id: "app_logs",
        name: "Application logs",
        description: "Log files older than 30 days.",
        safety: Safety::Caution,
        expected_rebuild: false,
        policy: Policy::OlderThanDays(30),
    },
    CategoryDef {
        id: "update_leftovers",
        name: "Update leftovers",
        description: "Windows Update download cache and Delivery Optimization cache. Skipped while Windows Update is running.",
        safety: Safety::Caution,
        expected_rebuild: false,
        policy: Policy::All,
    },
    CategoryDef {
        id: "recycle_bin",
        name: "Recycle Bin",
        description: "Empties the Recycle Bin. Off by default; use Disk Cleanup semantics.",
        safety: Safety::Caution,
        expected_rebuild: false,
        policy: Policy::All, // handled specially in scan/delete
    },
];

struct FileEntry {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
}

/// Scan every category and return its computed size and file count.
pub fn scan() -> Vec<CleanupCategory> {
    CATEGORIES
        .iter()
        .map(|def| {
            if def.id == "recycle_bin" {
                let (bytes, items) = crate::win::cleanup::recycle_bin_size();
                return CleanupCategory {
                    id: def.id.to_string(),
                    name: def.name.to_string(),
                    description: def.description.to_string(),
                    safety: def.safety.as_str().to_string(),
                    size_bytes: bytes,
                    file_count: items,
                    expected_rebuild: def.expected_rebuild,
                };
            }
            // Update leftovers are reported as empty while Windows Update is
            // mid-run (files are locked anyway).
            if def.id == "update_leftovers" && crate::win::cleanup::update_service_busy() {
                return CleanupCategory {
                    id: def.id.to_string(),
                    name: def.name.to_string(),
                    description: def.description.to_string(),
                    safety: def.safety.as_str().to_string(),
                    size_bytes: 0,
                    file_count: 0,
                    expected_rebuild: def.expected_rebuild,
                };
            }
            let entries = collect(def);
            let selected = apply_policy(&entries, def.policy);
            let size = selected
                .iter()
                .fold(0u64, |total, entry| total.saturating_add(entry.size));
            CleanupCategory {
                id: def.id.to_string(),
                name: def.name.to_string(),
                description: def.description.to_string(),
                safety: def.safety.as_str().to_string(),
                size_bytes: size,
                file_count: selected.len() as u64,
                expected_rebuild: def.expected_rebuild,
            }
        })
        .filter(|category| category.file_count > 0 || category.size_bytes > 0)
        .collect()
}

/// Validate category IDs at the backend boundary before creating a snapshot or
/// touching the filesystem. The frontend is not a security boundary.
pub fn validate_ids(ids: &[String]) -> Result<()> {
    if ids.is_empty() {
        return Err(OptixError::InvalidState(
            "select at least one cleanup category".to_string(),
        ));
    }
    if let Some(id) = ids
        .iter()
        .find(|id| !CATEGORIES.iter().any(|category| category.id == id.as_str()))
    {
        return Err(OptixError::InvalidState(format!(
            "unknown cleanup category: {id}"
        )));
    }
    Ok(())
}

/// Delete the selected categories' files. Never removes directory roots, only
/// their contents; in-use/locked files are skipped, not fatal.
pub fn delete_categories(ids: &[String]) -> Result<Vec<CategoryOutcome>> {
    let mut outcomes = Vec::new();
    for def in CATEGORIES.iter().filter(|d| ids.iter().any(|i| i == d.id)) {
        // Recycle Bin empties via the shell API (file deletion would corrupt
        // the $I metadata); freed bytes = size before minus size after.
        if def.id == "recycle_bin" {
            let (before_bytes, before_items) = crate::win::cleanup::recycle_bin_size();
            let mut removed = 0u64;
            if before_items > 0 {
                match crate::win::cleanup::empty_recycle_bin() {
                    Ok(()) => removed = before_items,
                    Err(e) => crate::logging::warn(&format!("recycle bin empty failed: {e}")),
                }
            }
            let (after_bytes, _) = crate::win::cleanup::recycle_bin_size();
            outcomes.push(CategoryOutcome {
                id: def.id.to_string(),
                before_bytes,
                freed_bytes: before_bytes.saturating_sub(after_bytes),
                files_removed: removed,
                files_skipped: before_items.saturating_sub(removed),
            });
            continue;
        }
        // Never touch the Update cache while the update service is running.
        if def.id == "update_leftovers" && crate::win::cleanup::update_service_busy() {
            continue;
        }
        let entries = collect(def);
        let selected = apply_policy(&entries, def.policy);
        let before_bytes = selected
            .iter()
            .fold(0u64, |total, entry| total.saturating_add(entry.size));

        let mut freed = 0u64;
        let mut removed = 0u64;
        let mut skipped = 0u64;
        for e in &selected {
            match fs::remove_file(&e.path) {
                Ok(()) => {
                    freed = freed.saturating_add(e.size);
                    removed = removed.saturating_add(1);
                }
                Err(_) => skipped = skipped.saturating_add(1),
            }
        }

        outcomes.push(CategoryOutcome {
            id: def.id.to_string(),
            before_bytes,
            freed_bytes: freed,
            files_removed: removed,
            files_skipped: skipped,
        });
    }
    Ok(outcomes)
}

fn collect(def: &CategoryDef) -> Vec<FileEntry> {
    let mut out = Vec::new();
    for path in category_paths(def.id) {
        if is_protected(&path) {
            continue;
        }
        collect_dir(&path, &mut out);
    }
    out
}

fn collect_dir(dir: &Path, out: &mut Vec<FileEntry>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if ft.is_symlink() || !owned_by_current_user(&entry) {
            continue;
        }
        if ft.is_dir() {
            if !is_protected(&path) {
                collect_dir(&path, out);
            }
        } else if ft.is_file() {
            if let Ok(meta) = entry.metadata() {
                out.push(FileEntry {
                    path,
                    size: meta.len(),
                    modified: meta.modified().ok(),
                });
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn owned_by_current_user(entry: &fs::DirEntry) -> bool {
    use std::os::unix::fs::MetadataExt;

    let Some(uid) = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status
                .lines()
                .find(|line| line.starts_with("Uid:"))
                .and_then(|line| line.split_whitespace().nth(1))
                .and_then(|value| value.parse::<u32>().ok())
        })
    else {
        return false;
    };
    entry.metadata().map(|metadata| metadata.uid() == uid).unwrap_or(false)
}

#[cfg(all(unix, not(target_os = "linux")))]
fn owned_by_current_user(_entry: &fs::DirEntry) -> bool {
    true
}

#[cfg(not(unix))]
fn owned_by_current_user(_entry: &fs::DirEntry) -> bool {
    true
}

fn apply_policy(entries: &[FileEntry], policy: Policy) -> Vec<&FileEntry> {
    match policy {
        Policy::All => entries.iter().collect(),
        Policy::KeepNewest(n) => {
            let mut idx: Vec<usize> = (0..entries.len()).collect();
            idx.sort_by(|&a, &b| {
                let (ma, mb) = (entries[a].modified, entries[b].modified);
                mb.cmp(&ma)
            });
            idx.into_iter().skip(n).map(|i| &entries[i]).collect()
        }
        Policy::OlderThanDays(days) => {
            let cutoff = SystemTime::now() - Duration::from_secs(days * 86_400);
            entries
                .iter()
                .filter(|e| e.modified.map(|m| m < cutoff).unwrap_or(false))
                .collect()
        }
    }
}

fn category_paths(id: &str) -> Vec<PathBuf> {
    match id {
        "user_temp" => user_temp_paths(),
        "windows_temp" => windows_base()
            .map(|w| w.join("Temp"))
            .filter(|p| p.is_dir())
            .into_iter()
            .collect(),
        "browser_cache" => browser_cache_paths(),
        "shader_cache" => shader_cache_paths(),
        "crash_dumps" => crash_dump_paths(),
        "app_logs" => app_log_paths(),
        "update_leftovers" => update_leftover_paths(),
        // Recycle Bin is handled by the shell API, not the file walk.
        "recycle_bin" => Vec::new(),
        _ => Vec::new(),
    }
}

fn windows_base() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("WINDIR")
            .map(PathBuf::from)
            .or_else(|| Some(PathBuf::from(r"C:\Windows")))
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(windows)]
fn local_app_data() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
}

#[cfg(windows)]
fn profile_subdirs(base: &Path, subs: &[&str]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(base) else {
        return out;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if !p.is_dir() {
            continue;
        }
        for s in subs {
            let c = p.join(s);
            if c.is_dir() {
                out.push(c);
            }
        }
    }
    out
}

fn user_temp_paths() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        vec![std::env::temp_dir()]
    }
    #[cfg(not(windows))]
    {
        // `/tmp` is shared by every process and may contain files belonging to
        // other users or currently running applications. Only clean an
        // explicitly configured, user-owned temp directory on Linux/Unix.
        let Some(temp) = std::env::var_os("TMPDIR").map(PathBuf::from) else {
            return Vec::new();
        };
        let home = std::env::var_os("HOME").map(PathBuf::from);
        if temp.is_dir() && home.as_ref().is_some_and(|home| temp.starts_with(home)) {
            vec![temp]
        } else {
            Vec::new()
        }
    }
}

fn browser_cache_paths() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        let mut out = Vec::new();
        if let Some(lad) = local_app_data() {
            for (base, subs) in [
                (
                    lad.join(r"Google\Chrome\User Data"),
                    vec!["Cache", "Code Cache"],
                ),
                (
                    lad.join(r"Microsoft\Edge\User Data"),
                    vec!["Cache", "Code Cache"],
                ),
                (lad.join(r"Mozilla\Firefox\Profiles"), vec!["cache2"]),
            ] {
                out.extend(profile_subdirs(&base, &subs));
            }
        }
        out
    }
    #[cfg(not(windows))]
    {
        let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
            return Vec::new();
        };
        let cache = home.join(".cache");
        let mut out = Vec::new();
        for browser in ["google-chrome", "chromium", "microsoft-edge"] {
            let base = cache.join(browser);
            if let Ok(profiles) = fs::read_dir(base) {
                for profile in profiles.flatten() {
                    if !profile.path().is_dir() {
                        continue;
                    }
                    for name in ["Cache", "Code Cache", "GPUCache"] {
                        let path = profile.path().join(name);
                        if path.is_dir() {
                            out.push(path);
                        }
                    }
                }
            }
        }
        let firefox = cache.join("mozilla/firefox");
        if let Ok(profiles) = fs::read_dir(firefox) {
            for profile in profiles.flatten() {
                let path = profile.path().join("cache2");
                if path.is_dir() {
                    out.push(path);
                }
            }
        }
        out
    }
}

fn shader_cache_paths() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        let mut out = Vec::new();
        if let Some(lad) = local_app_data() {
            for p in [
                lad.join(r"NVIDIA\DXCache"),
                lad.join(r"NVIDIA\GLCache"),
                lad.join(r"NVIDIA\ComputeCache"),
                lad.join(r"NVIDIA Corporation\NV_Cache"),
                lad.join(r"AMD\DxCache"),
                lad.join(r"AMD\DxcCache"),
                lad.join(r"AMD\VkCache"),
                lad.join("D3DSCache"),
            ] {
                if p.is_dir() {
                    out.push(p);
                }
            }
        }
        out
    }
    #[cfg(not(windows))]
    {
        let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
            return Vec::new();
        };
        [
            home.join(".cache/mesa_shader_cache"),
            home.join(".cache/mesa_shader_cache_db"),
            home.join(".cache/nvidia/GLCache"),
            home.join(".nv/ComputeCache"),
            home.join(".cache/AMD/DxCache"),
        ]
        .into_iter()
        .filter(|path| path.is_dir())
        .collect()
    }
}

fn crash_dump_paths() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        let mut out = Vec::new();
        if let Some(lad) = local_app_data() {
            let p = lad.join("CrashDumps");
            if p.is_dir() {
                out.push(p);
            }
        }
        if let Some(w) = windows_base() {
            let m = w.join("Minidump");
            if m.is_dir() {
                out.push(m);
            }
        }
        out
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

/// Windows Update download cache + Delivery Optimization cache (contents only;
/// Disk-Cleanup semantics apply: skip while `wuauserv` is mid-update — both
/// enforced by the scan/delete special cases).
fn update_leftover_paths() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        let mut out = Vec::new();
        if let Some(w) = windows_base() {
            let sw = w.join(r"SoftwareDistribution\Download");
            if sw.is_dir() {
                out.push(sw);
            }
        }
        if let Some(pd) = std::env::var_os("PROGRAMDATA") {
            let do_cache = PathBuf::from(&pd).join(r"Microsoft\Windows\DeliveryOptimization\Cache");
            if do_cache.is_dir() {
                out.push(do_cache);
            }
        }
        out
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

fn app_log_paths() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        let mut out = Vec::new();
        if let Some(lad) = local_app_data() {
            if let Ok(entries) = fs::read_dir(&lad) {
                for entry in entries.flatten() {
                    let logs = entry.path().join("Logs");
                    if logs.is_dir() {
                        out.push(logs);
                    }
                }
            }
        }
        out
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

/// Protected directories Optix must never scan or delete.
const PROTECTED: &[&str] = &[
    "system32",
    "winsxs",
    "driverstore",
    "windows\\installer",
    "program files",
    "program files (x86)",
];

fn is_protected(path: &Path) -> bool {
    let normalized = path.to_string_lossy().to_lowercase().replace('/', "\\");
    PROTECTED.iter().any(|p| normalized.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deny_list_blocks_protected_paths() {
        assert!(is_protected(Path::new(r"C:\Windows\System32\foo.dll")));
        assert!(is_protected(Path::new(r"C:\Windows\WinSxS\amd64")));
        assert!(is_protected(Path::new(r"C:\Windows\System32\DriverStore\x")));
        assert!(is_protected(Path::new(r"C:\Windows\Installer\{guid}")));
        assert!(is_protected(Path::new(r"C:\Program Files\App")));
        assert!(is_protected(Path::new(r"C:\Program Files (x86)\App")));
        assert!(!is_protected(Path::new(r"C:\Windows\Temp")));
        assert!(!is_protected(Path::new("/tmp")));
    }

    #[test]
    fn keep_newest_skips_most_recent() {
        let now = SystemTime::now();
        let entries = vec![
            FileEntry { path: "new".into(), size: 10, modified: Some(now) },
            FileEntry { path: "old1".into(), size: 20, modified: Some(now - Duration::from_secs(10)) },
            FileEntry { path: "old2".into(), size: 30, modified: Some(now - Duration::from_secs(20)) },
        ];
        let selected = apply_policy(&entries, Policy::KeepNewest(1));
        assert_eq!(selected.len(), 2);
        assert!(selected.iter().all(|e| e.path != PathBuf::from("new")));
    }

    #[test]
    fn validates_cleanup_ids_before_mutation() {
        assert!(validate_ids(&[]).is_err());
        assert!(validate_ids(&["not_a_category".to_string()]).is_err());
        assert!(validate_ids(&["user_temp".to_string(), "shader_cache".to_string()]).is_ok());
    }

    #[test]
    fn older_than_filters_by_age() {
        let now = SystemTime::now();
        let entries = vec![
            FileEntry { path: "old".into(), size: 1, modified: Some(now - Duration::from_secs(60 * 86_400)) },
            FileEntry { path: "new".into(), size: 1, modified: Some(now) },
        ];
        let selected = apply_policy(&entries, Policy::OlderThanDays(30));
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].path, PathBuf::from("old"));
    }
}
