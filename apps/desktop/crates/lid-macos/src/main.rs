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

use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();

    match cli::parse(&arguments) {
        cli::Command::MenuBar => {
            init_tracing();
            app::run();
            ExitCode::SUCCESS
        }
        command => cli::run(command),
    }
}

/// Logs to stderr, which `Console.app` captures for a bundled app. Quiet by
/// default: an app that runs all day should say nothing unless something went
/// wrong, and `RUST_LOG` turns the detail back on when it does.
fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| config::DEFAULT_LOG_LEVEL.into()),
        )
        .with_target(false)
        .init();
}
