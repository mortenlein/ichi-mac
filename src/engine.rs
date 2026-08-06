/// Pure functional coordinate math for Ichi snaps.
///
/// Ported verbatim from the Windows build's `engine.rs`, with Win32's `RECT`
/// replaced by a local `Rect` so the math carries no platform dependency.
///
/// All rects are in "flipped" screen space: origin top-left, Y increasing
/// downward. That is Win32's convention and also the Accessibility API's, so
/// the geometry below is identical on both platforms. Converting AppKit's
/// bottom-left `NSScreen` coordinates into this space is `window.rs`'s job.

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn width(&self) -> i32 {
        self.right - self.left
    }

    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }

    /// Area of the overlap with `other`, or 0 if they do not intersect.
    /// Used to pick which display a window mostly lives on.
    pub fn intersection_area(&self, other: &Rect) -> i64 {
        let w = (self.right.min(other.right) - self.left.max(other.left)).max(0) as i64;
        let h = (self.bottom.min(other.bottom) - self.top.max(other.top)).max(0) as i64;
        w * h
    }

    pub fn center(&self) -> (i32, i32) {
        (
            self.left + self.width() / 2,
            self.top + self.height() / 2,
        )
    }
}

/// Takes current window rect, monitor work area rect, and snap parameters.
pub fn calculate_snap(key: u32, cycle: usize, _current: Rect, work_area: Rect) -> Rect {
    let mw = work_area.right - work_area.left;
    let mh = work_area.bottom - work_area.top;

    // Multi-tap cycle ratios (1/2, 1/3, 2/3)
    let grid_ratio = match cycle % 3 {
        0 => 0.5,
        1 => 1.0 / 3.0,
        2 => 2.0 / 3.0,
        _ => 0.5,
    };

    let (nx, ny, nw, nh) = match key {
        1 => {
            // Bottom Left
            let nw = (mw as f64 * grid_ratio) as i32;
            let nh = (mh as f64 * grid_ratio) as i32;
            (work_area.left, work_area.bottom - nh, nw, nh)
        }
        2 => {
            // Bottom Center
            let nh = (mh as f64 * grid_ratio) as i32;
            (work_area.left, work_area.bottom - nh, mw, nh)
        }
        3 => {
            // Bottom Right
            let nw = (mw as f64 * grid_ratio) as i32;
            let nh = (mh as f64 * grid_ratio) as i32;
            (work_area.right - nw, work_area.bottom - nh, nw, nh)
        }
        4 => {
            // Left
            let nw = (mw as f64 * grid_ratio) as i32;
            (work_area.left, work_area.top, nw, mh)
        }
        5 => {
            // Center / Maximize (100, 80, 60 ratios)
            let ratio = match cycle % 3 {
                0 => 1.0,
                1 => 0.8,
                2 => 0.6,
                _ => 1.0,
            };
            let nw = (mw as f64 * ratio) as i32;
            let nh = (mh as f64 * ratio) as i32;
            (
                work_area.left + (mw - nw) / 2,
                work_area.top + (mh - nh) / 2,
                nw,
                nh,
            )
        }
        6 => {
            // Right
            let nw = (mw as f64 * grid_ratio) as i32;
            (work_area.right - nw, work_area.top, nw, mh)
        }
        7 => {
            // Top Left
            let nw = (mw as f64 * grid_ratio) as i32;
            let nh = (mh as f64 * grid_ratio) as i32;
            (work_area.left, work_area.top, nw, nh)
        }
        8 => {
            // Top Center
            let nh = (mh as f64 * grid_ratio) as i32;
            (work_area.left, work_area.top, mw, nh)
        }
        9 => {
            // Top Right
            let nw = (mw as f64 * grid_ratio) as i32;
            let nh = (mh as f64 * grid_ratio) as i32;
            (work_area.right - nw, work_area.top, nw, nh)
        }
        _ => (work_area.left, work_area.top, mw, mh),
    };

    Rect {
        left: nx,
        top: ny,
        right: nx + nw,
        bottom: ny + nh,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 1920x1080 display whose work area starts 25px down (menu bar).
    fn work() -> Rect {
        Rect { left: 0, top: 25, right: 1920, bottom: 1080 }
    }

    fn snap(key: u32, cycle: usize) -> Rect {
        calculate_snap(key, cycle, Rect::default(), work())
    }

    #[test]
    fn left_half_then_third_then_two_thirds() {
        let w = work();
        assert_eq!(snap(4, 0), Rect { left: 0, top: 25, right: 960, bottom: 1080 });
        assert_eq!(snap(4, 1).width(), (w.width() as f64 / 3.0) as i32);
        assert_eq!(snap(4, 2).width(), (w.width() as f64 * 2.0 / 3.0) as i32);
        // Cycle wraps back to a half on the fourth tap.
        assert_eq!(snap(4, 3), snap(4, 0));
    }

    #[test]
    fn left_and_right_halves_tile_without_gap_or_overlap() {
        let l = snap(4, 0);
        let r = snap(6, 0);
        assert_eq!(l.right, r.left);
        assert_eq!(l.left, work().left);
        assert_eq!(r.right, work().right);
    }

    #[test]
    fn vertical_halves_respect_the_menu_bar_inset() {
        let top = snap(8, 0);
        let bottom = snap(2, 0);
        assert_eq!(top.top, work().top, "top half must start below the menu bar");
        assert_eq!(bottom.bottom, work().bottom);
        // Full width for the vertical halves.
        assert_eq!(top.width(), work().width());

        // The two halves meet within a pixel. On an odd-height work area
        // (1055 here, once the menu bar is subtracted) each half truncates to
        // 527, leaving a 1px seam. That rounding comes from the original
        // Windows engine and is reproduced here deliberately rather than
        // "fixed", so both builds place windows identically.
        assert!(
            (top.bottom - bottom.top).abs() <= 1,
            "halves should meet within a pixel, got {} vs {}",
            top.bottom,
            bottom.top
        );
    }

    #[test]
    fn halves_meet_exactly_when_the_work_area_divides_evenly() {
        // Same check on an even-height work area: no seam at all.
        let w = Rect { left: 0, top: 0, right: 1920, bottom: 1080 };
        let top = calculate_snap(8, 0, Rect::default(), w);
        let bottom = calculate_snap(2, 0, Rect::default(), w);
        assert_eq!(top.bottom, bottom.top);
        assert_eq!(top.height(), 540);
    }

    #[test]
    fn center_cycles_100_80_60_percent_and_stays_centered() {
        let w = work();
        for (cycle, ratio) in [(0usize, 1.0f64), (1, 0.8), (2, 0.6)] {
            let c = snap(5, cycle);
            assert_eq!(c.width(), (w.width() as f64 * ratio) as i32);
            assert_eq!(c.height(), (w.height() as f64 * ratio) as i32);
            // Equal margins left/right (within a pixel of integer rounding).
            let left_margin = c.left - w.left;
            let right_margin = w.right - c.right;
            assert!((left_margin - right_margin).abs() <= 1, "cycle {cycle} not centered");
        }
    }

    #[test]
    fn all_four_corners_stay_inside_the_work_area() {
        let w = work();
        for key in [1, 3, 7, 9] {
            for cycle in 0..3 {
                let c = snap(key, cycle);
                assert!(c.left >= w.left, "key {key} cycle {cycle} escapes left");
                assert!(c.top >= w.top, "key {key} cycle {cycle} escapes top");
                assert!(c.right <= w.right, "key {key} cycle {cycle} escapes right");
                assert!(c.bottom <= w.bottom, "key {key} cycle {cycle} escapes bottom");
            }
        }
    }

    #[test]
    fn corners_anchor_to_their_named_edges() {
        let w = work();
        assert_eq!((snap(7, 0).left, snap(7, 0).top), (w.left, w.top));
        assert_eq!((snap(9, 0).right, snap(9, 0).top), (w.right, w.top));
        assert_eq!((snap(1, 0).left, snap(1, 0).bottom), (w.left, w.bottom));
        assert_eq!((snap(3, 0).right, snap(3, 0).bottom), (w.right, w.bottom));
    }

    #[test]
    fn work_area_offset_is_honoured_on_a_secondary_display() {
        // A second display sitting to the right of the primary one.
        let w = Rect { left: 1920, top: 0, right: 3840, bottom: 1080 };
        let left_half = calculate_snap(4, 0, Rect::default(), w);
        assert_eq!(left_half, Rect { left: 1920, top: 0, right: 2880, bottom: 1080 });
    }

    #[test]
    fn intersection_area_picks_the_dominant_display() {
        let primary = Rect { left: 0, top: 0, right: 1920, bottom: 1080 };
        let secondary = Rect { left: 1920, top: 0, right: 3840, bottom: 1080 };
        // Window straddling the seam, mostly on the secondary.
        let win = Rect { left: 1720, top: 100, right: 2920, bottom: 800 };
        assert!(win.intersection_area(&secondary) > win.intersection_area(&primary));
        // A window entirely off both displays intersects neither.
        let orphan = Rect { left: -900, top: 0, right: -100, bottom: 500 };
        assert_eq!(orphan.intersection_area(&primary), 0);
    }
}
