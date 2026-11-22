//! Opening, saving and clearing projects.

use crate::app::state::{App, Msg, NewProjectForm, Pending, VERSION};
use crate::formats::project::{self, BlobSource, CellRec, ProjectFile, RawSource, SampleRec};
use crate::formats::rawconv;
use crate::support::paths;
use eframe::egui;
use std::path::Path;
use std::path::PathBuf;

impl App {
    // ---------------- projects ----------------
    /// Collects everything a project stores (all of it except the raw data, which is copied separately).
    pub fn project_file(&self) -> ProjectFile {
        ProjectFile {
            app_version: VERSION.into(),
            user: self.audit.user.clone(),
            params: self.params_vec(),
            analytes: self.analytes.clone(),
            samples: self
                .samples
                .iter()
                .map(|s| SampleRec {
                    path: s.src_path.clone(),
                    raw_sha256: s.raw_sha.clone(),
                    kind: s.kind.label().into(),
                    conc: 0.0,
                    rt_shift: s.rt_shift,
                    show: s.show,
                    embedded: false,
                    cells: s
                        .cells
                        .iter()
                        .enumerate()
                        .map(|(i, c)| CellRec {
                            analyte: i,
                            found: c.found,
                            manual: c.manual,
                            start: c.start,
                            end: c.end,
                            include: c.include,
                            conc: Some(c.conc),
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    /// Saves the project in a background thread; with `force_dialog` it always asks for a path.
    pub(crate) fn save_project(&mut self, ctx: &egui::Context, force_dialog: bool) {
        let path = match (&self.project_path, force_dialog) {
            (Some(p), false) => p.clone(),
            _ => match self
                .dialog()
                .set_title("Save project")
                .add_filter("msQuant project", &["quant"])
                .set_file_name("project.quant")
                .save_file()
            {
                Some(p) => {
                    if p.extension().is_none() {
                        p.with_extension("quant")
                    } else {
                        p
                    }
                }
                None => return,
            },
        };
        let pf = self.project_file();
        let mut blobs: Vec<Option<BlobSource>> = vec![];
        let mut missing: Vec<String> = vec![];
        for s in &self.samples {
            let b = if !self.embed_raw {
                None
            } else {
                match &s.source {
                    RawSource::Disk(p) if p.is_file() => Some(BlobSource::File(p.clone())),
                    RawSource::Embedded {
                        project,
                        offset,
                        len,
                    } if project.is_file() => Some(BlobSource::Range {
                        path: project.clone(),
                        offset: *offset,
                        len: *len,
                    }),
                    _ => {
                        missing.push(s.data.name.clone());
                        None
                    }
                }
            };
            blobs.push(b);
        }
        let src_paths: Vec<String> = self.samples.iter().map(|s| s.src_path.clone()).collect();
        self.saving += 1;
        self.status = format!("Saving project {} ...", path.display());
        let (tx, ctx) = (self.tx.clone(), ctx.clone());
        let tmp_dir = paths::temp_dir();
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                project::write_container(&path, &pf, &blobs, &tmp_dir)
            }))
            .unwrap_or_else(|_| Err("internal error while saving the project".to_string()));
            let _ = tx.send(Msg::ProjectSaved {
                path,
                result,
                src_paths,
                missing,
            });
            ctx.request_repaint();
        });
    }

    /// Opens a project chosen in a file dialog.
    pub(crate) fn open_project_dialog(&mut self, ctx: &egui::Context) {
        if let Some(p) = self
            .dialog()
            .set_title("Open project")
            .add_filter("msQuant project", &["quant"])
            .pick_file()
        {
            self.open_project(ctx, &p);
        }
    }

    /// Opens a project: restores targets and settings, then re-imports the files in background threads.
    pub(crate) fn open_project(&mut self, ctx: &egui::Context, path: &Path) {
        let (pf, offs) = match project::read_container(path) {
            Ok(x) => x,
            Err(e) => {
                self.status = format!("Project error: {}", e);
                return;
            }
        };
        self.push_undo("open project");
        // The trail of the project takes over; what happened before stays in the old trail.
        self.audit
            .log("PROJECT_OPENING", &path.display().to_string());
        self.audit
            .open_trail(&paths::project_audit_file(path).to_string_lossy());
        self.audit.log(
            "PROJECT_OPENED",
            &format!(
                "{} text_sha256={} saved_by={} embedded_raw_files={}",
                path.display(),
                crate::support::sha256::sha256_hex(pf.to_text().as_bytes()),
                pf.user,
                offs.iter().filter(|o| o.is_some()).count()
            ),
        );
        self.samples.clear();
        self.analytes = pf.analytes.clone();
        self.fits = vec![None; self.analytes.len()];
        self.apply_params(&pf.params);
        self.sel_s = 0;
        self.sel_a = 0;
        self.anchor = 0;
        self.rows_dirty = true;
        self.project_path = Some(path.to_path_buf());
        if pf.samples.is_empty() {
            self.status = format!("Opened project {}", path.display());
            return;
        }
        self.pending = Some(Pending {
            recs: pf.samples.clone(),
            mismatches: vec![],
        });
        let mut started = 0;
        for (rec, off) in pf.samples.iter().zip(offs.iter()) {
            let ok = match off {
                Some((o, l)) => {
                    self.start_load_embedded(ctx, rec.path.clone(), path.to_path_buf(), *o, *l)
                }
                None => self.start_load_any(ctx, &rec.path),
            };
            if ok {
                started += 1;
            }
        }
        if started == 0 {
            self.pending = None;
        } else {
            self.status = format!("Opening project: re-importing {} raw file(s)...", started);
        }
    }

    /// Completes opening a project after all file loads have finished: restores the file order and reports problems.
    pub(crate) fn finish_project(&mut self, p: Pending) {
        let recs = &p.recs;
        self.samples.sort_by_key(|s| {
            recs.iter()
                .position(|r| r.path == s.src_path)
                .unwrap_or(usize::MAX)
        });
        let n = self.samples.len();
        self.fix_selection();
        for a in 0..self.analytes.len() {
            self.refit(a);
        }
        if self.align_ref >= n {
            self.align_ref = 0;
        }
        let mut msg = format!("Project loaded: {} of {} raw files", n, recs.len());
        if !p.mismatches.is_empty() {
            msg += &format!(
                "  ⚠ SHA-256 differs from the saved project for: {}",
                p.mismatches.join(", ")
            );
            self.audit
                .log("PROJECT_HASH_MISMATCH", &p.mismatches.join(", "));
        }
        if n < recs.len() {
            msg += "  ⚠ some files could not be re-imported (moved or converter missing)";
        }
        self.status = msg;
    }

    /// Clears the workspace; the step can be undone.
    pub(crate) fn new_project(&mut self) {
        self.push_undo("new project");
        self.audit.log("PROJECT_NEW", "");
        self.audit
            .open_trail(&paths::session_audit_file().to_string_lossy());
        self.samples.clear();
        self.analytes.clear();
        self.fits.clear();
        self.sel_s = 0;
        self.sel_a = 0;
        self.anchor = 0;
        self.project_path = None;
        self.table_path = None;
        self.pending = None;
        self.rows_dirty = true;
        self.status = "New project".into();
    }

    // ---------------- preferences ----------------
    /// Makes the audit trail of a freshly saved project the active one.
    ///
    /// The entries of the session (or of the project this one was saved from) are carried over;
    /// the file of a finished session trail is deleted afterwards, so that no entry exists twice.
    pub(crate) fn adopt_project_trail(&mut self, project: &Path) {
        let trail = paths::project_audit_file(project)
            .to_string_lossy()
            .to_string();
        if self.audit.path == trail {
            return;
        }
        let was_session =
            Path::new(&self.audit.path).parent() == Some(paths::audit_dir_path().as_path());
        if let Err(e) = self.audit.move_to(&trail, was_session) {
            self.status = format!("The audit trail of the project could not be written: {}", e);
        }
    }

    /// Opens the "New project" dialog, filled with the settings of the last session (or the
    /// defaults on the very first start). `welcome` is true on the first start, which also asks
    /// for the location of ThermoRawFileParser.
    pub fn open_new_project_form(&mut self, welcome: bool) {
        let folder = self
            .data_dir
            .clone()
            .unwrap_or_else(paths::default_dir_path);
        let trfp = if self.trfp_path.trim().is_empty() {
            rawconv::find_trfp("")
                .map(|p| p.display().to_string())
                .unwrap_or_default()
        } else {
            self.trfp_path.clone()
        };
        self.new_project_form = Some(NewProjectForm {
            tol: self.settings.tol,
            tol_ppm: self.settings.tol_ppm,
            method: self.settings.peak_method,
            folder: folder.display().to_string(),
            user: self.audit.user.clone(),
            embed: self.embed_raw,
            trfp,
            ask_trfp: welcome,
            welcome,
        });
    }

    /// Asks for the parameters of a new project (the menu item and Ctrl+N).
    pub(crate) fn request_new_project(&mut self) {
        self.open_new_project_form(false);
    }

    /// Applies the answers of the dialog and, unless this is the first start, clears the project.
    pub fn apply_new_project_form(&mut self, form: &NewProjectForm) {
        self.settings.tol = form.tol.clamp(0.001, 1000.0);
        self.settings.tol_ppm = form.tol_ppm;
        self.settings.peak_method = form.method;
        let folder = form.folder.trim();
        self.data_dir = if folder.is_empty() || PathBuf::from(folder) == paths::default_dir_path() {
            None
        } else {
            Some(PathBuf::from(folder))
        };
        let user = form.user.trim();
        if !user.is_empty() && user != self.audit.user {
            let change = format!("{} -> {}", self.user_logged, user);
            self.audit.user = user.to_string();
            self.audit.log("USER_CHANGED", &change);
            self.user_logged = user.to_string();
        }
        self.embed_raw = form.embed;
        if form.ask_trfp {
            self.trfp_path = form.trfp.trim().to_string();
        }
        self.prefs_dirty = true;
        if form.welcome {
            self.status =
                "Ready - drop a target table and your .raw / .mzML files onto the window".into();
        } else {
            self.new_project();
        }
    }
}
