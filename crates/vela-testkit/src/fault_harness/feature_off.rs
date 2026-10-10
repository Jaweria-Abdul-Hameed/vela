//! Keeps the F02 "feature off" configuration covered.
//!
//! This crate's manifest turns `vela-domain/fault-injection` on, and `cargo test --workspace` unifies features, so the
//! workspace test run alone no longer builds `vela-domain` with the feature off (F02's `const` compile tests, which
//! prove a disabled `fault_point!` generates no code and never evaluates its argument, would otherwise be compiled out
//! of CI). This test runs `vela-domain`'s own tests as a package of their own, where the feature is off, in a scratch
//! target directory so it neither takes the outer build's lock nor disturbs its cache.

use crate::fixture_repo::ScratchDir;
use std::process::Command;

#[test]
fn vela_domain_still_passes_with_the_fault_injection_feature_off() {
    let scratch = ScratchDir::create("feature-off").unwrap();
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .args([
            "test",
            "-p",
            "vela-domain",
            "--lib",
            "--locked",
            "--",
            // The domain's own feature-matrix test builds other configurations; this run only needs the default one.
            "--skip",
            "the_fault_injection_and_hook_table_configurations_build_and_pass",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("CARGO_TARGET_DIR", scratch.path())
        .output()
        .expect("spawn cargo");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "cargo test -p vela-domain failed:\n{stdout}\n{stderr}"
    );
    // The feature is really off in this build: the abort path runs through the always-compiled macro body, and the
    // test that only exists when the public macro is compiled in is absent.
    assert!(
        stdout.contains("fault::tests::an_armed_point_aborts_the_process ... ok"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("the_macro_is_a_no_op_for_an_unarmed_point_when_the_feature_is_on"),
        "the feature was on in a package-only build:\n{stdout}"
    );
}
