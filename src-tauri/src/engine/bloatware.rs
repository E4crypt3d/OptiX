//! Bloatware (AppX/MSIX) engine: classify installed packages against a
//! hard-coded allowlist/removal list (plan.md §11) and orchestrate removal.
//! Only ever *suggests* — the user confirms every package, and removal is
//! snapshot-first (the command layer records each change).

use crate::models::bloatware::AppxPackage;
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
    for raw in installed {
        let name = raw.name.unwrap_or_default();
        let full_name = raw.full_name.unwrap_or_default();
        if name.is_empty() || full_name.is_empty() {
            continue;
        }
        let provisioned = provisioned.iter().any(|p| {
            let display = p.display_name.as_deref().unwrap_or_default();
            let package = p.package_name.as_deref().unwrap_or_default();
            display.eq_ignore_ascii_case(&name)
                || (!package.is_empty() && package.starts_with(&name))
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
    out.sort_by(|a, b| rank(&a.classification).cmp(&rank(&b.classification)).then_with(|| a.name.cmp(&b.name)));
    Ok(out)
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
                        || (!package.is_empty() && package.starts_with(&name))
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
}
