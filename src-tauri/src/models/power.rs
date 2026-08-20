//! Power-management models (Phase 5): Windows power schemes, Optix power
//! profiles, and network-adapter power-saving state.

use serde::Serialize;

/// A Windows power scheme (plan), identified by GUID.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerScheme {
    pub guid: String,
    pub name: String,
    pub is_active: bool,
}

/// A ready-to-apply Optix power profile. Defined in `engine::power`; the
/// frontend renders these and requests application by `id`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerProfile {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Base scheme GUID this profile is cloned from.
    pub base_guid: String,
    /// Honest impact/risk note shown in the UI.
    pub note: String,
}

/// Result of applying a power profile. `snapshot_id` links the reversible
/// changes to the rollback center (empty when nothing was applied).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerApplyResult {
    pub snapshot_id: String,
    pub scheme_guid: String,
    pub scheme_name: String,
    pub change_count: usize,
    /// True when the active scheme already matched the profile (no-op).
    pub already_applied: bool,
}

/// One tracked power setting: its current AC value in the active scheme and
/// the value Optix gaming profiles set.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerSettingState {
    /// Human-readable name (e.g. "Processor minimum state").
    pub label: String,
    /// Raw value currently in the active scheme (AC).
    pub current: u32,
    /// Raw value Optix profiles set.
    pub optix_target: u32,
}

/// Current power state: the active scheme, the power source, and the settings
/// Optix tracks. `None` settings fields stay raw; the UI formats them.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivePowerState {
    pub scheme_guid: String,
    pub scheme_name: String,
    /// Some(true) on AC, Some(false) on battery, None when not reported.
    pub on_ac: Option<bool>,
    pub settings: Vec<PowerSettingState>,
}

/// What applying a profile would change on the currently active scheme.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerPreview {
    pub profile_id: String,
    pub profile_name: String,
    /// Name of the built-in scheme this profile clones.
    pub base_scheme_name: String,
    /// Settings whose AC value differs from the profile target.
    pub changes: Vec<PowerSettingState>,
    /// True when the active scheme already matches the profile.
    pub already_applied: bool,
    /// The active scheme's name at preview time.
    pub current_scheme_name: String,
}

/// A physical network adapter and its power-saving registry values. `None`
/// means the driver does not expose that value (its default applies).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NicAdapter {
    /// Class-key subkey (e.g. `0000`) identifying the adapter.
    pub key: String,
    /// Driver description (e.g. "Intel(R) Ethernet Controller I225-V").
    pub name: String,
    /// Energy Efficient Ethernet (`*EEE`): 1 = enabled, 0 = disabled.
    pub eee: Option<u32>,
    /// Vendor "Green Ethernet" (`EnableGreenEthernet`): 1 = enabled.
    pub green_ethernet: Option<u32>,
    /// Device power management (`PnPCapabilities`); 24 disables powering off.
    pub pnp_capabilities: Option<u32>,
    /// Driver power-management flag (`EnablePowerManagement`): 1 = enabled.
    pub power_management: Option<u32>,
}

impl NicAdapter {
    /// Whether any power-saving feature is currently enabled (i.e. there is
    /// something to disable). Unknown (absent) values are treated as inactive
    /// so Optix never writes a value it cannot verify and restore.
    pub fn power_saving_active(&self) -> bool {
        self.eee == Some(1)
            || self.green_ethernet == Some(1)
            || self.power_management == Some(1)
            || matches!(self.pnp_capabilities, Some(v) if v != 24)
    }
}

/// Result of disabling NIC power saving.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NicPowerResult {
    pub snapshot_id: Option<String>,
    /// Number of adapters that had at least one value changed.
    pub adapters_changed: usize,
    /// Number of registry values written.
    pub changes: usize,
}
