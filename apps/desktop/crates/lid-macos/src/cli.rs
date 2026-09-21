//! The commands the bundled executable answers without opening a window.
//!
//! Carried over from the Swift app's `CommandLineInterface`. This is
//! deliberately a small subset of the `close-my-lid` CLI: it exists because
//! the app bundle's own binary is what the watchdog LaunchAgent invokes, and
//! because a Homebrew install of the app should still answer `--version` and
//! `status`. Anything richer belongs in the `lid-cli` crate.

use std::process::ExitCode;

use lidcore::heartbeat::{self, HoldHeartbeat, HoldHeartbeatStore};
use lidcore::power::macos::PmsetLidGuard;
use lidcore::{
    APP_NAME, LidPowerBackend, SessionDuration, SleepSessionController, VERSION, WatchdogPolicy,
    watchdog_once,
};

/// What the process was asked to do.
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    /// No arguments: run the menu bar app.
    MenuBar,
    Enable,
    Disable,
    Status,
    Version,
    Help,
    /// One dead-man pass for the watchdog LaunchAgent.
    Watchdog,
    Unknown(String),
}

pub fn parse(arguments: &[String]) -> Command {
    let Some(first) = arguments.first() else {
        return Command::MenuBar;
    };

    match first.as_str() {
        "enable" => Command::Enable,
        "disable" => Command::Disable,
        "status" => Command::Status,
        "--version" | "-v" | "version" => Command::Version,
        "--help" | "-h" | "help" => Command::Help,
        lidcore::launchd::WATCHDOG_ARG => Command::Watchdog,
        other => Command::Unknown(other.to_string()),
    }
}

/// Runs a non-GUI command. Returns the process exit code.
pub fn run(command: Command) -> ExitCode {
    match command {
        Command::MenuBar => unreachable!("the menu bar app is launched by main"),
        Command::Enable => report(enable()),
        Command::Disable => report(disable()),
        Command::Status => report(status()),
        Command::Version => {
            println!("{APP_NAME} {VERSION}");
            ExitCode::SUCCESS
        }
        Command::Help => {
            println!("{}", help());
            ExitCode::SUCCESS
        }
        // Deliberately silent: launchd runs this every minute and output would
        // only fill logs.
        Command::Watchdog => match watchdog_pass() {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::FAILURE,
        },
        Command::Unknown(argument) => {
            eprintln!("Unknown command: {argument}");
            eprintln!("{}", help());
            // sysexits.h EX_USAGE, matching `lid-cli`.
            ExitCode::from(64)
        }
    }
}

fn report(result: lidcore::Result<()>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("close-my-lid: {error}");
            if let Some(hint) = error.hint() {
                eprintln!("\nhint: {hint}");
            }
            ExitCode::FAILURE
        }
    }
}

/// A hold taken from the command line is the same persistent `pmset` setting
/// the app takes, so applying it and exiting is enough — nothing has to stay
/// alive to hold it. A running menu bar app adopts the session on its next
/// reconciliation pass.
fn enable() -> lidcore::Result<()> {
    // A hold another live process already owns must not be taken a second
    // time; the CLI in `lid-cli` makes the same check.
    lidcore::HoldLock::check_available()?;

    let mut controller = SleepSessionController::new()?;
    controller.reconcile_at_launch()?;
    controller.start(SessionDuration::Indefinite)?;

    if let Ok(store) = HoldHeartbeatStore::new() {
        // Unsupervised: this process is about to exit and nothing will refresh
        // the record. The watchdog must judge the hold by its deadline alone,
        // or it would release it three minutes from now.
        let _ = store.write(&HoldHeartbeat::unsupervised(
            controller.state().ends_at(),
            heartbeat::now_utc(),
        ));
    }

    println!("{APP_NAME} is holding closed-lid sleep.");
    Ok(())
}

fn disable() -> lidcore::Result<()> {
    let mut controller = SleepSessionController::new()?;
    controller.reconcile_at_launch()?;
    controller.stop()?;

    if let Ok(store) = HoldHeartbeatStore::new() {
        store.remove();
    }

    println!("{APP_NAME} restored normal closed-lid sleep.");
    Ok(())
}

fn status() -> lidcore::Result<()> {
    let held = PmsetLidGuard::new().is_held()?;
    println!(
        "closed-lid sleep hold: {}",
        if held { "enabled" } else { "disabled" }
    );
    Ok(())
}

/// Uses the passwordless-only executor, so a missing sudoers grant can never
/// spawn an administrator dialog from a headless agent.
fn watchdog_pass() -> lidcore::Result<()> {
    let store = HoldHeartbeatStore::new()?;
    let mut backend = PmsetLidGuard::passwordless();
    watchdog_once(
        &store,
        &WatchdogPolicy::default(),
        &mut backend,
        heartbeat::now_utc(),
    )?;
    Ok(())
}

fn help() -> String {
    // Named after however this copy was invoked. Inside the bundle the
    // executable is `CloseMyLid`; a Homebrew install of the formula puts a
    // `close-my-lid` symlink on the PATH. Printing whichever one the reader
    // typed keeps the usage lines copy-pasteable.
    let command = std::env::args()
        .next()
        .and_then(|path| {
            std::path::Path::new(&path)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "close-my-lid".to_string());

    format!(
        "\
{APP_NAME} {VERSION}

Usage:
  {command}              Launch the menu bar app
  {command} enable       Hold closed-lid sleep
  {command} disable      Restore normal closed-lid sleep
  {command} status       Print the current closed-lid sleep hold status
  {command} --version    Print the version
  {command} --help       Show this help"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(arguments: &[&str]) -> Command {
        parse(
            &arguments
                .iter()
                .map(|argument| argument.to_string())
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn no_arguments_launches_the_menu_bar_app() {
        assert_eq!(parsed(&[]), Command::MenuBar);
    }

    #[test]
    fn every_documented_spelling_parses() {
        assert_eq!(parsed(&["enable"]), Command::Enable);
        assert_eq!(parsed(&["disable"]), Command::Disable);
        assert_eq!(parsed(&["status"]), Command::Status);
        for spelling in ["--version", "-v", "version"] {
            assert_eq!(parsed(&[spelling]), Command::Version, "{spelling}");
        }
        for spelling in ["--help", "-h", "help"] {
            assert_eq!(parsed(&[spelling]), Command::Help, "{spelling}");
        }
    }

    #[test]
    fn the_watchdog_flag_matches_what_the_launch_agent_passes() {
        // The plist is generated from this same constant, so a rename cannot
        // silently leave the agent invoking an unknown command every minute.
        assert_eq!(parsed(&[lidcore::launchd::WATCHDOG_ARG]), Command::Watchdog);
    }

    #[test]
    fn anything_else_is_reported_rather_than_launching_the_app() {
        assert_eq!(
            parsed(&["sleep-now"]),
            Command::Unknown("sleep-now".to_string())
        );
    }

    #[test]
    fn the_help_text_lists_every_command() {
        let help = help();
        for command in ["enable", "disable", "status", "--version", "--help"] {
            assert!(help.contains(command), "{command} is undocumented");
        }
    }
}
