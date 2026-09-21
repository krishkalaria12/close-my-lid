//! Close My Lid for macOS — the menu bar app.
//!
//! One binary with two jobs, as the Swift app had:
//!
//! - With no arguments it runs the menu bar app.
//! - With arguments it answers the small command set in [`cli`], including the
//!   hidden `--watchdog` pass that the dead-man LaunchAgent invokes every
//!   minute. The agent points at *this* executable, which is why the two
//!   cannot simply be split into separate binaries.
//!
//! The mechanism is `pmset -a disablesleep`, applied through a scoped sudoers
//! grant so a hold does not ask for a password every time; everything about
//! that, and about the session itself, lives in `lidcore` and is shared with
//! the Linux and Windows builds.

// Every dependency below is an Apple framework binding, so a build for another
// target would otherwise fail with a wall of missing-crate errors. Say why
// instead. Build per package (`cargo build -p …`); the workspace deliberately
// keeps one platform-specific front end per OS.
#[cfg(not(target_os = "macos"))]
compile_error!(
    "lid-macos is macOS-only; use `lid-gui` on Windows and the `close-my-lid` CLI on Linux"
);

mod alert;
mod app;
mod cli;
mod config;
mod controller;
mod icons;
mod login_item;
mod notifications;
mod panel;
mod settings;
mod ui;
mod updates;

use std::process::ExitCode;

use objc2_foundation::NSBundle;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let command = cli::parse(&arguments);

    // Every path, not just the menu bar one. The `--watchdog` pass is invoked
    // by launchd with nowhere for its output to go, so without this a release
    // that keeps failing every minute would say so to nobody.
    init_tracing();

    match command {
        cli::Command::MenuBar => {
            app::run();
            ExitCode::SUCCESS
        }
        command => cli::run(command),
    }
}

/// Logs to the unified log, and to stderr as well.
///
/// stderr alone was not enough. An app launched from Finder or registered as a
/// login item is started by `launchd` with its output discarded, and the
/// watchdog LaunchAgent declares no `StandardErrorPath` either — so for the
/// two ways this binary actually runs in production, everything it had to say
/// went nowhere. `os_log` is where macOS keeps this, which makes it reachable
/// with:
///
/// ```text
/// log stream --predicate 'subsystem == "app.closemylid.CloseMyLid"'
/// ```
///
/// stderr stays as well rather than being made conditional on a terminal:
/// it costs nothing when there is nowhere for it to go, and it is what a shell
/// pipeline and CI read.
///
/// Quiet by default: an app that runs all day should say nothing unless
/// something went wrong, and `RUST_LOG` turns the detail back on when it does.
fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| config::DEFAULT_LOG_LEVEL.into());

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_oslog::OsLogger::new(
            &*log_subsystem(),
            config::LOG_CATEGORY,
        ))
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_writer(std::io::stderr),
        )
        .init();
}

/// The bundle's own identifier, so the subsystem can never disagree with what
/// is in `Info.plist`. Falls back to a constant for unbundled builds, which
/// have no identifier at all.
fn log_subsystem() -> String {
    NSBundle::mainBundle()
        .bundleIdentifier()
        .map(|identifier| identifier.to_string())
        .unwrap_or_else(|| config::FALLBACK_LOG_SUBSYSTEM.to_string())
}
