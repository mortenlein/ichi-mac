//! Global hotkeys via Carbon's `RegisterEventHotKey`.
//!
//! This replaces the Windows build's `WH_KEYBOARD_LL` low-level keyboard hook.
//! `RegisterEventHotKey` is the better tool on macOS for two reasons:
//!
//! * It does **not** require Accessibility permission (only the window *moving*
//!   does), and it never sees keystrokes other than the ones registered — no
//!   keylogger-shaped access to the event stream.
//! * The system consumes a matched hotkey before it reaches the focused app,
//!   which is what the Windows hook achieves by returning `LRESULT(1)`.
//!
//! The hotkey events are delivered on the main run loop, so the callback runs
//! on the main thread — which `window.rs` relies on for `NSScreen` access.

use std::ffi::c_void;
use std::ptr;
use std::sync::OnceLock;

use crate::keycodes;

pub type OSStatus = i32;
type OSType = u32;

const NO_ERR: OSStatus = 0;

#[repr(C)]
#[derive(Clone, Copy)]
struct EventTypeSpec {
    event_class: u32,
    event_kind: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct EventHotKeyID {
    signature: OSType,
    id: u32,
}

type EventRef = *mut c_void;
type EventTargetRef = *mut c_void;
type EventHandlerCallRef = *mut c_void;
type EventHotKeyRef = *mut c_void;
type EventHandlerRef = *mut c_void;

type EventHandlerUPP = unsafe extern "C" fn(EventHandlerCallRef, EventRef, *mut c_void) -> OSStatus;

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn GetEventDispatcherTarget() -> EventTargetRef;
    fn InstallEventHandler(
        target: EventTargetRef,
        handler: EventHandlerUPP,
        num_types: usize,
        list: *const EventTypeSpec,
        user_data: *mut c_void,
        handler_ref: *mut EventHandlerRef,
    ) -> OSStatus;
    fn RegisterEventHotKey(
        hot_key_code: u32,
        hot_key_modifiers: u32,
        hot_key_id: EventHotKeyID,
        target: EventTargetRef,
        options: u32,
        hot_key_ref: *mut EventHotKeyRef,
    ) -> OSStatus;
    fn GetEventParameter(
        event: EventRef,
        name: u32,
        desired_type: u32,
        actual_type: *mut u32,
        buffer_size: usize,
        actual_size: *mut usize,
        data: *mut c_void,
    ) -> OSStatus;
}

/// Carbon four-character codes, e.g. `'keyb'`.
const fn fourcc(code: &[u8; 4]) -> u32 {
    ((code[0] as u32) << 24) | ((code[1] as u32) << 16) | ((code[2] as u32) << 8) | code[3] as u32
}

const K_EVENT_CLASS_KEYBOARD: u32 = fourcc(b"keyb");
const K_EVENT_HOT_KEY_PRESSED: u32 = 5;
const K_EVENT_PARAM_DIRECT_OBJECT: u32 = fourcc(b"----");
const TYPE_EVENT_HOT_KEY_ID: u32 = fourcc(b"hkid");

/// Tags our hotkeys so a stray event from another source is ignored.
const ICHI_SIGNATURE: u32 = fourcc(b"ichi");

/// Set once at startup, read from the Carbon callback.
static CALLBACK: OnceLock<fn(u32)> = OnceLock::new();

unsafe extern "C" fn hotkey_handler(
    _call_ref: EventHandlerCallRef,
    event: EventRef,
    _user_data: *mut c_void,
) -> OSStatus {
    let mut hot_key = EventHotKeyID {
        signature: 0,
        id: 0,
    };
    let status = unsafe {
        GetEventParameter(
            event,
            K_EVENT_PARAM_DIRECT_OBJECT,
            TYPE_EVENT_HOT_KEY_ID,
            ptr::null_mut(),
            size_of::<EventHotKeyID>(),
            ptr::null_mut(),
            &mut hot_key as *mut EventHotKeyID as *mut c_void,
        )
    };

    if status == NO_ERR
        && hot_key.signature == ICHI_SIGNATURE
        && let Some(callback) = CALLBACK.get()
    {
        callback(hot_key.id);
    }
    NO_ERR
}

/// Installing the Carbon handler failed, so no hotkey can ever fire.
/// Individual binding failures are reported separately and are not fatal.
#[derive(Debug)]
pub struct HandlerInstallFailed(pub OSStatus);

impl std::fmt::Display for HandlerInstallFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "InstallEventHandler failed (OSStatus {})", self.0)
    }
}

/// One shortcut that could not be claimed.
#[derive(Debug, PartialEq)]
pub struct BindingFailure {
    /// The config key, e.g. `"top_left"`.
    pub position_name: String,
    pub shortcut: String,
    pub reason: String,
}

impl std::fmt::Display for BindingFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} = {:?}: {}",
            self.position_name, self.shortcut, self.reason
        )
    }
}

/// Install the Carbon handler and claim the configured shortcuts.
///
/// A shortcut that fails to parse or is already owned by another app is
/// reported and skipped rather than aborting the whole set — one bad line in
/// the config should not leave you with no working hotkeys at all.
///
/// The registrations are deliberately leaked: they must live for the entire
/// process lifetime, and the process exits by being killed rather than by
/// unwinding, so there is nothing meaningful to unregister into.
pub fn install(
    callback: fn(u32),
    bindings: &[(&str, u32, &str)],
) -> Result<Vec<BindingFailure>, HandlerInstallFailed> {
    let _ = CALLBACK.set(callback);

    let event_type = EventTypeSpec {
        event_class: K_EVENT_CLASS_KEYBOARD,
        event_kind: K_EVENT_HOT_KEY_PRESSED,
    };

    let mut failures = Vec::new();

    unsafe {
        let target = GetEventDispatcherTarget();
        let status = InstallEventHandler(
            target,
            hotkey_handler,
            1,
            &event_type,
            ptr::null_mut(),
            ptr::null_mut(),
        );
        if status != NO_ERR {
            return Err(HandlerInstallFailed(status));
        }

        for (position_name, grid_position, shortcut) in bindings {
            let parsed = match keycodes::parse(shortcut) {
                Ok(parsed) => parsed,
                Err(error) => {
                    failures.push(BindingFailure {
                        position_name: (*position_name).to_string(),
                        shortcut: (*shortcut).to_string(),
                        reason: error.to_string(),
                    });
                    continue;
                }
            };

            let mut hot_key_ref: EventHotKeyRef = ptr::null_mut();
            let status = RegisterEventHotKey(
                parsed.key_code,
                parsed.modifiers,
                EventHotKeyID {
                    signature: ICHI_SIGNATURE,
                    id: *grid_position,
                },
                target,
                0,
                &mut hot_key_ref,
            );
            if status != NO_ERR {
                failures.push(BindingFailure {
                    position_name: (*position_name).to_string(),
                    shortcut: (*shortcut).to_string(),
                    reason: format!("already taken by another app (OSStatus {status})"),
                });
            }
        }
    }

    Ok(failures)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fourcc_matches_carbon_constants() {
        assert_eq!(K_EVENT_CLASS_KEYBOARD, 0x6B65_7962); // 'keyb'
        assert_eq!(K_EVENT_PARAM_DIRECT_OBJECT, 0x2D2D_2D2D); // '----'
        assert_eq!(TYPE_EVENT_HOT_KEY_ID, 0x686B_6964); // 'hkid'
    }

    #[test]
    fn default_bindings_resolve_to_ctrl_option_keypad() {
        // The defaults now live in config.rs; confirm they still parse to the
        // Ctrl+Opt+Numpad codes the Windows build uses.
        let expected = keycodes::CONTROL_KEY | keycodes::OPTION_KEY;
        for (_, position, shortcut) in crate::config::Hotkeys::default().bindings() {
            let parsed = keycodes::parse(shortcut)
                .unwrap_or_else(|e| panic!("default binding {shortcut:?} is invalid: {e}"));
            assert_eq!(parsed.modifiers, expected, "position {position}");
        }
    }

    #[test]
    fn old_hardcoded_keypad_codes_are_still_what_defaults_produce() {
        let expected: [(u32, u32); 9] = [
            (0x53, 1), // Keypad 1 — bottom left
            (0x54, 2), // Keypad 2 — bottom
            (0x55, 3), // Keypad 3 — bottom right
            (0x56, 4), // Keypad 4 — left
            (0x57, 5), // Keypad 5 — centre / maximise
            (0x58, 6), // Keypad 6 — right
            (0x59, 7), // Keypad 7 — top left
            (0x5B, 8), // Keypad 8 — top
            (0x5C, 9), // Keypad 9 — top right
        ];
        for (_, position, shortcut) in crate::config::Hotkeys::default().bindings() {
            let parsed = keycodes::parse(shortcut).unwrap();
            let (want_code, _) = expected
                .iter()
                .find(|(_, p)| *p == position)
                .copied()
                .unwrap();
            assert_eq!(parsed.key_code, want_code, "position {position}");
        }

        let mut codes: Vec<u32> = expected.iter().map(|(c, _)| *c).collect();
        codes.sort();
        codes.dedup();
        assert_eq!(codes.len(), 9, "duplicate key code in bindings");
        // 0x5A is kVK_F20 and must never appear among the keypad codes.
        assert!(!codes.contains(&0x5A));
    }
}
