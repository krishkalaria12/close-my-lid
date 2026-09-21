//! Every error the tray app can produce.
//!
//! A tray app has nowhere obvious to print a failure, so errors here are built
//! to be *displayed*: [`GuiError::headline`] gives the one line that fits in
//! the panel, and the hint carries the rest. Nothing is reported with a bare
//! `String`, so the panel can style refusals differently from bugs.

use lidcore::LidError;

pub type Result<T> = std::result::Result<T, GuiError>;

#[derive(Debug, thiserror::Error)]
pub enum GuiError {
    #[error(transparent)]
    Lid(#[from] LidError),

    /// No backend could be built at startup, so no control is possible. The
    /// panel shows this instead of controls that could not work.
    #[error("Close My Lid cannot control this system's lid behaviour")]
    NoBackend {
        #[source]
        source: LidError,
    },

    #[error("could not open the panel window")]
    Window { detail: String },
}

impl GuiError {
    /// Short enough for the panel's error row.
    pub fn headline(&self) -> String {
        match self {
            Self::Lid(error) => error.to_string(),
            Self::NoBackend { .. } => "Lid control unavailable".to_string(),
            Self::Window { .. } => "Could not open the panel".to_string(),
        }
    }

    /// The actionable next step, shown under the headline.
    pub fn hint(&self) -> Option<&str> {
        match self {
            Self::Lid(error) => error.hint(),
            Self::NoBackend { source } => source.hint(),
            Self::Window { .. } => None,
        }
    }

    /// Whether the failure is worth interrupting the user with a notification,
    /// as opposed to only showing in the panel. Refusals are; transient bus
    /// problems are not, because they usually resolve on the next attempt.
    pub fn deserves_notification(&self) -> bool {
        match self {
            Self::Lid(error) | Self::NoBackend { source: error } => !error.is_transient(),
            Self::Window { .. } => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headlines_stay_short_enough_for_the_panel() {
        let error = GuiError::NoBackend {
            source: LidError::UnsupportedPlatform { os: "macos" },
        };
        assert!(error.headline().len() < 40, "{}", error.headline());
    }

    #[test]
    fn hints_pass_through_from_the_core() {
        let error = GuiError::NoBackend {
            source: LidError::UnsupportedPlatform { os: "macos" },
        };
        assert!(error.hint().unwrap().contains("menu bar app"));
    }

    #[test]
    fn transient_bus_failures_do_not_raise_a_notification() {
        let transient = GuiError::Lid(LidError::bus("no socket"));
        let refusal = GuiError::Lid(LidError::denied("hold the lid", "no"));

        assert!(!transient.deserves_notification());
        assert!(refusal.deserves_notification());
    }
}
