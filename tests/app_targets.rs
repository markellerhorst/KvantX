//! Editing targets and the quick extracted-ion chromatogram.

mod common;

use common::{add, app, targets};
use msquant::app::Basis;
use msquant::formats::targets;
use msquant::support::testkit;

#[test]
fn live_target_editing_recomputes_areas() {
    let mut a = app();
    add(&mut a, 0, 0.0, 1.0);
    targets(&mut a);
    let first = a.samples[0].cells[0].areas[0];
    assert!(first > 0.0);
    a.sync_rows();
    a.rows[0][3] = "400".into(); // constant 10-count ion: different area
    a.apply_row(0);
    assert_eq!(a.analytes[0].ions, vec![400.0]);
    assert!((a.samples[0].cells[0].areas[0] - first).abs() > 1.0);
    a.rows[0][3] = "".into(); // invalid: must not be applied
    a.apply_row(0);
    assert_eq!(a.analytes[0].ions, vec![400.0]);
    a.add_target();
    assert_eq!(a.analytes.len(), 2);
    assert_eq!(a.samples[0].cells.len(), 2);
    a.delete_target(0);
    assert_eq!(a.analytes.len(), 1);
    assert_eq!(a.samples[0].cells.len(), 1);
    let csv_text = a.targets_csv();
    assert!(targets::parse_table(&csv_text).is_ok());
}

#[test]
fn target_without_precursor_gives_no_peak_on_ms1_basis() {
    let mut a = app();
    targets(&mut a);
    add(&mut a, 0, 0.0, 1.0);
    a.set_basis(Basis::Ms1);
    assert!(!a.samples[0].cells[0].found);
    assert!(a.status.contains("precursor"));
}

#[test]
fn quick_xic_traces_a_custom_mz_without_a_target() {
    let mut a = app();
    add(&mut a, 0, 0.0, 2.0);
    a.quick_mz = testkit::FRAG1;
    a.quick_level = 2;
    let x = a.quick_trace(0);
    let apex = x
        .iter()
        .cloned()
        .fold([0.0, 0.0], |m, p| if p[1] > m[1] { p } else { m });
    assert!(
        (apex[0] - 5.0).abs() < 0.02 && (apex[1] - 2000.0).abs() < 25.0,
        "{:?}",
        apex
    );
    a.quick_mz = 500.0;
    a.quick_level = 1;
    assert!(
        a.quick_trace(0).iter().any(|p| p[1] > 40000.0),
        "MS1 trace of the MS1 ion"
    );
    let before = a.analytes.len();
    a.add_quick_target();
    assert_eq!(a.analytes.len(), before + 1);
    assert_eq!(a.analytes[before].precursor, Some(500.0));
    assert!(a.samples[0].cells.len() == a.analytes.len());
}
