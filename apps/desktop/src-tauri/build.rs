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
    // Common-Controls v6 is needed by the window toolkit (TaskDialogIndirect). Tauri embeds its manifest only into the
    // executable, so the library's unit-test binary would fail to load (STATUS_ENTRYPOINT_NOT_FOUND). Embed one manifest for
    // every target ourselves and tell Tauri not to add its own, so no target gets two.
    let windows_msvc = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if windows_msvc {
        let manifest =
            std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"))
                .join("windows-app.manifest");
        println!("cargo:rerun-if-changed=windows-app.manifest");
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
        tauri_build::try_build(
            tauri_build::Attributes::new()
                .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest()),
        )
        .expect("failed to run the Tauri build");
    } else {
        tauri_build::build();
    }
}
