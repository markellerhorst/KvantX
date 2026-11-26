//! Import of files: `.raw`, `.mzML`, projects and target tables, in background threads.

use crate::app::state::{App, Cell, FinishGuard, LoadInfo, Msg, Sample, has_extension};
use crate::domain::MzData;
use crate::domain::types::{ImportInfo, ImportRoute, Kind};
use crate::formats::mzml;
use crate::formats::project::{self, RawSource};
use crate::formats::rawconv;
use crate::formats::targets;
use crate::support::paths;
use eframe::egui;
use std::path::{Path, PathBuf};
use std::sync::Arc;

impl App {
    // ---------------- loading ----------------
    /// Imports a Thermo `.raw` file in a background thread.
    ///
    /// Returns `false`, after setting a status message, if the import cannot be started.
    pub(crate) fn start_load(&mut self, ctx: &egui::Context, path: &str) -> bool {
        let raw = PathBuf::from(path);
        if !raw.is_file() || !has_extension(&raw, "raw") {
            self.status = format!(
                "Only single Thermo .raw files are supported (not found or not a .raw file): {}",
                path
            );
            return false;
        }
        let trfp = match rawconv::find_trfp(&self.trfp_path) {
            Some(p) => p,
            None => {
                self.show_prefs = true;
                self.status =
                    "ThermoRawFileParser not found: set its location under Settings → Preferences."
                        .into();
                return false;
            }
        };
        self.loading += 1;
        self.status = format!("Reading {} (held in memory only)...", path);
        let (tx, ctx, p) = (self.tx.clone(), ctx.clone(), path.to_string());
        std::thread::spawn(move || {
            let _finish = FinishGuard::new(tx.clone(), ctx.clone());
            let res = rawconv::hash_file(&raw)
                .and_then(|h| rawconv::convert_in_memory(&trfp, &raw).map(|c| (h, c)));
            match res {
                Ok((h, c)) => {
                    let name = raw
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    match mzml::parse_bytes(&name, &c.mzml) {
                        Ok(d) => {
                            let _ = tx.send(Msg::Loaded(
                                Box::new(d),
                                LoadInfo {
                                    path: p,
                                    raw_sha: h,
                                    detail: c.detail,
                                    route: ImportRoute::Conversion,
                                    source: RawSource::Disk(raw.clone()),
                                },
                            ));
                        }
                        Err(e) => {
                            let _ = tx.send(Msg::Err(e));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(Msg::Err(format!("{}: {}", p, e)));
                }
            }
        });
        true
    }

    /// Re-imports a raw file that is stored inside a project: it is copied to a temporary file (ThermoRawFileParser can only read
    /// from a path), converted in memory, and the temporary copy is deleted again right away.
    pub(crate) fn start_load_embedded(
        &mut self,
        ctx: &egui::Context,
        rec_path: String,
        project: PathBuf,
        offset: u64,
        len: u64,
    ) -> bool {
        if rec_path.to_lowercase().ends_with(".mzml") {
            // embedded mzML: parse straight from the project file, no converter and no temporary file
            self.loading += 1;
            self.status = format!("Reading {} from the project...", rec_path);
            let (tx, ctx) = (self.tx.clone(), ctx.clone());
            std::thread::spawn(move || {
                let _finish = FinishGuard::new(tx.clone(), ctx.clone());
                match project::read_blob(&project, offset, len) {
                    Ok(bytes) => {
                        let name = Path::new(&rec_path)
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| "embedded.mzML".into());
                        match mzml::parse_bytes(&name, &bytes) {
                            Ok(d) => {
                                let h = d.sha256.clone();
                                let _ = tx.send(Msg::Loaded(
                                    Box::new(d),
                                    LoadInfo {
                                        path: rec_path,
                                        raw_sha: h,
                                        detail: "format=mzML embedded_in_project".into(),
                                        route: ImportRoute::Direct,
                                        source: RawSource::Embedded {
                                            project,
                                            offset,
                                            len,
                                        },
                                    },
                                ));
                            }
                            Err(e) => {
                                let _ = tx.send(Msg::Err(e));
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Msg::Err(format!("{}: {}", rec_path, e)));
                    }
                }
            });
            return true;
        }
        let trfp = match rawconv::find_trfp(&self.trfp_path) {
            Some(p) => p,
            None => {
                self.show_prefs = true;
                self.status =
                    "ThermoRawFileParser not found: set its location under Settings → Preferences."
                        .into();
                return false;
            }
        };
        self.loading += 1;
        self.status = format!("Reading {} from the project...", rec_path);
        let (tx, ctx) = (self.tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            let _finish = FinishGuard::new(tx.clone(), ctx.clone());
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let dir = paths::temp_dir().join(format!("embedded_{}_{}", std::process::id(), nanos));
            let fname = Path::new(&rec_path)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "embedded.raw".into());
            // A fixed file name: the name stored in a project file is not used for a path.
            let tmp = dir.join("input.raw");
            let res = std::fs::create_dir_all(&dir)
                .map_err(|e| e.to_string())
                .and_then(|_| project::extract_blob(&project, offset, len, &tmp))
                .and_then(|h| rawconv::convert_in_memory(&trfp, &tmp).map(|c| (h, c)));
            let _ = std::fs::remove_file(&tmp);
            let _ = std::fs::remove_dir(&dir);
            match res {
                Ok((h, c)) => match mzml::parse_bytes(&fname, &c.mzml) {
                    Ok(d) => {
                        let detail = format!(
                            "embedded_in_project={} temp_copy_deleted=true {}",
                            project.display(),
                            c.detail
                        );
                        let _ = tx.send(Msg::Loaded(
                            Box::new(d),
                            LoadInfo {
                                path: rec_path,
                                raw_sha: h,
                                detail,
                                route: ImportRoute::Conversion,
                                source: RawSource::Embedded {
                                    project,
                                    offset,
                                    len,
                                },
                            },
                        ));
                    }
                    Err(e) => {
                        let _ = tx.send(Msg::Err(e));
                    }
                },
                Err(e) => {
                    let _ = tx.send(Msg::Err(format!("{}: {}", rec_path, e)));
                }
            }
        });
        true
    }

    /// Imports a dropped or selected file according to its extension: `.raw`, `.mzML`, `.kvantx` or a target table.
    pub(crate) fn load_path(&mut self, ctx: &egui::Context, path: &str) {
        let lower = path.to_lowercase();
        if lower.ends_with(".mzml") {
            self.start_load_mzml(ctx, path);
        } else if lower.ends_with(".raw") {
            self.start_load(ctx, path);
        } else if lower.ends_with(".kvantx") {
            self.open_project(ctx, Path::new(path));
        } else if lower.ends_with(".csv") || lower.ends_with(".tsv") || lower.ends_with(".txt") {
            self.load_table(Path::new(path));
        } else {
            self.status = "Unsupported file: drop Thermo .raw or .mzML files, a .csv target table or a .kvantx project.".into();
        }
    }

    /// Loads a target table and recomputes all chromatograms.
    pub(crate) fn load_table(&mut self, path: &Path) {
        match std::fs::read(path) {
            Ok(bytes) => match targets::parse_table(&String::from_utf8_lossy(&bytes)) {
                Ok(a) => {
                    self.push_undo("load target table");
                    self.audit.log(
                        "TABLE_LOADED",
                        &format!(
                            "{} sha256={} analytes={}",
                            path.display(),
                            crate::support::sha256::sha256_hex(&bytes),
                            a.len()
                        ),
                    );
                    self.analytes = a;
                    self.sel_a = 0;
                    self.table_path = Some(path.to_path_buf());
                    self.rows_dirty = true;
                    for s in self.samples.iter_mut() {
                        s.cells.clear();
                    }
                    self.recompute_all();
                    self.status = format!("Loaded {} targets", self.analytes.len());
                }
                Err(e) => self.status = format!("Table error: {}", e),
            },
            Err(e) => self.status = format!("{}: {}", path.display(), e),
        }
    }

    /// Adds a freshly loaded file.
    ///
    /// While a project is being opened the stored state of the file is applied; otherwise the file is
    /// aligned (if enabled) and all targets are integrated.
    pub fn add_sample(&mut self, d: MzData, info: LoadInfo) {
        self.audit.log("DATA_LOADED", &format!("{} sha256={} ms2_scans={} ms1_scans={} total_scans={} source_raw={} raw_sha256={} {}",
            d.name, d.sha256, d.spectra.len(), d.tic1.len(), d.n_total, info.path, info.raw_sha, info.detail));
        let in_project = self.pending.is_some();
        if !in_project {
            self.push_undo("import file");
        }
        let na = self.analytes.len();
        let mut sm = Sample {
            data: Arc::new(d),
            src_path: info.path.clone(),
            raw_sha: info.raw_sha.clone(),
            source: info.source.clone(),
            import: ImportInfo {
                route: info.route,
                detail: info.detail.clone(),
            },
            kind: Kind::Unknown,
            xics: vec![],
            cells: vec![],
            show: true,
            rt_shift: 0.0,
            shift_ui: 0.0,
        };
        if let Some(p) = self.pending.as_mut() {
            if let Some(rec) = p.recs.iter().find(|r| r.path == info.path) {
                sm.kind = Kind::from_label(&rec.kind);
                sm.rt_shift = rec.rt_shift;
                sm.shift_ui = rec.rt_shift;
                sm.show = rec.show;
                sm.cells = (0..na).map(|_| Cell::new()).collect();
                for c in &rec.cells {
                    if c.analyte < na {
                        let cell = &mut sm.cells[c.analyte];
                        cell.found = c.found;
                        cell.manual = c.manual;
                        cell.start = c.start;
                        cell.end = c.end;
                        cell.logged = (c.start, c.end);
                        cell.include = c.include;
                        cell.conc = c.conc;
                        cell.reason = c.reason.clone();
                    }
                }
                if !rec.raw_sha256.is_empty() && rec.raw_sha256 != info.raw_sha {
                    p.mismatches.push(sm.data.name.clone());
                }
            }
        }
        self.samples.push(sm);
        let i = self.samples.len() - 1;
        if !in_project {
            self.sel_s = i;
            self.anchor = i;
            if self.settings.align_on && self.align_ref < i {
                if let Some(sh) = self.compute_shift(i) {
                    self.samples[i].rt_shift = sh;
                    self.samples[i].shift_ui = sh;
                    let msg = format!(
                        "file={} sha256={} reference={} shift_min={:.4} max_search_min={}",
                        self.samples[i].data.name,
                        self.samples[i].data.sha256,
                        self.samples[self.align_ref].data.name,
                        sh,
                        self.settings.align_max
                    );
                    self.audit.log("RT_ALIGNMENT", &msg);
                }
            }
        }
        self.recompute_sample(i);
        for a in 0..self.analytes.len() {
            self.refit(a);
        }
        self.status = format!("Loaded {}", self.samples[i].data.name);
    }

    // ---------------- mzML import and quick XIC ----------------
    /// Imports `path` as mzML file or, otherwise, as Thermo `.raw` file.
    pub(crate) fn start_load_any(&mut self, ctx: &egui::Context, path: &str) -> bool {
        if path.to_lowercase().ends_with(".mzml") {
            self.start_load_mzml(ctx, path)
        } else {
            self.start_load(ctx, path)
        }
    }

    /// mzML files (from any vendor, e.g. converted with ProteoWizard msconvert) are parsed directly, in memory.
    pub(crate) fn start_load_mzml(&mut self, ctx: &egui::Context, path: &str) -> bool {
        let p = PathBuf::from(path);
        if !p.is_file() || !has_extension(&p, "mzml") {
            self.status = format!("Not found or not an .mzML file: {}", path);
            return false;
        }
        self.loading += 1;
        self.status = format!("Reading {} (held in memory only)...", path);
        let (tx, ctx, ps) = (self.tx.clone(), ctx.clone(), path.to_string());
        std::thread::spawn(move || {
            let _finish = FinishGuard::new(tx.clone(), ctx.clone());
            match std::fs::read(&p) {
                Ok(bytes) => {
                    let name = p
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    match mzml::parse_bytes(&name, &bytes) {
                        Ok(d) => {
                            let h = d.sha256.clone();
                            let _ = tx.send(Msg::Loaded(
                                Box::new(d),
                                LoadInfo {
                                    path: ps,
                                    raw_sha: h,
                                    detail: "format=mzML (no converter)".into(),
                                    route: ImportRoute::Direct,
                                    source: RawSource::Disk(p.clone()),
                                },
                            ));
                        }
                        Err(e) => {
                            let _ = tx.send(Msg::Err(e));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(Msg::Err(format!("{}: {}", ps, e)));
                }
            }
        });
        true
    }
}
