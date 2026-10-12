//! Interactive-session probe (P03, FR-041, AT-017, ADR-012): is an unlocked, connected desktop available for UI-automation
//! approval delivery? Anything uncertain is reported as unavailable, so delivery fails closed.
//!
//! Method: the session's connect state must be active (`WTSActive`), and the input desktop must be reachable
//! (`OpenInputDesktop` is denied while the workstation is locked or a secure desktop such as UAC is showing).

use vela_domain::ports::{SessionAvailability, SessionProbe};

/// Probes the current Windows session.
#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsSessionProbe;

impl WindowsSessionProbe {
    /// The probe is stateless.
    pub fn new() -> Self {
        Self
    }
}

#[cfg(windows)]
fn probe() -> SessionAvailability {
    use windows::Win32::System::RemoteDesktop::{
        ProcessIdToSessionId, WTS_CONNECTSTATE_CLASS, WTS_CURRENT_SERVER_HANDLE, WTSActive,
        WTSConnectState, WTSDisconnected, WTSFreeMemory, WTSQuerySessionInformationW,
    };
    use windows::Win32::System::StationsAndDesktops::{
        CloseDesktop, DESKTOP_CONTROL_FLAGS, DESKTOP_SWITCHDESKTOP, OpenInputDesktop,
    };
    use windows::Win32::System::Threading::GetCurrentProcessId;
    use windows::core::PWSTR;

    // SAFETY: documented Win32 calls; every out-pointer is a valid local and the WTS buffer is freed exactly once.
    unsafe {
        let mut session = 0u32;
        if ProcessIdToSessionId(GetCurrentProcessId(), &mut session).is_err() {
            return SessionAvailability::Unknown;
        }
        let mut buffer = PWSTR::null();
        let mut bytes = 0u32;
        if WTSQuerySessionInformationW(
            Some(WTS_CURRENT_SERVER_HANDLE),
            session,
            WTSConnectState,
            &mut buffer,
            &mut bytes,
        )
        .is_err()
        {
            return SessionAvailability::Unknown;
        }
        let state = if buffer.is_null() || (bytes as usize) < std::mem::size_of::<i32>() {
            None
        } else {
            Some(WTS_CONNECTSTATE_CLASS(*(buffer.0 as *const i32)))
        };
        if !buffer.is_null() {
            WTSFreeMemory(buffer.0.cast());
        }
        match state {
            Some(s) if s == WTSDisconnected => return SessionAvailability::Disconnected,
            Some(s) if s == WTSActive => {}
            _ => return SessionAvailability::Unknown,
        }
        match OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_SWITCHDESKTOP) {
            Ok(desktop) => {
                let _ = CloseDesktop(desktop);
                SessionAvailability::Available
            }
            // Denied while locked or on the secure desktop.
            Err(_) => SessionAvailability::Locked,
        }
    }
}

#[cfg(not(windows))]
fn probe() -> SessionAvailability {
    SessionAvailability::Unknown
}

impl SessionProbe for WindowsSessionProbe {
    fn interactive_session(&self) -> SessionAvailability {
        probe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_answers_without_panicking_and_only_available_permits_automation() {
        let answer = WindowsSessionProbe::new().interactive_session();
        assert_eq!(
            answer.permits_ui_automation(),
            answer == SessionAvailability::Available
        );
        #[cfg(not(windows))]
        assert_eq!(answer, SessionAvailability::Unknown);
    }
}
