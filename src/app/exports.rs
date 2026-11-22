//! Export of results, figures and data.

use crate::app::state::{App, Capture, ExportKind, VERSION, WEIGHTS};
use crate::domain::types::{Basis, Kind, Mode};
use crate::formats::csv;
use crate::formats::png;
use crate::formats::svg::{self, SvgPlot};
use crate::support::audit;
use eframe::egui;

impl App {
    /// Writes the results table, including the calibration parameters, as CSV.
    pub(crate) fn export(&mut self) -> Result<(), String> {
        let curve = self.settings.mode == Mode::Curve;
        let mut o = String::new();
        o += &format!(
            "# msQuant v{} export {} user={}\n",
            VERSION,
            audit::iso_now(),
            self.audit.user
        );
        o += &format!(
            "# quantification={}\n",
            if curve {
                "standard curve"
            } else {
                "integrated areas only (arbitrary units)"
            }
        );
        o += &format!(
            "# quantified_signal={}\n",
            if self.settings.basis == Basis::Ms1 {
                "MS1 precursor area"
            } else {
                "MS2 product-ion area"
            }
        );
        o += &format!(
            "# tol={}{} rt_window_min={} weighting={} baseline_subtraction={} ion_ratio_tol_pct={} conc_units={}\n",
            self.settings.tol,
            if self.settings.tol_ppm { "ppm" } else { "Da" },
            self.settings.rt_win,
            WEIGHTS[self.settings.weighting as usize],
            self.settings.baseline,
            self.settings.ratio_tol,
            self.settings.units
        );
        o += &format!(
            "# rt_alignment_on_import={} max_shift_min={} reference={}\n",
            self.settings.align_on,
            self.settings.align_max,
            self.samples
                .get(self.align_ref)
                .map(|s| s.data.name.clone())
                .unwrap_or_default()
        );
        if curve {
            for (a, an) in self.analytes.iter().enumerate() {
                match self.fits.get(a).copied().flatten() {
                    Some(f) => {
                        o += &format!(
                            "# CAL {}: area = {:.6e}*conc + {:.6e}; R2={:.5}; n={}; range {}-{}\n",
                            an.name, f.m, f.b, f.r2, f.n, f.lo, f.hi
                        )
                    }
                    None => o += &format!("# CAL {}: no valid calibration\n", an.name),
                }
            }
        }
        o += "file,sha256,type,level,rt_shift_min,analyte,start_min,end_min,area_quant_(int*s),manual,ion_ratios,calc_conc,flags\n";
        for s in 0..self.samples.len() {
            for a in 0..self.analytes.len() {
                let sm = &self.samples[s];
                let c = &sm.cells[a];
                let ratios: Vec<String> = (1..c.areas.len())
                    .map(|i| {
                        if c.areas[0] > 0.0 {
                            format!("{:.3}", c.areas[i] / c.areas[0])
                        } else {
                            "n/a".into()
                        }
                    })
                    .collect();
                let conc = self
                    .conc_of(s, a)
                    .map(|v| format!("{:.4}", v))
                    .unwrap_or("n/a".into());
                o += &format!(
                    "{},{},{},{},{:.4},{},{:.4},{:.4},{:.1},{},{},{},{}\n",
                    csv::text_field(&sm.data.name),
                    sm.data.sha256,
                    sm.kind.label(),
                    if sm.kind == Kind::Standard && curve && c.conc > 0.0 {
                        c.conc.to_string()
                    } else {
                        String::new()
                    },
                    sm.rt_shift,
                    csv::text_field(&self.analytes[a].name),
                    c.start,
                    c.end,
                    c.areas[0],
                    c.manual,
                    ratios.join(" "),
                    conc,
                    csv::text_field(&self.flags(s, a))
                );
            }
        }
        let path = self.export_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        std::fs::write(&path, o).map_err(|e| e.to_string())?;
        self.audit.log("EXPORT", &path.display().to_string());
        Ok(())
    }

    /// Lets the user pick the results file in a file dialog.
    pub(crate) fn choose_out_path(&mut self) {
        let current = self.export_path();
        let mut d = self
            .dialog()
            .set_title("Select analysis output file")
            .add_filter("CSV", &["csv"]);
        if let Some(dir) = current.parent() {
            if dir.is_dir() {
                d = d.set_directory(dir);
            }
        }
        d = d.set_file_name(
            current
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "results.csv".into()),
        );
        if let Some(path) = d.save_file() {
            self.out_path = path.to_string_lossy().to_string();
        }
    }

    /// Asks for a file name, then writes the SVG at once or schedules a PNG capture of the plot rectangle.
    pub(crate) fn do_export(
        &mut self,
        kind: ExportKind,
        plot: SvgPlot,
        rect: egui::Rect,
        stem: &str,
    ) {
        let (ext, label) = if kind == ExportKind::Svg {
            ("svg", "SVG image")
        } else {
            ("png", "PNG image")
        };
        let d = self
            .dialog()
            .set_title("Save figure")
            .add_filter(label, &[ext])
            .set_file_name(format!("{}.{}", stem, ext));
        let path = match d.save_file() {
            Some(p) => p,
            None => return,
        };
        match kind {
            ExportKind::Png => {
                self.status = format!("Capturing the plot to {} ...", path.display());
                self.capture = Some(Capture {
                    rect,
                    path,
                    frames: 3,
                    sent: false,
                });
            }
            _ => match std::fs::write(&path, svg::render(&plot)) {
                Ok(_) => {
                    self.audit.log(
                        "FIGURE_EXPORTED",
                        &format!("{} ({})", path.display(), plot.title),
                    );
                    self.status = format!("Saved {}", path.display());
                }
                Err(e) => self.status = format!("Saving the figure failed: {}", e),
            },
        }
    }

    /// Two-column data export (RT/Intensity or m/z/Intensity); the suggested name says what is exported.
    pub(crate) fn save_csv(&mut self, stem: &str, text: &str) {
        let d = self
            .dialog()
            .set_title("Save data as CSV")
            .add_filter("CSV", &["csv"])
            .set_file_name(format!("{}.csv", stem));
        let path = match d.save_file() {
            Some(p) => p,
            None => return,
        };
        match std::fs::write(&path, text) {
            Ok(_) => {
                self.audit.log("DATA_EXPORTED", &path.display().to_string());
                self.status = format!("Saved {}", path.display());
            }
            Err(e) => self.status = format!("Saving the data failed: {}", e),
        }
    }

    /// Crops the window screenshot to the plot rectangle and writes it as PNG.
    pub(crate) fn save_png(&mut self, ctx: &egui::Context, img: &egui::ColorImage, cap: Capture) {
        let ppp = ctx.pixels_per_point();
        let (iw, ih) = (img.size[0], img.size[1]);
        let px = |v: f32, max: usize| ((v * ppp).round().max(0.0) as usize).min(max);
        let (x0, y0, x1, y1) = (
            px(cap.rect.min.x, iw),
            px(cap.rect.min.y, ih),
            px(cap.rect.max.x, iw),
            px(cap.rect.max.y, ih),
        );
        if x1 <= x0 || y1 <= y0 {
            self.status = "Capture failed: the plot is not visible.".into();
            return;
        }
        let mut rgb = Vec::with_capacity((x1 - x0) * (y1 - y0) * 3);
        for y in y0..y1 {
            for x in x0..x1 {
                let p = img.pixels[y * iw + x];
                rgb.extend_from_slice(&[p.r(), p.g(), p.b()]);
            }
        }
        match png::encode_rgb((x1 - x0) as u32, (y1 - y0) as u32, &rgb) {
            Ok(bytes) => match std::fs::write(&cap.path, bytes) {
                Ok(_) => {
                    self.audit
                        .log("FIGURE_EXPORTED", &cap.path.display().to_string());
                    self.status = format!("Saved {}", cap.path.display());
                }
                Err(e) => self.status = format!("Saving the figure failed: {}", e),
            },
            Err(e) => self.status = format!("PNG encoding failed: {}", e),
        }
    }

    /// Asks for the results file and exports the results table.
    pub(crate) fn export_results_dialog(&mut self) {
        self.choose_out_path();
        self.status = match self.export() {
            Ok(()) => format!("Exported {}", self.export_path().display()),
            Err(e) => format!("Export failed: {}", e),
        };
    }
}
