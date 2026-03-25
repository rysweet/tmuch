pub const GITHUB_REPO: &str = "rysweet/tmuch";
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Allowed URL prefixes for release asset downloads.
pub const ALLOWED_DOWNLOAD_HOSTS: &[&str] = &[
    "https://github.com/rysweet/tmuch/",
    "https://objects.githubusercontent.com/",
];

/// Strip ASCII control characters to prevent terminal escape injection.
pub fn sanitise_for_display(s: &str) -> String {
    s.chars().filter(|c| !c.is_ascii_control()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_version_valid() {
        assert!(!CURRENT_VERSION.is_empty());
        assert!(CURRENT_VERSION.contains('.'));
    }

    #[test]
    fn test_sanitise_strips_escape_sequences() {
        assert_eq!(sanitise_for_display("1.0.0\x1b[2J"), "1.0.0[2J");
    }

    #[test]
    fn test_sanitise_passes_normal_version() {
        assert_eq!(sanitise_for_display("0.2.0"), "0.2.0");
    }

    #[test]
    fn test_sanitise_strips_null_bytes() {
        assert_eq!(sanitise_for_display("1.0\x00.0"), "1.0.0");
    }
}
