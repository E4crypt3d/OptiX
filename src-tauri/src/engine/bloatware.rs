//! Bloatware (AppX/MSIX) engine: classify installed packages against a
//! hard-coded allowlist/removal list and orchestrate removal. Only ever
//! *suggests* — the user confirms every package, and removal is snapshot-first
//! (the command layer records each change).

use crate::models::bloatware::AppxPackage;
use std::collections::HashSet;
use crate::win::appx;

/// Packages Optix must never flag. Includes the Xbox gaming components and
/// .NET/WindowsAppRuntime runtimes that other apps depend on.
const PROTECTED: &[&str] = &[
    "microsoft.windowscalculator",
    "microsoft.windowsnotepad",
    "microsoft.mspaint",
    "microsoft.windows.photos",
    "microsoft.windowsstore",
    "microsoft.windowsterminal",
    "microsoft.windowscamera",
    "microsoft.xbox",
    "microsoft.xboxgamingoverlay",
    "microsoft.xboxgamecallableui",
    "microsoft.xboxidentityprovider",
    "microsoft.xbox.tcui",
    "microsoft.net.",
    "microsoft.windowsappruntime",
    "microsoft.windows.shellexperiencehost",
    "microsoft.windows.startmenuexperiencehost",
    "microsoft.windows.search",
    "microsoft.windows.sechealthui",
    "microsoft.windows.settings",
    "microsoft.windows.contentdeliverymanager",
    "microsoft.windows.cloudexperiencehost",
    "microsoft.microsoftedge",
    "microsoft.storepurchaseapp",
];

/// Removable, but with a caveat (other apps may depend on them).
const CAUTION: &[&str] = &["microsoft.advertising.xaml", "dolby"];

/// Well-known removal candidates (promotional, streaming, shopping, OEM trial).
const REMOVAL: &[&str] = &[
    "clipchamp",
    "microsoftsolitairecollection",
    "microsoft.bingnews",
    "microsoft.bingweather",
    "microsoft.gethelp",
    "microsoft.windowsfeedbackhub",
    "microsoft.skypeapp",
    "microsoft.mixedreality.portal",
    "microsoft.windowsmaps",
    "microsoft.people",
    "microsoft.todos",
    "microsoft.microsoftofficehub",
    "microsoft.xboxapp",
    "facebook",
    "instagram",
    "twitter",
    "linkedin",
    "whatsapp",
    "telegram",
    "netflix",
    "disney",
    "spotify",
    "tiktok",
    "amazon",
    "temu",
    "ebay",
    "booking",
    "mcafee",
];

/// Classify a package family name as `protected`, `caution`, `removal`, or
/// `unknown`. Protected wins over everything; then caution; then removal.
pub fn classify(name: &str) -> &'static str {
    let n = name.to_lowercase();
    if PROTECTED.iter().any(|p| n.contains(p)) {
        return "protected";
    }
    if CAUTION.iter().any(|c| n.contains(c)) {
        return "caution";
    }
    if REMOVAL.iter().any(|r| n.contains(r)) {
        return "removal";
    }
    "unknown"
}

/// Per-package removal outcome (internal to the command layer).
pub struct AppxOutcome {
    pub full_name: String,
    pub name: String,
    pub install_location: String,
    pub provisioned: bool,
    pub ok: bool,
    pub error: Option<String>,
}

/// Enumerate installed packages, classified and annotated with their
/// provisioned status.
pub fn scan() -> Result<Vec<AppxPackage>, String> {
    let installed = appx::list_installed()?;
    let provisioned = appx::list_provisioned().unwrap_or_default();

    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for raw in installed {
        let name = raw.name.unwrap_or_default();
        let full_name = raw.full_name.unwrap_or_default();
        if name.is_empty() || full_name.is_empty() || !seen.insert(full_name.clone()) {
            continue;
        }
        let name_lower = name.to_ascii_lowercase();
        let provisioned = provisioned.iter().any(|p| {
            let display = p.display_name.as_deref().unwrap_or_default();
            let package = p.package_name.as_deref().unwrap_or_default();
            display.eq_ignore_ascii_case(&name)
                || (!package.is_empty() && package.to_ascii_lowercase().starts_with(&name_lower))
        });
        let classification = classify(&name).to_string();
        out.push(AppxPackage {
            name,
            full_name,
            publisher: raw.publisher.unwrap_or_default(),
            version: raw.version.unwrap_or_default(),
            architecture: raw.architecture.unwrap_or_default(),
            install_location: raw.install_location.unwrap_or_default(),
            classification,
            provisioned,
        });
    }

    // Removal candidates first, protected last.
    fn rank(c: &str) -> u8 {
        match c {
            "removal" => 0,
            "caution" => 1,
            "unknown" => 2,
            _ => 3,
        }
    }
    out.sort_by(|a, b| {
        rank(&a.classification)
            .cmp(&rank(&b.classification))
            .then_with(|| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()))
            .then_with(|| a.full_name.cmp(&b.full_name))
    });
    Ok(out)
}

/// Validate package names against a fresh installed-package scan before any
/// snapshot or mutation. The frontend is not a security boundary.
pub fn validate_removal(full_names: &[String]) -> Result<(), String> {
    if full_names.is_empty() {
        return Err("select at least one package".to_string());
    }
    let installed = appx::list_installed()?;
    for full_name in full_names {
        let Some(raw) = installed
            .iter()
            .find(|package| package.full_name.as_deref() == Some(full_name.as_str()))
        else {
            return Err(format!("package is no longer installed: {full_name}"));
        };
        let name = raw.name.as_deref().unwrap_or_default();
        match classify(name) {
            "removal" | "caution" => {}
            classification => {
                return Err(format!(
                    "package cannot be removed by this tool ({classification}): {name}"
                ));
            }
        }
    }
    Ok(())
}

/// Remove the given packages (by full name). Each package's provisioned copy
/// is removed first so it does not reinstall for new users. Best-effort per
/// package: one failure does not stop the rest.
pub fn remove(full_names: &[String]) -> Vec<AppxOutcome> {
    let installed = appx::list_installed().unwrap_or_default();
    let provisioned = appx::list_provisioned().unwrap_or_default();

    full_names
        .iter()
        .map(|full| {
            let pkg = installed
                .iter()
                .find(|r| r.full_name.as_deref() == Some(full.as_str()));
            let name = pkg
                .and_then(|r| r.name.clone())
                .unwrap_or_else(|| family_of(full));
            let install_location = pkg
                .and_then(|r| r.install_location.clone())
                .unwrap_or_default();

            let matching: Vec<&appx::RawProvisioned> = provisioned
                .iter()
                .filter(|p| {
                    let display = p.display_name.as_deref().unwrap_or_default();
                    let package = p.package_name.as_deref().unwrap_or_default();
                    display.eq_ignore_ascii_case(&name)
                        || (!package.is_empty()
                            && package
                                .to_ascii_lowercase()
                                .starts_with(&name.to_ascii_lowercase()))
                })
                .collect();

            let mut ok = true;
            let mut error = None;
            for p in &matching {
                if let Some(package_name) = p.package_name.as_deref() {
                    if let Err(e) = appx::remove_provisioned(package_name) {
                        ok = false;
                        error = Some(e);
                    }
                }
            }
            if let Err(e) = appx::remove_installed(full) {
                ok = false;
                error = Some(e);
            }

            AppxOutcome {
                full_name: full.clone(),
                name,
                install_location,
                provisioned: !matching.is_empty(),
                ok,
                error,
            }
        })
        .collect()
}

fn family_of(full_name: &str) -> String {
    full_name.split('_').next().unwrap_or(full_name).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protects_core_and_xbox_packages() {
        assert_eq!(classify("Microsoft.WindowsCalculator"), "protected");
        assert_eq!(classify("Microsoft.XboxGamingOverlay"), "protected");
        assert_eq!(classify("Microsoft.XboxIdentityProvider"), "protected");
        assert_eq!(classify("Microsoft.NET.Native.Runtime.2.2"), "protected");
        assert_eq!(classify("Microsoft.WindowsAppRuntime.1.7"), "protected");
        assert_eq!(classify("Microsoft.Windows.Search"), "protected");
    }

    #[test]
    fn flags_removal_candidates() {
        assert_eq!(classify("Clipchamp.Clipchamp"), "removal");
        assert_eq!(classify("Microsoft.BingNews"), "removal");
        assert_eq!(classify("Microsoft.GetHelp"), "removal");
        assert_eq!(classify("SpotifyAB.SpotifyMusic"), "removal");
        assert_eq!(classify("Microsoft.WindowsMaps"), "removal");
    }

    #[test]
    fn flags_caution_packages() {
        assert_eq!(classify("Microsoft.Advertising.Xaml"), "caution");
        assert_eq!(classify("Dolby.DolbyAudio"), "caution");
    }

    #[test]
    fn leaves_unknown_packages_unflagged() {
        assert_eq!(classify("Microsoft.GamingApp"), "unknown");
        assert_eq!(classify("Microsoft.WindowsStore"), "protected");
        assert_eq!(classify("Some.Random.App"), "unknown");
    }

    #[test]
    fn family_of_strips_version_suffix() {
        assert_eq!(
            family_of("Microsoft.WindowsCalculator_10.1.2.3_x64__8wekyb3d8bbwe"),
            "Microsoft.WindowsCalculator"
        );
    }

    #[test]
    fn protects_every_xbox_family() {
        assert_eq!(classify("Microsoft.XboxApp"), "protected");
        assert_eq!(classify("Microsoft.Xbox.TCUI"), "protected");
    }
}
