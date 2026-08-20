//! Windows AppX/MSIX package management (bloatware removal).
//!
//! Uses PowerShell cmdlets (`Get-AppxPackage` / `Get-AppxProvisionedPackage`
//! for enumeration, `Remove-AppxPackage` / `Remove-AppxProvisionedPackage`
//! for removal) rather than the WinRT `PackageManager` COM surface. All
//! functions are `#[cfg(windows)]`-gated with non-Windows fallbacks so the
//! crate still builds on Linux.

use serde::Deserialize;

use crate::models::snapshot::ChangeRecord;

/// A row from `Get-AppxPackage` (projected to strings so `ConvertTo-Json`
/// yields clean JSON instead of nested `Version`/`Architecture` objects).
#[derive(Debug, Clone, Deserialize)]
pub struct RawAppx {
    #[serde(rename = "Name")]
    pub name: Option<String>,
    #[serde(rename = "PackageFullName")]
    pub full_name: Option<String>,
    #[serde(rename = "Publisher")]
    pub publisher: Option<String>,
    #[serde(rename = "Version")]
    pub version: Option<String>,
    #[serde(rename = "Architecture")]
    pub architecture: Option<String>,
    #[serde(rename = "InstallLocation")]
    pub install_location: Option<String>,
}

/// A row from `Get-AppxProvisionedPackage -Online`.
#[derive(Debug, Clone, Deserialize)]
pub struct RawProvisioned {
    #[serde(rename = "DisplayName")]
    pub display_name: Option<String>,
    #[serde(rename = "PackageName")]
    pub package_name: Option<String>,
}

#[cfg(windows)]
fn run_powershell(script: &str) -> Result<String, String> {
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .map_err(|e| format!("failed to launch PowerShell: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        Err(if stderr.trim().is_empty() {
            format!("PowerShell exited with {}", output.status)
        } else {
            stderr.trim().to_string()
        })
    }
}

/// Parse `ConvertTo-Json` output that may be a single object, an array, or
/// empty, into a `Vec<T>`.
#[cfg(windows)]
fn parse_json_list<T: serde::de::DeserializeOwned>(text: &str) -> Result<Vec<T>, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let value: serde_json::Value =
        serde_json::from_str(trimmed).map_err(|e| format!("invalid JSON from PowerShell: {e}"))?;
    match value {
        serde_json::Value::Array(items) => items
            .into_iter()
            .map(|v| serde_json::from_value(v).map_err(|e| format!("invalid package row: {e}")))
            .collect(),
        serde_json::Value::Null => Ok(Vec::new()),
        single => Ok(vec![
            serde_json::from_value(single).map_err(|e| format!("invalid package row: {e}"))?,
        ]),
    }
}

/// Enumerate installed AppX packages.
#[cfg(windows)]
pub fn list_installed() -> Result<Vec<RawAppx>, String> {
    const SCRIPT: &str = r#"Get-AppxPackage | Select-Object Name, PackageFullName, Publisher, @{N='Version';E={$_.Version.ToString()}}, @{N='Architecture';E={$_.Architecture.ToString()}}, InstallLocation | ConvertTo-Json -Compress"#;
    parse_json_list(&run_powershell(SCRIPT)?)
}

#[cfg(not(windows))]
pub fn list_installed() -> Result<Vec<RawAppx>, String> {
    Ok(Vec::new())
}

/// Enumerate provisioned AppX packages (reinstallable for new users).
#[cfg(windows)]
pub fn list_provisioned() -> Result<Vec<RawProvisioned>, String> {
    const SCRIPT: &str = r#"Get-AppxProvisionedPackage -Online | Select-Object DisplayName, PackageName | ConvertTo-Json -Compress"#;
    parse_json_list(&run_powershell(SCRIPT)?)
}

#[cfg(not(windows))]
pub fn list_provisioned() -> Result<Vec<RawProvisioned>, String> {
    Ok(Vec::new())
}

/// Remove an installed package for the current user.
#[cfg(windows)]
pub fn remove_installed(full_name: &str) -> Result<(), String> {
    let escaped = full_name.replace('\'', "''");
    let script = format!("Remove-AppxPackage -Package '{escaped}' -ErrorAction Stop");
    run_powershell(&script)?;
    Ok(())
}

#[cfg(not(windows))]
pub fn remove_installed(_full_name: &str) -> Result<(), String> {
    Err("AppX removal is only available on Windows".into())
}

/// Remove a provisioned package so it does not reinstall for new users.
#[cfg(windows)]
pub fn remove_provisioned(package_name: &str) -> Result<(), String> {
    let escaped = package_name.replace('\'', "''");
    let script =
        format!("Remove-AppxProvisionedPackage -Online -PackageName '{escaped}' -ErrorAction Stop");
    run_powershell(&script)?;
    Ok(())
}

#[cfg(not(windows))]
pub fn remove_provisioned(_package_name: &str) -> Result<(), String> {
    Err("AppX removal is only available on Windows".into())
}

/// Best-effort AppX rollback: re-register the package from its still-present
/// install location. If the files are gone, this is a no-op and the user
/// reinstalls from the Store — so it never hard-fails a snapshot restore.
#[cfg(windows)]
pub fn rollback_appx(change: &ChangeRecord) -> crate::error::Result<()> {
    let Some(json) = &change.old_json else {
        return Ok(());
    };
    let Some(loc) = json.get("install_location").and_then(|v| v.as_str()) else {
        return Ok(());
    };
    let loc = loc.trim_end_matches('\\');
    if loc.is_empty() {
        return Ok(());
    }
    let manifest = format!(r#"{loc}\AppxManifest.xml"#);
    let script = format!("Add-AppxPackage -Register '{manifest}' -ErrorAction SilentlyContinue");
    let _ = run_powershell(&script);
    Ok(())
}

#[cfg(not(windows))]
pub fn rollback_appx(_change: &ChangeRecord) -> crate::error::Result<()> {
    Ok(())
}
