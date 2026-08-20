//! Phase 5 — Power Management.
//!
//! Optix power profiles are clones of built-in schemes (never edited in
//! place), with processor min/max state pinned and PCIe ASPM + USB selective
//! suspend disabled. Network-adapter power saving (Energy Efficient Ethernet,
//! Green Ethernet, device power management) is disabled via registry writes
//! that reuse the registry rollback path.

use crate::db::sqlite::Database;
use crate::engine::{rollback, snapshot};
use crate::error::{OptixError, Result};
use crate::models::power::{
    ActivePowerState, NicPowerResult, PowerApplyResult, PowerPreview, PowerProfile, PowerSettingState,
};
use crate::models::snapshot::ChangeRecord;
use crate::win;

// Power subgroup / setting GUIDs (stable across Windows 10/11).
const SUB_PROCESSOR: &str = "54533251-82be-4824-96c1-47b60b740d00";
const SET_MIN_STATE: &str = "893dee8e-2bef-41e0-89c6-b55d0929964c";
const SET_MAX_STATE: &str = "bc5038f7-23e0-4960-96da-33abaf5935ec";
const SUB_PCIE: &str = "501a4d13-42af-4429-9fd1-a8218c268e20";
const SET_ASPM: &str = "ee12f906-d277-404b-b6da-e5fa1a576df5";
const SUB_USB: &str = "2a737441-1930-4402-8d77-b2bebba308a3";
const SET_SELECTIVE_SUSPEND: &str = "48e6b7a6-50f5-4782-a5d4-53bb8f07e226";

// Base (built-in) scheme GUIDs we clone from.
const BASE_BALANCED: &str = "381b4222-f694-41f0-9685-ff5bb260df2e";
const BASE_HIGH_PERF: &str = "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c";
const BASE_ULTIMATE: &str = "e9a42b02-d5df-432d-aa00-6a11a9fd3e6e";

/// Registry path prefix for network adapter class keys (used in change-record
/// locations so the registry rollback path can restore them).
const NET_CLASS_HKLM: &str =
    r"HKLM\SYSTEM\CurrentControlSet\Control\Class\{4d36e972-e325-11ce-bfc1-08002be10318}";

/// `PnPCapabilities` value that stops Windows from powering the device off.
const PNP_NO_POWER_OFF: u32 = 24;

struct ProfileDef {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    base: &'static str,
    note: &'static str,
}

const PROFILES: &[ProfileDef] = &[
    ProfileDef {
        id: "balanced_gaming",
        name: "Balanced Gaming",
        description: "Balanced base with the processor pinned high on AC and link power saving off.",
        base: BASE_BALANCED,
        note: "Keeps other Balanced timers; best all-rounder for desktops.",
    },
    ProfileDef {
        id: "competitive_gaming",
        name: "Competitive Gaming",
        description: "High performance base with processor, PCIe and USB power saving disabled.",
        base: BASE_HIGH_PERF,
        note: "Prioritizes low latency; uses more power and heat on battery.",
    },
    ProfileDef {
        id: "maximum_performance",
        name: "Maximum Performance",
        description: "Ultimate Performance base (hidden plan) with all power saving off.",
        base: BASE_ULTIMATE,
        note: "Requires the Ultimate Performance scheme; not present on every PC.",
    },
];

/// One tracked power setting: subgroup/setting GUIDs, a display label, and the
/// AC value Optix gaming profiles write. DC (battery) values are left at their
/// cloned defaults — Optix never forces 100% on battery.
struct SettingDef {
    subgroup: &'static str,
    setting: &'static str,
    label: &'static str,
    target_ac: u32,
}

/// The settings every Optix profile sets on AC, and the ones the UI tracks on
/// the active scheme. All four have a legitimate purpose (pinning the CPU,
/// removing link power-saving latency); none are placebo.
const TRACKED_SETTINGS: &[SettingDef] = &[
    SettingDef {
        subgroup: SUB_PROCESSOR,
        setting: SET_MIN_STATE,
        label: "Processor minimum state",
        target_ac: 100,
    },
    SettingDef {
        subgroup: SUB_PROCESSOR,
        setting: SET_MAX_STATE,
        label: "Processor maximum state",
        target_ac: 100,
    },
    SettingDef {
        subgroup: SUB_PCIE,
        setting: SET_ASPM,
        label: "PCI Express link state",
        target_ac: 0,
    },
    SettingDef {
        subgroup: SUB_USB,
        setting: SET_SELECTIVE_SUSPEND,
        label: "USB selective suspend",
        target_ac: 0,
    },
];

/// The display name Optix gives cloned schemes, so re-applies and rollback can
/// recognize them.
fn clone_name(profile_name: &str) -> String {
    format!("Optix - {profile_name}")
}

/// The ready-to-apply Optix power profiles.
pub fn list_profiles() -> Vec<PowerProfile> {
    PROFILES
        .iter()
        .map(|p| PowerProfile {
            id: p.id.to_string(),
            name: p.name.to_string(),
            description: p.description.to_string(),
            base_guid: p.base.to_string(),
            note: p.note.to_string(),
        })
        .collect()
}

/// Apply a power profile: snapshot → clone base scheme → write AC settings →
/// set active → verify → record. Rollback restores the previous scheme and
/// deletes the clone (change records are applied in reverse order).
pub fn apply_profile(db: &Database, id: &str) -> Result<PowerApplyResult> {
    let profile = PROFILES
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| OptixError::InvalidState(format!("unknown power profile: {id}")))?;

    let old_guid = win::power::active_scheme()
        .ok_or_else(|| OptixError::Windows("cannot read the active power scheme".into()))?;

    // Fast path: if the active scheme is already an Optix clone of this
    // profile with every tracked setting at target, there is nothing to do —
    // no clone, no writes, no snapshot.
    if win::power::scheme_name(&old_guid).as_deref() == Some(clone_name(profile.name).as_str())
        && plan_changes(&old_guid).is_empty()
    {
        return Ok(PowerApplyResult {
            snapshot_id: String::new(),
            scheme_guid: old_guid,
            scheme_name: profile.name.to_string(),
            change_count: 0,
            already_applied: true,
        });
    }

    let snap = snapshot::create_lightweight(
        db,
        &format!("Power profile: {}", profile.name),
        Some(&format!("apply power profile {id}")),
    )?;

    // On any failure, restore the previous scheme, delete the clone, and drop
    // the (now-empty) snapshot so no half-applied state is recorded. These
    // cleanup steps are best-effort but their failures must not be hidden.
    let abort = |db: &Database, err: OptixError| -> OptixError {
        if let Err(e) = win::power::set_active_scheme(&old_guid) {
            crate::logging::error("power abort: restore previous scheme", &e);
        }
        if let Err(e) = snapshot::delete(db, &snap.id) {
            crate::logging::error("power abort: delete snapshot", &e);
        }
        err
    };

    let new_guid = match win::power::duplicate_scheme(profile.base) {
        Ok(g) => g,
        Err(e) => return Err(abort(db, e)),
    };
    // Best effort: name the clone so it shows up readably in Windows settings.
    if let Err(e) = win::power::write_friendly_name(&new_guid, &format!("Optix - {}", profile.name))
    {
        crate::logging::warn(&format!("power profile naming failed: {e}"));
    }

    for def in TRACKED_SETTINGS {
        if let Err(e) = win::power::write_ac_index(&new_guid, def.subgroup, def.setting, def.target_ac)
        {
            if let Err(del) = win::power::delete_scheme(&new_guid) {
                crate::logging::error("power abort: delete cloned scheme", &del);
            }
            return Err(abort(db, e));
        }
    }

    if let Err(e) = win::power::set_active_scheme(&new_guid) {
        if let Err(del) = win::power::delete_scheme(&new_guid) {
            crate::logging::error("power abort: delete cloned scheme", &del);
        }
        return Err(abort(db, e));
    }

    // Verify the scheme actually activated; on failure restore the old scheme.
    let active = win::power::active_scheme();
    let max_state = win::power::read_ac_index(&new_guid, SUB_PROCESSOR, SET_MAX_STATE);
    if active.as_deref() != Some(new_guid.as_str()) || max_state != Some(100) {
        if let Err(del) = win::power::delete_scheme(&new_guid) {
            crate::logging::error("power abort: delete cloned scheme", &del);
        }
        return Err(abort(
            db,
            OptixError::Windows("power profile verification failed; reverted".into()),
        ));
    }

    // Reverse-order rollback: restore active scheme, then delete the clone.
    if let Err(error) = record(db, &snap.id, "power", "scheme:create", "create", None, Some(&new_guid)) {
        if let Err(delete_error) = win::power::delete_scheme(&new_guid) {
            crate::logging::error("power abort: delete cloned scheme after audit failure", &delete_error);
        }
        return Err(abort(db, error));
    }
    if let Err(error) = record(db, &snap.id, "power", "scheme:active", "set", Some(&old_guid), Some(&new_guid)) {
        if let Err(delete_error) = win::power::delete_scheme(&new_guid) {
            crate::logging::error("power abort: delete cloned scheme after audit failure", &delete_error);
        }
        return Err(abort(db, error));
    }

    Ok(PowerApplyResult {
        snapshot_id: snap.id,
        scheme_guid: new_guid,
        scheme_name: profile.name.to_string(),
        change_count: 2,
        already_applied: false,
    })
}

/// Current power state: the active scheme, AC/battery, and the tracked
/// settings' current AC values (for the "current vs recommended" UI).
pub fn active_state() -> Option<ActivePowerState> {
    let scheme_guid = win::power::active_scheme()?;
    let scheme_name = win::power::scheme_name(&scheme_guid)
        .unwrap_or_else(|| "Active power scheme".to_string());
    let settings = current_settings(&scheme_guid);
    Some(ActivePowerState {
        scheme_guid,
        scheme_name,
        on_ac: win::power::on_ac_power(),
        settings,
    })
}

/// What applying a profile would change on the currently active scheme, so the
/// UI can show exactly the deltas before the user confirms.
pub fn preview_profile(id: &str) -> Result<PowerPreview> {
    let profile = PROFILES
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| OptixError::InvalidState(format!("unknown power profile: {id}")))?;
    let base_scheme_name = win::power::scheme_name(profile.base)
        .unwrap_or_else(|| "built-in scheme".to_string());
    let active_guid = win::power::active_scheme()
        .ok_or_else(|| OptixError::Windows("cannot read the active power scheme".into()))?;
    let current_scheme_name = win::power::scheme_name(&active_guid)
        .unwrap_or_else(|| "Active power scheme".to_string());

    let changes = plan_changes(&active_guid);
    let already_applied =
        current_scheme_name == clone_name(profile.name) && changes.is_empty();

    Ok(PowerPreview {
        profile_id: profile.id.to_string(),
        profile_name: profile.name.to_string(),
        base_scheme_name,
        changes,
        already_applied,
        current_scheme_name,
    })
}

/// Current AC values of every tracked setting in `scheme` that the platform
/// can read.
fn current_settings(scheme: &str) -> Vec<PowerSettingState> {
    current_settings_with(TRACKED_SETTINGS, |def| {
        win::power::read_ac_index(scheme, def.subgroup, def.setting)
    })
}

/// Pure over the `read` accessor: build the settings state, then reduce to the
/// ones that differ from the Optix targets. Unit-tested without Windows APIs.
fn diff_settings(
    tracked: &[SettingDef],
    read: impl Fn(&SettingDef) -> Option<u32>,
) -> Vec<PowerSettingState> {
    current_settings_with(tracked, read)
        .into_iter()
        .filter(|s| s.current != s.optix_target)
        .collect()
}

fn current_settings_with(
    tracked: &[SettingDef],
    read: impl Fn(&SettingDef) -> Option<u32>,
) -> Vec<PowerSettingState> {
    tracked
        .iter()
        .filter_map(|def| {
            read(def).map(|current| PowerSettingState {
                label: def.label.to_string(),
                current,
                optix_target: def.target_ac,
            })
        })
        .collect()
}

/// Current AC values of the tracked settings in `scheme`, reduced to the ones
/// that differ from the Optix targets.
fn plan_changes(scheme: &str) -> Vec<PowerSettingState> {
    diff_settings(TRACKED_SETTINGS, |def| {
        win::power::read_ac_index(scheme, def.subgroup, def.setting)
    })
}

/// Disable network-adapter power saving across every detected adapter.
/// Writes only values that are currently enabled (so rollback always has a
/// real previous value to restore), recording each as a `registry` change.
pub fn disable_nic_power_saving(db: &Database) -> Result<NicPowerResult> {
    let adapters = win::nic::list_adapters();
    let planned_changes = adapters
        .iter()
        .map(|adapter| {
            usize::from(adapter.eee == Some(1))
                + usize::from(adapter.green_ethernet == Some(1))
                + usize::from(adapter.power_management == Some(1))
                + usize::from(matches!(
                    adapter.pnp_capabilities,
                    Some(value) if value != PNP_NO_POWER_OFF
                ))
        })
        .sum::<usize>();
    if planned_changes == 0 {
        return Ok(NicPowerResult {
            snapshot_id: None,
            adapters_changed: 0,
            changes: 0,
        });
    }

    let snap = snapshot::create_lightweight(db, "NIC power saving", Some("disable NIC power saving"))?;
    let mut adapters_changed = 0usize;
    let mut changes = 0usize;

    let applied: Result<()> = (|| {
        for adapter in &adapters {
            let mut adapter_changed = false;

            if adapter.eee == Some(1) {
                apply_nic_value(db, &snap.id, &adapter.key, "*EEE", Some("1"), "0")?;
                adapter_changed = true;
                changes += 1;
            }
            if adapter.green_ethernet == Some(1) {
                apply_nic_value(db, &snap.id, &adapter.key, "EnableGreenEthernet", Some("1"), "0")?;
                adapter_changed = true;
                changes += 1;
            }
            if adapter.power_management == Some(1) {
                apply_nic_value(db, &snap.id, &adapter.key, "EnablePowerManagement", Some("1"), "0")?;
                adapter_changed = true;
                changes += 1;
            }
            if let Some(current) = adapter.pnp_capabilities {
                if current != PNP_NO_POWER_OFF {
                    let old = current.to_string();
                    let new = PNP_NO_POWER_OFF.to_string();
                    apply_nic_value(
                        db,
                        &snap.id,
                        &adapter.key,
                        "PnPCapabilities",
                        Some(&old),
                        &new,
                    )?;
                    adapter_changed = true;
                    changes += 1;
                }
            }

            if adapter_changed {
                adapters_changed += 1;
            }
        }
        Ok(())
    })();

    if let Err(error) = applied {
        if let Err(rollback_error) = rollback::restore(db, &snap.id) {
            crate::logging::error("NIC power-saving abort rollback failed", &rollback_error);
        }
        if let Err(delete_error) = snapshot::delete(db, &snap.id) {
            crate::logging::error("NIC power-saving abort snapshot cleanup failed", &delete_error);
        }
        return Err(error);
    }

    Ok(NicPowerResult {
        snapshot_id: Some(snap.id),
        adapters_changed,
        changes,
    })
}

/// Write a NIC value and record the reversible registry change against the
/// active snapshot.
fn apply_nic_value(
    db: &Database,
    snapshot_id: &str,
    adapter_key: &str,
    value_name: &str,
    old_value: Option<&str>,
    new_value: &str,
) -> Result<()> {
    let parsed: u32 = new_value
        .parse()
        .map_err(|_| OptixError::InvalidState(format!("invalid NIC value: {new_value}")))?;
    win::nic::set_dword(adapter_key, value_name, parsed)?;
    if let Err(error) = record(
        db,
        snapshot_id,
        "registry",
        &format!(r"{NET_CLASS_HKLM}\{adapter_key}\{value_name}"),
        "set",
        old_value,
        Some(new_value),
    ) {
        if let Some(old) = old_value.and_then(|value| value.parse::<u32>().ok()) {
            if let Err(restore_error) = win::nic::set_dword(adapter_key, value_name, old) {
                crate::logging::error("NIC power-saving write-audit rollback failed", &restore_error);
            }
        }
        return Err(error);
    }
    Ok(())
}

/// Record a change through the rollback engine.
#[allow(clippy::too_many_arguments)]
fn record(
    db: &Database,
    snapshot_id: &str,
    domain: &str,
    location: &str,
    kind: &str,
    old_value: Option<&str>,
    new_value: Option<&str>,
) -> Result<()> {
    rollback::record_change(
        db,
        snapshot_id,
        ChangeRecord {
            id: None,
            snapshot_id: String::new(),
            domain: domain.to_string(),
            location: location.to_string(),
            kind: kind.to_string(),
            old_value: old_value.map(str::to_string),
            new_value: new_value.map(str::to_string),
            old_json: None,
            new_json: None,
            applied_at_ms: None,
            verified: true,
            rolled_back: false,
        },
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_are_well_formed() {
        let profiles = list_profiles();
        assert_eq!(profiles.len(), 3);
        for p in &profiles {
            assert!(!p.id.is_empty());
            assert!(!p.name.is_empty());
            // Base GUIDs must parse as 32 hex digits (dashes stripped).
            let hex: String = p.base_guid.chars().filter(|c| c.is_ascii_hexdigit()).collect();
            assert_eq!(hex.len(), 32, "bad base GUID for {}", p.id);
        }
    }

    #[test]
    fn profile_ids_are_unique() {
        let profiles = list_profiles();
        let mut ids: Vec<&str> = profiles.iter().map(|p| p.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), profiles.len());
    }

    #[test]
    fn tracked_settings_are_well_formed() {
        let mut labels = std::collections::HashSet::new();
        for def in TRACKED_SETTINGS {
            for g in [def.subgroup, def.setting] {
                let hex: String = g.chars().filter(|c| c.is_ascii_hexdigit()).collect();
                assert_eq!(hex.len(), 32, "bad GUID {g}");
            }
            assert!(def.target_ac <= 100, "target out of range: {}", def.label);
            assert!(labels.insert(def.label), "duplicate label: {}", def.label);
        }
        // Every profile writes exactly the tracked settings — no hidden extras.
        assert_eq!(TRACKED_SETTINGS.len(), 4);
    }

    #[test]
    fn diff_settings_reports_only_differing() {
        use std::collections::HashMap;
        // Two settings already at target, two differing (ASPM 1 vs 0, USB 1 vs 0).
        let mut values = HashMap::new();
        values.insert("Processor minimum state", 100u32);
        values.insert("Processor maximum state", 100u32);
        values.insert("PCI Express link state", 1u32);
        values.insert("USB selective suspend", 1u32);

        let changes = diff_settings(TRACKED_SETTINGS, |def| values.get(def.label).copied());
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].label, "PCI Express link state");
        assert_eq!((changes[0].current, changes[0].optix_target), (1, 0));
        assert_eq!(changes[1].label, "USB selective suspend");
        assert_eq!((changes[1].current, changes[1].optix_target), (1, 0));
    }

    #[test]
    fn diff_settings_skips_unreadable_values() {
        // A setting the platform cannot read is omitted, not reported as a change.
        let changes = diff_settings(TRACKED_SETTINGS, |_| None);
        assert!(changes.is_empty());
    }

    #[test]
    fn clone_name_is_recognizable() {
        assert_eq!(clone_name("Balanced Gaming"), "Optix - Balanced Gaming");
        // The already-applied check matches the clone naming exactly.
        assert_eq!(clone_name("Competitive Gaming"), "Optix - Competitive Gaming");
    }
}
