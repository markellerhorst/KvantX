//! Chromatography mathematics: target tables, extraction, integration, peak detection, calibration and alignment.

use kvantx::analysis::{alignment, calibration, integration, peaks, xic};
use kvantx::formats::mzml;
use kvantx::formats::targets;
use kvantx::support::testkit;

#[test]
fn target_table_parsing() {
    let t = targets::parse_table("name,rt_min,ions\nMorphine,n/a,286.14;201.07;165.05\nCodeine,3.2,300.16 215.10\nFentanyl,5.8,337.23,188.13,105.07\n").unwrap();
    assert_eq!(t.len(), 3);
    assert_eq!(t[0].rt, None);
    assert_eq!(t[0].ions, vec![286.14, 201.07, 165.05]);
    assert_eq!(t[1].rt, Some(3.2));
    assert_eq!(t[1].ions.len(), 2);
    assert_eq!(t[2].ions.len(), 3);
    assert!(targets::parse_table("").is_err());
    assert!(targets::parse_table("name,rt,ions\nGood,1.0,100\nBad,x,100\n").is_err());
    assert_eq!(
        targets::parse_table("Drug\t2.5\t100.1\t200.2\n").unwrap()[0].ions,
        vec![100.1, 200.2]
    );
    let p = targets::parse_table("name,rt,ions\nMorphine,n/a,286.14;201.07;p:286.1438\n").unwrap();
    assert_eq!(p[0].precursor, Some(286.1438));
    assert_eq!(
        p[0].ions,
        vec![286.14, 201.07],
        "the precursor tag is not a product ion"
    );
    assert_eq!(
        targets::parse_table("A,1.0,100\n").unwrap()[0].precursor,
        None
    );
}

#[test]
fn xic_integration_matches_the_analytical_area() {
    let d = mzml::parse_bytes("s", &testkit::synthetic_mzml(0.0, 1.0)).unwrap();
    let x = xic::extract(&d.spectra, testkit::FRAG1, 0.01);
    assert_eq!(x.len(), d.spectra.len());
    let area = integration::integrate(&x, 4.5, 5.5, false);
    let expect = testkit::expected_area_seconds(1.0);
    assert!(
        (area - expect).abs() / expect < 0.01,
        "area {} expected {}",
        area,
        expect
    );
    let x2 = xic::extract(&d.spectra, testkit::FRAG2, 0.01);
    let a2 = integration::integrate(&x2, 4.5, 5.5, false);
    assert!((a2 / area - 0.5).abs() < 0.005);
    assert!(
        xic::extract(&d.spectra, 999.0, 0.01)
            .iter()
            .all(|p| p[1] == 0.0)
    );
}

#[test]
fn baseline_subtraction_removes_a_constant_offset() {
    let pts: Vec<[f64; 2]> = (0..1000)
        .map(|i| {
            let t = 4.0 + i as f64 * 0.002;
            [
                t,
                100.0 + 1000.0 * (-0.5f64 * ((t - 5.0) / 0.05).powi(2)).exp(),
            ]
        })
        .collect();
    let with = integration::integrate(&pts, 4.7, 5.3, true);
    let without = integration::integrate(&pts, 4.7, 5.3, false);
    let expect = 1000.0 * 0.05 * (2.0 * std::f64::consts::PI).sqrt() * 60.0;
    assert!(
        (with - expect).abs() / expect < 0.01,
        "{} vs {}",
        with,
        expect
    );
    assert!(without > with * 1.2);
}

#[test]
fn auto_peak_brackets_the_apex_and_honours_the_rt_hint() {
    let g = |t: f64, c: f64, h: f64| h * (-0.5f64 * ((t - c) / 0.05).powi(2)).exp();
    let pts: Vec<[f64; 2]> = (0..1500)
        .map(|i| {
            let t = i as f64 * 0.01;
            [t, g(t, 5.0, 1000.0) + g(t, 8.0, 400.0)]
        })
        .collect();
    let (s, e) = peaks::auto_peak(&pts, None, 0.0).unwrap();
    assert!(s < 5.0 && e > 5.0 && s > 4.6 && e < 5.4, "{} {}", s, e);
    let (s2, e2) = peaks::auto_peak(&pts, Some(8.0), 0.3).unwrap();
    assert!(
        s2 < 8.0 && e2 > 8.0 && s2 > 7.6 && e2 < 8.4,
        "{} {}",
        s2,
        e2
    );
    assert!(peaks::auto_peak(&pts, Some(11.0), 0.3).is_none());
}

#[test]
fn calibration_fit_recovers_the_line_for_every_weighting() {
    let pts: Vec<(f64, f64)> = [1.0, 2.0, 5.0, 10.0, 50.0]
        .iter()
        .map(|&x| (x, 250.0 * x + 40.0))
        .collect();
    for w in 0..3u8 {
        let f = calibration::fit(&pts, w).unwrap();
        assert!(
            (f.m - 250.0).abs() < 1e-6 && (f.b - 40.0).abs() < 1e-5,
            "weighting {}",
            w
        );
        assert!(f.r2 > 0.999999);
        assert_eq!((f.lo, f.hi, f.n), (1.0, 50.0, 5));
    }
    assert!(
        calibration::fit(&[(1.0, 2.0), (1.0, 3.0)], 0).is_none(),
        "one level is not a curve"
    );
}

#[test]
fn rt_alignment_estimates_the_shift_with_the_right_sign() {
    let g = |t: f64, c: f64| (-0.5f64 * ((t - c) / 0.08).powi(2)).exp();
    let make = |off: f64| -> Vec<[f64; 2]> {
        (0..1500)
            .map(|i| {
                let t = i as f64 * 0.01;
                [
                    t,
                    100.0
                        + 5000.0 * g(t, 5.0 + off)
                        + 3000.0 * g(t, 8.0 + off)
                        + 2000.0 * g(t, 12.0 + off),
                ]
            })
            .collect()
    };
    let r = make(0.0);
    assert!(alignment::estimate_shift(&r, &r, 1.0).abs() < 0.011);
    let s = alignment::estimate_shift(&r, &make(0.25), 1.0);
    assert!(
        (s + 0.25).abs() < 0.02,
        "late run must be moved earlier, got {}",
        s
    );
    let s = alignment::estimate_shift(&r, &make(-0.4), 1.0);
    assert!(
        (s - 0.4).abs() < 0.02,
        "early run must be moved later, got {}",
        s
    );
}
