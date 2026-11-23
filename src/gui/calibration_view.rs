//! Results table, calibration, results matrix and calibration setup.
use crate::gui::widgets::{dv, dv_conc, export_req_cal, plot_outer_rect, safe_name};

use crate::app::state::{App, ExportKind, WEIGHTS};
use crate::domain::types::{Kind, Mode};
use crate::formats::svg::{SvgPlot, SvgSeries};
use crate::support::fmt;
use eframe::egui::{self, Color32};
use egui_plot::{Line, Plot, PlotPoints, Points};

impl App {
    /// Table of all targets for the current file.
    pub(crate) fn ui_results(&mut self, ui: &mut egui::Ui, cur: Option<usize>) {
        let curve = self.settings.mode == Mode::Curve;
        egui::ScrollArea::vertical()
            .id_salt("res")
            .max_height(220.0)
            .show(ui, |ui| {
                egui::Grid::new("restab").striped(true).show(ui, |ui| {
                    for h in ["Target", "Start", "End", "Area (a.u.)"] {
                        ui.strong(h);
                    }
                    if curve {
                        ui.strong("Conc");
                    }
                    ui.strong("Flags");
                    ui.end_row();
                    for a in 0..self.analytes.len() {
                        if ui
                            .selectable_label(self.sel_a == a, &self.analytes[a].name)
                            .clicked()
                        {
                            self.sel_a = a;
                        }
                        match cur.and_then(|s| self.samples[s].cells.get(a)) {
                            Some(c) => {
                                ui.label(format!("{:.2}", c.start));
                                ui.label(format!("{:.2}", c.end));
                                ui.label(fmt::sci(c.areas.first().copied().unwrap_or(0.0)));
                                if curve {
                                    ui.label(
                                        cur.and_then(|s| self.conc_of(s, a))
                                            .map(fmt::sci)
                                            .unwrap_or_else(|| "-".into()),
                                    );
                                }
                                ui.label(cur.map(|s| self.flags(s, a)).unwrap_or_default());
                            }
                            None => {
                                for _ in 0..(if curve { 5 } else { 4 }) {
                                    ui.label("-");
                                }
                            }
                        }
                        ui.end_row();
                    }
                });
            });
    }

    /// Calibration of the selected target: levels with their replicates, the curve plot and its export.
    pub(crate) fn ui_cal(&mut self, ui: &mut egui::Ui) {
        let a = self.sel_a;
        let fit = self.fits.get(a).copied().flatten();
        match fit {
            Some(f) => ui.label(format!("{}: area = {}·conc + {}   R² = {}   n = {}   weighting {}{}", self.analytes[a].name, fmt::sci(f.m), fmt::sci(f.b), fmt::sci(f.r2),
                fmt::sci(f.n as f64), WEIGHTS[self.settings.weighting as usize], if f.n < 5 { "   ⚠ fewer than 5 points" } else { "" })),
            None => ui.label("No calibration for this target: assign standard concentrations in the 'Calibration setup' tab (≥2 levels)."),
        };
        // standards of THIS target, grouped into concentration levels (replicates share a level)
        let mut std_idx: Vec<usize> = (0..self.samples.len())
            .filter(|&i| {
                let s = &self.samples[i];
                s.kind == Kind::Standard && s.cells.get(a).map(|c| c.conc > 0.0).unwrap_or(false)
            })
            .collect();
        std_idx.sort_by(|&x, &y| {
            let (cx, cy) = (self.samples[x].cells[a].conc, self.samples[y].cells[a].conc);
            cx.partial_cmp(&cy)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| self.samples[x].data.name.cmp(&self.samples[y].data.name))
        });
        let mut toggled: Option<usize> = None;
        let mut use_all = false;
        ui.horizontal(|ui| {
            if ui.button("Use all replicates").clicked() { use_all = true; }
            ui.weak(format!("{} standard file(s) assigned to this target; untick a replicate to exclude it from the curve", std_idx.len()));
        });
        let reset_cal_now = std::mem::take(&mut self.reset_cal);
        let mut cal_req: Option<(ExportKind, SvgPlot, egui::Rect, String)> = None;
        let mut cal_reset = false;
        ui.columns(2, |cols| {
            cols[0].push_id("calt", |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("calscroll")
                    .max_height(240.0)
                    .show(ui, |ui| {
                        egui::Grid::new("caltab").striped(true).show(ui, |ui| {
                            for h in ["Level / file", "Conc", "Area", "Accuracy %", "Use"] {
                                ui.strong(h);
                            }
                            ui.end_row();
                            let mut k = 0;
                            while k < std_idx.len() {
                                let c0 = self.samples[std_idx[k]].cells[a].conc;
                                let mut m = k;
                                while m < std_idx.len()
                                    && (self.samples[std_idx[m]].cells[a].conc - c0).abs()
                                        <= 1e-9 * c0.max(1.0)
                                {
                                    m += 1;
                                }
                                let group = &std_idx[k..m];
                                let used: Vec<f64> = group
                                    .iter()
                                    .filter(|&&i| {
                                        let c = &self.samples[i].cells[a];
                                        c.found && c.include
                                    })
                                    .map(|&i| self.samples[i].cells[a].areas[0])
                                    .collect();
                                let mean = if used.is_empty() {
                                    0.0
                                } else {
                                    used.iter().sum::<f64>() / used.len() as f64
                                };
                                let rsd = if used.len() >= 2 && mean > 0.0 {
                                    let var = used.iter().map(|v| (v - mean).powi(2)).sum::<f64>()
                                        / (used.len() as f64 - 1.0);
                                    100.0 * var.sqrt() / mean
                                } else {
                                    0.0
                                };
                                ui.strong(format!(
                                    "Level {} {}",
                                    fmt::sci(c0),
                                    self.settings.units
                                ));
                                ui.label(format!("n = {}/{}", used.len(), group.len()));
                                ui.label(format!("mean {}", fmt::sci(mean)));
                                ui.label(if used.len() >= 2 {
                                    format!("RSD {} %", fmt::sci(rsd))
                                } else {
                                    String::new()
                                });
                                ui.label("");
                                ui.end_row();
                                for &si in group {
                                    let sm = &self.samples[si];
                                    let c = &sm.cells[a];
                                    ui.label(format!("   {}", sm.data.name));
                                    ui.label(fmt::sci(c.conc));
                                    ui.label(fmt::sci(c.areas[0]));
                                    ui.label(match fit {
                                        Some(f) if f.m != 0.0 => {
                                            fmt::sci(100.0 * ((c.areas[0] - f.b) / f.m) / c.conc)
                                        }
                                        _ => "-".into(),
                                    });
                                    let mut inc = c.include;
                                    if ui.checkbox(&mut inc, "").changed() {
                                        toggled = Some(si);
                                    }
                                    ui.end_row();
                                }
                                k = m;
                            }
                        });
                    });
            });
            let (mut inc_pts, mut exc_pts) = (vec![], vec![]);
            for &si in &std_idx {
                let c = &self.samples[si].cells[a];
                if !c.found {
                    continue;
                }
                let p = [c.conc, c.areas[0]];
                if c.include {
                    inc_pts.push(p)
                } else {
                    exc_pts.push(p)
                }
            }
            let units = self.settings.units.clone();
            let line_pts: Option<Vec<[f64; 2]>> =
                fit.map(|f| vec![[0.0, f.b], [f.hi * 1.05, f.m * f.hi * 1.05 + f.b]]);
            // regression formula and R² are written into the plot itself (and therefore into PNG / SVG exports)
            let notes: Vec<String> = match fit {
                Some(f) => vec![
                    format!(
                        "area = {}·conc + {}",
                        fmt::sci_prec(f.m, 4),
                        fmt::sci_prec(f.b, 4)
                    ),
                    format!(
                        "R² = {}   n = {}",
                        fmt::sci_prec(f.r2, 5),
                        fmt::sci(f.n as f64)
                    ),
                    format!("weighting {}", WEIGHTS[self.settings.weighting as usize]),
                ],
                None => vec![],
            };
            let (inc_c, exc_c) = (inc_pts.clone(), exc_pts.clone());
            let mut plot = sciplot!(
                Plot::new("cal")
                    .height((self.panel_height - 170.0).clamp(220.0, 900.0))
                    .x_axis_label(format!("Concentration ({})", units))
                    .y_axis_label("Area (intensity·s)")
            );
            if reset_cal_now {
                plot = plot.reset();
            }
            let pr = plot.show(&mut cols[1], |pu| {
                pu.points(Points::new(inc_pts).radius(4.0_f32).name("used"));
                pu.points(
                    Points::new(exc_pts)
                        .radius(4.0_f32)
                        .color(Color32::GRAY)
                        .name("excluded"),
                );
                if let Some(lp) = &line_pts {
                    pu.line(Line::new(PlotPoints::from(lp.clone())).name("fit"));
                }
            });
            let rect = plot_outer_rect(&cols[1], pr.response.rect);
            let (x0, x1, y0, y1) = {
                let b = pr.transform.bounds();
                (b.min()[0], b.max()[0], b.min()[1], b.max()[1])
            };
            // The formula and R² are painted on top of the plot instead of being a plot item: an item at the corner of the
            // visible area feeds back into the automatic bounds and made the plot grow with every repaint.
            if !notes.is_empty() {
                let painter = cols[1].painter().with_clip_rect(rect);
                let galley = painter.layout_no_wrap(
                    notes.join("\n"),
                    egui::FontId::proportional(13.0),
                    Color32::WHITE,
                );
                let pos = rect.min + egui::vec2(95.0, 8.0);
                let frame = egui::Rect::from_min_size(
                    pos - egui::vec2(5.0, 3.0),
                    galley.size() + egui::vec2(10.0, 6.0),
                );
                painter.rect_filled(frame, 3.0_f32, Color32::from_black_alpha(190));
                painter.galley(pos, galley, Color32::WHITE);
            }
            let names: Vec<String> = vec![];
            let mut ex: Option<ExportKind> = None;
            export_menu!(pr.response, "calibration curve", ex, &names);
            if let Some(k) = ex {
                match k {
                    ExportKind::Reset => {
                        cal_reset = true;
                    }
                    ExportKind::Csv(_) => {}
                    _ => {
                        let mut ss = vec![SvgSeries {
                            name: "used".into(),
                            rgb: (31, 119, 180),
                            width: 1.0,
                            points: inc_c,
                            marker: true,
                        }];
                        if !exc_c.is_empty() {
                            ss.push(SvgSeries {
                                name: "excluded".into(),
                                rgb: (150, 150, 150),
                                width: 1.0,
                                points: exc_c,
                                marker: true,
                            });
                        }
                        if let Some(lp) = line_pts {
                            ss.push(SvgSeries {
                                name: "fit".into(),
                                rgb: (214, 39, 40),
                                width: 2.0,
                                points: lp,
                                marker: false,
                            });
                        }
                        export_req_cal(
                            &mut cal_req,
                            k,
                            SvgPlot {
                                title: format!("Calibration curve · {}", self.analytes[a].name),
                                x_label: format!("Concentration ({})", units),
                                y_label: "Area (intensity·s)".into(),
                                series: ss,
                                vlines: vec![],
                                labels: vec![],
                                notes,
                                bounds: [x0, x1, y0, y1],
                                x_fmt: fmt::sci3,
                                y_fmt: fmt::sci3,
                            },
                            rect,
                            format!("Calibration_curve_{}", safe_name(&self.analytes[a].name)),
                        );
                    }
                }
            }
        });
        if cal_reset {
            self.reset_cal = true;
            ui.ctx().request_repaint();
        }
        if let Some((k, plot, rect, stem)) = cal_req {
            self.do_export(k, plot, rect, &stem);
        }
        if use_all {
            self.push_undo("use all replicates");
            for &si in &std_idx {
                self.samples[si].cells[a].include = true;
            }
            self.refit(a);
        }
        if let Some(si) = toggled {
            self.push_undo("calibration point");
            let inc = !self.samples[si].cells[a].include;
            self.samples[si].cells[a].include = inc;
            let msg = format!(
                "file={} analyte={} include={} reason={}",
                self.samples[si].data.name,
                self.analytes[a].name,
                inc,
                if self.reason.trim().is_empty() {
                    "NONE GIVEN"
                } else {
                    self.reason.trim()
                }
            );
            self.audit.log("CAL_POINT_TOGGLE", &msg);
            self.refit(a);
        }
    }

    /// Areas or concentrations of every target in every imported file.
    pub(crate) fn ui_matrix(&mut self, ui: &mut egui::Ui) {
        if self.analytes.is_empty() {
            ui.label("Load a target table to see the results matrix.");
            return;
        }
        let curve = self.settings.mode == Mode::Curve;
        let use_conc = curve && self.matrix_conc;
        ui.horizontal(|ui| {
            ui.label("Values:");
            ui.selectable_value(&mut self.matrix_conc, false, "Areas (a.u.)");
            if curve {
                ui.selectable_value(
                    &mut self.matrix_conc,
                    true,
                    format!("Concentrations ({})", self.settings.units),
                );
            }
            if ui.button("Copy as TSV").clicked() {
                let mut t = String::from("file\ttype");
                for a in &self.analytes {
                    t += &format!("\t{}", a.name);
                }
                t.push('\n');
                for s in 0..self.samples.len() {
                    t += &format!(
                        "{}\t{}",
                        self.samples[s].data.name,
                        self.samples[s].kind.label()
                    );
                    for a in 0..self.analytes.len() {
                        let c = self.samples[s].cells.get(a);
                        let v = match c {
                            Some(c) if c.found && c.areas[0] > 0.0 => {
                                if use_conc {
                                    self.conc_of(s, a)
                                } else {
                                    Some(c.areas[0])
                                }
                            }
                            _ => None,
                        };
                        t += &match v {
                            Some(v) => format!("\t{:.6e}", v),
                            None => "\tND".to_string(),
                        };
                    }
                    t.push('\n');
                }
                ui.ctx().copy_text(t);
                self.status = "Results matrix copied to the clipboard (tab-separated)".into();
            }
            ui.weak("yellow = flagged (see Flags in the chromatogram tab), ND = not detected");
        });
        egui::ScrollArea::both().id_salt("matrix").show(ui, |ui| {
            egui::Grid::new("matrix").striped(true).show(ui, |ui| {
                ui.strong("File");
                ui.strong("Type");
                for a in &self.analytes {
                    ui.strong(&a.name);
                }
                ui.end_row();
                for s in 0..self.samples.len() {
                    ui.label(&self.samples[s].data.name);
                    ui.label(self.samples[s].kind.label());
                    for a in 0..self.analytes.len() {
                        match self.samples[s].cells.get(a) {
                            Some(c) if c.found && c.areas[0] > 0.0 => {
                                let v = if use_conc {
                                    self.conc_of(s, a)
                                        .map(fmt::sci)
                                        .unwrap_or_else(|| "-".into())
                                } else {
                                    fmt::sci(c.areas[0])
                                };
                                let fl = self.flags(s, a);
                                if fl.is_empty() || fl == "manual" {
                                    ui.label(v);
                                } else {
                                    ui.colored_label(Color32::YELLOW, v).on_hover_text(fl);
                                }
                            }
                            _ => {
                                ui.label("ND");
                            }
                        }
                    }
                    ui.end_row();
                }
            });
        });
    }

    /// Calibration setup tab: assign standard concentrations per target and file.
    pub(crate) fn ui_setup(&mut self, ui: &mut egui::Ui) {
        if self.analytes.is_empty() {
            ui.label("Load a target table first.");
            return;
        }
        ui.heading("Calibration setup");
        ui.weak("Each target has its own calibration. A standard file carries a concentration per target (blank = not a standard for that target). \
                 Replicates simply share the same concentration; all replicates are used unless you exclude one in the calibration table (Chromatograms tab).");
        if self.assign_target >= self.analytes.len() {
            self.assign_target = 0;
        }
        let n_sel = self.shown().len();
        let (mut assign, mut clear) = (false, false);
        ui.horizontal(|ui| {
            ui.label(format!("{} selected file(s) →", n_sel));
            ui.label("target");
            egui::ComboBox::new("assign_t", "")
                .selected_text(self.analytes[self.assign_target].name.clone())
                .show_ui(ui, |ui| {
                    for i in 0..self.analytes.len() {
                        let n = self.analytes[i].name.clone();
                        ui.selectable_value(&mut self.assign_target, i, n);
                    }
                });
            ui.label("concentration");
            ui.add(dv(&mut self.assign_conc).speed(0.1).range(0.0..=1e9));
            ui.label(self.settings.units.clone());
            if ui
                .add_enabled(
                    n_sel > 0 && self.assign_conc > 0.0,
                    egui::Button::new("Assign as standards"),
                )
                .clicked()
            {
                assign = true;
            }
            if ui
                .add_enabled(n_sel > 0, egui::Button::new("Remove concentration"))
                .clicked()
            {
                clear = true;
            }
        });
        ui.weak("Changing the type of a file that is part of a multi-selection sets all selected files to that type.");
        ui.weak("Only files of type Standard are listed below.");
        ui.weak("Tip: select the replicate files in the sample list (Ctrl/Shift+click), pick the target and the level, press Assign; repeat for the next level.");
        if assign {
            self.assign_standards(self.assign_target, self.assign_conc);
        }
        if clear {
            self.assign_standards(self.assign_target, 0.0);
        }
        ui.separator();
        let (na, ns) = (self.analytes.len(), self.samples.len());
        let std_files: Vec<usize> = (0..ns)
            .filter(|&s| self.samples[s].kind == Kind::Standard)
            .collect();
        if std_files.is_empty() {
            ui.label("No files are marked as Standard yet: select files in the sample list and press 'Assign as standards' (or set their type to Standard).");
        }
        let mut change: Option<(usize, usize, f64)> = None;
        let mut kind_change: Option<(usize, Kind)> = None;
        egui::ScrollArea::both().id_salt("setup").show(ui, |ui| {
            egui::Grid::new("setupgrid").striped(true).show(ui, |ui| {
                ui.strong("File");
                ui.strong("Type");
                for an in &self.analytes {
                    ui.strong(format!("{} ({})", an.name, self.settings.units));
                }
                ui.end_row();
                for &s in &std_files {
                    ui.label(&self.samples[s].data.name);
                    let mut k = self.samples[s].kind;
                    egui::ComboBox::new(("setupkind", s), "")
                        .selected_text(k.label())
                        .show_ui(ui, |ui| {
                            for kk in Kind::ALL {
                                ui.selectable_value(&mut k, kk, kk.label());
                            }
                        });
                    if k != self.samples[s].kind {
                        kind_change = Some((s, k));
                    }
                    for a in 0..na {
                        let old = self.samples[s].cells.get(a).map(|c| c.conc).unwrap_or(0.0);
                        let mut v = old;
                        if ui
                            .add(dv_conc(&mut v).speed(0.1).range(0.0..=1e9))
                            .changed()
                            && v != old
                        {
                            change = Some((s, a, v));
                        }
                    }
                    ui.end_row();
                }
            });
        });
        let dragging = ui.ctx().input(|i| i.pointer.any_down());
        if let Some((s, a, v)) = change {
            if !(dragging && self.last_undo_label == "concentration") {
                self.push_undo("concentration");
            }
            if let Some(c) = self.samples[s].cells.get_mut(a) {
                c.conc = v;
            }
            if v > 0.0 && self.samples[s].kind == Kind::Unknown {
                self.samples[s].kind = Kind::Standard;
            }
            self.refit(a);
            self.audit.log(
                "CONCENTRATION_SET",
                &format!(
                    "file={} target={} conc={}",
                    self.samples[s].data.name, self.analytes[a].name, v
                ),
            );
        }
        if let Some((s, k)) = kind_change {
            self.set_type(s, k);
        }
    }

    /// The calibration pane: target and concentration unit on top, then the curve and its table.
    pub(crate) fn ui_calibration_curve(&mut self, ui: &mut egui::Ui) {
        if self.analytes.is_empty() {
            ui.weak("No targets yet: add targets in the target table.");
            return;
        }
        ui.horizontal(|ui| {
            ui.label("Target:");
            let name = self.analytes[self.sel_a.min(self.analytes.len() - 1)]
                .name
                .clone();
            egui::ComboBox::new("calibration_target", "")
                .selected_text(name)
                .show_ui(ui, |ui| {
                    for i in 0..self.analytes.len() {
                        let n = self.analytes[i].name.clone();
                        ui.selectable_value(&mut self.sel_a, i, n);
                    }
                });
        });
        self.ui_unit_selector(ui);
        ui.separator();
        self.ui_cal(ui);
    }

    /// The concentration unit: type any text or pick a common unit.
    pub(crate) fn ui_unit_selector(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Concentration unit:");
            ui.add(egui::TextEdit::singleline(&mut self.settings.units).desired_width(90.0))
                .on_hover_text("Type any unit or pick one from the list");
            egui::ComboBox::new("unit_presets", "")
                .selected_text("presets")
                .width(80.0)
                .show_ui(ui, |ui| {
                    for unit in [
                        "pg/mL", "ng/mL", "µg/mL", "mg/mL", "pg/L", "ng/L", "µg/L", "mg/L", "pM",
                        "nM", "µM", "mM", "fmol", "pmol", "nmol",
                    ] {
                        if ui
                            .selectable_label(self.settings.units == unit, unit)
                            .clicked()
                        {
                            self.settings.units = unit.to_string();
                        }
                    }
                });
        });
    }
}
