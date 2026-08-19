//! Windows OS version detection via the `windows-version` crate.

/// Pure logic: Windows 11 uses build numbers >= 22000.
pub fn build_is_windows_11(build: u32) -> bool {
    build >= 22000
}

/// Current Windows build number; `None` off Windows.
#[cfg(windows)]
pub fn current_build() -> Option<u32> {
    Some(windows_version::OsVersion::current().build)
}

#[cfg(not(windows))]
pub fn current_build() -> Option<u32> {
    None
}

#[cfg(test)]
mod tests {
    use super::build_is_windows_11;

    #[test]
    fn detects_windows_11_by_build() {
        assert!(!build_is_windows_11(19045)); // Windows 10 22H2
        assert!(build_is_windows_11(22000)); // Windows 11 21H2
        assert!(build_is_windows_11(22631)); // Windows 11 23H2
    }
}
