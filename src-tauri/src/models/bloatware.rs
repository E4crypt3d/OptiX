//! Bloatware (AppX/MSIX) models — the packages Optix can safely remove,
//! with a classification that separates protected system packages from
//! removal candidates.

use serde::Serialize;

/// An installed AppX/MSIX package, enriched with a removal classification.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppxPackage {
    /// Package family name (e.g. `Microsoft.WindowsCalculator`).
    pub name: String,
    /// Full package name including version/arch/publisher hash.
    pub full_name: String,
    pub publisher: String,
    pub version: String,
    pub architecture: String,
    pub install_location: String,
    /// `protected` | `removal` | `caution` | `unknown`.
    pub classification: String,
    /// Present in the provisioned store — removing it there prevents the
    /// package from reinstalling for new users / after feature updates.
    pub provisioned: bool,
}

/// A package that failed to remove, with the reason.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppxRemovalFailure {
    pub full_name: String,
    pub error: String,
}

/// Outcome of a bloatware removal run.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BloatwareRemoveResult {
    pub snapshot_id: String,
    /// Full names of packages that were removed successfully.
    pub removed: Vec<String>,
    /// Packages that failed to remove, with the reason.
    pub failed: Vec<AppxRemovalFailure>,
}
