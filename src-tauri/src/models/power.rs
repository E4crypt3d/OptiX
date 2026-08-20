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
/// changes to the rollback center.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerApplyResult {
    pub snapshot_id: String,
    pub scheme_guid: String,
    pub scheme_name: String,
    pub change_count: usize,
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
