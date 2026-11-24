//! Local tests against private instrument data. They are NOT run by `cargo test` and never in CI.
//!
//! ```text
//! cargo test --test real_data -- --ignored            all real-data tests
//! cargo test --test real_data thermo -- --ignored     Thermo RAW only (needs ThermoRawFileParser)
//! cargo test --test real_data mzml -- --ignored       mzML only
//! cargo test -- --include-ignored                     synthetic and real-data tests together
//! ```
//!
//! The data lives in `./testData` or in the folder named by `KVANTX_TEST_DATA_DIR` (see
//! `docs/testdata/README.md`). A test that cannot find its data or the converter FAILS with a clear
//! message; it never passes silently. The tests check structure and behavior. No analytical result is
//! asserted, because none has been established independently yet.

use kvantx::analysis::{alignment, integration, peaks, xic};
use kvantx::domain::MzData;
use kvantx::formats::mzml;
use kvantx::formats::rawconv;
use kvantx::formats::targets;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const IGNORE: &str = "needs private instrument data (docs/testdata/README.md)";

/// Folder with the private test data.
fn data_dir() -> PathBuf {
    match std::env::var_os("KVANTX_TEST_DATA_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testData"),
    }
}

/// All files with the extension `ext` below `testData/<sub>` (at most three folder levels), sorted.
fn files_in(sub: &str, ext: &str) -> Vec<PathBuf> {
    fn walk(dir: &Path, ext: &str, depth: usize, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for path in entries.filter_map(|e| e.ok()).map(|e| e.path()) {
            if path.is_dir() {
                if depth < 3 {
                    walk(&path, ext, depth + 1, out);
                }
            } else if path
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case(ext))
            {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(&data_dir().join(sub), ext, 0, &mut out);
    out.sort();
    out
}

/// Like [`files_in`], but fails the test with an explanation if nothing is found.
fn require_files(sub: &str, ext: &str) -> Vec<PathBuf> {
    let found = files_in(sub, ext);
    assert!(
        !found.is_empty(),
        "no .{ext} files in {}. Put representative data there or set KVANTX_TEST_DATA_DIR (docs/testdata/README.md).",
        data_dir().join(sub).display()
    );
    found
}

/// Location of ThermoRawFileParser; fails the test if it cannot be found.
fn require_trfp() -> PathBuf {
    rawconv::find_trfp("").unwrap_or_else(|| {
        panic!(
            "ThermoRawFileParser not found. Unzip it to tools/ThermoRawFileParser*/ or set THERMO_RAW_FILE_PARSER \
             (it needs the .NET 8 runtime)."
        )
    })
}

/// Structural properties every imported run must have.
fn assert_structure(d: &MzData, label: &str) {
    assert_eq!(d.sha256.len(), 64, "{label}: hash of the data");
    assert!(
        !d.spectra.is_empty() || !d.tic1.is_empty(),
        "{label}: no scans were read"
    );
    assert!(
        d.n_total >= d.spectra.len() + d.tic1.len(),
        "{label}: scan counts are inconsistent"
    );
    for tr in [&d.tic1, &d.bpc1, &d.tic2, &d.bpc2] {
        assert!(
            tr.windows(2).all(|w| w[0][0] <= w[1][0]),
            "{label}: retention times must be ascending"
        );
        assert!(
            tr.iter()
                .all(|p| p[0].is_finite() && p[1].is_finite() && p[1] >= 0.0),
            "{label}: non-finite or negative values"
        );
    }
    assert_eq!(
        d.tic1.len(),
        d.bpc1.len(),
        "{label}: MS1 traces differ in length"
    );
    assert_eq!(
        d.tic2.len(),
        d.spectra.len(),
        "{label}: one MS2 point per MS2 scan"
    );
    for (t, b) in d.tic1.iter().zip(d.bpc1.iter()) {
        assert!(
            b[1] <= t[1] + 1e-6,
            "{label}: base peak above the total ion current"
        );
    }
    for s in &d.spectra {
        assert_eq!(
            s.mz.len(),
            s.inten.len(),
            "{label}: m/z and intensity arrays differ in length"
        );
        assert!(
            s.mz.iter().all(|m| m.is_finite()),
            "{label}: non-finite m/z"
        );
    }
}

struct Loaded {
    path: PathBuf,
    raw_sha: String,
    data: MzData,
}

/// Converts the first Thermo RAW file once (conversion can take a while) and shares it between tests.
fn first_thermo_run() -> &'static Loaded {
    static CELL: OnceLock<Loaded> = OnceLock::new();
    CELL.get_or_init(|| {
        let raw = require_files("thermo_raw", "raw").remove(0);
        let trfp = require_trfp();
        let raw_sha = rawconv::hash_file(&raw).expect("hash the raw file");
        let conv = rawconv::convert_in_memory(&trfp, &raw)
            .unwrap_or_else(|e| panic!("conversion of {} failed: {}", raw.display(), e));
        let name = raw.file_name().unwrap().to_string_lossy().to_string();
        let data = mzml::parse_bytes(&name, &conv.mzml)
            .unwrap_or_else(|e| panic!("{} could not be parsed: {}", name, e));
        Loaded {
            path: raw,
            raw_sha,
            data,
        }
    })
}

#[test]
#[ignore = "needs private instrument data (docs/testdata/README.md)"]
fn thermo_raw_import_hashes_converts_and_parses() {
    let l = first_thermo_run();
    assert_eq!(l.raw_sha.len(), 64);
    assert_structure(&l.data, &l.path.display().to_string());
}

#[test]
#[ignore = "needs private instrument data (docs/testdata/README.md)"]
fn thermo_import_leaves_no_files_next_to_the_raw_file() {
    let l = first_thermo_run();
    let stray = std::fs::read_dir(l.path.parent().unwrap())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("mzml"))
        })
        .count();
    assert_eq!(
        stray, 0,
        "in-memory conversion must not leave .mzML files behind"
    );
}

#[test]
#[ignore = "needs private instrument data (docs/testdata/README.md)"]
fn thermo_xic_extraction_and_integration_on_the_strongest_fragment() {
    let l = first_thermo_run();
    assert!(
        !l.data.spectra.is_empty(),
        "the file has no MS2 scans; use a targeted MS/MS file"
    );
    // data-driven target: the most intense fragment of the most intense MS2 scan
    let best = l
        .data
        .spectra
        .iter()
        .max_by(|a, b| {
            a.inten
                .iter()
                .sum::<f32>()
                .total_cmp(&b.inten.iter().sum::<f32>())
        })
        .unwrap();
    let (mut mz, mut top) = (0.0, 0.0f32);
    for (m, i) in best.mz.iter().zip(best.inten.iter()) {
        if *i > top {
            top = *i;
            mz = *m;
        }
    }
    let x = xic::extract(&l.data.spectra, mz, mz * 10e-6);
    assert_eq!(x.len(), l.data.spectra.len());
    assert!(
        x.iter().any(|p| p[1] > 0.0),
        "the chosen fragment must appear in its own scan"
    );
    let (t0, t1) = (x.first().unwrap()[0], x.last().unwrap()[0]);
    let total = integration::integrate(&x, t0, t1, false);
    assert!(total > 0.0);
    let (s, e) = peaks::auto_peak(&x, None, 0.0).expect("a peak is found");
    assert!(s >= t0 && e <= t1 && s < e);
    let peak = integration::integrate(&x, s, e, false);
    assert!(
        peak > 0.0 && peak <= total * 1.000001,
        "peak area {peak} exceeds the whole-run area {total}"
    );
}

#[test]
#[ignore = "needs private instrument data (docs/testdata/README.md)"]
fn thermo_a_run_aligns_to_itself_with_zero_shift() {
    let l = first_thermo_run();
    let tr = if l.data.tic1.len() >= 5 {
        &l.data.tic1
    } else {
        &l.data.tic2
    };
    assert!(alignment::estimate_shift(tr, tr, 1.0).abs() < 0.011);
}

#[test]
#[ignore = "needs private instrument data (docs/testdata/README.md)"]
fn thermo_two_runs_get_a_bounded_alignment_shift() {
    let raws = require_files("thermo_raw", "raw");
    assert!(
        raws.len() >= 2,
        "this test needs two .raw files in testData/thermo_raw, found {}",
        raws.len()
    );
    let trfp = require_trfp();
    let load = |p: &PathBuf| -> MzData {
        let conv = rawconv::convert_in_memory(&trfp, p)
            .unwrap_or_else(|e| panic!("{}: {}", p.display(), e));
        mzml::parse_bytes(&p.file_name().unwrap().to_string_lossy(), &conv.mzml).expect("parse")
    };
    let (a, b) = (load(&raws[0]), load(&raws[1]));
    let tr = |d: &MzData| {
        if d.tic1.len() >= 5 {
            d.tic1.clone()
        } else {
            d.tic2.clone()
        }
    };
    let s = alignment::estimate_shift(&tr(&a), &tr(&b), 1.5);
    assert!(s.abs() <= 1.5 + 1e-9, "shift {s} outside the search window");
}

#[test]
#[ignore = "needs private instrument data (docs/testdata/README.md)"]
fn thermo_target_table_applied_to_the_real_run_never_panics() {
    let l = first_thermo_run();
    let table = require_files("targets", "csv").remove(0);
    let list = targets::parse_table(&std::fs::read_to_string(&table).unwrap())
        .unwrap_or_else(|e| panic!("{}: {}", table.display(), e));
    for a in &list {
        for &mz in &a.ions {
            let x = xic::extract(&l.data.spectra, mz, mz * 10e-6);
            if let Some((s, e)) = peaks::auto_peak(&x, a.rt, 0.3) {
                assert!(integration::integrate(&x, s, e, true).is_finite());
            }
        }
    }
}

#[test]
#[ignore = "needs private instrument data (docs/testdata/README.md)"]
fn mzml_files_import_with_consistent_structure() {
    for path in require_files("mzml", "mzml") {
        let bytes = std::fs::read(&path).unwrap();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let data = mzml::parse_bytes(&name, &bytes)
            .unwrap_or_else(|e| panic!("{}: {}", path.display(), e));
        assert_structure(&data, &name);
    }
}

#[test]
#[ignore = "needs private instrument data (docs/testdata/README.md)"]
fn targets_tables_in_testdata_parse() {
    for table in require_files("targets", "csv") {
        let list = targets::parse_table(&std::fs::read_to_string(&table).unwrap())
            .unwrap_or_else(|e| panic!("{}: {}", table.display(), e));
        assert!(
            list.iter()
                .all(|x| !x.ions.is_empty() && !x.name.is_empty())
        );
    }
}

#[test]
fn the_ignore_reason_is_documented() {
    // keeps the constant used and the documentation path honest
    assert!(IGNORE.contains("docs/testdata/README.md"));
    assert!(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("docs/testdata/README.md")
            .exists()
    );
}
