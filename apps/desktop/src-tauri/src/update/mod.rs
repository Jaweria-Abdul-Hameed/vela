//! Updates (Z02, AT-024): signed, never applied during a run, backed up first.
//!
//! * **Signature.** An update is verified with the Tauri updater signature (a minisign Ed25519 signature), which is not Windows
//!   Authenticode (Z01). A tampered artifact and an artifact signed with another key are rejected.
//! * **Deferral.** An update is only ever applied when no run is active, and only on an explicit user action (`UpdateGate`).
//! * **Backup and rollback.** The state store is backed up before the update is applied; the next start applies migrations after
//!   its own backup and restores it if a migration fails, refusing to run newer logic on the old store (F06).
//! * **Keys.** The committed configuration carries no production updater key. Development and tests use a *disposable* keypair
//!   whose public key is marked as a test key; a release-profile configuration that carries a test key fails closed.
//!
//! Where the update comes from (the endpoint) is configuration, not code; the production location is decided with H04.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use minisign_verify::{PublicKey, Signature};

/// The marker a disposable test updater key carries in its public key comment. A release build refuses any key with it.
pub const TEST_KEY_MARKER: &str = "disposable test updater key";

/// What the application knows about its runs when an update is offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunActivity {
    /// Runs that are starting, running or stopping.
    pub active: usize,
    /// Runs that are paused or waiting for a person (quiescent: safe to update, resumable afterwards).
    pub quiescent: usize,
}

/// Whether an update may be applied now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateDecision {
    /// Nothing is running: the user may apply it.
    MayApply,
    /// It must wait, and why (shown to the user as is).
    Deferred(String),
}

/// The deferral rule (AT-024). The user's explicit action is a separate input: an update never applies by itself.
pub fn can_apply(activity: RunActivity, user_asked: bool) -> UpdateDecision {
    if activity.active > 0 {
        return UpdateDecision::Deferred(format!(
            "{} run{} still working. The update waits until they are paused or finished.",
            activity.active,
            if activity.active == 1 { " is" } else { "s are" }
        ));
    }
    if !user_asked {
        return UpdateDecision::Deferred(
            "An update is ready. It is applied only when you choose to.".to_owned(),
        );
    }
    UpdateDecision::MayApply
}

/// Why a signature was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    /// The public key or the signature could not be read.
    Malformed(String),
    /// The signature does not match the artifact for this key.
    Rejected,
}

fn decode_text(b64: &str) -> Result<String, VerifyError> {
    let bytes = STANDARD
        .decode(b64.trim())
        .map_err(|e| VerifyError::Malformed(e.to_string()))?;
    String::from_utf8(bytes).map_err(|e| VerifyError::Malformed(e.to_string()))
}

/// Verifies `artifact` against a Tauri updater signature. Both strings are in the form Tauri uses: the base64 of the minisign
/// public key file, and the base64 of the minisign signature file.
pub fn verify_artifact(
    public_key_b64: &str,
    signature_b64: &str,
    artifact: &[u8],
) -> Result<(), VerifyError> {
    let key = PublicKey::decode(&decode_text(public_key_b64)?)
        .map_err(|e| VerifyError::Malformed(e.to_string()))?;
    let signature = Signature::decode(&decode_text(signature_b64)?)
        .map_err(|e| VerifyError::Malformed(e.to_string()))?;
    key.verify(artifact, &signature, false)
        .map_err(|_| VerifyError::Rejected)
}

/// The comment line of a Tauri-style public key (the base64 of a minisign public key file).
pub fn key_comment(public_key_b64: &str) -> Option<String> {
    decode_text(public_key_b64)
        .ok()?
        .lines()
        .next()
        .map(|l| l.trim_start_matches("untrusted comment:").trim().to_owned())
}

/// Whether `public_key_b64` is a disposable test key.
pub fn is_test_key(public_key_b64: &str) -> bool {
    key_comment(public_key_b64).is_some_and(|c| c.to_ascii_lowercase().contains(TEST_KEY_MARKER))
}

/// What a release-profile configuration must satisfy, checked against the merged Tauri configuration JSON.
///
/// * the updater, when configured, carries a non-empty public key that is **not** a test key; and
/// * the test overlay's key never reaches a release build.
pub fn check_release_config(config_json: &str) -> Result<(), String> {
    let v: serde_json::Value = serde_json::from_str(config_json)
        .map_err(|e| format!("the configuration is not JSON: {e}"))?;
    let Some(updater) = v.pointer("/plugins/updater") else {
        return Ok(());
    };
    let key = updater
        .get("pubkey")
        .and_then(|k| k.as_str())
        .unwrap_or("")
        .trim();
    if key.is_empty() {
        return Err("a release build with the updater configured needs the production public key (H04); none is set".into());
    }
    if is_test_key(key) {
        return Err(
            "the updater public key is a disposable test key; a release build refuses it".into(),
        );
    }
    if PublicKey::decode(&decode_text(key).map_err(|e| format!("{e:?}"))?).is_err() {
        return Err("the updater public key is not a valid minisign key".into());
    }
    Ok(())
}

/// Registers the updater plugin when, and only when, the configuration has an updater section (so a build without a key never
/// starts a plugin that cannot verify anything).
pub fn install<R: tauri::Runtime>(
    app: &mut tauri::App<R>,
) -> Result<(), Box<dyn std::error::Error>> {
    if app.config().plugins.0.contains_key("updater") {
        app.handle()
            .plugin(tauri_plugin_updater::Builder::new().build())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
