//! The manifest of a project: schema 1, round trip and validation.

use kvantx::domain::Analyte;
use kvantx::domain::types::{ImportInfo, ImportRoute};
use kvantx::formats::project::{
    AuditAnchor, CellRec, FORMAT_NAME, ProjectFile, SCHEMA_VERSION, SampleRec,
};
use kvantx::support::json::{self, Value};

fn sample_project() -> ProjectFile {
    ProjectFile {
        app_version: "0.3.0".into(),
        saved_utc: "2026-11-25T12:00:00Z".into(),
        user: "eller".into(),
        params: vec![
            ("tol".into(), "5".into()),
            ("tol_ppm".into(), "true".into()),
            ("units".into(), "ng/mL".into()),
            ("peak_method".into(), "adaptive".into()),
            ("zoom".into(), "1.5".into()),
        ],
        analytes: vec![
            Analyte {
                name: "Analyte \"A\" µ".into(),
                rt: Some(5.5),
                ions: vec![150.0, 250.5],
                precursor: Some(300.1),
            },
            Analyte {
                name: "B".into(),
                rt: None,
                ions: vec![100.25],
                precursor: None,
            },
        ],
        samples: vec![SampleRec {
            path: "C:\\data\\a b.raw".into(),
            raw_sha256: "ab".repeat(32),
            kind: "Standard".into(),
            rt_shift: -0.125,
            show: true,
            embedded: false,
            import: ImportInfo {
                route: ImportRoute::Conversion,
                detail: "converter=ThermoRawFileParser".into(),
            },
            cells: vec![
                CellRec {
                    analyte: 0,
                    found: true,
                    manual: true,
                    start: 5.4,
                    end: 5.6,
                    include: true,
                    conc: 0.5,
                    reason: "shoulder on the left".into(),
                },
                CellRec {
                    analyte: 1,
                    found: false,
                    manual: false,
                    start: 0.0,
                    end: 0.0,
                    include: false,
                    conc: 0.0,
                    reason: String::new(),
                },
            ],
        }],
        audit: AuditAnchor {
            entries: 7,
            head: "f".repeat(64),
        },
    }
}

fn set(v: &mut Value, path: &[&str], new: Value) {
    match path {
        [] => *v = new,
        [key, rest @ ..] => match v {
            Value::Object(pairs) => {
                let slot = pairs
                    .iter_mut()
                    .find(|(k, _)| k == key)
                    .unwrap_or_else(|| panic!("no key {key}"));
                set(&mut slot.1, rest, new);
            }
            Value::Array(items) => set(&mut items[key.parse::<usize>().unwrap()], rest, new),
            _ => panic!("cannot descend into a scalar at {key}"),
        },
    }
}

/// The manifest of the sample project with one value replaced.
fn mutated(path: &[&str], new: Value) -> String {
    let mut v = json::parse(&sample_project().to_json().unwrap()).unwrap();
    set(&mut v, path, new);
    v.to_pretty_string().unwrap()
}

fn num(n: f64) -> Value {
    Value::Number(n)
}

#[test]
fn the_manifest_round_trips_exactly() {
    let pf = sample_project();
    assert_eq!(ProjectFile::from_json(&pf.to_json().unwrap()).unwrap(), pf);
}

#[test]
fn the_manifest_names_its_format_and_schema() {
    let v = json::parse(&sample_project().to_json().unwrap()).unwrap();
    assert_eq!(v.get("format").and_then(Value::as_str), Some(FORMAT_NAME));
    assert_eq!(
        v.get("schema").and_then(Value::as_u64),
        Some(SCHEMA_VERSION)
    );
    assert_eq!(
        v.get("saved_by")
            .unwrap()
            .get("application")
            .and_then(Value::as_str),
        Some("KvantX")
    );
}

#[test]
fn a_newer_schema_is_refused_and_explained() {
    let err = ProjectFile::from_json(&mutated(&["schema"], num(2.0))).unwrap_err();
    assert!(
        err.contains("newer KvantX") && err.contains("schema 2"),
        "{err}"
    );
}

#[test]
fn a_wrong_format_or_schema_is_refused() {
    assert!(
        ProjectFile::from_json(&mutated(&["format"], json::string("something-else")))
            .unwrap_err()
            .contains("not a KvantX project")
    );
    assert!(ProjectFile::from_json(&mutated(&["schema"], num(0.0))).is_err());
    assert!(ProjectFile::from_json(&mutated(&["schema"], num(1.5))).is_err());
    assert!(ProjectFile::from_json(&mutated(&["schema"], json::string("1"))).is_err());
    assert!(ProjectFile::from_json("[]").is_err());
    assert!(ProjectFile::from_json("not json").is_err());
}

#[test]
fn invalid_values_are_rejected_with_a_message_that_names_the_problem() {
    let cases: Vec<(Vec<&str>, Value, &str)> = vec![
        (
            vec!["samples", "0", "raw_sha256"],
            json::string("xyz"),
            "raw_sha256",
        ),
        (
            vec!["samples", "0", "raw_sha256"],
            json::string(&"AB".repeat(32)),
            "lowercase",
        ),
        (
            vec!["samples", "0", "kind"],
            json::string("Bogus"),
            "unknown type",
        ),
        (vec!["samples", "0", "rt_shift_min"], num(1e6), "shift"),
        (
            vec!["samples", "0", "import", "route"],
            json::string("teleport"),
            "route",
        ),
        (
            vec!["samples", "0", "path"],
            json::string(""),
            "path is empty",
        ),
        (
            vec!["samples", "0", "cells", "0", "target"],
            num(9.0),
            "does not exist",
        ),
        (
            vec!["samples", "0", "cells", "1", "target"],
            num(0.0),
            "twice",
        ),
        (
            vec!["samples", "0", "cells", "0", "start_min"],
            num(9.0),
            "reversed",
        ),
        (
            vec!["samples", "0", "cells", "0", "standard_conc"],
            num(-1.0),
            "negative",
        ),
        (
            vec!["samples", "0", "cells", "0", "manual"],
            json::string("yes"),
            "true or false",
        ),
        (
            vec!["targets", "0", "product_ions"],
            Value::Array(vec![]),
            "no product ion",
        ),
        (
            vec!["targets", "0", "product_ions"],
            Value::Array(vec![num(-1.0)]),
            "positive",
        ),
        (vec!["targets", "0", "name"], json::string("   "), "empty"),
        (vec!["targets", "0", "rt_min"], num(-1.0), "negative"),
        (vec!["targets", "0", "precursor_mz"], num(0.0), "positive"),
        (
            vec!["audit", "head"],
            json::string("bad hash!"),
            "invalid characters",
        ),
        (vec!["audit", "entries"], num(-1.0), "non-negative"),
        (
            vec!["settings", "tol"],
            Value::Array(vec![]),
            "must be text",
        ),
    ];
    for (path, value, expected) in cases {
        let err = ProjectFile::from_json(&mutated(&path, value))
            .expect_err(&format!("{path:?} should be rejected"));
        assert!(err.contains(expected), "{path:?}: {err}");
    }
}

#[test]
fn a_missing_field_is_reported() {
    let mut v = json::parse(&sample_project().to_json().unwrap()).unwrap();
    if let Value::Object(pairs) = &mut v {
        pairs.retain(|(k, _)| k != "samples");
    }
    let err = ProjectFile::from_json(&v.to_pretty_string().unwrap()).unwrap_err();
    assert!(err.contains("samples"), "{err}");
}

#[test]
fn unknown_extra_fields_are_ignored_within_the_same_schema() {
    let mut v = json::parse(&sample_project().to_json().unwrap()).unwrap();
    if let Value::Object(pairs) = &mut v {
        pairs.push(("future_feature".into(), Value::Bool(true)));
    }
    assert_eq!(
        ProjectFile::from_json(&v.to_pretty_string().unwrap()).unwrap(),
        sample_project()
    );
}

#[test]
fn the_same_file_cannot_be_listed_twice() {
    let mut pf = sample_project();
    pf.samples.push(pf.samples[0].clone());
    assert!(pf.to_json().unwrap_err().contains("twice"));
}

#[test]
fn a_project_cannot_start_thousands_of_jobs() {
    let mut pf = sample_project();
    pf.samples = (0..2001)
        .map(|i| SampleRec {
            path: format!("file{i}.raw"),
            cells: vec![],
            ..sample_project().samples[0].clone()
        })
        .collect();
    assert!(pf.to_json().unwrap_err().contains("more than 2000"));
    let mut pf = sample_project();
    pf.samples.clear();
    pf.analytes = (0..2001)
        .map(|i| Analyte {
            name: format!("T{i}"),
            rt: None,
            ions: vec![100.0],
            precursor: None,
        })
        .collect();
    assert!(pf.to_json().unwrap_err().contains("more than 2000"));
}

#[test]
fn values_that_are_not_finite_cannot_be_written() {
    for bad in [f64::NAN, f64::INFINITY] {
        let mut pf = sample_project();
        pf.samples[0].cells[0].start = bad;
        assert!(pf.to_json().is_err(), "{bad}");
        let mut pf = sample_project();
        pf.samples[0].rt_shift = bad;
        assert!(pf.to_json().is_err(), "{bad}");
    }
}

#[test]
fn settings_keep_their_exact_text() {
    let mut pf = sample_project();
    pf.params.push(("unit_note".into(), "1e3".into()));
    pf.params.push(("flag".into(), "false".into()));
    pf.params.push(("word".into(), "true".into()));
    pf.params.push(("odd".into(), " 5".into()));
    let back = ProjectFile::from_json(&pf.to_json().unwrap()).unwrap();
    assert_eq!(back.params, pf.params);
}
