//! State of the application: the data types, the application struct and its constants.

use crate::analysis::calibration::Fit;
use crate::app::layout::Layout;
use crate::domain::settings::AnalysisSettings;
use crate::domain::types::{Boundary, Kind, PeakMethod, Theme};
use crate::domain::{Analyte, MzData};
use crate::formats::project::{EmbeddedBlobs, RawSource, SampleRec};
use crate::support::audit::Audit;
use crate::support::history::History;
use eframe::egui;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

/// Version shown in the Help menu and written to result exports.
pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Labels of the regression weightings, indexed like `App::weighting`.
pub(crate) const WEIGHTS: [&str; 3] = ["none", "1/x", "1/x²"];

/// Text of the README, embedded so that it can be shown offline from the Help menu.
pub(crate) const README: &str = include_str!("../../README.md");

/// Number of undo (and of redo) steps that are kept.
pub(crate) const MAX_UNDO: usize = 10;

/// What the user chose in the right-click menu of a plot.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ExportKind {
    Png,
    Svg,
    Csv(usize),
    Reset,
}

/// The answers in the "New project" dialog.
#[derive(Clone)]
pub struct NewProjectForm {
    /// Half-width of the m/z window.
    pub tol: f64,
    /// Whether the m/z window is in ppm (otherwise Da).
    pub tol_ppm: bool,
    /// How peak boundaries are found.
    pub method: PeakMethod,
    /// Folder offered for saving and loading files.
    pub folder: String,
    /// Name written into the audit trail.
    pub user: String,
    /// Whether projects embed the raw files.
    pub embed: bool,
    /// Location of ThermoRawFileParser.
    pub trfp: String,
    /// Whether the location of ThermoRawFileParser is asked (first start only).
    pub ask_trfp: bool,
    /// Whether this is the welcome dialog of the first start (there is no project to replace).
    pub welcome: bool,
}

/// A pending PNG capture of one plot (taken from the window screenshot after the context menu has closed).
pub(crate) struct Capture {
    pub(crate) rect: egui::Rect,
    pub(crate) path: PathBuf,
    pub(crate) frames: u8,
    pub(crate) sent: bool,
}

/// Provenance of a freshly imported file, sent from the loading thread to the UI thread.
pub struct LoadInfo {
    /// Path the file was read from (the original path for a file taken from a project).
    pub path: String,
    /// SHA-256 of the raw file.
    pub raw_sha: String,
    /// Description of the conversion, for the audit trail.
    pub detail: String,
    /// Where the raw bytes can be found again when the project is saved.
    pub source: RawSource,
}

/// Messages from background threads to the UI thread.
pub(crate) enum Msg {
    Loaded(Box<MzData>, LoadInfo),
    Err(String),
    Finished,
    ProjectSaved {
        path: PathBuf,
        result: Result<EmbeddedBlobs, String>,
        src_paths: Vec<String>,
        missing: Vec<String>,
    },
}

/// Sends [`Msg::Finished`] when dropped, also if the background thread panics. Without it a
/// failed job would leave the progress indicator spinning forever.
pub(crate) struct FinishGuard {
    pub(crate) tx: Sender<Msg>,
    pub(crate) ctx: egui::Context,
}

impl FinishGuard {
    pub(crate) fn new(tx: Sender<Msg>, ctx: egui::Context) -> Self {
        Self { tx, ctx }
    }
}

impl Drop for FinishGuard {
    fn drop(&mut self) {
        let _ = self.tx.send(Msg::Finished);
        self.ctx.request_repaint();
    }
}

/// Whether `path` has the file extension `ext` (compared without regard to case).
pub(crate) fn has_extension(path: &Path, ext: &str) -> bool {
    path.extension()
        .map(|e| e.eq_ignore_ascii_case(ext))
        .unwrap_or(false)
}

/// Text of the credits window.
pub(crate) const CREDITS: &str = "Thermo .raw files are read through ThermoRawFileParser (CompOmics, Apache-2.0), run as an external program; nothing is bundled or linked: https://github.com/compomics/ThermoRawFileParser\n\
Hulstaert N, Shofstahl J, Sachsenberg T, Walzer M, Barsnes H, Martens L, Perez-Riverol Y. J Proteome Res 2020;19:537-542 (doi:10.1021/acs.jproteome.9b00328).\n\
It uses Thermo Fisher Scientific's RawFileReader library (Copyright Thermo Fisher Scientific, Inc.), subject to Thermo's license.\n\
The external-converter approach follows mzmine: https://github.com/mzmine/mzmine (MIT).\n\
GUI: egui / eframe / egui_plot (MIT or Apache-2.0), https://github.com/emilk/egui; file dialogs: rfd (MIT).\n\
Project: https://github.com/markellerhorst/KvantX\n\
Built with Claude Sonnet 5.5 (Medium); app icon generated with Gemini.";

/// Which chromatogram the chromatogram tab shows.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum View {
    Xic,
    Tic,
    Bpc,
    Quick,
}

/// Which traces the target view of the chromatogram tab shows.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Scope {
    Ions,
    AllTargets,
    Files,
}

/// Tabs of the central panel.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Tab {
    /// Files, chromatogram, compound information and spectrum.
    Analyze,
    /// Calibration files and the target table.
    Calibration,
    /// The results of all files.
    Results,
}

/// Integration state of one target in one file (one cell of the results matrix).
#[derive(Clone)]
pub struct Cell {
    /// Start of the integration window in minutes.
    pub start: f64,
    /// End of the integration window in minutes.
    pub end: f64,
    /// Whether a peak was found or defined.
    pub found: bool,
    /// Whether the boundaries were set by hand.
    pub manual: bool,
    /// Area of every trace of the target (intensity times seconds); the first is the quantifier.
    pub areas: Vec<f64>,
    /// Boundaries as last written to the audit trail.
    pub logged: (f64, f64),
    /// Whether this file is used as a calibration point (if it is a standard).
    pub include: bool,
    /// Standard concentration of this target in this file; 0 if the file is no standard for it.
    pub conc: f64,
}

impl Cell {
    /// Creates an empty cell: no peak, no standard concentration, included in calibrations.
    pub fn new() -> Cell {
        Cell {
            start: 0.0,
            end: 0.0,
            found: false,
            manual: false,
            areas: vec![],
            logged: (0.0, 0.0),
            include: true,
            conc: 0.0,
        }
    }
}

impl Default for Cell {
    fn default() -> Self {
        Self::new()
    }
}

/// An imported file together with its analysis state.
pub struct Sample {
    /// The data of the file; shared with the undo history.
    pub data: Arc<MzData>,
    /// Path of the file as it was imported.
    pub src_path: String,
    /// SHA-256 of the raw file.
    pub raw_sha: String,
    /// Where the raw bytes can be found again.
    pub source: RawSource,
    /// Role of the file.
    pub kind: Kind,
    /// Extracted chromatograms: target, then trace, then points `[time, intensity]`.
    pub xics: Vec<Vec<Vec<[f64; 2]>>>,
    /// Integration state of every target.
    pub cells: Vec<Cell>,
    /// Whether the file is selected.
    pub show: bool,
    /// Retention-time shift in minutes.
    pub rt_shift: f64,
    /// Value shown in the shift box while it is being edited.
    pub shift_ui: f64,
}

/// Everything undo/redo needs. Raw data is shared (Arc), derived chromatograms are recomputed on restore.
#[derive(Clone)]
pub(crate) struct SampleState {
    pub(crate) data: Arc<MzData>,
    pub(crate) src_path: String,
    pub(crate) raw_sha: String,
    pub(crate) source: RawSource,
    pub(crate) kind: Kind,
    pub(crate) cells: Vec<Cell>,
    pub(crate) show: bool,
    pub(crate) rt_shift: f64,
}

/// Complete analysis state at one point in time, used for undo and redo.
///
/// The raw data is shared with the live state through [`Arc`] instead of being copied: a snapshot
/// must keep the data of a removed file alive without duplicating hundreds of megabytes. The
/// extracted chromatograms are not part of a snapshot, they are recomputed on restore.
#[derive(Clone)]
pub(crate) struct Snapshot {
    pub(crate) label: String,
    pub(crate) analytes: Vec<Analyte>,
    pub(crate) samples: Vec<SampleState>,
    pub(crate) sel_s: usize,
    pub(crate) sel_a: usize,
    pub(crate) params: Vec<(String, String)>,
}

/// Bookkeeping while a project is opened and its files are re-imported in the background.
pub(crate) struct Pending {
    pub(crate) recs: Vec<SampleRec>,
    pub(crate) mismatches: Vec<String>,
}

/// Application state and user interface of KvantX.
pub struct App {
    /// The analysis parameters (saved with projects and as the settings of the last session).
    pub settings: AnalysisSettings,
    /// Public for the integration tests, which drive the application without a window.
    pub analytes: Vec<Analyte>,
    /// Public for the integration tests, which drive the application without a window.
    pub samples: Vec<Sample>,
    /// Public for the integration tests, which drive the application without a window.
    pub fits: Vec<Option<Fit>>,
    /// Index of the current file.
    pub sel_s: usize,
    pub(crate) sel_a: usize,
    pub(crate) anchor: usize,
    /// Public for the integration tests, which drive the application without a window.
    pub reason: String,
    pub(crate) settings_dirty: bool,
    pub(crate) out_path: String,
    pub(crate) data_dir: Option<PathBuf>,
    /// Public for the integration tests, which drive the application without a window.
    pub status: String,
    pub(crate) show_credits: bool,
    pub(crate) show_readme: bool,
    pub(crate) show_about: bool,
    pub(crate) grab: Option<Boundary>,
    pub(crate) editing: bool,
    /// Public for the integration tests, which drive the application without a window.
    pub audit: Audit,
    pub(crate) user_logged: String,
    pub(crate) tx: Sender<Msg>,
    pub(crate) rx: Receiver<Msg>,
    pub(crate) loading: usize,
    /// Public for the integration tests, which drive the application without a window.
    pub trfp_path: String,
    pub(crate) view: View,
    pub(crate) scope: Scope,
    pub(crate) tab: Tab,
    pub(crate) tic_level: u8,
    pub(crate) normalize: bool,
    pub(crate) matrix_conc: bool,
    pub(crate) align_ref: usize,
    pub(crate) history: History<Snapshot>,
    pub(crate) last_undo_label: String,
    /// Public for the integration tests, which drive the application without a window.
    pub rows: Vec<[String; 4]>,
    pub(crate) rows_dirty: bool,
    pub(crate) table_path: Option<PathBuf>,
    pub(crate) project_path: Option<PathBuf>,
    pub(crate) pending: Option<Pending>,
    pub(crate) title: String,
    pub(crate) spec_autofit: bool,
    /// Public for the integration tests, which drive the application without a window.
    pub follow_hover: bool,
    pub(crate) pinned_scan: Option<(usize, usize)>,
    /// Public for the integration tests, which drive the application without a window.
    pub hover: Option<(usize, f64)>,
    pub(crate) col_w: [f32; 4],
    pub(crate) embed_raw: bool,
    pub(crate) saving: usize,
    pub(crate) capture: Option<Capture>,
    /// Which panels and windows are open and how large they are.
    pub layout: Layout,
    /// Changes whenever a layout is applied, so that egui forgets the sizes it remembers.
    pub(crate) layout_gen: u32,
    /// The layout as last written to the default folder.
    pub(crate) saved_layout: Layout,
    /// The settings as last written to the default folder.
    pub(crate) saved_settings: AnalysisSettings,
    /// Color theme of the interface.
    pub(crate) theme: Theme,
    /// The theme as last written to the default folder.
    pub(crate) saved_theme: Theme,
    /// The theme that was last handed to egui.
    pub(crate) applied_theme: Option<Theme>,
    /// Whether the minimalist style was applied to the dark (`true`) or light style.
    pub(crate) styled_dark: Option<bool>,
    /// Height in points of the panel that is being drawn, so that plots can fill it.
    pub(crate) panel_height: f32,
    /// The "New project" dialog while it is open.
    pub new_project_form: Option<NewProjectForm>,
    /// Whether the user settings were changed outside the settings value (converter, folder, user).
    pub(crate) prefs_dirty: bool,
    pub(crate) show_prefs: bool,
    pub(crate) reset_cal: bool,
    pub(crate) zoom_chrom: Option<[f64; 2]>,
    pub(crate) zoom_spec: Option<[f64; 2]>,
    pub(crate) reset_chrom: bool,
    pub(crate) reset_spec: bool,
    pub(crate) spec_last: Option<(usize, usize)>,
    pub(crate) assign_target: usize,
    pub(crate) assign_conc: f64,
    /// Public for the integration tests, which drive the application without a window.
    pub quick_mz: f64,
    /// Public for the integration tests, which drive the application without a window.
    pub quick_level: u8,
}
