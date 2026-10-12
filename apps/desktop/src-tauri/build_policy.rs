//! The release-profile guard, shared by `build.rs` (which enforces it) and the crate's tests (which prove it).
//!
//! The `test-agent` feature compiles a scripted agent that exists only for tests and is never a product feature
//! (`SHARED_SURFACE_PROTOCOL.md` section 5). A release build with that feature enabled must fail.

/// Fails when the `release` profile is built with the `test-agent` feature enabled.
pub fn check_release_features(profile: &str, test_agent: bool) -> Result<(), String> {
    if profile == "release" && test_agent {
        return Err(
            "the `test-agent` feature must not be enabled in a release build: it compiles a scripted agent \
             that exists only for tests and is never a product feature"
                .to_owned(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::check_release_features;

    #[test]
    fn a_release_build_with_the_test_agent_feature_fails_the_check() {
        let error = check_release_features("release", true).unwrap_err();
        assert!(error.contains("test-agent"), "{error}");
    }

    #[test]
    fn a_release_build_without_the_feature_passes() {
        assert_eq!(check_release_features("release", false), Ok(()));
    }

    #[test]
    fn debug_builds_may_enable_the_feature() {
        assert_eq!(check_release_features("debug", true), Ok(()));
        assert_eq!(check_release_features("debug", false), Ok(()));
    }
}
