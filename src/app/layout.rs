//! The layout of the main window: which panels and windows are open and how large they are.
//!
//! The layout of the last session is stored in the default folder and restored at the next start.
//! It can also be saved to, and loaded from, a file of the user's choice, and reset to the
//! standard layout.

use crate::app::state::App;
use crate::support::paths;
use std::path::Path;

/// Position and size of a floating window.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowState {
    /// Whether the window is open.
    pub open: bool,
    /// Left edge in points; 0 together with a width of 0 means "use the default placement".
    pub x: f32,
    /// Top edge in points.
    pub y: f32,
    /// Width in points (0 = default).
    pub w: f32,
    /// Height in points (0 = default).
    pub h: f32,
}

impl WindowState {
    /// A closed window with the default placement.
    #[must_use]
    pub fn closed() -> Self {
        Self {
            open: false,
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
        }
    }
}

/// Which panels are open and how the main window is divided.
///
/// Sizes are fractions of the window (not pixels), so that the panes keep their proportions when
/// the window is resized.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// The panel with the imported files.
    pub files: bool,
    /// The panel with the results and the calibration of the selected target.
    pub compound: bool,
    /// The chromatogram panel.
    pub chromatogram: bool,
    /// The MS2 spectrum panel.
    pub spectrum: bool,
    /// Share of the window width taken by the left column of the analysis workspace.
    pub left_frac: f32,
    /// Share of the height of the left column taken by the file panel.
    pub left_top_frac: f32,
    /// Share of the height of the right column taken by the chromatogram.
    pub right_top_frac: f32,
    /// Share of the width taken by the left half of the calibration workspace.
    pub cal_left_frac: f32,
    /// Share of the height of that left half taken by the target table.
    pub cal_left_top_frac: f32,
    /// The window with the analysis settings.
    pub settings: WindowState,
    /// The window with the audit trail.
    pub audit: WindowState,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            files: true,
            compound: true,
            chromatogram: true,
            spectrum: true,
            left_frac: 0.30,
            left_top_frac: 0.45,
            right_top_frac: 0.62,
            cal_left_frac: 0.50,
            cal_left_top_frac: 0.50,
            settings: WindowState::closed(),
            audit: WindowState::closed(),
        }
    }
}

impl Layout {
    /// The layout as text, one `key=value` per line.
    #[must_use]
    pub fn to_text(&self) -> String {
        let window = |name: &str, w: &WindowState| {
            format!(
                "{n}={}\n{n}_x={}\n{n}_y={}\n{n}_w={}\n{n}_h={}\n",
                w.open,
                w.x,
                w.y,
                w.w,
                w.h,
                n = name
            )
        };
        format!(
            "files={}\ncompound={}\nchromatogram={}\nspectrum={}\nleft_frac={}\nleft_top_frac={}\nright_top_frac={}\ncal_left_frac={}\ncal_left_top_frac={}\n{}{}",
            self.files,
            self.compound,
            self.chromatogram,
            self.spectrum,
            self.left_frac,
            self.left_top_frac,
            self.right_top_frac,
            self.cal_left_frac,
            self.cal_left_top_frac,
            window("settings", &self.settings),
            window("audit", &self.audit),
        )
    }

    /// Parses text written by [`Layout::to_text`]. Unknown keys are ignored and fractions are kept
    /// between 0.1 and 0.9: the text may come from a file that was edited by hand.
    #[must_use]
    pub fn from_text(text: &str) -> Self {
        let mut l = Self::default();
        let number = |v: &str, lo: f32, hi: f32| {
            v.parse::<f32>()
                .ok()
                .filter(|x| x.is_finite())
                .map(|x| x.clamp(lo, hi))
        };
        for (k, v) in text.lines().filter_map(|line| line.split_once('=')) {
            let (k, v) = (k.trim(), v.trim());
            let flag = v.parse::<bool>().ok();
            match k {
                "files" => l.files = flag.unwrap_or(l.files),
                "compound" => l.compound = flag.unwrap_or(l.compound),
                "chromatogram" => l.chromatogram = flag.unwrap_or(l.chromatogram),
                "spectrum" => l.spectrum = flag.unwrap_or(l.spectrum),
                "left_frac" => l.left_frac = number(v, 0.1, 0.9).unwrap_or(l.left_frac),
                "left_top_frac" => l.left_top_frac = number(v, 0.1, 0.9).unwrap_or(l.left_top_frac),
                "right_top_frac" => {
                    l.right_top_frac = number(v, 0.1, 0.9).unwrap_or(l.right_top_frac)
                }
                "cal_left_frac" => l.cal_left_frac = number(v, 0.1, 0.9).unwrap_or(l.cal_left_frac),
                "cal_left_top_frac" => {
                    l.cal_left_top_frac = number(v, 0.1, 0.9).unwrap_or(l.cal_left_top_frac)
                }
                _ => {
                    for (name, w) in [("settings", &mut l.settings), ("audit", &mut l.audit)] {
                        let Some(field) = k.strip_prefix(name) else {
                            continue;
                        };
                        match field {
                            "" => w.open = flag.unwrap_or(w.open),
                            "_x" => w.x = number(v, -10000.0, 10000.0).unwrap_or(w.x),
                            "_y" => w.y = number(v, -10000.0, 10000.0).unwrap_or(w.y),
                            "_w" => w.w = number(v, 0.0, 10000.0).unwrap_or(w.w),
                            "_h" => w.h = number(v, 0.0, 10000.0).unwrap_or(w.h),
                            _ => {}
                        }
                    }
                }
            }
        }
        l
    }
}

impl App {
    /// Restores the layout of the last session, if there is one.
    pub(crate) fn load_last_layout(&mut self) {
        if let Ok(text) = std::fs::read_to_string(paths::layout_file()) {
            self.layout = Layout::from_text(&text);
            self.saved_layout = self.layout.clone();
            self.layout_gen += 1;
        }
    }

    /// Stores the current layout as the layout of the last session.
    pub(crate) fn write_layout_file(&self) {
        paths::ensure_default_dir();
        let _ = std::fs::write(paths::layout_file(), self.layout.to_text());
    }

    /// Saves the layout to a file chosen by the user.
    pub(crate) fn save_layout_dialog(&mut self) {
        let Some(path) = self
            .dialog()
            .set_title("Save the window layout")
            .add_filter("KvantX layout", &["layout"])
            .set_file_name("KvantX.layout")
            .save_file()
        else {
            return;
        };
        self.status = match std::fs::write(&path, self.layout.to_text()) {
            Ok(()) => format!("Saved the layout to {}", path.display()),
            Err(e) => format!("Saving the layout failed: {}", e),
        };
    }

    /// Loads a layout from a file chosen by the user.
    pub(crate) fn load_layout_dialog(&mut self) {
        let Some(path) = self
            .dialog()
            .set_title("Load a window layout")
            .add_filter("KvantX layout", &["layout"])
            .pick_file()
        else {
            return;
        };
        self.load_layout_from(&path);
    }

    /// Applies the layout in `path`.
    pub fn load_layout_from(&mut self, path: &Path) {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                self.layout = Layout::from_text(&text);
                self.layout_gen += 1;
                self.status = format!("Loaded the layout {}", path.display());
            }
            Err(e) => self.status = format!("{}: {}", path.display(), e),
        }
    }

    /// Restores the standard layout.
    pub fn reset_layout(&mut self) {
        self.layout = Layout::default();
        self.layout_gen += 1;
        self.status = "Layout reset to the standard layout".into();
    }
}
