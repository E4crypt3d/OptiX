use tauri::State;

use crate::db::sqlite::Database;
use crate::engine::power;
use crate::error::Result;
use crate::models::power::{NicAdapter, NicPowerResult, PowerApplyResult, PowerProfile, PowerScheme};
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
/// snapshot-first and reversible via the rollback center.
#[tauri::command]
pub fn apply_power_profile(db: State<'_, Database>, id: String) -> Result<PowerApplyResult> {
    power::apply_profile(db.inner(), &id)
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
