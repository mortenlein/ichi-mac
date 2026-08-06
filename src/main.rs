//! ICHI ・ イチ — window snapping for macOS.
//!
//! A port of the Windows original. The snap geometry in `engine.rs` is
//! unchanged; only the OS-facing halves are rewritten:
//!
//! | Concern            | Windows                       | macOS (here)             |
//! |--------------------|-------------------------------|--------------------------|
//! | Hotkeys            | `WH_KEYBOARD_LL` hook         | `RegisterEventHotKey`    |
//! | Move / resize      | `SetWindowPos`                | Accessibility API        |
//! | Work area          | `GetMonitorInfoW`             | `NSScreen.visibleFrame`  |
//! | Flicker control    | `WM_SETREDRAW`                | not needed (Quartz)      |
//! | Frame correction   | `DWMWA_EXTENDED_FRAME_BOUNDS` | not needed (AX is exact) |
//!
//! Settings live in `~/Library/Application Support/Ichi/config.json` and are
//! read once at startup; the menu bar has a Restart item to apply edits.

mod config;
mod engine;
mod hotkeys;
mod keycodes;
mod login;
mod statusbar;
mod window;

use std::cell::RefCell;

use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use objc2_foundation::MainThreadMarker;

use engine::SnapConfig;
use window::AxElement;

/// Multi-tap cycle state, mirroring the Windows build's `AppState`.
///
/// Lives in a `thread_local` rather than a mutex: hotkey events are delivered
/// on the main run loop, so this is only ever touched from the main thread.
struct AppState {
    last_window: Option<AxElement>,
    last_key: Option<u32>,
    cycle_count: usize,
}

thread_local! {
    static STATE: RefCell<AppState> = const {
        RefCell::new(AppState {
            last_window: None,
            last_key: None,
            cycle_count: 0,
        })
    };

    /// Snap parameters resolved from the user's config at startup.
    static SNAP_CONFIG: RefCell<SnapConfig> = RefCell::new(SnapConfig::default());
}

/// Tapping the same hotkey on the same window advances the ratio cycle
/// (1/2 → 1/3 → 2/3 by default); anything else resets it.
fn next_cycle(window: &AxElement, key: u32) -> usize {
    STATE.with(|state| {
        let mut state = state.borrow_mut();

        let same_target = state.last_key == Some(key)
            && state
                .last_window
                .as_ref()
                .is_some_and(|previous| previous.same_as(window));

        state.cycle_count = if same_target {
            state.cycle_count + 1
        } else {
            0
        };
        state.last_key = Some(key);
        state.last_window = Some(window.clone());
        state.cycle_count
    })
}

fn perform_snap(key: u32) {
    let Some(window) = window::focused_window() else {
        return;
    };
    let Some(current) = window::window_rect(&window) else {
        return;
    };
    let Some(work_area) = window::work_area_for(current) else {
        return;
    };

    let cycle = next_cycle(&window, key);
    let target =
        SNAP_CONFIG.with(|c| engine::calculate_snap(key, cycle, current, work_area, &c.borrow()));

    // No border compensation step here — unlike Win32's GetWindowRect, the AX
    // frame has no invisible resize margin to correct for.
    window::set_window_rect(&window, target);
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main() must run on the main thread");

    let (settings, mut warnings) = config::load();
    SNAP_CONFIG.with(|c| *c.borrow_mut() = SnapConfig::from(&settings));

    // Moving another application's windows requires Accessibility access.
    // Passing `true` raises the system prompt on first launch.
    let accessibility_granted = window::is_trusted(true);
    if !accessibility_granted {
        eprintln!(
            "ichi: Accessibility access not granted yet.\n\
             \n\
             Grant it under System Settings → Privacy & Security → Accessibility,\n\
             then relaunch Ichi. Hotkeys are registered either way, but windows\n\
             cannot be moved until access is granted."
        );
    }

    let bindings = settings.hotkeys.bindings();
    match hotkeys::install(perform_snap, &bindings) {
        Ok(failures) => {
            // Collected rather than printed here; the shared loop below emits
            // every warning exactly once, and the menu shows the same list.
            for failure in &failures {
                warnings.push(failure.to_string());
            }
            if failures.len() == bindings.len() {
                warnings.push("no hotkeys could be registered".into());
            }
        }
        Err(error) => {
            // Nothing can work without the handler, so this one is fatal.
            eprintln!("ichi: {error}");
            std::process::exit(1);
        }
    }

    for warning in &warnings {
        eprintln!("ichi: {warning}");
    }

    // Accessory policy keeps Stealth Mode intact: no Dock icon, no Cmd-Tab
    // entry, never steals focus — the menu bar item below is the only visible
    // surface. NSApplication (rather than a bare CFRunLoop) is also what pumps
    // the Carbon event queue the hotkeys are delivered on.
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    statusbar::install(mtm, accessibility_granted, &warnings);
    app.run();
}
