//! Phase 8 — GPU Management.
//!
//! Gaming toggles (HAGS, GameDVR, Memory Integrity/VBS, Game Mode, MPO) are
//! registry values applied snapshot-first and reversed via the registry rollback
//! path. Shader-cache inventory walks the NVIDIA/AMD/DirectX cache directories.

use std::fs;
use std::path::PathBuf;

use crate::db::sqlite::Database;
use crate::engine::{rollback, snapshot};
use crate::error::{OptixError, Result};
use crate::models::gpu::{
    AmdShaderCache, GamingToggle, GpuAdapter, GpuToggleResult, ShaderCache,
};
use crate::models::snapshot::ChangeRecord;
use crate::win;

#[derive(Clone, Copy)]
enum EnabledWhen {
    Equals(u32),
    NotEquals(u32),
}

#[derive(Clone, Copy)]
enum Action {
    Set(u32),
    Delete,
}

struct ToggleDef {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    impact: &'static str,
    risk: &'static str,
    requires_restart: bool,
    location: &'static str,
    enabled_when: EnabledWhen,
    enable: Action,
    disable: Action,
}

const TOGGLES: &[ToggleDef] = &[
    ToggleDef {
        id: "hags",
        name: "Hardware-accelerated GPU scheduling (HAGS)",
        description: "Lets the GPU manage its own memory queue.",
        impact: "±2-3% FPS; mainly 1-3ms latency; required for Reflex/frame-gen.",
        risk: "medium",
        requires_restart: true,
        location: r"HKLM\SYSTEM\CurrentControlSet\Control\GraphicsDrivers\HwSchMode",
        enabled_when: EnabledWhen::Equals(2),
        enable: Action::Set(2),
        disable: Action::Set(1),
    },
    ToggleDef {
        id: "gamedvr",
        name: "Game DVR background recording",
        description: "Xbox Game Bar background capture buffer.",
        impact: "Frees 200-400 MB RAM and lowers input latency.",
        risk: "low",
        requires_restart: false,
        location: r"HKCU\System\GameConfigStore\GameDVR_Enabled",
        enabled_when: EnabledWhen::Equals(1),
        enable: Action::Set(1),
        disable: Action::Set(0),
    },
    ToggleDef {
        id: "gamebar_capture",
        name: "Game Bar app capture",
        description: "AppCaptureEnabled for the legacy GameDVR path.",
        impact: "Disables background clip capture.",
        risk: "low",
        requires_restart: false,
        location: r"HKCU\Software\Microsoft\Windows\CurrentVersion\GameDVR\AppCaptureEnabled",
        enabled_when: EnabledWhen::Equals(1),
        enable: Action::Set(1),
        disable: Action::Set(0),
    },
    ToggleDef {
        id: "memory_integrity",
        name: "Memory Integrity (VBS)",
        description: "Hypervisor-protected code integrity (on by default in Windows 11).",
        impact: "The biggest single FPS lever (5-15% in some titles), but reduces security.",
        risk: "high",
        requires_restart: true,
        location: r"HKLM\SYSTEM\CurrentControlSet\Control\DeviceGuard\Scenarios\HypervisorEnforcedCodeIntegrity\Enabled",
        enabled_when: EnabledWhen::Equals(1),
        enable: Action::Set(1),
        disable: Action::Set(0),
    },
    ToggleDef {
        id: "game_mode",
        name: "Game Mode",
        description: "Windows Game Mode (AutoGameMode).",
        impact: "Prioritizes the foreground game; small effect.",
        risk: "low",
        requires_restart: false,
        location: r"HKCU\Software\Microsoft\GameBar\AutoGameModeEnabled",
        enabled_when: EnabledWhen::Equals(1),
        enable: Action::Set(1),
        disable: Action::Set(0),
    },
    ToggleDef {
        id: "mpo",
        name: "Multi-Plane Overlay (MPO)",
        description: "Hardware overlay composition for windowed apps.",
        impact: "Disabling fixes flicker/black screens and some input lag on affected GPUs.",
        risk: "medium",
        requires_restart: true,
        location: r"HKLM\SOFTWARE\Microsoft\Windows\Dwm\OverlayTestMode",
        enabled_when: EnabledWhen::NotEquals(5),
        enable: Action::Delete,
        disable: Action::Set(5),
    },
];

fn interpret(enabled_when: EnabledWhen, value: Option<u32>) -> bool {
    match enabled_when {
        EnabledWhen::Equals(v) => value == Some(v),
        EnabledWhen::NotEquals(v) => value != Some(v),
    }
}

/// Current state of every gaming toggle.
pub fn list_toggles() -> Vec<GamingToggle> {
    TOGGLES
        .iter()
        .map(|t| {
            let value = win::registry::read_registry_value(t.location)
                .and_then(|s| s.parse::<u32>().ok());
            GamingToggle {
                id: t.id.to_string(),
                name: t.name.to_string(),
                description: t.description.to_string(),
                enabled: interpret(t.enabled_when, value),
                known: value.is_some(),
                impact_note: t.impact.to_string(),
                risk: t.risk.to_string(),
                requires_restart: t.requires_restart,
            }
        })
        .collect()
}

/// Apply a toggle (snapshot-first, reversible).
pub fn set_toggle(db: &Database, id: &str, enabled: bool) -> Result<GpuToggleResult> {
    let def = TOGGLES
        .iter()
        .find(|t| t.id == id)
        .ok_or_else(|| OptixError::InvalidState(format!("unknown GPU toggle: {id}")))?;
    let action = if enabled { def.enable } else { def.disable };

    let snap = snapshot::create_lightweight(db, &format!("GPU toggle: {}", def.name), None)?;
    let old = win::registry::read_registry_value(def.location);

    match action {
        Action::Set(v) => win::registry::set_registry_value(def.location, &v.to_string())?,
        Action::Delete => win::registry::delete_registry_value(def.location)?,
    }

    // Verify; on failure restore the previous value.
    let new = win::registry::read_registry_value(def.location);
    let ok = match action {
        Action::Set(v) => new.as_deref() == Some(v.to_string().as_str()),
        Action::Delete => new.is_none(),
    };
    if !ok {
        match &old {
            Some(prev) => {
                let _ = win::registry::set_registry_value(def.location, prev);
            }
            None => {
                let _ = win::registry::delete_registry_value(def.location);
            }
        }
        return Err(OptixError::Windows(
            "GPU toggle verification failed; reverted".into(),
        ));
    }

    let new_value = match action {
        Action::Set(v) => Some(v.to_string()),
        Action::Delete => None,
    };
    rollback::record_change(
        db,
        &snap.id,
        ChangeRecord {
            id: None,
            snapshot_id: String::new(),
            domain: "registry".to_string(),
            location: def.location.to_string(),
            kind: "set".to_string(),
            old_value: old,
            new_value,
            old_json: None,
            new_json: None,
            applied_at_ms: None,
            verified: true,
            rolled_back: false,
        },
    )?;

    Ok(GpuToggleResult {
        snapshot_id: snap.id,
        changes: 1,
    })
}

struct CacheDef {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    env: &'static str,
    relative: &'static str,
}

const CACHES: &[CacheDef] = &[
    CacheDef {
        id: "nvidia_dxcache",
        name: "NVIDIA DXCache",
        description: "DirectX shader cache (rebuilt on next launch).",
        env: "LOCALAPPDATA",
        relative: r"NVIDIA\DXCache",
    },
    CacheDef {
        id: "nvidia_glcache",
        name: "NVIDIA GLCache",
        description: "OpenGL shader cache.",
        env: "LOCALAPPDATA",
        relative: r"NVIDIA\GLCache",
    },
    CacheDef {
        id: "nvidia_compute",
        name: "NVIDIA ComputeCache",
        description: "Compute shader cache.",
        env: "LOCALAPPDATA",
        relative: r"NVIDIA\ComputeCache",
    },
    CacheDef {
        id: "nvidia_nvcache",
        name: "NVIDIA NV_Cache",
        description: "Per-user legacy shader cache.",
        env: "LOCALAPPDATA",
        relative: r"NVIDIA Corporation\NV_Cache",
    },
    CacheDef {
        id: "nvidia_nvcache_pd",
        name: "NVIDIA NV_Cache (global)",
        description: "Machine-wide legacy shader cache.",
        env: "PROGRAMDATA",
        relative: r"NVIDIA Corporation\NV_Cache",
    },
    CacheDef {
        id: "amd_dxcache",
        name: "AMD DxCache",
        description: "DirectX shader cache.",
        env: "LOCALAPPDATA",
        relative: r"AMD\DxCache",
    },
    CacheDef {
        id: "amd_dxccache",
        name: "AMD DxcCache",
        description: "Compute shader cache.",
        env: "LOCALAPPDATA",
        relative: r"AMD\DxcCache",
    },
    CacheDef {
        id: "amd_vkcache",
        name: "AMD VkCache",
        description: "Vulkan shader cache.",
        env: "LOCALAPPDATA",
        relative: r"AMD\VkCache",
    },
    CacheDef {
        id: "d3dscache",
        name: "DirectX D3DSCache",
        description: "System DirectX shader cache.",
        env: "LOCALAPPDATA",
        relative: "D3DSCache",
    },
];

fn cache_path(def: &CacheDef) -> Option<PathBuf> {
    let base = std::env::var_os(def.env)?;
    Some(PathBuf::from(base).join(def.relative))
}

/// Inventory shader caches with sizes (existing directories only).
pub fn scan_caches() -> Vec<ShaderCache> {
    CACHES
        .iter()
        .filter_map(|def| {
            let path = cache_path(def)?;
            if !path.is_dir() {
                return None;
            }
            let (size, count) = dir_size(&path);
            Some(ShaderCache {
                id: def.id.to_string(),
                name: def.name.to_string(),
                path: path.to_string_lossy().into_owned(),
                size_bytes: size,
                file_count: count,
                description: def.description.to_string(),
            })
        })
        .collect()
}

/// Per-cache clearing outcome (internal; recorded by the command layer).
pub struct CacheOutcome {
    pub id: String,
    pub before_bytes: u64,
    pub freed_bytes: u64,
    pub files_removed: u64,
}

/// Clear selected shader caches' contents (never the directory roots).
pub fn clear_cache_dirs(ids: &[String]) -> Vec<CacheOutcome> {
    let mut out = Vec::new();
    for def in CACHES.iter().filter(|d| ids.iter().any(|i| i == d.id)) {
        let Some(path) = cache_path(def) else {
            continue;
        };
        if !path.is_dir() {
            continue;
        }
        let (before, _) = dir_size(&path);
        let (freed, removed) = clear_dir_contents(&path);
        out.push(CacheOutcome {
            id: def.id.to_string(),
            before_bytes: before,
            freed_bytes: freed,
            files_removed: removed,
        });
    }
    out
}

/// Recursively sum file sizes and count under a directory.
fn dir_size(dir: &PathBuf) -> (u64, u64) {
    let mut size = 0u64;
    let mut count = 0u64;
    walk(dir, &mut |meta| {
        if meta.is_file() {
            size += meta.len();
            count += 1;
        }
    });
    (size, count)
}

/// Recursively remove a directory's contents (not the root itself).
fn clear_dir_contents(dir: &PathBuf) -> (u64, u64) {
    let mut freed = 0u64;
    let mut removed = 0u64;
    let Ok(entries) = fs::read_dir(dir) else {
        return (0, 0);
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            let (before, count) = dir_size(&path);
            if fs::remove_dir_all(&path).is_ok() {
                freed += before;
                removed += count;
            }
        } else if ft.is_file() {
            if let Ok(meta) = entry.metadata() {
                if fs::remove_file(&path).is_ok() {
                    freed += meta.len();
                    removed += 1;
                }
            }
        }
    }
    (freed, removed)
}

fn walk(dir: &PathBuf, visit: &mut impl FnMut(&fs::Metadata)) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        if ft.is_symlink() {
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            visit(&meta);
            if ft.is_dir() {
                walk(&path, visit);
            }
        }
    }
}

/// Detected display adapters (VRAM from WMI where available).
pub fn list_adapters() -> Vec<GpuAdapter> {
    let gpus = win::hardware::detect_gpus();
    let vram = win::wmi::video_controllers();
    gpus
        .into_iter()
        .map(|g| {
            let gpu_name = g.name.to_ascii_lowercase();
            let wmi_memory = vram
                .iter()
                .find(|v| {
                    let controller_name = v.name.to_ascii_lowercase();
                    gpu_name == controller_name
                        || (!gpu_name.is_empty()
                            && !controller_name.is_empty()
                            && (gpu_name.contains(&controller_name)
                                || controller_name.contains(&gpu_name)))
                })
                .map(|v| v.adapter_ram_bytes)
                .unwrap_or(0);
            GpuAdapter {
                name: g.name,
                vendor: g.vendor,
                driver_version: g.driver_version,
                memory_bytes: wmi_memory.max(g.memory_bytes),
            }
        })
        .collect()
}

/// Current AMD shader cache mode.
pub fn amd_shader_cache() -> AmdShaderCache {
    win::gpu::amd_shader_cache_status()
}

/// Set the AMD shader cache mode (`always_on` or `optimized`), snapshot-first
/// and reversible. The value is REG_BINARY, which the generic registry
/// rollback cannot restore, so the change is recorded under the `gpu` domain
/// and reverted by `win::gpu::rollback_gpu`.
pub fn set_amd_shader_cache(db: &Database, always_on: bool) -> Result<AmdShaderCache> {
    let location = win::gpu::amd_shader_cache_location().ok_or_else(|| {
        OptixError::Windows("no AMD adapter with a UMD key found".into())
    })?;
    let expected_mode = if always_on { "always_on" } else { "optimized" };
    if win::gpu::amd_shader_cache_status().mode == expected_mode {
        return Ok(win::gpu::amd_shader_cache_status());
    }

    let old = win::gpu::amd_shader_cache_bytes();
    let expected: Vec<u8> = if always_on { vec![0x32, 0x00] } else { vec![0x31, 0x00] };

    let snap = snapshot::create_lightweight(
        db,
        "AMD shader cache",
        Some(&format!("set AMD shader cache to {expected_mode}")),
    )?;

    win::gpu::set_amd_shader_cache(always_on)?;

    // Verify the write landed; on failure restore the previous bytes.
    if win::gpu::amd_shader_cache_bytes().as_deref() != Some(expected.as_slice()) {
        if let Some(prev) = old {
            if let Err(e) = win::gpu::write_shader_cache_bytes(&location, &prev) {
                crate::logging::error("AMD shader cache revert (restore value)", &e);
            }
        }
        return Err(OptixError::Windows(
            "AMD shader cache write verification failed; reverted".into(),
        ));
    }

    rollback::record_change(
        db,
        &snap.id,
        ChangeRecord {
            id: None,
            snapshot_id: String::new(),
            domain: "gpu".to_string(),
            location,
            kind: "set_amd_shader_cache".to_string(),
            old_value: old.map(|b| to_hex(&b)),
            new_value: Some(to_hex(&expected)),
            old_json: None,
            new_json: None,
            applied_at_ms: None,
            verified: true,
            rolled_back: false,
        },
    )?;

    Ok(win::gpu::amd_shader_cache_status())
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interprets_enabled_when() {
        assert!(interpret(EnabledWhen::Equals(1), Some(1)));
        assert!(!interpret(EnabledWhen::Equals(1), Some(0)));
        assert!(!interpret(EnabledWhen::Equals(1), None));
        // MPO: absent or any value other than 5 means "enabled".
        assert!(interpret(EnabledWhen::NotEquals(5), None));
        assert!(interpret(EnabledWhen::NotEquals(5), Some(0)));
        assert!(!interpret(EnabledWhen::NotEquals(5), Some(5)));
    }

    #[test]
    fn toggle_ids_are_unique() {
        let mut ids: Vec<&str> = TOGGLES.iter().map(|t| t.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), TOGGLES.len());
    }

    #[test]
    fn cache_ids_are_unique() {
        let mut ids: Vec<&str> = CACHES.iter().map(|c| c.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), CACHES.len());
    }

    #[test]
    fn dir_size_walks_recursively() {
        let root = std::env::temp_dir().join(format!("optix_gpu_test_{}", std::process::id()));
        let sub = root.join("sub");
        fs::create_dir_all(&sub).unwrap();
        fs::write(root.join("a.txt"), vec![0u8; 10]).unwrap();
        fs::write(sub.join("b.txt"), vec![0u8; 20]).unwrap();

        let (size, count) = dir_size(&root);
        assert_eq!(size, 30);
        assert_eq!(count, 2);

        let (freed, removed) = clear_dir_contents(&root);
        assert_eq!(freed, 30);
        assert_eq!(removed, 2);
        assert!(root.is_dir(), "root directory must remain after clearing");

        fs::remove_dir_all(&root).unwrap();
    }
}
