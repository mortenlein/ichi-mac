//! macOS window manipulation via the Accessibility (AX) API.
//!
//! This is the counterpart to the Windows build's `SetWindowPos` /
//! `GetMonitorInfoW` block. Two things differ from Win32 and both matter:
//!
//! 1. **Coordinate space.** AppKit's `NSScreen` uses a bottom-left origin with
//!    Y increasing upward. The AX API uses a top-left origin with Y increasing
//!    downward — the same convention as Win32. Everything leaving this module
//!    is in AX space, so `engine.rs` needs no changes from the Windows version.
//!
//! 2. **No border compensation.** Win32 needs `DwmGetWindowAttribute` /
//!    `DWMWA_EXTENDED_FRAME_BOUNDS` because a window's `GetWindowRect` includes
//!    invisible resize borders. AX position/size are the true on-screen frame,
//!    so that whole correction step is simply absent here.

use std::ffi::c_void;
use std::ptr;

use accessibility_sys::{
    AXIsProcessTrustedWithOptions, AXUIElementCopyAttributeValue, AXUIElementCreateSystemWide,
    AXUIElementRef, AXUIElementSetAttributeValue, AXValueCreate, AXValueGetValue, kAXErrorSuccess,
    kAXFocusedApplicationAttribute, kAXFocusedWindowAttribute, kAXPositionAttribute,
    kAXSizeAttribute, kAXTrustedCheckOptionPrompt, kAXValueTypeCGPoint, kAXValueTypeCGSize,
};
use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
use core_foundation_sys::base::{CFEqual, CFRelease, CFRetain, CFTypeRef};
use objc2_app_kit::NSScreen;
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize};

use crate::engine::Rect;

/// An owned (+1 retained) AX element. Releases on drop.
pub struct AxElement(AXUIElementRef);

impl AxElement {
    fn as_ref(&self) -> AXUIElementRef {
        self.0
    }

    /// Whether two elements refer to the same underlying UI object.
    /// AX element pointers are not stable across lookups, but `CFEqual` on them
    /// is — this is how we tell "same window as last keypress" for multi-tap.
    pub fn same_as(&self, other: &AxElement) -> bool {
        unsafe { CFEqual(self.0 as CFTypeRef, other.0 as CFTypeRef) != 0 }
    }
}

impl Clone for AxElement {
    fn clone(&self) -> Self {
        if !self.0.is_null() {
            unsafe { CFRetain(self.0 as CFTypeRef) };
        }
        AxElement(self.0)
    }
}

impl Drop for AxElement {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0 as CFTypeRef) };
        }
    }
}

/// Copy an attribute that is itself an AX element (e.g. focused app/window).
unsafe fn copy_element_attr(element: AXUIElementRef, attr: &str) -> Option<AxElement> {
    unsafe {
        let key = CFString::new(attr);
        let mut value: CFTypeRef = ptr::null();
        let err = AXUIElementCopyAttributeValue(element, key.as_concrete_TypeRef(), &mut value);
        if err != kAXErrorSuccess || value.is_null() {
            return None;
        }
        Some(AxElement(value as AXUIElementRef))
    }
}

/// Copy an `AXValue` attribute and unwrap it into `T` (NSPoint or NSSize).
unsafe fn copy_axvalue_attr<T>(element: AXUIElementRef, attr: &str, ty: u32) -> Option<T> {
    unsafe {
        let key = CFString::new(attr);
        let mut value: CFTypeRef = ptr::null();
        let err = AXUIElementCopyAttributeValue(element, key.as_concrete_TypeRef(), &mut value);
        if err != kAXErrorSuccess || value.is_null() {
            return None;
        }
        let mut out = std::mem::MaybeUninit::<T>::uninit();
        let ok = AXValueGetValue(value as _, ty, out.as_mut_ptr() as *mut c_void);
        CFRelease(value);
        if ok { Some(out.assume_init()) } else { None }
    }
}

/// Write an `AXValue` attribute built from `payload`.
unsafe fn set_axvalue_attr<T>(element: AXUIElementRef, attr: &str, ty: u32, payload: &T) -> bool {
    unsafe {
        let key = CFString::new(attr);
        let value = AXValueCreate(ty, payload as *const T as *const c_void);
        if value.is_null() {
            return false;
        }
        let err =
            AXUIElementSetAttributeValue(element, key.as_concrete_TypeRef(), value as CFTypeRef);
        CFRelease(value as CFTypeRef);
        err == kAXErrorSuccess
    }
}

/// Is this process allowed to drive other apps' windows?
///
/// `prompt: true` surfaces the system "grant Accessibility access" dialog.
/// The grant is tied to the signed binary, which is why Ichi ships as a
/// codesigned `.app` bundle rather than a bare executable.
pub fn is_trusted(prompt: bool) -> bool {
    unsafe {
        let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
        let options = CFDictionary::from_CFType_pairs(&[(key, CFBoolean::from(prompt))]);
        AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef())
    }
}

/// The frontmost window of the frontmost application, if any.
pub fn focused_window() -> Option<AxElement> {
    unsafe {
        let system_wide = AxElement(AXUIElementCreateSystemWide());
        if system_wide.as_ref().is_null() {
            return None;
        }
        let app = copy_element_attr(system_wide.as_ref(), kAXFocusedApplicationAttribute)?;
        copy_element_attr(app.as_ref(), kAXFocusedWindowAttribute)
    }
}

/// Current on-screen frame of `window`, in AX (top-left origin) coordinates.
pub fn window_rect(window: &AxElement) -> Option<Rect> {
    unsafe {
        let origin: NSPoint =
            copy_axvalue_attr(window.as_ref(), kAXPositionAttribute, kAXValueTypeCGPoint)?;
        let size: NSSize =
            copy_axvalue_attr(window.as_ref(), kAXSizeAttribute, kAXValueTypeCGSize)?;
        Some(Rect {
            left: origin.x.round() as i32,
            top: origin.y.round() as i32,
            right: (origin.x + size.width).round() as i32,
            bottom: (origin.y + size.height).round() as i32,
        })
    }
}

/// Move and resize `window` to `target` (AX coordinates).
///
/// Position is written twice on purpose. An app that clamps its size against
/// the screen edge — or one with a minimum size — can silently reject the first
/// pass; setting position, then size, then position again converges on the
/// requested frame for the windows that would otherwise land short.
pub fn set_window_rect(window: &AxElement, target: Rect) -> bool {
    let origin = NSPoint {
        x: target.left as f64,
        y: target.top as f64,
    };
    let size = NSSize {
        width: target.width() as f64,
        height: target.height() as f64,
    };

    unsafe {
        let el = window.as_ref();
        set_axvalue_attr(el, kAXPositionAttribute, kAXValueTypeCGPoint, &origin);
        let sized = set_axvalue_attr(el, kAXSizeAttribute, kAXValueTypeCGSize, &size);
        let placed = set_axvalue_attr(el, kAXPositionAttribute, kAXValueTypeCGPoint, &origin);
        sized && placed
    }
}

/// Convert an AppKit rect (bottom-left origin, Y up) into AX space
/// (top-left origin, Y down). `primary_height` is the height of the display
/// whose origin is (0, 0) — the flip axis for the whole global coordinate space.
fn ns_rect_to_ax(r: NSRect, primary_height: f64) -> Rect {
    let left = r.origin.x;
    let top = primary_height - (r.origin.y + r.size.height);
    Rect {
        left: left.round() as i32,
        top: top.round() as i32,
        right: (left + r.size.width).round() as i32,
        bottom: (top + r.size.height).round() as i32,
    }
}

/// The usable area of the display that `window_rect` mostly occupies, in AX
/// coordinates. This is the macOS analogue of `MONITORINFO.rcWork` obtained via
/// `MONITOR_DEFAULTTONEAREST`: `visibleFrame` already excludes the menu bar and
/// the Dock, so snapped windows never slide underneath either.
pub fn work_area_for(window_rect: Rect) -> Option<Rect> {
    let mtm = MainThreadMarker::new()?;
    let screens = NSScreen::screens(mtm);

    // screens[0] is the display with origin (0,0); its height is the flip axis.
    let primary_height = screens.iter().next()?.frame().size.height;

    let mut best: Option<(i64, Rect)> = None;
    let mut nearest: Option<(i64, Rect)> = None;
    let (wcx, wcy) = window_rect.center();

    for screen in screens.iter() {
        let visible = ns_rect_to_ax(screen.visibleFrame(), primary_height);
        let overlap = window_rect.intersection_area(&visible);
        if best.is_none_or(|(area, _)| overlap > area) {
            best = Some((overlap, visible));
        }

        // Fallback for a window that overlaps nothing (fully offscreen):
        // pick the display whose centre is closest, mirroring DEFAULTTONEAREST.
        let (scx, scy) = visible.center();
        let dx = (scx - wcx) as i64;
        let dy = (scy - wcy) as i64;
        let dist = dx * dx + dy * dy;
        if nearest.is_none_or(|(d, _)| dist < d) {
            nearest = Some((dist, visible));
        }
    }

    match best {
        Some((area, rect)) if area > 0 => Some(rect),
        _ => nearest.map(|(_, rect)| rect),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flip_converts_a_full_primary_screen() {
        // 1920x1080 primary, menu bar 25px tall, Dock hidden.
        // AppKit reports visibleFrame origin at y=0 with height 1055.
        let visible = NSRect::new(
            NSPoint { x: 0.0, y: 0.0 },
            NSSize {
                width: 1920.0,
                height: 1055.0,
            },
        );
        let ax = ns_rect_to_ax(visible, 1080.0);
        assert_eq!(
            ax,
            Rect {
                left: 0,
                top: 25,
                right: 1920,
                bottom: 1080
            }
        );
    }

    #[test]
    fn flip_handles_a_display_above_the_primary() {
        // Secondary 1920x1080 sitting directly above primary:
        // AppKit origin y = 1080, so in AX space its top is -1080.
        let visible = NSRect::new(
            NSPoint { x: 0.0, y: 1080.0 },
            NSSize {
                width: 1920.0,
                height: 1080.0,
            },
        );
        let ax = ns_rect_to_ax(visible, 1080.0);
        assert_eq!(
            ax,
            Rect {
                left: 0,
                top: -1080,
                right: 1920,
                bottom: 0
            }
        );
    }

    #[test]
    fn flip_preserves_dimensions() {
        let visible = NSRect::new(
            NSPoint {
                x: -1440.0,
                y: 200.0,
            },
            NSSize {
                width: 1440.0,
                height: 900.0,
            },
        );
        let ax = ns_rect_to_ax(visible, 1080.0);
        assert_eq!(ax.width(), 1440);
        assert_eq!(ax.height(), 900);
        assert_eq!(ax.left, -1440);
    }
}
