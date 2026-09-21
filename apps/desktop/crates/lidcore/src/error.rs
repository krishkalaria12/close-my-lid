//! Every error the core can produce.
//!
//! Two things matter for a utility that runs in the background and touches
//! system power settings: the message must say what was being attempted, and
//! it must say what the user can do about it. So each variant carries the
//! attempted *action*, the platform's own wording as `detail`, and an optional
//! [`LidError::hint`] with the actionable next step.
//!
//! Backends supply their own hints rather than this module switching on the
//! platform, which keeps OS knowledge with the OS code.

use std::io;
use std::path::PathBuf;

/// The result type used throughout the core.
pub type Result<T> = std::result::Result<T, LidError>;

#[derive(Debug, thiserror::Error)]
pub enum LidError {
    /// No lid backend exists for this OS. macOS hits this by design.
    #[error("Close My Lid has no lid backend for {os}")]
    UnsupportedPlatform { os: &'static str },

    /// The OS understood the request and refused it. Usually a permissions
    /// problem, and usually fixable by the user, so the hint matters most here.
    #[error("the system refused to {action}: {detail}")]
    Denied {
        action: String,
        detail: String,
        hint: Option<String>,
    },

    /// The mechanism is present but did not behave as expected.
    #[error("could not {action}: {detail}")]
    Backend {
        action: String,
        detail: String,
        hint: Option<String>,
    },

    /// The system D-Bus could not be reached at all (Linux).
    #[error("could not reach the system bus: {detail}")]
    Bus { detail: String },

    #[error("could not {action} {}", path.display())]
    Io {
        action: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("saved session state at {} is not valid JSON", path.display())]
    Decode {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("could not work out where to store configuration")]
    NoConfigDir,
}

impl LidError {
    pub fn denied(action: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Denied {
            action: action.into(),
            detail: detail.into(),
            hint: None,
        }
    }

    pub fn backend(action: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Backend {
            action: action.into(),
            detail: detail.into(),
            hint: None,
        }
    }

    pub fn bus(detail: impl Into<String>) -> Self {
        Self::Bus {
            detail: detail.into(),
        }
    }

    pub fn io(action: &'static str, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            action,
            path: path.into(),
            source,
        }
    }

    /// Attaches the actionable next step. Ignored by variants that have no
    /// room for one, so callers can apply it unconditionally.
    #[must_use]
    pub fn with_hint(mut self, text: impl Into<String>) -> Self {
        match &mut self {
            Self::Denied { hint, .. } | Self::Backend { hint, .. } => *hint = Some(text.into()),
            _ => {}
        }
        self
    }

    /// What the user should try next, if anything.
    ///
    /// Shown by the CLI under the error and by the GUI in the panel, so it is
    /// written as a full sentence addressed to the user.
    pub fn hint(&self) -> Option<&str> {
        match self {
            Self::Denied { hint, .. } | Self::Backend { hint, .. } => hint.as_deref(),
            Self::UnsupportedPlatform { .. } => Some(
                "On macOS use the Close My Lid menu bar app in /Applications, \
                 which handles this natively.",
            ),
            Self::Bus { .. } => Some(
                "Close My Lid needs systemd-logind on the system bus. Check that \
                 systemd is running with `systemctl is-system-running`.",
            ),
            Self::NoConfigDir => Some(
                "No home directory could be determined. Set HOME (or APPDATA on \
                 Windows) and try again.",
            ),
            Self::Io { .. } | Self::Decode { .. } => None,
        }
    }

    /// True when retrying unchanged might work — used to decide whether a
    /// failure is worth surfacing to the user or just logging.
    pub fn is_transient(&self) -> bool {
        matches!(self, Self::Bus { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_name_the_attempted_action() {
        let error = LidError::denied("start a lid hold", "polkit said no");
        let rendered = error.to_string();
        assert!(rendered.contains("start a lid hold"), "{rendered}");
        assert!(rendered.contains("polkit said no"), "{rendered}");
    }

    #[test]
    fn hints_can_be_attached_and_read_back() {
        let error = LidError::denied("start a lid hold", "refused").with_hint("Log in locally.");
        assert_eq!(error.hint(), Some("Log in locally."));
    }

    #[test]
    fn platform_errors_carry_a_default_hint() {
        let error = LidError::UnsupportedPlatform { os: "macos" };
        assert!(error.hint().unwrap().contains("menu bar app"));
    }

    #[test]
    fn io_errors_name_the_path() {
        let error = LidError::io(
            "write",
            "/tmp/session.json",
            io::Error::new(io::ErrorKind::PermissionDenied, "denied"),
        );
        assert!(error.to_string().contains("/tmp/session.json"));
        // The underlying cause stays reachable for logging.
        assert!(std::error::Error::source(&error).is_some());
    }

    #[test]
    fn only_bus_failures_are_transient() {
        assert!(LidError::bus("no socket").is_transient());
        assert!(!LidError::denied("hold", "no").is_transient());
    }
}
