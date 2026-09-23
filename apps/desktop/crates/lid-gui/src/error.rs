//! Every error the desktop app can produce.
//!
//! A windowed app has no console to print a failure to, so errors here are
//! built to be *displayed*: [`GuiError::headline`] gives the one line that fits
//! in the panel's banner, and the hint carries the rest. Nothing is reported
//! with a bare `String`, so the panel can style refusals differently from bugs.

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

    #[error("could not open the window: {detail}")]
    Window { detail: String },

    #[error("could not read the update feed: {detail}")]
    UpdateFeed { detail: String },

    #[error("could not change launch at login: {detail}")]
    LoginItem { detail: String },

    #[error("could not save settings: {detail}")]
    Settings { detail: String },
}

impl GuiError {
    /// Short enough for the panel's banner.
    pub fn headline(&self) -> String {
        match self {
            Self::Lid(error) => capitalise(&error.to_string()),
            Self::NoBackend { .. } => "Lid control unavailable".to_string(),
            Self::Window { .. } => "Could not open the window".to_string(),
            Self::UpdateFeed { .. } => "Could not check for updates".to_string(),
            Self::LoginItem { .. } => "Could not change launch at login".to_string(),
            Self::Settings { .. } => "Could not save settings".to_string(),
        }
    }

    /// The actionable next step, shown under the headline.
    pub fn hint(&self) -> Option<&str> {
        match self {
            Self::Lid(error) => error.hint(),
            Self::NoBackend { source } => source.hint(),
            Self::UpdateFeed { .. } => Some("Check your connection and try again."),
            Self::LoginItem { detail } | Self::Settings { detail } | Self::Window { detail } => {
                Some(detail)
            }
        }
    }
}

/// Core errors are written as sentence fragments for the CLI's `error: …`
/// prefix; the banner shows them on their own.
fn capitalise(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headlines_stay_short_enough_for_the_banner() {
        let error = GuiError::NoBackend {
            source: LidError::UnsupportedPlatform { os: "macos" },
        };
        assert!(error.headline().len() < 40, "{}", error.headline());
    }

    #[test]
    fn hints_pass_through_from_the_core() {
        let expected = LidError::UnsupportedPlatform { os: "macos" }
            .hint()
            .map(str::to_owned);
        let error = GuiError::NoBackend {
            source: LidError::UnsupportedPlatform { os: "macos" },
        };
        assert!(expected.is_some());
        assert_eq!(error.hint().map(str::to_owned), expected);
    }

    #[test]
    fn core_messages_read_as_sentences_in_the_banner() {
        assert_eq!(capitalise("could not hold"), "Could not hold");
        assert_eq!(capitalise(""), "");
    }
}
