//! Launch at login, via `SMAppService`.
//!
//! `SMAppService.mainApp` registers the *bundle* as a login item, so this only
//! does anything for an installed `Ichi.app` — running the bare binary from
//! `cargo run` has no bundle to register and reports `NotFound`.
//!
//! The system is the source of truth here, not Ichi's config file: the user can
//! disable Ichi from System Settings → General → Login Items at any time, and
//! the menu has to reflect that rather than its own stale copy.
//!
//! Requires macOS 13. Earlier systems have only the deprecated
//! `SMLoginItemSetEnabled` or a hand-written LaunchAgent plist, neither of
//! which shows up correctly in modern System Settings.

use objc2_service_management::{SMAppService, SMAppServiceStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginItemState {
    Enabled,
    Disabled,
    /// Registered, but the user has not yet approved it in System Settings.
    RequiresApproval,
    /// No bundle to register — running unbundled, e.g. under `cargo run`.
    Unavailable,
}

impl LoginItemState {
    /// Whether the menu item should show a checkmark.
    pub fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }

    pub fn menu_title(self) -> &'static str {
        match self {
            Self::RequiresApproval => "Launch at Login (approve in Settings)",
            Self::Unavailable => "Launch at Login (install to /Applications)",
            _ => "Launch at Login",
        }
    }
}

pub fn state() -> LoginItemState {
    unsafe {
        let service = SMAppService::mainAppService();
        match service.status() {
            SMAppServiceStatus::Enabled => LoginItemState::Enabled,
            SMAppServiceStatus::RequiresApproval => LoginItemState::RequiresApproval,
            SMAppServiceStatus::NotFound => LoginItemState::Unavailable,
            _ => LoginItemState::Disabled,
        }
    }
}

/// Turn launch-at-login on or off, returning the resulting state.
///
/// Errors are surfaced to stderr rather than propagated: this is driven from a
/// menu click with nowhere useful to report a failure, and the returned state
/// already tells the caller whether it took effect.
pub fn set(enabled: bool) -> LoginItemState {
    unsafe {
        let service = SMAppService::mainAppService();
        let result = if enabled {
            service.registerAndReturnError()
        } else {
            service.unregisterAndReturnError()
        };

        if let Err(error) = result {
            eprintln!(
                "ichi: could not {} launch at login: {}",
                if enabled { "enable" } else { "disable" },
                error.localizedDescription()
            );
        }
    }
    state()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_enabled_shows_a_checkmark() {
        assert!(LoginItemState::Enabled.is_enabled());
        assert!(!LoginItemState::Disabled.is_enabled());
        // Registered-but-unapproved must not claim to be on, or the checkmark
        // would contradict what System Settings shows.
        assert!(!LoginItemState::RequiresApproval.is_enabled());
        assert!(!LoginItemState::Unavailable.is_enabled());
    }

    #[test]
    fn ambiguous_states_explain_themselves_in_the_menu() {
        assert_eq!(LoginItemState::Enabled.menu_title(), "Launch at Login");
        assert_eq!(LoginItemState::Disabled.menu_title(), "Launch at Login");
        assert!(
            LoginItemState::RequiresApproval
                .menu_title()
                .contains("approve")
        );
        assert!(
            LoginItemState::Unavailable
                .menu_title()
                .contains("/Applications")
        );
    }

    #[test]
    fn querying_state_does_not_crash_unbundled() {
        // Under `cargo test` there is no app bundle, so this should report
        // Unavailable rather than trapping in SMAppService.
        assert_eq!(state(), LoginItemState::Unavailable);
    }
}
