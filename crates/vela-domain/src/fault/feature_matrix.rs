//! Runs this crate's own tests in the feature configurations that the workspace build never selects.
//!
//! `cargo test --workspace` unifies Cargo features, so `vela-domain` is always built with its default `full` feature
//! and never with `fault-injection`, nor in the `hook-table`-only shape that the hook binary uses. The standard CI job
//! is `cargo test --workspace`, so this test is how those configurations are exercised there: it invokes `cargo test`
//! for this crate with `--no-default-features` and each configuration, in a scratch target directory (so it neither
//! takes the outer build's lock nor disturbs its cache), with warnings denied so that dead code in a configuration
//! fails the test.
//!
//! - `fault-injection`: the public `fault_point!` macro is compiled in (not the `()` expansion) and an armed point
//!   aborts a child process through it.
//! - `hook-table`: the crate builds and its tests pass with only `serde` (no `ts-rs`, no `serde_json`), as the hook
//!   binary needs.
//!
//! Inside the nested run the environment variable below is set and this test returns immediately, so it cannot recurse.

use std::path::Path;
use std::process::Command;

const NESTED_ENV: &str = "VELA_DOMAIN_FEATURE_MATRIX";

/// Runs `cargo test -p vela-domain --lib --no-default-features --features <features>` and returns the test output.
fn nested_test(features: &str, target_dir: &Path) -> String {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .args([
            "test",
            "-p",
            "vela-domain",
            "--lib",
            "--locked",
            "--no-default-features",
            "--features",
            features,
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env(NESTED_ENV, "1")
        .env("CARGO_TARGET_DIR", target_dir)
        .env("RUSTFLAGS", "-D warnings")
        .output()
        .expect("spawn cargo");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        output.status.success(),
        "cargo test --no-default-features --features {features} failed:\n{stdout}\n{stderr}"
    );
    stdout
}

#[test]
fn the_fault_injection_and_hook_table_configurations_build_and_pass() {
    if std::env::var_os(NESTED_ENV).is_some() {
        return;
    }
    let target_dir =
        std::env::temp_dir().join(format!("vela-domain-feature-matrix-{}", std::process::id()));

    let fault = nested_test("fault-injection", &target_dir);
    // The public macro really ran: the armed child aborted and the unarmed child survived, both through `fault_point!`.
    for test in [
        "fault::tests::an_armed_point_aborts_the_process ... ok",
        "fault::tests::an_unarmed_or_differently_armed_point_does_not_abort ... ok",
        "fault::tests::the_macro_is_a_no_op_for_an_unarmed_point_when_the_feature_is_on ... ok",
    ] {
        assert!(fault.contains(test), "missing `{test}` in:\n{fault}");
    }

    let hook = nested_test("hook-table", &target_dir);
    assert!(hook.contains("test result: ok."), "{hook}");
    assert!(
        !hook.contains("contract_export"),
        "the TypeScript export must not exist without the `full` feature:\n{hook}"
    );
    assert!(
        hook.contains("fault::tests::a_point_is_armed_only_by_its_exact_name ... ok"),
        "{hook}"
    );

    let _ = std::fs::remove_dir_all(&target_dir);
}
