#[cfg(test)]
mod tests {
    use windows_settings::cleanup::*;
    use windows_settings::config::*;
    use windows_settings::telemetry::*;
    use windows_settings::update::*;
    use windows_settings::uwp::*;

    #[test]
    fn test_app_version_is_valid_semver() {
        assert!(
            semver::Version::parse(APP_VERSION).is_ok(),
            "APP_VERSION is not valid semver: {APP_VERSION}"
        );
    }

    #[test]
    fn test_all_modules_load() {
        let _ = uwp_apps();
        let _ = telemetry_items();
        let _ = cleanup_items();
    }

    #[test]
    fn test_config_roundtrip() {
        let mut cfg = Config::default();
        assert!(cfg.github_token.is_none());
        cfg.github_token = Some("ghp_test123".to_string());
        assert_eq!(cfg.github_token, Some("ghp_test123".to_string()));
    }
}
