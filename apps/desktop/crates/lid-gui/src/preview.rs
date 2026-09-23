//! The stand-in core for a macOS build of this crate.
//!
//! A Mac build exists only to work on the interface, and on a Mac the real
//! backend is `pmset` behind an administrator grant, sharing its config
//! directory with the installed menu bar app. Launch reconciliation alone
//! could release that app's live hold. So here the hold is an in-memory flag,
//! the session file lives in the temp directory, and nothing is written where
//! the real app would read it.

use lidcore::{LidPowerBackend, Result, SleepSessionController, SleepSessionStore};

/// True in a build that must not touch the machine's real lid state.
pub const ACTIVE: bool = cfg!(target_os = "macos");

#[derive(Default)]
struct PreviewBackend {
    held: bool,
}

impl LidPowerBackend for PreviewBackend {
    fn acquire(&mut self) -> Result<()> {
        self.held = true;
        Ok(())
    }

    fn release(&mut self) -> Result<()> {
        self.held = false;
        Ok(())
    }

    fn is_held(&self) -> Result<bool> {
        Ok(self.held)
    }

    fn describe(&self) -> &'static str {
        "Interface preview: nothing is held. The macOS app is Close My Lid.app."
    }
}

pub fn controller() -> SleepSessionController {
    let store = SleepSessionStore::at(
        std::env::temp_dir()
            .join("close-my-lid-gui-preview")
            .join("session.json"),
    );
    SleepSessionController::with_parts(Box::<PreviewBackend>::default(), store)
}

/// Where a preview build opens, from `CLOSE_MY_LID_PREVIEW`: a page
/// (`agents`, `settings`) and optionally a running hold (`holding` for an
/// unlimited one, `holding-1h` for a timed one), comma-separated. For looking
/// at states that otherwise need clicking through, from a script.
pub struct Scene {
    pub page: Option<crate::shell::Page>,
    pub hold: Option<lidcore::SessionDuration>,
}

pub fn scene() -> Scene {
    let mut scene = Scene {
        page: None,
        hold: None,
    };
    if !ACTIVE {
        return scene;
    }
    let raw = std::env::var("CLOSE_MY_LID_PREVIEW").unwrap_or_default();
    for part in raw.split(',').map(str::trim) {
        match part {
            "agents" => scene.page = Some(crate::shell::Page::Agents),
            "settings" => scene.page = Some(crate::shell::Page::Settings),
            "holding" => scene.hold = Some(lidcore::SessionDuration::Indefinite),
            "holding-1h" => scene.hold = Some(lidcore::SessionDuration::ONE_HOUR),
            _ => {}
        }
    }
    scene
}
