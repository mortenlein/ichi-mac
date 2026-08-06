//! Menu bar status item.
//!
//! The macOS counterpart to a Windows system tray icon. This is the one place
//! Ichi is visible: the app stays `Accessory` (no Dock icon, no Cmd-Tab entry,
//! never takes focus), so the menu bar is the only surface a user can click.

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol};
use objc2::{AnyThread, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSImage, NSMenu, NSMenuItem, NSStatusBar, NSStatusItem,
    NSVariableStatusItemLength, NSWorkspace,
};
use objc2_foundation::{MainThreadMarker, NSData, NSSize, NSString, NSURL};

use crate::{config, login};

/// The red hinomaru circle at 36x36 with a transparent surround, generated
/// from `icon.png` by `make_menubar_icon.swift`.
///
/// Embedded rather than read from the bundle's Resources so that `cargo run`
/// and the installed `.app` behave identically — there is no path to get wrong.
const ICON_PNG: &[u8] = include_bytes!("menubar-icon.png");

/// Menu bar icons are sized in points; the 36px asset then renders @2x.
const ICON_POINTS: f64 = 18.0;

thread_local! {
    /// The status item is removed from the bar as soon as it is released, so
    /// it has to be held for the lifetime of the process rather than dropped
    /// at the end of `install`.
    static STATUS_ITEM: RefCell<Option<Retained<NSStatusItem>>> = const { RefCell::new(None) };
    /// Likewise the menu's action target: NSMenuItem does not retain it.
    static MENU_TARGET: RefCell<Option<Retained<MenuTarget>>> = const { RefCell::new(None) };
}

define_class!(
    /// Receives the menu actions that need real behaviour.
    ///
    /// `Quit` gets by without this by riding the responder chain to NSApp's
    /// `terminate:`, but toggling a login item or opening a file has no such
    /// free selector, so it needs an actual Objective-C object to target.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IchiMenuTarget"]
    struct MenuTarget;

    unsafe impl NSObjectProtocol for MenuTarget {}

    impl MenuTarget {
        #[unsafe(method(toggleLaunchAtLogin:))]
        fn toggle_launch_at_login(&self, _sender: Option<&NSMenuItem>) {
            let now_enabled = !login::state().is_enabled();
            let resulting = login::set(now_enabled);
            if let Some(mtm) = MainThreadMarker::new() {
                rebuild_menu(mtm, resulting);
            }
        }

        #[unsafe(method(openConfigFile:))]
        fn open_config_file(&self, _sender: Option<&NSMenuItem>) {
            let Some(path) = config::config_path() else {
                return;
            };
            // Make sure the file exists before asking the system to open it,
            // otherwise first-run users get a confusing "can't be found".
            if !path.exists() {
                let (loaded, _) = config::load();
                let _ = config::save(&loaded);
            }
            let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
            NSWorkspace::sharedWorkspace().openURL(&url);
        }

        #[unsafe(method(showConfigFolder:))]
        fn show_config_folder(&self, _sender: Option<&NSMenuItem>) {
            let Some(path) = config::config_path() else {
                return;
            };
            let Some(parent) = path.parent() else {
                return;
            };
            let url = NSURL::fileURLWithPath(&NSString::from_str(&parent.to_string_lossy()));
            NSWorkspace::sharedWorkspace().openURL(&url);
        }

        #[unsafe(method(restartIchi:))]
        fn restart_ichi(&self, _sender: Option<&NSMenuItem>) {
            // Config is read once at startup, so applying an edited config
            // means relaunching. Doing it from the menu saves the user from
            // the stale-process trap: quitting and double-clicking the app is
            // easy to get wrong when there is no window to tell you which
            // build you are talking to.
            if let Ok(exe) = std::env::current_exe()
                && std::process::Command::new(&exe).spawn().is_ok()
            {
                let mtm = MainThreadMarker::new().expect("menu actions run on the main thread");
                NSApplication::sharedApplication(mtm).terminate(None);
            }
        }
    }
);

impl MenuTarget {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        unsafe { msg_send![Self::alloc(mtm), init] }
    }
}

fn disabled_item(mtm: MainThreadMarker, title: &str) -> Retained<NSMenuItem> {
    let item = NSMenuItem::new(mtm);
    item.setTitle(&NSString::from_str(title));
    item.setEnabled(false);
    item
}

fn action_item(
    mtm: MainThreadMarker,
    title: &str,
    selector: objc2::runtime::Sel,
    key_equivalent: &str,
    target: &MenuTarget,
) -> Retained<NSMenuItem> {
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str(title),
            Some(selector),
            &NSString::from_str(key_equivalent),
        )
    };
    unsafe { item.setTarget(Some(target)) };
    item
}

/// Build (or rebuild) the menu against the current login-item state.
fn rebuild_menu(mtm: MainThreadMarker, login_state: login::LoginItemState) {
    let target = MENU_TARGET.with(|cell| cell.borrow().clone());
    let Some(target) = target else {
        return;
    };

    let menu = NSMenu::new(mtm);
    menu.addItem(&disabled_item(
        mtm,
        &format!("Ichi {}", env!("CARGO_PKG_VERSION")),
    ));
    menu.addItem(&NSMenuItem::separatorItem(mtm));

    let launch = action_item(
        mtm,
        login_state.menu_title(),
        sel!(toggleLaunchAtLogin:),
        "",
        &target,
    );
    launch.setState(if login_state.is_enabled() {
        objc2_app_kit::NSControlStateValueOn
    } else {
        objc2_app_kit::NSControlStateValueOff
    });
    // Nothing to register when running unbundled, so do not offer it.
    launch.setEnabled(login_state != login::LoginItemState::Unavailable);
    menu.addItem(&launch);

    menu.addItem(&NSMenuItem::separatorItem(mtm));
    menu.addItem(&action_item(
        mtm,
        "Edit Settings…",
        sel!(openConfigFile:),
        ",",
        &target,
    ));
    menu.addItem(&action_item(
        mtm,
        "Reveal Settings in Finder",
        sel!(showConfigFolder:),
        "",
        &target,
    ));
    menu.addItem(&action_item(
        mtm,
        "Restart to Apply Settings",
        sel!(restartIchi:),
        "r",
        &target,
    ));

    menu.addItem(&NSMenuItem::separatorItem(mtm));

    // A nil target sends `terminate:` up the responder chain to NSApp, which
    // implements it — so this one needs no custom target.
    let quit = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str("Quit Ichi"),
            Some(sel!(terminate:)),
            &NSString::from_str("q"),
        )
    };
    menu.addItem(&quit);

    STATUS_ITEM.with(|cell| {
        if let Some(item) = cell.borrow().as_ref() {
            item.setMenu(Some(&menu));
        }
    });
}

/// Put Ichi's icon in the menu bar with its menu.
///
/// `warnings` carries anything that went wrong during startup — an unparseable
/// config, a shortcut another app already owns. They are shown in the menu
/// because a background agent has nowhere else to tell you: without this, a
/// typo'd shortcut just silently does nothing.
pub fn install(mtm: MainThreadMarker, accessibility_granted: bool, warnings: &[String]) {
    let status_item =
        NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);

    let data = NSData::with_bytes(ICON_PNG);
    if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
        image.setSize(NSSize::new(ICON_POINTS, ICON_POINTS));
        // Full colour rather than a monochrome mask: the red hinomaru is the
        // brand. It does not adapt to light/dark, which is the accepted cost.
        image.setTemplate(false);

        if let Some(button) = status_item.button(mtm) {
            button.setImage(Some(&image));
        }
    }

    STATUS_ITEM.with(|cell| *cell.borrow_mut() = Some(status_item));
    MENU_TARGET.with(|cell| *cell.borrow_mut() = Some(MenuTarget::new(mtm)));

    rebuild_menu(mtm, login::state());

    // Prepend status lines that need attention, above the built menu.
    if !accessibility_granted || !warnings.is_empty() {
        prepend_warnings(mtm, accessibility_granted, warnings);
    }
}

fn prepend_warnings(mtm: MainThreadMarker, accessibility_granted: bool, warnings: &[String]) {
    STATUS_ITEM.with(|cell| {
        let borrowed = cell.borrow();
        let Some(item) = borrowed.as_ref() else {
            return;
        };
        let Some(menu) = item.menu(mtm) else {
            return;
        };

        let mut index = 1isize;
        if !accessibility_granted {
            let warning = disabled_item(mtm, "⚠︎ Accessibility access needed");
            menu.insertItem_atIndex(&warning, index);
            index += 1;
        }
        for warning in warnings {
            let item = disabled_item(mtm, &format!("⚠︎ {warning}"));
            menu.insertItem_atIndex(&item, index);
            index += 1;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_icon_decodes_to_a_valid_image() {
        let data = NSData::with_bytes(ICON_PNG);
        let image = NSImage::initWithData(NSImage::alloc(), &data)
            .expect("menubar-icon.png should decode; regenerate via make_menubar_icon.swift");
        assert!(image.isValid());

        // The asset ships at 36x36 so it renders crisply at 18pt on Retina.
        let size = image.size();
        assert_eq!(size.width, 36.0);
        assert_eq!(size.height, 36.0);
    }

    #[test]
    fn embedded_icon_is_a_png_with_transparency() {
        // PNG magic number, then check an IHDR colour type that carries alpha
        // (6 = RGBA). A flattened icon would have a white square around the
        // circle and look wrong in the menu bar.
        assert_eq!(&ICON_PNG[0..8], b"\x89PNG\r\n\x1a\n");
        let colour_type = ICON_PNG[25];
        assert_eq!(
            colour_type, 6,
            "icon must be RGBA, got PNG colour type {colour_type}"
        );
    }
}
