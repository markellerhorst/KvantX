//! The mzML reader.

use msquant::formats::mzml;
use msquant::support::testkit;

#[test]
fn mzml_parsing_of_synthetic_run() {
    let d = mzml::parse_bytes("synthetic", &testkit::synthetic_mzml(0.0, 1.0)).unwrap();
    assert_eq!(d.spectra.len(), 1000, "MS2 scans");
    assert_eq!(d.tic1.len(), 1000, "MS1 points");
    assert_eq!(d.n_total, 2000);
    assert_eq!(d.sha256.len(), 64);
    assert_eq!(
        d.ms1.len(),
        1000,
        "MS1 spectra are kept for MS1-based quantification"
    );
    assert_eq!(d.ms1[3].mz, vec![500.0]);
    assert_eq!(
        d.spectra[0].precursor,
        Some(testkit::PRECURSOR),
        "selected ion m/z is preferred over the isolation window target (286)"
    );
    assert!(
        d.ms1.iter().all(|s| s.precursor.is_none()),
        "MS1 scans have no precursor"
    );
    assert_eq!(
        (d.ms1[0].scan, d.spectra[0].scan, d.spectra[1].scan),
        (0, 1, 3),
        "scan numbers come from the spectrum id"
    );
    assert!((d.spectra[10].rt - 0.105).abs() < 1e-9, "minute unit");
    assert!(
        (d.tic1[10][0] - 0.1).abs() < 1e-9,
        "second unit converted to minutes"
    );
    assert_eq!(d.spectra[0].mz, vec![150.0, 250.0, 400.0]);
    assert!(d.spectra.windows(2).all(|w| w[0].rt <= w[1].rt));
    let apex = d
        .bpc2
        .iter()
        .cloned()
        .fold([0.0, 0.0], |m, p| if p[1] > m[1] { p } else { m });
    assert!((apex[0] - 5.005).abs() < 0.011);
    assert!((apex[1] - 1000.0).abs() < 10.0);
    assert!(mzml::parse_bytes("empty", b"<mzML></mzML>").is_err());
}
