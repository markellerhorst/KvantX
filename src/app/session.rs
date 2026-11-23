//! Session handling: construction, settings files, folders, selection and undo.

use crate::app::layout::Layout;
use crate::app::state::{App, MAX_UNDO, Sample, SampleState, Scope, Snapshot, Tab, View};
use crate::domain::settings::AnalysisSettings;
use crate::domain::types::Kind;
use crate::domain::types::Theme;
use crate::support::audit::Audit;
use crate::support::history::History;
use crate::support::paths;
use std::path::PathBuf;
use std::sync::mpsc;

impl Default for App {
    /// Same as [`App::new`].
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    /// Starts msQuant.
    ///
    /// Creates the default folder, removes temporary files of an interrupted run and opens the audit
    /// trail in the default folder.
    pub fn new() -> App {
        paths::ensure_default_dir();
        paths::clean_temp();
        let first_run = !paths::settings_file().exists();
        let mut app = App::new_with_audit(&paths::session_audit_file().to_string_lossy());
        app.load_settings();
        app.load_last_layout();
        app.saved_settings = app.settings.clone();
        app.saved_theme = app.theme;
        if first_run {
            app.open_new_project_form(true);
        }
        app
    }

    /// Creates the application with the audit trail in `audit_path` (tests use a private file).
    pub fn new_with_audit(audit_path: &str) -> App {
        let (tx, rx) = mpsc::channel();
        let app = App {
            settings: AnalysisSettings::default(),
            analytes: vec![], samples: vec![], fits: vec![], sel_s: 0, sel_a: 0, anchor: 0,
            reason: String::new(), settings_dirty: false,
            out_path: String::new(), data_dir: None,
            status: "Drop a .csv target table (optional) and Thermo .raw / mzML files onto the window, or use the File menu.".into(),
            show_credits: false, show_readme: false,
            grab: None, editing: false, audit: Audit::new(audit_path), user_logged: "N/A".into(), tx, rx, loading: 0,
            trfp_path: String::new(),
            view: View::Xic, scope: Scope::Ions, tab: Tab::Analyze, tic_level: 1, normalize: false, matrix_conc: true,
            align_ref: 0,
            history: History::new(MAX_UNDO), last_undo_label: String::new(),
            rows: vec![], rows_dirty: true, table_path: None, project_path: None, pending: None,
            title: String::new(),
            spec_autofit: true, follow_hover: false, pinned_scan: None, hover: None,
            col_w: [0.0; 4], embed_raw: true, saving: 0, capture: None,
            layout: Layout::default(), layout_gen: 0, saved_layout: Layout::default(), saved_settings: AnalysisSettings::default(), theme: Theme::System, saved_theme: Theme::System, applied_theme: None, styled_dark: None,
            panel_height: 400.0, new_project_form: None, prefs_dirty: false,
            show_prefs: false, reset_cal: false, zoom_chrom: None, zoom_spec: None, reset_chrom: false, reset_spec: false, spec_last: None, assign_target: 0, assign_conc: 1.0, quick_mz: 100.0, quick_level: 1,
        };
        app
    }

    /// Reads the user settings from the default folder, if there are any.
    pub(crate) fn load_settings(&mut self) {
        let Ok(text) = std::fs::read_to_string(paths::settings_file()) else {
            return;
        };
        let pairs: Vec<(String, String)> = text
            .lines()
            .filter_map(|l| l.split_once('='))
            .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
            .collect();
        self.settings.apply_pairs(&pairs);
        for (key, value) in &pairs {
            match key.as_str() {
                "trfp" => self.trfp_path = value.clone(),
                "user" => {
                    if !value.is_empty() {
                        self.audit.user = value.clone();
                        self.user_logged = value.clone();
                    }
                }
                "embed" => self.embed_raw = value != "false",
                "theme" => self.theme = Theme::from_key(value).unwrap_or(Theme::System),
                "data_dir" => {
                    self.data_dir = if value.is_empty() {
                        None
                    } else {
                        Some(PathBuf::from(value))
                    };
                }
                _ => {}
            }
        }
    }

    /// Writes the user settings to the default folder.
    pub(crate) fn write_settings(&self) {
        paths::ensure_default_dir();
        let text = format!(
            "trfp={}\nuser={}\nembed={}\ndata_dir={}\ntheme={}\n{}",
            self.trfp_path.trim(),
            self.audit.user.trim(),
            self.embed_raw,
            self.data_dir
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
            self.theme.key(),
            self.settings.to_text(),
        );
        let _ = std::fs::write(paths::settings_file(), text);
    }

    /// Stores the location of the converter in the settings and in the audit trail.
    pub(crate) fn save_trfp(&mut self) {
        self.write_settings();
        self.audit.log(
            "CONVERTER_PATHS",
            &format!("trfp={}", self.trfp_path.trim()),
        );
    }

    /// Standard folder for projects, results and figures, without creating it.
    pub(crate) fn standard_dir_path(&self) -> PathBuf {
        match &self.data_dir {
            Some(dir) => dir.clone(),
            None => paths::default_dir_path(),
        }
    }

    /// Standard folder for projects, results and figures, created if necessary.
    pub(crate) fn standard_dir(&self) -> PathBuf {
        let dir = self.standard_dir_path();
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    /// A file dialog that starts in the standard folder.
    pub(crate) fn dialog(&self) -> rfd::FileDialog {
        rfd::FileDialog::new().set_directory(self.standard_dir())
    }

    /// File the analysis results are exported to: the chosen path, or `results.csv` in the standard folder.
    pub(crate) fn export_path(&self) -> PathBuf {
        if self.out_path.trim().is_empty() {
            self.standard_dir().join("results.csv")
        } else {
            PathBuf::from(self.out_path.trim())
        }
    }

    /// Converts the m/z tolerance setting to Da for the given m/z.
    pub(crate) fn tol_da(&self, mz: f64) -> f64 {
        if self.settings.tol_ppm {
            mz * self.settings.tol * 1e-6
        } else {
            self.settings.tol
        }
    }

    // ---------------- undo / redo ----------------
    /// Analysis parameters as key/value pairs, for undo snapshots and project files.
    pub(crate) fn params_vec(&self) -> Vec<(String, String)> {
        let mut pairs = self.settings.to_pairs();
        pairs.push(("align_ref".into(), self.align_ref.to_string()));
        pairs
    }

    /// Applies key/value pairs written by [`App::params_vec`]; unknown keys are ignored.
    pub fn apply_params(&mut self, kv: &[(String, String)]) {
        self.settings.apply_pairs(kv);
        if let Some((_, v)) = kv.iter().find(|(k, _)| k == "align_ref") {
            if let Ok(x) = v.parse() {
                self.align_ref = x;
            }
        }
    }

    /// Captures the analysis state for undo and redo.
    pub(crate) fn snapshot(&self, label: &str) -> Snapshot {
        Snapshot {
            label: label.to_string(),
            analytes: self.analytes.clone(),
            samples: self
                .samples
                .iter()
                .map(|s| SampleState {
                    data: s.data.clone(),
                    src_path: s.src_path.clone(),
                    raw_sha: s.raw_sha.clone(),
                    source: s.source.clone(),
                    kind: s.kind,
                    cells: s.cells.clone(),
                    show: s.show,
                    rt_shift: s.rt_shift,
                })
                .collect(),
            sel_s: self.sel_s,
            sel_a: self.sel_a,
            params: self.params_vec(),
        }
    }

    /// Replaces the analysis state by a snapshot and recomputes all chromatograms.
    pub(crate) fn restore(&mut self, s: Snapshot) {
        self.analytes = s.analytes;
        self.apply_params(&s.params);
        self.samples = s
            .samples
            .into_iter()
            .map(|st| Sample {
                data: st.data,
                src_path: st.src_path,
                raw_sha: st.raw_sha,
                source: st.source,
                kind: st.kind,
                xics: vec![],
                cells: st.cells,
                show: st.show,
                rt_shift: st.rt_shift,
                shift_ui: st.rt_shift,
            })
            .collect();
        self.sel_s = s.sel_s.min(self.samples.len().saturating_sub(1));
        self.sel_a = s.sel_a.min(self.analytes.len().saturating_sub(1));
        self.anchor = self.sel_s;
        self.rows_dirty = true;
        self.editing = false;
        self.pinned_scan = None;
        self.recompute_all();
        self.fix_selection();
    }

    /// Records the current state as an undo step called `label`; call this before changing the state.
    pub(crate) fn push_undo(&mut self, label: &str) {
        let s = self.snapshot(label);
        self.history.record(s);
        self.last_undo_label = label.to_string();
    }

    /// Reverts the last change, if there is one.
    pub fn undo(&mut self) {
        let label = match self.history.peek_undo() {
            Some(s) => s.label.clone(),
            None => return,
        };
        let cur = self.snapshot(&label);
        if let Some(prev) = self.history.undo(cur) {
            self.restore(prev);
            self.status = format!("Undid: {}", label);
            self.audit.log("UNDO", &label);
        }
    }

    /// Re-applies the last undone change, if there is one.
    pub fn redo(&mut self) {
        let label = match self.history.peek_redo() {
            Some(s) => s.label.clone(),
            None => return,
        };
        let cur = self.snapshot(&label);
        if let Some(next) = self.history.redo(cur) {
            self.restore(next);
            self.status = format!("Redid: {}", label);
            self.audit.log("REDO", &label);
        }
    }

    // ---------------- selection ----------------
    /// Index of the current file; `None` when no file is selected.
    pub fn cur(&self) -> Option<usize> {
        if self.sel_s < self.samples.len() && self.samples[self.sel_s].show {
            Some(self.sel_s)
        } else {
            None
        }
    }

    /// Keeps the current-file index and the selection anchor valid after files were added, removed or deselected.
    pub fn fix_selection(&mut self) {
        let n = self.samples.len();
        if n == 0 {
            self.sel_s = 0;
            self.anchor = 0;
            return;
        }
        if self.sel_s >= n {
            self.sel_s = n - 1;
        }
        if self.anchor >= n {
            self.anchor = n - 1;
        }
        if !self.samples[self.sel_s].show {
            if let Some(k) = (0..n).rev().find(|&k| self.samples[k].show) {
                self.sel_s = k;
            }
        }
    }

    /// Indices of all selected files.
    pub fn shown(&self) -> Vec<usize> {
        (0..self.samples.len())
            .filter(|&i| self.samples[i].show)
            .collect()
    }

    /// Explorer-style: click = only this, Ctrl+click = toggle, Shift+click = range from anchor (Ctrl+Shift adds the range).
    pub fn apply_click(&mut self, i: usize, ctrl: bool, shift: bool) {
        if i >= self.samples.len() {
            return;
        }
        if shift {
            let a = self.anchor.min(self.samples.len() - 1);
            let (lo, hi) = (a.min(i), a.max(i));
            if !ctrl {
                for s in self.samples.iter_mut() {
                    s.show = false;
                }
            }
            for k in lo..=hi {
                self.samples[k].show = true;
            }
            self.sel_s = i;
        } else if ctrl {
            self.samples[i].show = !self.samples[i].show;
            self.anchor = i;
            if self.samples[i].show {
                self.sel_s = i;
            }
        } else {
            for s in self.samples.iter_mut() {
                s.show = false;
            }
            self.samples[i].show = true;
            self.sel_s = i;
            self.anchor = i;
        }
        self.fix_selection();
    }

    /// Removes a file from the project; the file on disk is not touched.
    pub fn remove_sample(&mut self, i: usize) {
        if i >= self.samples.len() {
            return;
        }
        self.push_undo("remove file");
        let s = self.samples.remove(i);
        self.pinned_scan = None;
        self.audit.log(
            "DATA_REMOVED",
            &format!(
                "{} sha256={} (file on disk untouched)",
                s.data.name, s.data.sha256
            ),
        );
        if self.align_ref > i {
            self.align_ref -= 1;
        } else if self.align_ref == i {
            self.align_ref = 0;
        }
        self.fix_selection();
        for a in 0..self.analytes.len() {
            self.refit(a);
        }
        self.status = format!(
            "Removed {} from the project (file on disk untouched)",
            s.data.name
        );
    }

    /// Sets the sample type. If `file` is one of several selected files, ALL selected files get the type; otherwise only `file`.
    pub fn set_type(&mut self, file: usize, kind: Kind) {
        if file >= self.samples.len() {
            return;
        }
        let group: Vec<usize> = if self.samples[file].show && self.shown().len() > 1 {
            self.shown()
        } else {
            vec![file]
        };
        if group.iter().all(|&f| self.samples[f].kind == kind) {
            return;
        }
        self.push_undo("sample type");
        for &f in &group {
            self.samples[f].kind = kind;
        }
        for a in 0..self.analytes.len() {
            self.refit(a);
        }
        let names: Vec<String> = group
            .iter()
            .map(|&f| self.samples[f].data.name.clone())
            .collect();
        self.audit.log(
            "SAMPLE_TYPE",
            &format!("type={} files={}", kind.label(), names.join(",")),
        );
        if group.len() > 1 {
            self.status = format!("{} selected files set to {}", group.len(), kind.label());
        }
    }

    /// Numbers of undo and redo steps that are available.
    pub fn history_lens(&self) -> (usize, usize) {
        self.history.lens()
    }

    /// Sorts the selected files alphabetically (ignoring case) among the positions they occupy;
    /// files that are not selected stay where they are.
    pub fn sort_selected(&mut self) {
        let slots: Vec<usize> = (0..self.samples.len())
            .filter(|&i| self.samples[i].show)
            .collect();
        if slots.len() < 2 {
            return;
        }
        self.push_undo("sort files");
        let mut order = slots.clone();
        order.sort_by_key(|&i| self.samples[i].data.name.to_lowercase());
        // new_pos[old index] = index after sorting
        let mut new_pos: Vec<usize> = (0..self.samples.len()).collect();
        for (k, &old) in order.iter().enumerate() {
            new_pos[old] = slots[k];
        }
        let mut moved: Vec<Option<Sample>> = (0..self.samples.len()).map(|_| None).collect();
        for (old, sample) in self.samples.drain(..).enumerate() {
            moved[new_pos[old]] = Some(sample);
        }
        self.samples = moved.into_iter().flatten().collect();
        let last = self.samples.len() - 1;
        self.sel_s = new_pos[self.sel_s.min(last)];
        self.anchor = new_pos[self.anchor.min(last)];
        self.align_ref = new_pos[self.align_ref.min(last)];
        self.pinned_scan = None;
        self.spec_last = None;
        self.hover = None;
        self.audit
            .log("FILES_SORTED", &format!("{} files", slots.len()));
        self.status = format!("Sorted {} selected files alphabetically", slots.len());
    }
}
