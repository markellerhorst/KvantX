//! Helpers shared by the integration tests.
//!
//! Every test file is its own crate and uses a different subset of these functions.
#![allow(dead_code)]

use msquant::app::{App, LoadInfo};
use msquant::mzml;
use msquant::project::{RawSource, SampleRec};
use msquant::quant;
use msquant::testkit;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A fresh, empty temporary folder that is unique for this process and `name`.
pub fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("msquant_test_{}_{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temporary test folder");
    dir
}

/// A project record of a file without any analysis state.
pub fn sample_rec(path: &str) -> SampleRec {
    SampleRec {
        path: path.into(),
        raw_sha256: String::new(),
        kind: "Unknown".into(),
        conc: 0.0,
        rt_shift: 0.0,
        show: true,
        embedded: false,
        cells: vec![],
    }
}

/// An application without a window. It does not read the user settings and writes its audit
/// trail to a private temporary file.
pub fn app() -> App {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let trail = std::env::temp_dir().join(format!(
        "msquant_test_audit_{}_{}.log",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_file(&trail);
    let mut app = App::new_with_audit(&trail.to_string_lossy());
    app.trfp_path.clear();
    app
}

/// Provenance record of the `n`-th synthetic file.
pub fn info(n: usize) -> LoadInfo {
    LoadInfo {
        path: format!("synthetic{}.raw", n),
        raw_sha: "0".repeat(64),
        detail: String::new(),
        source: RawSource::Disk(PathBuf::from(format!("synthetic{}.raw", n))),
    }
}

/// Imports a synthetic run (see [`testkit`]) as the `n`-th file.
pub fn add(app: &mut App, n: usize, offset: f64, scale: f64) {
    let data = mzml::parse_bytes(
        &format!("run{}.raw", n),
        &testkit::synthetic_mzml(offset, scale),
    )
    .expect("the synthetic mzML parses");
    app.add_sample(data, info(n));
}

/// Defines the single target `Synth` (RT 5.0 min, product ions 150 and 250).
pub fn targets(app: &mut App) {
    app.analytes = quant::parse_table("name,rt,ions\nSynth,5.0,150;250\n").expect("valid table");
    app.recompute_all();
}
