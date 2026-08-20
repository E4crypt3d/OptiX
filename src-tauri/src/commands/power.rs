use tauri::State;

use crate::db::sqlite::Database;
use crate::engine::power;
use crate::error::Result;
use crate::models::power::{
    ActivePowerState, NicAdapter, NicPowerResult, PowerApplyResult, PowerPreview, PowerProfile,
    PowerScheme,
};
use crate::win;

/// Enumerate power schemes (active scheme flagged).
#[tauri::command]
pub fn list_power_schemes() -> Result<Vec<PowerScheme>> {
    Ok(win::power::list_schemes())
}

/// The ready-to-apply Optix power profiles.
#[tauri::command]
pub fn list_power_profiles() -> Vec<PowerProfile> {
    power::list_profiles()
}

/// Apply a power profile (clone base scheme → write settings → set active),
/// snapshot-first and reversible via the rollback center. No-op (fast path)
/// when the active scheme already matches the profile.
#[tauri::command]
pub fn apply_power_profile(db: State<'_, Database>, id: String) -> Result<PowerApplyResult> {
    power::apply_profile(db.inner(), &id)
}

/// Current active scheme, AC/battery, and its tracked setting values.
#[tauri::command]
pub fn active_power_state() -> Option<ActivePowerState> {
    power::active_state()
}

/// What applying a profile would change on the currently active scheme.
#[tauri::command]
pub fn preview_power_profile(id: String) -> Result<PowerPreview> {
    power::preview_profile(&id)
}

/// Enumerate network adapters and their power-saving state.
#[tauri::command]
pub fn list_nic_adapters() -> Vec<NicAdapter> {
    win::nic::list_adapters()
}

/// Disable NIC power saving on all adapters, snapshot-first and reversible.
#[tauri::command]
pub fn disable_nic_power_saving(db: State<'_, Database>) -> Result<NicPowerResult> {
    power::disable_nic_power_saving(db.inner())
}
