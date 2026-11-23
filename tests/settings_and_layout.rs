//! The analysis settings, the layout and the theme: text formats, limits and defaults.

use msquant::app::{Basis, Layout, Mode, PeakMethod, Theme, WindowState};
use msquant::domain::settings::AnalysisSettings;

#[test]
fn settings_survive_a_round_trip_through_text() {
    let mut s = AnalysisSettings::default();
    assert_eq!(AnalysisSettings::from_text(&s.to_text()), s);
    s.tol_ppm = false;
    s.tol = 0.005;
    s.rt_win = 1.5;
    s.weighting = 1;
    s.baseline = false;
    s.ratio_tol = 45.0;
    s.units = "µM".into();
    s.mode = Mode::Areas;
    s.peak_method = PeakMethod::Adaptive;
    s.basis = Basis::Ms1;
    s.zoom = 1.25;
    s.align_on = true;
    s.align_max = 2.5;
    assert_eq!(AnalysisSettings::from_text(&s.to_text()), s);
}

#[test]
fn the_defaults_are_what_msquant_starts_with() {
    let s = AnalysisSettings::default();
    assert!(s.tol_ppm && s.tol == 5.0 && s.rt_win == 0.3 && s.weighting == 2);
    assert_eq!((s.mode, s.basis, s.peak_method), (Mode::Curve, Basis::Ms2, PeakMethod::Adaptive));
    assert_eq!(s.zoom, 1.0);
}

#[test]
fn hand_edited_settings_are_clamped_and_unknown_keys_ignored() {
    let s = AnalysisSettings::from_text(
        "tol=1e9\nrt_win=-5\nzoom=100\nweighting=9\nmode=nonsense\npeak_method=bogus\nratio_tol=0\nalign_max=NaN\nfoo=bar\nno separator\n",
    );
    assert_eq!(s.tol, 1000.0);
    assert_eq!(s.rt_win, 0.01);
    assert_eq!(s.zoom, 3.0);
    assert_eq!(s.weighting, 2);
    assert_eq!(s.mode, Mode::Curve);
    assert_eq!(s.peak_method, PeakMethod::Adaptive, "an unknown method keeps the default");
    assert_eq!(s.ratio_tol, 1.0);
    assert!(s.align_max.is_finite());
}

#[test]
fn the_layout_survives_a_round_trip_through_text() {
    let mut l = Layout::default();
    assert_eq!(Layout::from_text(&l.to_text()), l);
    l.files = false;
    l.spectrum = false;
    l.left_frac = 0.4;
    l.right_top_frac = 0.7;
    l.cal_left_frac = 0.35;
    l.settings = WindowState { open: true, x: 20.0, y: 30.0, w: 400.0, h: 500.0 };
    assert_eq!(Layout::from_text(&l.to_text()), l);
}

#[test]
fn a_damaged_layout_cannot_produce_absurd_sizes() {
    let l = Layout::from_text("left_frac=-50\nleft_top_frac=1e30\ncal_left_frac=NaN\nsettings_w=abc\nfiles=maybe\n");
    assert_eq!(l.left_frac, 0.1);
    assert_eq!(l.left_top_frac, 0.9);
    assert_eq!(l.cal_left_frac, Layout::default().cal_left_frac);
    assert_eq!(l.settings.w, 0.0);
    assert!(l.files, "an unreadable flag keeps the default");
}

#[test]
fn themes_have_stable_keys() {
    for theme in [Theme::System, Theme::Light, Theme::Dark] {
        assert_eq!(Theme::from_key(theme.key()), Some(theme));
    }
    assert_eq!(Theme::from_key("sepia"), None);
}
