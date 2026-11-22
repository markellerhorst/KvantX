//! Selection of the MS2 scan that is shown, and precursor ions.

mod common;

use common::{add, app, targets};
use msquant::gui::plotting;
use msquant::support::testkit;

#[test]
fn spectrum_panel_scan_selection() {
    let mut a = app();
    targets(&mut a);
    add(&mut a, 0, 0.0, 1.0);
    let i = a.apex_scan(0, 0).expect("apex scan");
    assert!((a.samples[0].data.spectra[i].rt - 5.0).abs() < 0.02);
    assert!(matches!(
        a.spectrum_pick(),
        Some((0, _, "apex of the selected target"))
    ));
    let j = a.nearest_ms2(0, 5.0).unwrap();
    assert!((a.samples[0].data.spectra[j].rt - 5.0).abs() <= 0.005 + 1e-9);
    a.pin_scan(0, 7.0);
    assert!(matches!(a.spectrum_pick(), Some((0, _, "pinned"))));
    a.hover = Some((0, 2.0));
    assert!(
        matches!(a.spectrum_pick(), Some((0, _, "pinned"))),
        "hover is ignored by default"
    );
    a.follow_hover = true;
    assert!(matches!(a.spectrum_pick(), Some((0, _, "hovered"))));
    a.follow_hover = false;
    assert!(matches!(a.spectrum_pick(), Some((0, _, "pinned"))));
}

#[test]
fn spectrum_panel_does_not_follow_hover_by_default() {
    assert!(!app().follow_hover);
}

#[test]
fn precursor_of_each_ms2_scan_is_available() {
    let mut a = app();
    add(&mut a, 0, 0.0, 1.0);
    let sp = &a.samples[0].data.spectra;
    assert_eq!(
        sp[0].precursor,
        Some(testkit::PRECURSOR),
        "selected ion m/z wins over the isolation window target"
    );
    assert_eq!(plotting::prec_str(sp[0].precursor), "286.1438");
    assert_eq!(plotting::prec_str(None), "n/a");
    assert!(a.samples[0].data.ms1.iter().all(|s| s.precursor.is_none()));
    assert!(plotting::scan_info(&a.samples[0].data, 2, 0).contains("precursor m/z 286.1438"));
    assert!(plotting::scan_info(&a.samples[0].data, 1, 0).starts_with("MS1 scan"));
}
