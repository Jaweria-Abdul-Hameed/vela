//! Build script: the release-profile guard, then the Tauri build (capability validation, Windows resources).

#[path = "build_policy.rs"]
mod build_policy;

fn main() {
    println!("cargo:rerun-if-changed=build_policy.rs");
    let profile = std::env::var("PROFILE").unwrap_or_default();
    let test_agent = std::env::var_os("CARGO_FEATURE_TEST_AGENT").is_some();
    if let Err(message) = build_policy::check_release_features(&profile, test_agent) {
        panic!("{message}");
    }
    tauri_build::build();
}
