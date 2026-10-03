//! Real-environment Antigravity compatibility suite (A15). Nothing here talks to Antigravity yet.
//!
//! Every test that touches the real account must be gated by `VELA_REAL_ANTIGRAVITY=1`.

/// Name of the environment variable that enables the real-environment suite.
pub const GATE_ENV: &str = "VELA_REAL_ANTIGRAVITY";

/// True only for the exact value `1`; absent, empty, `0`, or `true` all keep the suite disabled.
pub fn gate_enabled(value: Option<&str>) -> bool {
    value == Some("1")
}

#[cfg(test)]
mod tests {
    use super::gate_enabled;

    #[test]
    fn gate_requires_exactly_one() {
        assert!(gate_enabled(Some("1")));
        assert!(!gate_enabled(None));
        for other in ["", "0", "true", " 1", "1 "] {
            assert!(
                !gate_enabled(Some(other)),
                "{other:?} must not enable the suite"
            );
        }
    }
}
