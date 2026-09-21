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

mod render;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration as StdDuration;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use lidcore::{APP_NAME, SessionDuration, SleepSessionController, VERSION};

/// How often the supervision loop checks for expiry and low battery.
const TICK: StdDuration = StdDuration::from_secs(15);

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
        #[arg(long = "for", default_value = "unlimited", value_parser = SessionDuration::parse)]
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

fn main() -> Result<()> {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    match cli.command {
        Command::Enable { duration } => enable(duration),
        Command::Disable => disable(),
        Command::Status { json } => status(json),
        Command::Agents { json } => {
            render::agents(&lidcore::sessions_now(), json);
            Ok(())
        }
        Command::Systemd => {
            print!("{}", render::SYSTEMD_UNIT);
            Ok(())
        }
    }
}

fn init_tracing(verbose: bool) {
    let level = if verbose { "debug" } else { "warn" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| level.into()),
        )
        .with_target(false)
        .without_time()
        .init();
}

fn controller() -> Result<SleepSessionController> {
    SleepSessionController::new().context("could not set up a lid backend for this system")
}

fn enable(duration: SessionDuration) -> Result<()> {
    let mut controller = controller()?;
    controller.reconcile_at_launch()?;
    controller.start(duration)?;

    println!(
        "{APP_NAME} is holding the lid open ({}).\nMechanism: {}.\nPress Ctrl-C to release.",
        duration.label(),
        controller.describe_backend()
    );

    let running = Arc::new(AtomicBool::new(true));
    let flag = running.clone();
    ctrlc::set_handler(move || flag.store(false, Ordering::SeqCst))
        .context("could not install a Ctrl-C handler")?;

    // The hold lives as long as this loop does. On Linux that is literal: the
    // logind descriptor is owned by the controller and closes when we return.
    while running.load(Ordering::SeqCst) {
        if controller.tick()? {
            println!("\nSession ended; normal sleep restored.");
            return Ok(());
        }
        std::thread::sleep(TICK);
    }

    controller.stop()?;
    println!("\nReleased. Normal sleep restored.");
    Ok(())
}

fn disable() -> Result<()> {
    let mut controller = controller()?;
    controller.stop()?;
    println!("{APP_NAME} restored normal sleep behaviour.");
    Ok(())
}

fn status(json: bool) -> Result<()> {
    let controller = controller()?;
    render::status(&controller.state(), controller.describe_backend(), json);
    Ok(())
}
