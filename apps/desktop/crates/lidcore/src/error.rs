use std::io;

/// Every failure mode the core can surface to a UI.
#[derive(Debug, thiserror::Error)]
pub enum LidError {
    /// No lid backend exists for this OS. macOS hits this by design: it is
    /// served by the Swift app in `apps/macos`.
    #[error("close-my-lid has no lid backend for {0}; on macOS use the Close My Lid app")]
    UnsupportedPlatform(&'static str),

    /// The OS refused the request. On Linux this is usually polkit denying the
    /// inhibitor; on Windows, the power scheme write failing.
    #[error("the system refused the lid request: {0}")]
    Denied(String),

    /// The platform mechanism is present but did not behave as expected.
    #[error("lid backend error: {0}")]
    Backend(String),

    #[error("could not reach the system bus: {0}")]
    Bus(String),

    #[error("could not read or write saved session state: {0}")]
    Store(#[from] io::Error),

    #[error("saved session state was malformed: {0}")]
    Decode(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, LidError>;
