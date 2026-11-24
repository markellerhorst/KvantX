//! Quantification and calibration through the application model.

mod common;

use common::{add, app, targets};
use kvantx::app::{Basis, Kind, Mode};
use kvantx::formats::targets;
use kvantx::support::testkit;

#[test]
fn standard_curve_quantifies_a_known_sample() {
    let mut a = app();
    targets(&mut a);
    for (i, s) in [1.0, 2.0, 4.0, 8.0].iter().enumerate() {
        add(&mut a, i, 0.0, *s);
        a.samples[i].kind = Kind::Standard;
        a.samples[i].cells[0].conc = *s;
    }
    add(&mut a, 9, 0.0, 3.0);
    for k in 0..a.analytes.len() {
        a.refit(k);
    }
    let f = a.fits[0].expect("calibration fitted");
    assert!(f.r2 > 0.9999, "r2 = {}", f.r2);
    let c = a.conc_of(4, 0).expect("unknown quantified");
    assert!((c - 3.0).abs() / 3.0 < 0.02, "got {}", c);
    a.settings.mode = Mode::Areas;
    assert!(
        a.conc_of(4, 0).is_none(),
        "areas-only mode reports no concentrations"
    );
    assert!(a.samples[4].cells[0].areas[0] > 0.0);
}

#[test]
fn ion_ratio_and_area_are_close_to_analytical_values() {
    let mut a = app();
    targets(&mut a);
    add(&mut a, 0, 0.0, 2.0);
    assert!(a.samples[0].cells[0].found);
    let expect = testkit::expected_area_seconds(2.0);
    let with_baseline = a.samples[0].cells[0].areas[0];
    // Without baseline subtraction only the tails beyond the automatic window (5 % of the apex) are missing.
    a.settings.baseline = false;
    a.recompute_all();
    let c = &a.samples[0].cells[0];
    assert!(
        (c.areas[0] - expect).abs() / expect < 0.03,
        "area {} vs {}",
        c.areas[0],
        expect
    );
    assert!((c.areas[1] / c.areas[0] - 0.5).abs() < 0.01);
    // With baseline subtraction the 5 % pedestal at the window edges is removed as well: lower, but not by much.
    assert!(
        with_baseline < c.areas[0] && with_baseline > 0.8 * expect,
        "{} vs {}",
        with_baseline,
        expect
    );
}

#[test]
fn several_targets_each_have_their_own_standards_and_replicates() {
    let mut a = app();
    a.analytes = targets::parse_table("name,rt,ions\nMorph,5.0,150\nFent,5.0,250\n").unwrap();
    a.recompute_all();
    // Morph standards: levels 1, 2, 2 (replicate), 4;  Fent standards: levels 1, 2, 4;  one unknown with scale 3
    let morph = [1.0, 2.0, 2.0, 4.0];
    let fent = [1.0, 2.0, 4.0];
    for (i, s) in morph.iter().enumerate() {
        add(&mut a, i, 0.0, *s);
        a.samples[i].cells[0].conc = *s;
        a.samples[i].kind = Kind::Standard;
    }
    for (j, s) in fent.iter().enumerate() {
        let i = morph.len() + j;
        add(&mut a, i, 0.0, *s);
        a.samples[i].cells[1].conc = *s;
        a.samples[i].kind = Kind::Standard;
    }
    let u = morph.len() + fent.len();
    add(&mut a, u, 0.0, 3.0);
    for k in 0..2 {
        a.refit(k);
    }
    assert_eq!(
        a.fits[0].unwrap().n,
        4,
        "all Morph replicates are used by default"
    );
    assert_eq!(
        a.fits[1].unwrap().n,
        3,
        "Fent standards do not leak into the Morph curve and vice versa"
    );
    let (cm, cf) = (a.conc_of(u, 0).unwrap(), a.conc_of(u, 1).unwrap());
    assert!((cm - 3.0).abs() / 3.0 < 0.02, "Morph {}", cm);
    assert!((cf - 3.0).abs() / 3.0 < 0.02, "Fent {}", cf);
    // exclude one replicate of the level-2 pair
    a.samples[2].cells[0].include = false;
    a.refit(0);
    assert_eq!(a.fits[0].unwrap().n, 3);
    let c2 = a.conc_of(u, 0).unwrap();
    assert!((c2 - 3.0).abs() / 3.0 < 0.02);
    assert_eq!(a.fits[1].unwrap().n, 3, "other target unaffected");
}

#[test]
fn assigning_a_replicate_group_with_one_action() {
    let mut a = app();
    a.analytes = targets::parse_table("name,rt,ions\nMorph,5.0,150\nFent,5.0,250\n").unwrap();
    a.recompute_all();
    for i in 0..4 {
        add(&mut a, i, 0.0, 1.0 + i as f64);
    }
    a.apply_click(0, false, false);
    a.apply_click(2, false, true); // files 0..=2
    a.assign_standards(1, 10.0);
    for i in 0..3 {
        assert!(
            a.samples[i].kind == Kind::Standard
                && a.samples[i].cells[1].conc == 10.0
                && a.samples[i].cells[0].conc == 0.0
        );
    }
    assert!(a.samples[3].kind == Kind::Unknown && a.samples[3].cells[1].conc == 0.0);
    a.undo();
    assert!(a.samples[0].kind == Kind::Unknown && a.samples[0].cells[1].conc == 0.0);
}

#[test]
fn ms1_basis_quantifies_the_precursor_signal() {
    let mut a = app();
    a.analytes = targets::parse_table("name,rt,ions\nSynth,5.0,150;250;p:500\n").unwrap();
    a.recompute_all();
    add(&mut a, 0, 0.0, 1.0);
    let ms2 = a.samples[0].cells[0].areas[0];
    assert_eq!(a.samples[0].xics[0].len(), 2);
    a.set_basis(Basis::Ms1);
    assert_eq!(a.samples[0].xics[0].len(), 1, "one precursor trace");
    let ms1 = a.samples[0].cells[0].areas[0];
    let expect = 50000.0 * 0.1 * (2.0 * std::f64::consts::PI).sqrt() * 60.0;
    assert!(
        (ms1 - expect).abs() / expect < 0.15,
        "MS1 area {} vs {}",
        ms1,
        expect
    );
    assert!(
        (ms1 - ms2).abs() > 1.0,
        "the two signals are different measurements"
    );
    a.undo();
    assert!(a.settings.basis == Basis::Ms2);
    assert_eq!(a.samples[0].xics[0].len(), 2);
}

#[test]
fn alignment_on_import_corrects_a_shifted_run() {
    let mut a = app();
    a.settings.align_on = true;
    targets(&mut a);
    add(&mut a, 0, 0.0, 1.0);
    add(&mut a, 1, 0.2, 1.0);
    assert!(
        (a.samples[1].rt_shift + 0.2).abs() < 0.02,
        "shift {}",
        a.samples[1].rt_shift
    );
    // after alignment both runs integrate to the same area
    let (x, y) = (
        a.samples[0].cells[0].areas[0],
        a.samples[1].cells[0].areas[0],
    );
    assert!((x - y).abs() / x < 0.03);
}
