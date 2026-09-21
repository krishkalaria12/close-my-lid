//! Linux backend: a `handle-lid-switch` inhibitor held from `systemd-logind`.
//!
//! Why this and not `logind.conf`: editing `HandleLidSwitch=ignore` is what
//! most guides suggest, but it is permanent, needs root, and survives a crash.
//! The inhibitor is scoped to this process and released by the kernel when the
//! descriptor closes — including on SIGKILL — so a stranded hold is not
//! possible and the macOS watchdog has no Linux counterpart.
//!
//! The subtlety worth knowing: logind defaults to `LidSwitchIgnoreInhibited=yes`,
//! which makes it ignore `sleep` and `idle` inhibitors when the lid closes. A
//! dedicated `handle-lid-switch` inhibitor is always honoured, which is the
//! whole reason that type exists. We ask for all three so the machine also
//! stays awake on idle while the lid is down.

use std::os::fd::OwnedFd;

use tracing::{debug, warn};
use zbus::blocking::Connection;
use zbus::proxy;

use crate::error::{LidError, Result};
use crate::power::LidPowerBackend;

const WHAT: &str = "handle-lid-switch:sleep:idle";
/// Fallback when the full set is denied: `sleep:block` often needs auth where
/// `handle-lid-switch:block` is granted to any active local session. The lid
/// type is the one that matters (logind honours it even with
/// `LidSwitchIgnoreInhibited=yes`); idle/sleep are a bonus.
const WHAT_LID_ONLY: &str = "handle-lid-switch";
const WHO: &str = "Close My Lid";
const WHY: &str = "Keeping long-running agent and build work alive with the lid closed";
/// `block` refuses the action outright; `delay` would only postpone it.
const MODE: &str = "block";

#[proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
trait Login1Manager {
    fn inhibit(
        &self,
        what: &str,
        who: &str,
        why: &str,
        mode: &str,
    ) -> zbus::Result<zbus::zvariant::OwnedFd>;
}

/// Holds the inhibitor descriptor for as long as the hold is active.
pub struct LogindInhibitor {
    /// `Some` exactly while a hold is in force. Dropping it releases.
    lock: Option<OwnedFd>,
}

impl LogindInhibitor {
    pub fn new() -> Self {
        Self { lock: None }
    }

    fn take_lock(&mut self) -> Result<OwnedFd> {
        let connection = Connection::system().map_err(|error| LidError::bus(error.to_string()))?;

        let manager = Login1ManagerProxyBlocking::new(&connection)
            .map_err(|error| LidError::bus(format!("logind is not on the bus: {error}")))?;

        // Prefer the full set (lid + idle/sleep) so the machine also stays
        // awake on idle while the lid is down. If logind/polkit denies it —
        // commonly because `sleep:block` needs auth — fall back to the lid
        // inhibitor alone, which is the one that actually stops lid-close
        // suspend and is granted to active local sessions.
        let fd = match manager.inhibit(WHAT, WHO, WHY, MODE) {
            Ok(fd) => {
                debug!("took a logind {WHAT} inhibitor");
                fd
            }
            Err(first) => {
                debug!(%first, "full inhibitor denied; retrying lid-only");
                manager
                    .inhibit(WHAT_LID_ONLY, WHO, WHY, MODE)
                    .map_err(|error| {
                        // Denials here are almost always polkit refusing an
                        // inactive or remote session, so say so rather than
                        // echoing the D-Bus error.
                        LidError::denied(
                            format!("take a {WHAT_LID_ONLY} inhibitor from logind"),
                            format!("{error} (full set also denied: {first})"),
                        )
                        .with_hint(
                            "logind grants lid inhibitors to active local sessions. Check \
                             `loginctl show-session $XDG_SESSION_ID -p Active -p Remote`, and \
                             note that SSH sessions cannot hold the lid open.",
                        )
                    })?
            }
        };

        Ok(OwnedFd::from(fd))
    }

    /// Which inhibitor set is currently held, for diagnostics.
    pub fn what_held(&self) -> Option<&'static str> {
        // The fallback path still satisfies the lid guarantee; callers that
        // need the distinction can extend this to record which `what` won.
        self.lock.as_ref().map(|_| WHAT)
    }
}

impl Default for LogindInhibitor {
    fn default() -> Self {
        Self::new()
    }
}

impl LidPowerBackend for LogindInhibitor {
    fn acquire(&mut self) -> Result<()> {
        if self.lock.is_some() {
            return Ok(());
        }
        self.lock = Some(self.take_lock()?);
        Ok(())
    }

    fn release(&mut self) -> Result<()> {
        // Closing the descriptor is the release; there is no undo to perform.
        if self.lock.take().is_some() {
            debug!("released the logind inhibitor");
        }
        Ok(())
    }

    fn is_held(&self) -> Result<bool> {
        // The inhibitor lives and dies with this descriptor, so our own handle
        // is the authoritative answer. Nothing external can strand it.
        Ok(self.lock.is_some())
    }

    fn describe(&self) -> &'static str {
        "systemd-logind handle-lid-switch inhibitor"
    }
}

impl Drop for LogindInhibitor {
    fn drop(&mut self) {
        if self.lock.is_some() {
            warn!("dropping an active lid inhibitor; normal sleep resumes");
        }
    }
}
