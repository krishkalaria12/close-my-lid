//! The work that runs on its own schedule: supervising the hold, refreshing
//! the readouts, checking for updates — and quitting, which has to release
//! the hold on the way out.
//!
//! | loop | interval | does |
//! |---|---|---|
//! | clock | 1s, only while a hold runs | re-renders the countdown |
//! | supervision | 15s, or sooner when the hold or a notification is due | expiry, battery release, scheduled notifications |
//! | readouts | 5s focused, 30s in the background | battery and agent sessions, off the main thread |
//! | updates | 2s after launch, then 6h | reads the appcast, off the main thread |

use std::time::Instant;

use chrono::Utc;
use gpui_kit::{AnyWindowHandle, App, Entity};
use lidcore::battery;
use tracing::warn;

use crate::config;
use crate::state::{AppState, UpdateStatus};
use crate::updates;

pub fn start(state: &Entity<AppState>, window: AnyWindowHandle, cx: &mut App) {
    supervise(state.clone(), cx);
    tick_clock(state.clone(), cx);
    refresh_readouts(state.clone(), window, cx);
    watch_for_updates(state.clone(), cx);
}

/// Drives expiry, the battery safety release and the scheduled notifications,
/// whatever the window is doing — the hold must end on time while it is
/// minimised.
fn supervise(state: Entity<AppState>, cx: &mut App) {
    cx.spawn(async move |cx| {
        loop {
            let wait = cx.update(|cx| state.read(cx).next_wake(Utc::now()));
            cx.background_executor().timer(wait).await;

            // Notified on every pass, not only on a release: the header builds
            // "45m left" from the clock at render time, so without it the
            // countdown would sit frozen.
            cx.update(|cx| {
                state.update(cx, |state, cx| {
                    state.tick(Utc::now());
                    cx.notify();
                })
            });
        }
    })
    .detach();
}

/// Re-renders once a second while a hold runs, so the Overview countdown
/// moves. Only a notify — no system reads — and idle while nothing is held.
fn tick_clock(state: Entity<AppState>, cx: &mut App) {
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(config::CLOCK_INTERVAL).await;
            cx.update(|cx| {
                state.update(cx, |state, cx| {
                    if state.is_active() {
                        cx.notify();
                    }
                })
            });
        }
    })
    .detach();
}

/// Re-reads the battery and the process table. Both are blocking reads — the
/// process walk especially — so they run on the background executor and only
/// the answer comes back to the main thread.
fn refresh_readouts(state: Entity<AppState>, window: AnyWindowHandle, cx: &mut App) {
    cx.spawn(async move |cx| {
        let mut last: Option<Instant> = None;
        loop {
            let focused = cx
                .update(|cx| window.update(cx, |_, window, _| window.is_window_active()))
                .unwrap_or(false);
            let interval = if focused {
                config::READOUT_INTERVAL
            } else {
                config::BACKGROUND_READOUT_INTERVAL
            };

            if last.is_none_or(|at| at.elapsed() >= interval) {
                let (battery, agents) = cx
                    .background_executor()
                    .spawn(async { (battery::read(), lidcore::sessions_now()) })
                    .await;
                cx.update(|cx| {
                    state.update(cx, |state, cx| {
                        state.apply_readouts(battery, agents);
                        cx.notify();
                    })
                });
                last = Some(Instant::now());
            }

            cx.background_executor()
                .timer(config::READOUT_INTERVAL)
                .await;
        }
    })
    .detach();
}

fn watch_for_updates(state: Entity<AppState>, cx: &mut App) {
    cx.spawn(async move |cx| {
        cx.background_executor()
            .timer(config::FIRST_UPDATE_CHECK)
            .await;
        loop {
            cx.update(|cx| check_for_updates(state.clone(), false, cx));
            cx.background_executor()
                .timer(config::UPDATE_INTERVAL)
                .await;
        }
    })
    .detach();
}

/// Reads the appcast. A check the user asked for reports its answer — "Up to
/// date" for a moment, or the error in the banner; a scheduled one stays quiet
/// unless there is something to offer.
pub fn check_for_updates(state: Entity<AppState>, manual: bool, cx: &mut App) {
    let previous = state.read(cx).update.clone();
    if previous == UpdateStatus::Checking {
        return;
    }
    if manual {
        state.update(cx, |state, cx| {
            state.update = UpdateStatus::Checking;
            cx.notify();
        });
    }

    cx.spawn(async move |cx| {
        let result = cx
            .background_executor()
            .spawn(async { updates::check() })
            .await;

        let found_nothing = cx.update(|cx| {
            state.update(cx, |state, cx| {
                let found_nothing = matches!(result, Ok(None));
                state.update = match result {
                    Ok(Some(update)) => UpdateStatus::Available(update),
                    Ok(None) if manual => UpdateStatus::Current,
                    Ok(None) => UpdateStatus::Unchecked,
                    Err(error) => {
                        if manual {
                            state.show_error(error);
                        } else {
                            warn!(%error, "scheduled update check failed");
                        }
                        // Keep offering an update found earlier.
                        match previous {
                            UpdateStatus::Available(update) => UpdateStatus::Available(update),
                            _ => UpdateStatus::Unchecked,
                        }
                    }
                };
                cx.notify();
                found_nothing
            })
        });

        if manual && found_nothing {
            cx.background_executor()
                .timer(config::UPDATE_RESULT_LINGER)
                .await;
            cx.update(|cx| {
                state.update(cx, |state, cx| {
                    if state.update == UpdateStatus::Current {
                        state.update = UpdateStatus::Unchecked;
                        cx.notify();
                    }
                })
            });
        }
    })
    .detach();
}

/// Releases the hold, then quits. Every way out of the app goes through here,
/// so none of them can leave the lid held.
pub fn quit(state: &Entity<AppState>, cx: &mut App) {
    state.update(cx, |state, _| state.release_for_quit());
    cx.quit();
}
