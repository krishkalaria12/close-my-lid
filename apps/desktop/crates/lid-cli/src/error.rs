//! Every error the CLI can produce, and how it is reported.
//!
//! The CLI is scripted against (that is the whole point on Linux), so failures
//! need two things a bare message does not give: a distinct exit code per
//! category, and a hint the user can act on. Both live here rather than being
//! formatted ad hoc at each call site.

use std::process::ExitCode;

use lidcore::LidError;

pub type Result<T> = std::result::Result<T, CliError>;

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error(transparent)]
    Lid(#[from] LidError),

    #[error("could not listen for Ctrl-C")]
    SignalHandler(#[source] ctrlc::Error),

    #[error("could not write output")]
    Output(#[source] std::io::Error),
}

impl CliError {
    /// Actionable next step, where there is one.
    pub fn hint(&self) -> Option<&str> {
        match self {
            Self::Lid(error) => error.hint(),
            Self::SignalHandler(_) => Some(
                "Another handler may already be installed. Try running the command \
                 directly rather than under a wrapper.",
            ),
            Self::Output(_) => None,
        }
    }

    /// Distinct codes so scripts can branch on the failure without parsing
    /// text. Values follow `sysexits.h`, which is what CLI tooling expects.
    pub fn exit_code(&self) -> ExitCode {
        match self {
            // EX_UNAVAILABLE: the feature does not exist on this machine.
            Self::Lid(LidError::UnsupportedPlatform { .. }) => ExitCode::from(69),
            // EX_NOPERM: understood and refused.
            Self::Lid(LidError::Denied { .. }) => ExitCode::from(77),
            // EX_OSFILE / EX_IOERR for state and output problems.
            Self::Lid(LidError::Io { .. } | LidError::Decode { .. } | LidError::Encode { .. }) => {
                ExitCode::from(72)
            }
            Self::Output(_) => ExitCode::from(74),
            _ => ExitCode::FAILURE,
        }
    }

    /// Prints the error, its underlying causes and its hint to stderr.
    ///
    /// Causes are printed because the platform's own wording is often the part
    /// that actually identifies the problem.
    pub fn report(&self) {
        eprintln!("close-my-lid: {self}");

        let mut source = std::error::Error::source(self);
        while let Some(cause) = source {
            eprintln!("  caused by: {cause}");
            source = std::error::Error::source(cause);
        }

        if let Some(hint) = self.hint() {
            eprintln!("\nhint: {hint}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unsupported_platform_is_distinguishable_from_a_refusal() {
        let unsupported = CliError::Lid(LidError::UnsupportedPlatform { os: "macos" });
        let denied = CliError::Lid(LidError::denied("hold the lid", "no"));

        // Scripts branch on these, so they must not collapse together.
        assert_ne!(
            format!("{:?}", unsupported.exit_code()),
            format!("{:?}", denied.exit_code())
        );
    }

    #[test]
    fn hints_pass_through_from_the_core() {
        let error =
            CliError::Lid(LidError::denied("hold the lid", "refused").with_hint("Log in locally."));
        assert_eq!(error.hint(), Some("Log in locally."));
    }

    #[test]
    fn core_errors_convert_without_losing_their_message() {
        let error: CliError = LidError::bus("no socket").into();
        assert!(error.to_string().contains("no socket"));
    }
}
