//! The parameters of an analysis.
//!
//! They are kept in one value so that they can be saved with a project, stored as the settings
//! of the last session, exported to a settings file and reset to their defaults in one place.

use super::types::{Basis, Mode, PeakMethod};

/// Analysis parameters. The defaults are what msQuant starts with.
#[derive(Clone, Debug, PartialEq)]
pub struct AnalysisSettings {
    /// Whether the m/z tolerance is in ppm (otherwise in Da).
    pub tol_ppm: bool,
    /// Half-width of the m/z window of an extracted-ion chromatogram.
    pub tol: f64,
    /// Half-width in minutes of the window around the expected retention time.
    pub rt_win: f64,
    /// Calibration weighting: 0 none, 1 = 1/x, 2 = 1/x squared.
    pub weighting: u8,
    /// Whether the legacy method subtracts a straight baseline between the boundaries.
    pub baseline: bool,
    /// Tolerance of the ion ratio in percent.
    pub ratio_tol: f64,
    /// Unit of the concentrations.
    pub units: String,
    /// Calibration or areas only.
    pub mode: Mode,
    /// How peak boundaries are found.
    pub peak_method: PeakMethod,
    /// MS1 precursor or MS2 product ions.
    pub basis: Basis,
    /// Zoom factor of the user interface.
    pub zoom: f32,
    /// Whether retention times are aligned when a file is imported.
    pub align_on: bool,
    /// Largest alignment shift that is searched, in minutes.
    pub align_max: f64,
}

impl Default for AnalysisSettings {
    fn default() -> Self {
        Self {
            tol_ppm: true,
            tol: 5.0,
            rt_win: 0.3,
            weighting: 2,
            baseline: true,
            ratio_tol: 30.0,
            units: "ng/mL".into(),
            mode: Mode::Curve,
            peak_method: PeakMethod::Adaptive,
            basis: Basis::Ms2,
            zoom: 1.0,
            align_on: false,
            align_max: 1.0,
        }
    }
}

impl AnalysisSettings {
    /// The parameters as `key=value` pairs, for projects and settings files.
    #[must_use]
    pub fn to_pairs(&self) -> Vec<(String, String)> {
        let kv = |k: &str, v: String| (k.to_string(), v);
        vec![
            kv("tol_ppm", self.tol_ppm.to_string()),
            kv("tol", self.tol.to_string()),
            kv("rt_win", self.rt_win.to_string()),
            kv("weighting", self.weighting.to_string()),
            kv("baseline", self.baseline.to_string()),
            kv("ratio_tol", self.ratio_tol.to_string()),
            kv("units", self.units.clone()),
            kv(
                "mode",
                if self.mode == Mode::Curve {
                    "curve"
                } else {
                    "areas"
                }
                .into(),
            ),
            kv("peak_method", self.peak_method.key().into()),
            kv(
                "basis",
                if self.basis == Basis::Ms1 {
                    "ms1"
                } else {
                    "ms2"
                }
                .into(),
            ),
            kv("zoom", self.zoom.to_string()),
            kv("align_on", self.align_on.to_string()),
            kv("align_max", self.align_max.to_string()),
        ]
    }

    /// Applies `key=value` pairs. Unknown keys are ignored and values outside their sensible
    /// range are clamped: the text may come from a file that was edited by hand.
    pub fn apply_pairs(&mut self, pairs: &[(String, String)]) {
        for (k, v) in pairs {
            match k.as_str() {
                "tol_ppm" => {
                    if let Ok(x) = v.parse() {
                        self.tol_ppm = x;
                    }
                }
                "tol" => {
                    if let Some(x) = v.parse::<f64>().ok().filter(|x| x.is_finite()) {
                        self.tol = x.clamp(0.001, 1000.0);
                    }
                }
                "rt_win" => {
                    if let Some(x) = v.parse::<f64>().ok().filter(|x| x.is_finite()) {
                        self.rt_win = x.clamp(0.01, 60.0);
                    }
                }
                "weighting" => {
                    if let Ok(x) = v.parse::<u8>() {
                        self.weighting = x.min(2);
                    }
                }
                "baseline" => {
                    if let Ok(x) = v.parse() {
                        self.baseline = x;
                    }
                }
                "ratio_tol" => {
                    if let Some(x) = v.parse::<f64>().ok().filter(|x| x.is_finite()) {
                        self.ratio_tol = x.clamp(1.0, 100.0);
                    }
                }
                "units" => self.units = v.chars().take(32).collect(),
                "mode" => {
                    self.mode = if v == "areas" {
                        Mode::Areas
                    } else {
                        Mode::Curve
                    }
                }
                "peak_method" => {
                    if let Some(m) = PeakMethod::from_key(v) {
                        self.peak_method = m;
                    }
                }
                "basis" => self.basis = if v == "ms1" { Basis::Ms1 } else { Basis::Ms2 },
                "zoom" => {
                    if let Some(x) = v.parse::<f32>().ok().filter(|x| x.is_finite()) {
                        self.zoom = x.clamp(0.5, 3.0);
                    }
                }
                "align_on" => {
                    if let Ok(x) = v.parse() {
                        self.align_on = x;
                    }
                }
                "align_max" => {
                    if let Some(x) = v.parse::<f64>().ok().filter(|x| x.is_finite()) {
                        self.align_max = x.clamp(0.1, 10.0);
                    }
                }
                _ => {}
            }
        }
    }

    /// The parameters as text, one `key=value` per line.
    #[must_use]
    pub fn to_text(&self) -> String {
        self.to_pairs()
            .iter()
            .map(|(k, v)| format!("{}={}\n", k, v))
            .collect()
    }

    /// Parses text written by [`AnalysisSettings::to_text`]; missing keys keep their defaults.
    #[must_use]
    pub fn from_text(text: &str) -> Self {
        let pairs: Vec<(String, String)> = text
            .lines()
            .filter_map(|l| l.split_once('='))
            .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
            .collect();
        let mut s = Self::default();
        s.apply_pairs(&pairs);
        s
    }
}
