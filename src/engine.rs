/// Pure functional coordinate math for Ichi snaps.
///
/// Ported from the Windows build's `engine.rs`, with Win32's `RECT` replaced by
/// a local `Rect` so the math carries no platform dependency. The macOS build
/// adds configurable gaps and ratio sequences on top; with the default config
/// (zero gaps, 1/2-1/3-2/3) the output is identical to the Windows original.
///
/// All rects are in "flipped" screen space: origin top-left, Y increasing
/// downward. That is Win32's convention and also the Accessibility API's, so
/// the geometry below is identical on both platforms. Converting AppKit's
/// bottom-left `NSScreen` coordinates into this space is `window.rs`'s job.
use crate::config::Config;

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
        (self.left + self.width() / 2, self.top + self.height() / 2)
    }

    fn inset(&self, by: i32) -> Rect {
        Rect {
            left: self.left + by,
            top: self.top + by,
            right: self.right - by,
            bottom: self.bottom - by,
        }
    }
}

/// The snap parameters `calculate_snap` needs, extracted from user config.
#[derive(Debug, Clone, PartialEq)]
pub struct SnapConfig {
    pub cycle_ratios: Vec<f64>,
    pub center_ratios: Vec<f64>,
    pub screen_edge_gap: i32,
    pub window_gap: i32,
}

impl Default for SnapConfig {
    fn default() -> Self {
        Self::from(&Config::default())
    }
}

impl From<&Config> for SnapConfig {
    fn from(config: &Config) -> Self {
        Self {
            cycle_ratios: config.cycle_ratios.clone(),
            center_ratios: config.center_ratios.clone(),
            screen_edge_gap: config.gaps.screen_edge,
            window_gap: config.gaps.between_windows,
        }
    }
}

/// Takes current window rect, monitor work area rect, and snap parameters.
pub fn calculate_snap(
    key: u32,
    cycle: usize,
    _current: Rect,
    work_area: Rect,
    config: &SnapConfig,
) -> Rect {
    // The screen-edge gap simply shrinks the area everything is laid out in.
    let work_area = work_area.inset(config.screen_edge_gap);

    let mw = work_area.right - work_area.left;
    let mh = work_area.bottom - work_area.top;

    // Multi-tap cycle ratios — 1/2, 1/3, 2/3 unless reconfigured.
    let grid_ratio = ratio_at(&config.cycle_ratios, cycle, 0.5);

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
            let ratio = ratio_at(&config.center_ratios, cycle, 1.0);
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

    let snapped = Rect {
        left: nx,
        top: ny,
        right: nx + nw,
        bottom: ny + nh,
    };

    apply_window_gap(snapped, work_area, config.window_gap)
}

/// Pick `ratios[cycle]`, wrapping, with a fallback for an empty list.
///
/// `Config::sanitise` guarantees a non-empty list, but this is called with
/// whatever it is handed, so the fallback keeps a stray empty vec from
/// panicking on the modulo.
fn ratio_at(ratios: &[f64], cycle: usize, fallback: f64) -> f64 {
    if ratios.is_empty() {
        return fallback;
    }
    ratios[cycle % ratios.len()]
}

/// Pull the window's *interior* edges in by half the gap.
///
/// Only interior edges move. An edge sitting on the work-area boundary is left
/// alone, because the screen-edge gap has already accounted for it — otherwise
/// the two settings would compound and the outer margin would be wrong.
/// Two windows meeting in the middle each give up half, so the visible gap
/// between them is the full configured value.
fn apply_window_gap(rect: Rect, work_area: Rect, gap: i32) -> Rect {
    if gap <= 0 {
        return rect;
    }
    let half = gap / 2;

    let mut gapped = rect;
    if rect.left > work_area.left {
        gapped.left += half;
    }
    if rect.top > work_area.top {
        gapped.top += half;
    }
    if rect.right < work_area.right {
        gapped.right -= half;
    }
    if rect.bottom < work_area.bottom {
        gapped.bottom -= half;
    }

    // Never let a large gap invert the rect on a small screen.
    if gapped.width() <= 0 || gapped.height() <= 0 {
        return rect;
    }
    gapped
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 1920x1080 display whose work area starts 25px down (menu bar).
    fn work() -> Rect {
        Rect {
            left: 0,
            top: 25,
            right: 1920,
            bottom: 1080,
        }
    }

    fn snap(key: u32, cycle: usize) -> Rect {
        calculate_snap(key, cycle, Rect::default(), work(), &SnapConfig::default())
    }

    fn snap_with(key: u32, cycle: usize, config: &SnapConfig) -> Rect {
        calculate_snap(key, cycle, Rect::default(), work(), config)
    }

    #[test]
    fn left_half_then_third_then_two_thirds() {
        let w = work();
        assert_eq!(
            snap(4, 0),
            Rect {
                left: 0,
                top: 25,
                right: 960,
                bottom: 1080
            }
        );
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
        assert_eq!(
            top.top,
            work().top,
            "top half must start below the menu bar"
        );
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
        let w = Rect {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        };
        let cfg = SnapConfig::default();
        let top = calculate_snap(8, 0, Rect::default(), w, &cfg);
        let bottom = calculate_snap(2, 0, Rect::default(), w, &cfg);
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
            assert!(
                (left_margin - right_margin).abs() <= 1,
                "cycle {cycle} not centered"
            );
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
                assert!(
                    c.bottom <= w.bottom,
                    "key {key} cycle {cycle} escapes bottom"
                );
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
        let w = Rect {
            left: 1920,
            top: 0,
            right: 3840,
            bottom: 1080,
        };
        let left_half = calculate_snap(4, 0, Rect::default(), w, &SnapConfig::default());
        assert_eq!(
            left_half,
            Rect {
                left: 1920,
                top: 0,
                right: 2880,
                bottom: 1080
            }
        );
    }

    #[test]
    fn intersection_area_picks_the_dominant_display() {
        let primary = Rect {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        };
        let secondary = Rect {
            left: 1920,
            top: 0,
            right: 3840,
            bottom: 1080,
        };
        // Window straddling the seam, mostly on the secondary.
        let win = Rect {
            left: 1720,
            top: 100,
            right: 2920,
            bottom: 800,
        };
        assert!(win.intersection_area(&secondary) > win.intersection_area(&primary));
        // A window entirely off both displays intersects neither.
        let orphan = Rect {
            left: -900,
            top: 0,
            right: -100,
            bottom: 500,
        };
        assert_eq!(orphan.intersection_area(&primary), 0);
    }

    // --- Gaps ---

    fn gapped(screen_edge: i32, between_windows: i32) -> SnapConfig {
        SnapConfig {
            screen_edge_gap: screen_edge,
            window_gap: between_windows,
            ..SnapConfig::default()
        }
    }

    #[test]
    fn default_config_matches_the_windows_build_exactly() {
        // Zero gaps must be a true no-op, not an off-by-one.
        let cfg = gapped(0, 0);
        for key in 1..=9 {
            for cycle in 0..3 {
                assert_eq!(
                    snap_with(key, cycle, &cfg),
                    snap(key, cycle),
                    "key {key} cycle {cycle} drifted from the default"
                );
            }
        }
    }

    #[test]
    fn screen_edge_gap_insets_every_outer_edge() {
        let w = work();
        let cfg = gapped(10, 0);
        let left = snap_with(4, 0, &cfg);
        assert_eq!(left.left, w.left + 10);
        assert_eq!(left.top, w.top + 10);
        assert_eq!(left.bottom, w.bottom - 10);

        let right = snap_with(6, 0, &cfg);
        assert_eq!(right.right, w.right - 10);
    }

    #[test]
    fn window_gap_separates_adjacent_windows_by_the_full_value() {
        let cfg = gapped(0, 20);
        let left = snap_with(4, 0, &cfg);
        let right = snap_with(6, 0, &cfg);
        // Each side gives up half, so the visible channel is the full gap.
        assert_eq!(right.left - left.right, 20);
    }

    #[test]
    fn window_gap_leaves_outer_edges_untouched() {
        let w = work();
        let cfg = gapped(0, 20);
        let left = snap_with(4, 0, &cfg);
        // Only the interior (right) edge moves; the screen-facing edges do not,
        // otherwise the two gap settings would compound.
        assert_eq!(left.left, w.left);
        assert_eq!(left.top, w.top);
        assert_eq!(left.bottom, w.bottom);
        assert!(left.right < w.right / 2);
    }

    #[test]
    fn both_gaps_combine_without_double_counting_the_outer_margin() {
        let w = work();
        let cfg = gapped(10, 20);
        let left = snap_with(4, 0, &cfg);
        let right = snap_with(6, 0, &cfg);
        // Outer margin is exactly the screen-edge gap, not edge + window gap.
        assert_eq!(left.left, w.left + 10);
        assert_eq!(right.right, w.right - 10);
        // Interior channel is exactly the window gap.
        assert_eq!(right.left - left.right, 20);
    }

    #[test]
    fn an_absurd_gap_does_not_invert_the_rect() {
        // A gap wider than the screen must degrade gracefully, not produce a
        // negative-size window that the AX API would reject.
        let cfg = gapped(0, 100_000);
        let r = snap_with(4, 0, &cfg);
        assert!(r.width() > 0 && r.height() > 0);
    }

    #[test]
    fn centered_window_gets_the_gap_on_all_four_sides() {
        let cfg = gapped(0, 20);
        // At 80% the centred window touches no work-area edge, so every side
        // is interior and gets inset.
        let c = snap_with(5, 1, &cfg);
        let plain = snap(5, 1);
        assert_eq!(c.left - plain.left, 10);
        assert_eq!(c.top - plain.top, 10);
        assert_eq!(plain.right - c.right, 10);
        assert_eq!(plain.bottom - c.bottom, 10);
    }

    // --- Custom ratios ---

    #[test]
    fn custom_cycle_ratios_replace_the_defaults() {
        let cfg = SnapConfig {
            cycle_ratios: vec![0.25, 0.75],
            ..SnapConfig::default()
        };
        let w = work();
        assert_eq!(
            snap_with(4, 0, &cfg).width(),
            (w.width() as f64 * 0.25) as i32
        );
        assert_eq!(
            snap_with(4, 1, &cfg).width(),
            (w.width() as f64 * 0.75) as i32
        );
        // Two entries means the cycle wraps every two taps, not every three.
        assert_eq!(snap_with(4, 2, &cfg), snap_with(4, 0, &cfg));
    }

    #[test]
    fn a_single_ratio_disables_cycling() {
        let cfg = SnapConfig {
            cycle_ratios: vec![0.5],
            ..SnapConfig::default()
        };
        for cycle in 0..5 {
            assert_eq!(snap_with(4, cycle, &cfg), snap_with(4, 0, &cfg));
        }
    }

    #[test]
    fn custom_center_ratios_are_independent_of_the_grid_ratios() {
        let cfg = SnapConfig {
            cycle_ratios: vec![0.5],
            center_ratios: vec![0.9, 0.45],
            ..SnapConfig::default()
        };
        let w = work();
        assert_eq!(
            snap_with(5, 0, &cfg).width(),
            (w.width() as f64 * 0.9) as i32
        );
        assert_eq!(
            snap_with(5, 1, &cfg).width(),
            (w.width() as f64 * 0.45) as i32
        );
    }

    #[test]
    fn an_empty_ratio_list_falls_back_instead_of_panicking() {
        // Config::sanitise prevents this, but calculate_snap must not divide
        // by zero if handed one anyway.
        let cfg = SnapConfig {
            cycle_ratios: vec![],
            center_ratios: vec![],
            ..SnapConfig::default()
        };
        assert_eq!(snap_with(4, 0, &cfg).width(), work().width() / 2);
        assert_eq!(snap_with(5, 0, &cfg).width(), work().width());
    }

    #[test]
    fn snap_config_derives_from_user_config() {
        let mut config = Config::default();
        config.gaps.screen_edge = 7;
        config.gaps.between_windows = 3;
        config.cycle_ratios = vec![0.4];
        let snap_config = SnapConfig::from(&config);
        assert_eq!(snap_config.screen_edge_gap, 7);
        assert_eq!(snap_config.window_gap, 3);
        assert_eq!(snap_config.cycle_ratios, vec![0.4]);
    }
}
