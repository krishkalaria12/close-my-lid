//! `close-my-lid` — the command line interface.
//!
//! This is the primary surface on Linux, where a system tray cannot be relied
//! on: stock GNOME ships no tray without the AppIndicator extension, and
//! Wayland will not let a client anchor a panel under a tray icon anyway. It
//! is also available on Windows alongside the tray app.
//!
//! The command deliberately blocks while holding, in the same spirit as
//! `systemd-inhibit` and macOS `caffeinate`. On Linux it has to: the logind
//! inhibitor lives only as long as the file descriptor is open, so something
//! must stay alive. Use the systemd user unit from `close-my-lid systemd` to
//! run it in the background.
//!
//! Errors and exit codes are defined in [`error`]; tunables in [`config`].

mod config;
mod error;
mod render;

use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::{Parser, Subcommand};
use lidcore::{APP_NAME, HoldLock, SessionDuration, SleepSessionController, VERSION};

use crate::error::{CliError, Result};

#[derive(Parser)]
#[command(
    name = "close-my-lid",
    version = VERSION,
    about = "Keep this machine awake with the lid closed while long-running work finishes."
)]
struct Cli {
    /// Print verbose diagnostics.
    #[arg(long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Hold the lid open until the duration elapses or you press Ctrl-C.
    Enable {
        /// `30m`, `2h`, `90` (minutes) or `unlimited`.
        #[arg(
            long = "for",
            default_value = config::DEFAULT_DURATION,
            value_parser = SessionDuration::parse,
        )]
        duration: SessionDuration,
    },
    /// Release any hold this machine is under.
    Disable,
    /// Report whether a hold is active.
    Status {
        /// Emit JSON for scripting.
        #[arg(long)]
        json: bool,
    },
    /// List detected coding-agent sessions.
    Agents {
        #[arg(long)]
        json: bool,
    },
    /// Print a systemd user unit for running a hold in the background.
    Systemd,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    match run(cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            error.report();
            error.exit_code()
        }
    }
}

fn run(command: Command) -> Result<()> {
    match command {
        Command::Enable { duration } => enable(duration),
        Command::Disable => disable(),
        Command::Status { json } => status(json),
        Command::Agents { json } => render::agents(&lidcore::sessions_now(), json),
        Command::Systemd => render::systemd_unit(),
    }
}

fn init_tracing(verbose: bool) {
    let level = if verbose {
        config::VERBOSE_LOG_LEVEL
    } else {
        config::DEFAULT_LOG_LEVEL
    };

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| level.into()),
        )
        .with_target(false)
        .without_time()
        .init();
}

fn enable(duration: SessionDuration) -> Result<()> {
    // Held for the lifetime of this function. A second `enable` fails here
    // rather than taking an overlapping hold, which on Windows would see the
    // first hold's temporary values as if they were the user's own.
    let _lock = HoldLock::acquire()?;

    let mut controller = SleepSessionController::new()?;
    controller.reconcile_at_launch()?;
    controller.start(duration)?;

    render::hold_started(duration, controller.describe_backend())?;

    let running = Arc::new(AtomicBool::new(true));
    let flag = running.clone();
    ctrlc::set_handler(move || flag.store(false, Ordering::SeqCst))
        .map_err(CliError::SignalHandler)?;

    // The hold lives as long as this loop does. On Linux that is literal: the
    // logind descriptor is owned by the controller and closes when we return.
    // Sleep in short slices so Ctrl-C exits promptly instead of waiting out
    // the full 15s supervision interval.
    while running.load(Ordering::SeqCst) {
        if controller.tick()? {
            return render::hold_expired();
        }
        let deadline = std::time::Instant::now() + config::SUPERVISION_INTERVAL;
        while running.load(Ordering::SeqCst) && std::time::Instant::now() < deadline {
            std::thread::sleep(config::SHUTDOWN_POLL_INTERVAL);
        }
    }

    controller.stop()?;
    render::hold_released()
}

fn disable() -> Result<()> {
    // A hold taken by another process cannot be released from here: on Linux
    // the inhibitor is a descriptor that only its owner can close. Ask that
    // process to stop and let it run its own release path.
    if let Some(pid) = HoldLock::signal_owner() {
        render::asked_owner_to_stop(pid)?;
        wait_for_owner_to_exit();
        // If the owner is still alive, its hold is still in force. Clearing
        // our own state file now would report "Off" while the machine is
        // still held, so fail loudly instead.
        if let Some(still) = HoldLock::owner() {
            return Err(lidcore::LidError::denied(
                "stop the existing hold",
                format!("the holding process (pid {still}) did not exit"),
            )
            .with_hint(
                "The holding process may be ignoring the stop request. \
                 Press Ctrl-C in its terminal, or stop it from Task Manager / `kill`.",
            )
            .into());
        }
        // Owner exited cleanly: it already ran its own release path.
        // Fall through to clean any stranded OS state below.
    }

    let mut controller = SleepSessionController::new()?;
    controller.reconcile_at_launch()?;
    controller.stop()?;
    render::line(&format!("{APP_NAME} restored normal sleep behaviour."))
}

/// Waits for the signalled owner to release, so the message this command
/// prints is true by the time the user reads it.
fn wait_for_owner_to_exit() {
    for _ in 0..config::RELEASE_WAIT_POLLS {
        if HoldLock::owner().is_none() {
            return;
        }
        std::thread::sleep(config::RELEASE_POLL_INTERVAL);
    }
    tracing::warn!("the holding process did not exit in time");
}

fn status(json: bool) -> Result<()> {
    // Read-only backend: on Windows this avoids adopting another process's
    // recovery record (which must never be released by an inspector).
    let backend = lidcore::backend_readonly()?;
    let store = lidcore::SleepSessionStore::new()?;
    let controller = SleepSessionController::with_parts(backend, store);
    // The state file records intent, not reality. A killed `enable` on Linux
    // loses its inhibitor silently, so ask what is actually in force.
    render::status(
        &controller.state(),
        controller.is_really_held(),
        HoldLock::owner(),
        controller.describe_backend(),
        json,
    )
}
