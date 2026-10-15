use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use blake2::{Blake2b512, Digest};
use ed25519_dalek::{Signer, SigningKey};

use super::*;

/// A disposable updater keypair in the minisign format Tauri uses. It exists only in memory for one test, is derived from the
/// clock, the process and a label, and is never written down or reused.
struct TestKey {
    signing: SigningKey,
    key_id: [u8; 8],
    comment: String,
}

impl TestKey {
    fn new(label: &str, comment: &str) -> Self {
        let mut h = Blake2b512::new();
        h.update(label.as_bytes());
        h.update(std::process::id().to_le_bytes());
        h.update(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
                .to_le_bytes(),
        );
        let digest = h.finalize();
        let mut seed = [0u8; 32];
        seed.copy_from_slice(&digest[..32]);
        let mut key_id = [0u8; 8];
        key_id.copy_from_slice(&digest[32..40]);
        Self {
            signing: SigningKey::from_bytes(&seed),
            key_id,
            comment: comment.to_owned(),
        }
    }

    /// The `plugins.updater.pubkey` form: the base64 of the public key file.
    fn public_b64(&self) -> String {
        let mut raw = b"Ed".to_vec();
        raw.extend_from_slice(&self.key_id);
        raw.extend_from_slice(self.signing.verifying_key().as_bytes());
        let file = format!(
            "untrusted comment: {}\n{}\n",
            self.comment,
            STANDARD.encode(raw)
        );
        STANDARD.encode(file)
    }

    /// The update manifest\x27s `signature` form: the base64 of the signature file.
    fn sign_b64(&self, artifact: &[u8]) -> String {
        let prehash = Blake2b512::digest(artifact);
        let sig = self.signing.sign(&prehash).to_bytes();
        let mut raw = b"ED".to_vec();
        raw.extend_from_slice(&self.key_id);
        raw.extend_from_slice(&sig);
        let trusted = "timestamp:1700000000\tfile:vela-setup.exe";
        let mut global_input = sig.to_vec();
        global_input.extend_from_slice(trusted.as_bytes());
        let global = self.signing.sign(&global_input).to_bytes();
        let file = format!(
            "untrusted comment: signature from tauri secret key\n{}\ntrusted comment: {trusted}\n{}\n",
            STANDARD.encode(raw),
            STANDARD.encode(global)
        );
        STANDARD.encode(file)
    }
}

fn test_key(label: &str) -> TestKey {
    TestKey::new(
        label,
        "Vela disposable test updater key (never for production)",
    )
}

const ARTIFACT: &[u8] = b"pretend this is the NSIS installer for build N";

#[test]
fn an_artifact_signed_with_the_test_key_verifies() {
    let k = test_key("ok");
    assert_eq!(
        verify_artifact(&k.public_b64(), &k.sign_b64(ARTIFACT), ARTIFACT),
        Ok(())
    );
}

#[test]
fn a_tampered_artifact_is_rejected() {
    let k = test_key("tamper");
    let sig = k.sign_b64(ARTIFACT);
    let mut bad = ARTIFACT.to_vec();
    bad[0] ^= 1;
    assert_eq!(
        verify_artifact(&k.public_b64(), &sig, &bad),
        Err(VerifyError::Rejected)
    );
    assert_eq!(
        verify_artifact(&k.public_b64(), &sig, b""),
        Err(VerifyError::Rejected)
    );
}

#[test]
fn an_artifact_signed_with_a_different_key_is_rejected() {
    let ours = test_key("ours");
    let theirs = test_key("theirs");
    assert_eq!(
        verify_artifact(&ours.public_b64(), &theirs.sign_b64(ARTIFACT), ARTIFACT),
        Err(VerifyError::Rejected)
    );
}

#[test]
fn garbage_keys_and_signatures_are_malformed_not_accepted() {
    let k = test_key("garbage");
    for (key, sig) in [
        ("not base64!!", k.sign_b64(ARTIFACT)),
        (k.public_b64().as_str(), "not base64!!".to_owned()),
        (
            k.public_b64().as_str(),
            STANDARD.encode("untrusted comment: x\nAAAA\n"),
        ),
        ("", String::new()),
    ] {
        assert!(
            matches!(
                verify_artifact(key, &sig, ARTIFACT),
                Err(VerifyError::Malformed(_))
            ),
            "{key:?} {sig:?}"
        );
    }
}

#[test]
fn the_test_key_is_recognised_and_an_ordinary_key_is_not() {
    let test = test_key("marker");
    assert!(is_test_key(&test.public_b64()));
    assert!(!is_test_key(
        &TestKey::new("prod", "minisign public key: ABCDEF").public_b64()
    ));
    assert_eq!(key_comment("###"), None);
}

#[test]
fn a_release_configuration_that_carries_the_test_key_fails_closed() {
    let test = test_key("release");
    let with = |key: &str| {
        format!(
            r#"{{"plugins":{{"updater":{{"pubkey":"{key}","endpoints":["https://updates.invalid/latest.json"]}}}}}}"#
        )
    };
    let err = check_release_config(&with(&test.public_b64())).unwrap_err();
    assert!(err.contains("test key"), "{err}");
    // No key at all with the updater configured also fails closed.
    assert!(
        check_release_config(&with(""))
            .unwrap_err()
            .contains("production public key")
    );
    // A key that is not a minisign key is refused.
    assert!(check_release_config(&with(&STANDARD.encode("hello"))).is_err());
    // A real-shaped production key passes; no updater section at all passes (the updater is simply off).
    let prod = TestKey::new("prod", "minisign public key: 1A2B3C4D5E6F7A8B");
    assert_eq!(check_release_config(&with(&prod.public_b64())), Ok(()));
    assert_eq!(check_release_config(r#"{"bundle":{}}"#), Ok(()));
    assert!(check_release_config("{").is_err());
}

#[test]
fn the_committed_configuration_carries_no_updater_key() {
    let base = include_str!("../../tauri.conf.json");
    assert!(
        !base.contains("\"updater\""),
        "the base configuration must not name an updater key; development uses the test overlay"
    );
    assert_eq!(check_release_config(base), Ok(()));
    let release = include_str!("../../tauri.release.conf.json");
    assert_eq!(check_release_config(release), Ok(()));
}

#[test]
fn an_update_is_deferred_while_runs_work_and_never_applies_without_the_user() {
    let busy = RunActivity {
        active: 2,
        quiescent: 0,
    };
    for asked in [false, true] {
        let UpdateDecision::Deferred(why) = can_apply(busy, asked) else {
            panic!("an update must wait for active runs")
        };
        assert!(why.contains("2 runs are still working"), "{why}");
    }
    let one = RunActivity {
        active: 1,
        quiescent: 0,
    };
    assert!(
        matches!(can_apply(one, true), UpdateDecision::Deferred(w) if w.contains("1 run is still"))
    );
    // Quiescent (paused or waiting) runs do not block, but nothing applies until the user asks.
    let quiet = RunActivity {
        active: 0,
        quiescent: 3,
    };
    assert!(matches!(
        can_apply(quiet, false),
        UpdateDecision::Deferred(_)
    ));
    assert_eq!(can_apply(quiet, true), UpdateDecision::MayApply);
    assert_eq!(
        can_apply(
            RunActivity {
                active: 0,
                quiescent: 0
            },
            true
        ),
        UpdateDecision::MayApply
    );
}
