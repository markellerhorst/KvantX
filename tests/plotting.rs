//! Geometry helpers of the plots: nearest point and peak, zoom window, wheel scaling.

use egui_plot::{PlotBounds, PlotPoint};
use msquant::gui::plotting::{self, Boundary, Series};

#[test]
fn hover_picks_the_nearest_trace_and_point() {
    let mk = |name: &str, y: f64, ci: usize| Series {
        name: name.into(), data: (0..100).map(|i| [i as f64 * 0.1, y]).collect(), ci, cur: false, file: 0, level: 2,
        fname: "f".into(), desc: name.into(), scale: 1.0,
    };
    let s = vec![mk("low", 10.0, 0), mk("high", 1000.0, 1)];
    let h = plotting::nearest_hover(&s, 5.04, 900.0, 10.0).unwrap();
    assert_eq!((h.desc.as_str(), h.idx), ("high", 50));
    assert_eq!(plotting::nearest_hover(&s, 5.04, 20.0, 10.0).unwrap().desc, "low");
    assert!(plotting::nearest_hover(&s, 50.0, 10.0, 10.0).is_none(), "cursor far outside the data");
}

#[test]
fn cursor_snaps_to_the_nearest_peak_and_top_peaks_are_ranked() {
    let (mz, inten) = (vec![100.0, 150.0, 200.0, 250.0], vec![10.0f32, 100.0, 50.0, 70.0]);
    let (xw, yw) = (200.0, 100.0);
    assert_eq!(plotting::nearest_peak(&mz, &inten, 1.0, 151.0, 50.0, xw, yw), Some((150.0, 100.0)));
    assert_eq!(plotting::nearest_peak(&mz, &inten, 1.0, 197.0, 60.0, xw, yw), Some((200.0, 50.0)));
    assert_eq!(plotting::nearest_peak(&mz, &inten, 0.5, 250.0, 10.0, xw, yw), Some((250.0, 35.0)), "scaled intensity is returned");
    assert_eq!(plotting::nearest_peak(&mz, &inten, 1.0, 900.0, 10.0, xw, yw), None, "nothing near the cursor");
    let t = plotting::top_peaks(&mz, &inten, 3);
    assert_eq!(t.iter().map(|p| p.0).collect::<Vec<_>>(), vec![150.0, 250.0, 200.0]);
    assert_eq!(plotting::top_peaks(&mz, &inten, 10).len(), 4);
}

#[test]
fn zoom_window_needs_a_real_drag_and_is_order_independent() {
    let (xw, yw) = (10.0, 1000.0);
    assert_eq!(plotting::zoom_box([2.0, 100.0], [4.0, 600.0], xw, yw), Some(([2.0, 100.0], [4.0, 600.0])));
    assert_eq!(plotting::zoom_box([4.0, 600.0], [2.0, 100.0], xw, yw), Some(([2.0, 100.0], [4.0, 600.0])), "dragging up-left works too");
    assert_eq!(plotting::zoom_box([2.0, 100.0], [2.05, 600.0], xw, yw), None, "a click or a tiny jitter does not zoom");
    assert_eq!(plotting::zoom_box([2.0, 100.0], [4.0, 105.0], xw, yw), None);
}

#[test]
fn boundary_lines_are_only_grabbed_when_the_pointer_is_on_them() {
    // 100 px per minute: 8 px = 0.08 min
    assert_eq!(plotting::nearest_boundary(4.95, 5.0, 5.4, 100.0, 8.0), Some(Boundary::Start));
    assert_eq!(plotting::nearest_boundary(5.37, 5.0, 5.4, 100.0, 8.0), Some(Boundary::End));
    assert_eq!(plotting::nearest_boundary(5.2, 5.0, 5.4, 100.0, 8.0), None, "in the middle of the peak a drag draws a zoom window");
    assert_eq!(plotting::nearest_boundary(5.2, 5.0, 5.4, 1000.0, 8.0), None);
    assert_eq!(plotting::nearest_boundary(5.004, 5.0, 5.4, 1000.0, 8.0), Some(Boundary::Start));
}

#[test]
fn wheel_scaling_keeps_the_pivot_in_place() {
    assert_eq!(plotting::scale_range(0.0, 10.0, 5.0, 0.5), (2.5, 7.5));
    assert_eq!(plotting::scale_range(0.0, 10.0, 0.0, 0.5), (0.0, 5.0), "pivot at the edge: that edge stays");
    assert_eq!(plotting::scale_range(0.0, 10.0, 5.0, 2.0), (-5.0, 15.0), "factor > 1 zooms out");
    let (lo, hi) = plotting::scale_range(100.0, 200.0, 120.0, 0.25);
    assert!((120.0 - lo) / (hi - lo) - 0.2 < 1e-12, "relative pivot position is unchanged");
}

#[test]
fn cursor_following_is_limited_to_the_data_area() {
    let b = PlotBounds::from_min_max([2.0, 0.0], [8.0, 100.0]);
    assert!(plotting::in_view(&PlotPoint::new(5.0, 50.0), &b));
    assert!(plotting::in_view(&PlotPoint::new(2.0, 0.0), &b), "the border counts as inside");
    assert!(!plotting::in_view(&PlotPoint::new(1.9, 50.0), &b), "over the y axis labels");
    assert!(!plotting::in_view(&PlotPoint::new(5.0, -3.0), &b), "below the x axis");
    assert!(!plotting::in_view(&PlotPoint::new(8.5, 50.0), &b));
}
