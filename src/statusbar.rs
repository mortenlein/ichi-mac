//! Menu bar status item.
//!
//! The macOS counterpart to a Windows system tray icon. This is the one place
//! Ichi is visible: the app stays `Accessory` (no Dock icon, no Cmd-Tab entry,
//! never takes focus), so the menu bar is the only surface a user can click.

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::{sel, AnyThread, MainThreadOnly};
use objc2_app_kit::{
    NSImage, NSMenu, NSMenuItem, NSStatusBar, NSStatusItem, NSVariableStatusItemLength,
};
use objc2_foundation::{MainThreadMarker, NSData, NSSize, NSString};

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
}

fn disabled_item(mtm: MainThreadMarker, title: &str) -> Retained<NSMenuItem> {
    let item = NSMenuItem::new(mtm);
    item.setTitle(&NSString::from_str(title));
    item.setEnabled(false);
    item
}

/// Put Ichi's icon in the menu bar with a small informational menu.
///
/// `accessibility_granted` is reported in the menu because it is the one
/// failure mode that looks like "the app is broken": without it the hotkeys
/// fire but nothing moves.
pub fn install(mtm: MainThreadMarker, accessibility_granted: bool) {
    let status_item = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);

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

    let menu = NSMenu::new(mtm);
    menu.addItem(&disabled_item(
        mtm,
        &format!("Ichi {}", env!("CARGO_PKG_VERSION")),
    ));
    menu.addItem(&disabled_item(
        mtm,
        if accessibility_granted {
            "✓ Accessibility granted"
        } else {
            "⚠︎ Accessibility access needed"
        },
    ));
    menu.addItem(&NSMenuItem::separatorItem(mtm));

    // A nil target sends `terminate:` up the responder chain to NSApp, which
    // implements it — so no custom Objective-C class is needed just to quit.
    let quit = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str("Quit Ichi"),
            Some(sel!(terminate:)),
            &NSString::from_str("q"),
        )
    };
    menu.addItem(&quit);

    status_item.setMenu(Some(&menu));
    STATUS_ITEM.with(|cell| *cell.borrow_mut() = Some(status_item));
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
        assert_eq!(colour_type, 6, "icon must be RGBA, got PNG colour type {colour_type}");
    }
}
