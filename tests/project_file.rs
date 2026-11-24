//! The text part of a project file.

use kvantx::domain::Analyte;
use kvantx::formats::project::{CellRec, ProjectFile, SampleRec};

#[test]
fn project_file_round_trip() {
    let pf = ProjectFile {
        app_version: "0.3.0".into(),
        user: "eller".into(),
        params: vec![("tol".into(), "10".into()), ("mode".into(), "curve".into())],
        analytes: vec![
            Analyte {
                name: "A".into(),
                rt: None,
                ions: vec![100.5, 200.25],
                precursor: Some(286.1438),
            },
            Analyte {
                name: "B".into(),
                rt: Some(3.25),
                ions: vec![300.0],
                precursor: None,
            },
        ],
        samples: vec![SampleRec {
            path: r"C:\data\a b.raw".into(),
            raw_sha256: "ab".repeat(32),
            kind: "Standard".into(),
            conc: 0.1,
            rt_shift: -0.0123,
            show: true,
            embedded: true,
            cells: vec![
                CellRec {
                    analyte: 0,
                    found: true,
                    manual: true,
                    start: 1.0 / 3.0,
                    end: 2.5,
                    include: false,
                    conc: Some(2.5),
                },
                CellRec {
                    analyte: 1,
                    found: false,
                    manual: false,
                    start: 0.0,
                    end: 0.0,
                    include: true,
                    conc: None,
                },
            ],
        }],
    };
    assert_eq!(ProjectFile::parse(&pf.to_text()).unwrap(), pf);
    assert_eq!(pf.param("mode"), Some("curve"));
    assert!(ProjectFile::parse("garbage").is_err());
    assert!(
        ProjectFile::parse("LCMSQUANT-PROJECT\t1\nC\t0\ttrue\ttrue\t1\t2\ttrue\n").is_err(),
        "cell before sample"
    );
}
