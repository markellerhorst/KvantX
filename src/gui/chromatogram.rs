//! Chromatogram views: the target view with editable boundaries, overlays and the quick extracted-ion chromatogram.
use crate::gui::widgets::{dv_mz, dv_rt, pal, safe_name, svg_series};

use crate::analysis::integration;
use crate::app::analysis_ops::{SummaryTrace, chrom};
use crate::app::state::{App, ExportKind, Scope, View};
use crate::domain::types::{Basis, Mode};
use crate::formats::csv;
use crate::formats::svg::{SvgPlot, SvgVLine};
use crate::gui::plotting::{self, Boundary, Hover, Series};
use crate::support::fmt;
use eframe::egui::{self, Color32, PointerButton, RichText};
use egui_plot::{
    Legend, Line, Plot, PlotBounds, PlotPoint, PlotPoints, Points, Polygon, Text, VLine,
};

impl App {
    // ---------------- UI ----------------
    /// Selected target, all traces, in file `s`, with draggable integration boundaries and hover read-out.
    pub(crate) fn ui_plot(&mut self, ui: &mut egui::Ui, s: usize) {
        let a = self.sel_a;
        let mut grab = self.grab;
        let mut edit: Option<(f64, f64)> = None;
        let mut stopped = false;
        let clicked;
        let mut hover: Option<Hover> = None;
        let mut ex: Option<ExportKind> = None;
        let mut export_req: Option<(ExportKind, SvgPlot, egui::Rect, String)> = None;
        let mut csv_req: Option<(String, String)> = None;
        let tmz = self.trace_mz(a);
        let capturing = self.capture.is_some();
        let reset_now = std::mem::take(&mut self.reset_chrom);
        let alt = ui.input(|i| i.modifiers.alt);
        let mut zoom_c = self.zoom_chrom;
        let mut reset_req = false;
        {
            let sm = &self.samples[s];
            let c = &sm.cells[a];
            let norm = self.normalize;
            let level: u8 = if self.settings.basis == Basis::Ms1 {
                1
            } else {
                2
            };
            let series: Vec<Series> = sm.xics[a]
                .iter()
                .enumerate()
                .map(|(i, x)| {
                    let mx = x.iter().fold(0.0f64, |m, p| m.max(p[1]));
                    let d = format!("m/z {}", fmt::mz4(tmz[i]));
                    Series {
                        name: d.clone(),
                        data: x.clone(),
                        ci: i,
                        cur: false,
                        file: s,
                        level,
                        fname: sm.data.name.clone(),
                        desc: format!("{} · {}", self.analytes[a].name, d),
                        scale: if norm && mx > 0.0 { 100.0 / mx } else { 1.0 },
                    }
                })
                .collect();
            let scan_of = |f: usize, l: u8, j: usize| -> String {
                plotting::scan_info(&self.samples[f].data, l, j)
            };
            ui.weak("drag a box = zoom · grab a boundary line to move it · click = pin the scan · right-click = menu");
            let mut plot = rtplot!(
                Plot::new("xic")
                    .height((self.panel_height - 200.0).max(160.0))
                    .allow_drag(false)
                    .allow_boxed_zoom(false)
                    .allow_scroll(!alt)
                    .legend(Legend::default())
                    .x_axis_label("Retention time (min)")
                    .y_axis_label(if norm {
                        "Intensity (% of maximum)"
                    } else {
                        "Intensity"
                    }),
                norm
            );
            if reset_now {
                plot = plot.reset();
            }
            let pr = plot.show(ui, |pu| {
                draw_series!(pu, &series, hover, scan_of, capturing);
                if c.found && !norm {
                    pu.polygon(
                        Polygon::new(PlotPoints::from(integration::shade(
                            &sm.xics[a][0],
                            c.start,
                            c.end,
                        )))
                        .fill_color(Color32::from_rgba_unmultiplied(31, 119, 180, 60))
                        .name("integrated"),
                    );
                }
                if c.found {
                    pu.vline(VLine::new(c.start).color(Color32::DARK_GREEN));
                    pu.vline(VLine::new(c.end).color(Color32::DARK_RED));
                }
                let r = pu.response().clone();
                let bnd = pu.plot_bounds();
                let pxw = r.rect.width() as f64 / (bnd.max()[0] - bnd.min()[0]).max(1e-12);
                if let Some(p) = pu.pointer_coordinate() {
                    // a boundary line is only moved when it is grabbed (within 8 px); anywhere else a drag draws the zoom window
                    let near = if c.found && plotting::in_view(&p, &bnd) {
                        plotting::nearest_boundary(p.x, c.start, c.end, pxw, 8.0)
                    } else {
                        None
                    };
                    if near.is_some() && r.hovered() {
                        r.ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                    }
                    if r.drag_started_by(PointerButton::Primary) {
                        grab = near;
                    }
                    if r.dragged_by(PointerButton::Primary) {
                        if let Some(boundary) = grab {
                            let (mut st, mut en) = (c.start, c.end);
                            match boundary {
                                Boundary::Start => st = p.x.min(en - 1e-4),
                                Boundary::End => en = p.x.max(st + 1e-4),
                            }
                            edit = Some((st, en));
                        }
                    }
                }
                let _ = zoom_pan!(pu, r, zoom_c, grab.is_some());
                if r.drag_stopped() {
                    stopped = true;
                }
            });
            clicked = pr.response.clicked() && !capturing;
            let rect = pr.response.rect;
            let (x0, x1, y0, y1) = {
                let b = pr.transform.bounds();
                (b.min()[0], b.max()[0], b.min()[1], b.max()[1])
            };
            let names: Vec<String> = series.iter().map(|s| s.name.clone()).collect();
            export_menu!(pr.response, "chromatogram", ex, &names);
            if let Some(k) = ex {
                let (file, target) = (safe_name(&sm.data.name), safe_name(&self.analytes[a].name));
                match k {
                    ExportKind::Reset => {
                        reset_req = true;
                    }
                    ExportKind::Csv(i) => {
                        if let Some(sr) = series.get(i) {
                            csv_req = Some((
                                format!("XIC_{}_mz{}_{}", target, fmt::mz4(tmz[i]), file),
                                csv::two_columns(
                                    "RT (min)",
                                    "Intensity",
                                    sr.data.iter().map(|p| (p[0], p[1])),
                                ),
                            ));
                        }
                    }
                    _ => {
                        let mut vl = vec![];
                        if c.found {
                            vl.push(SvgVLine {
                                x: c.start,
                                rgb: (0, 100, 0),
                                label: "integration start".into(),
                            });
                            vl.push(SvgVLine {
                                x: c.end,
                                rgb: (139, 0, 0),
                                label: "integration end".into(),
                            });
                        }
                        let yf: fn(f64) -> String = if norm { fmt::pct0 } else { fmt::sci3 };
                        export_req = Some((
                            k,
                            SvgPlot {
                                title: format!("{} · {}", sm.data.name, self.analytes[a].name),
                                x_label: "Retention time (min)".into(),
                                y_label: if norm {
                                    "Intensity (% of maximum)".into()
                                } else {
                                    "Intensity".into()
                                },
                                series: svg_series(&series),
                                vlines: vl,
                                labels: vec![],
                                notes: vec![],
                                bounds: [x0, x1, y0, y1],
                                x_fmt: fmt::rt2,
                                y_fmt: yf,
                            },
                            rect,
                            format!("XIC_{}_{}", target, file),
                        ));
                    }
                }
            }
        }
        if let Some(h) = &hover {
            self.hover = Some((h.file, h.rt));
            if clicked {
                self.pin_scan(h.file, h.rt);
            }
        }
        self.zoom_chrom = zoom_c;
        if reset_req {
            self.reset_chrom = true;
            ui.ctx().request_repaint();
        }
        if let Some((k, plot, rect, stem)) = export_req {
            self.do_export(k, plot, rect, &stem);
        }
        if let Some((stem, text)) = csv_req {
            self.save_csv(&stem, &text);
        }
        if let Some((st, en)) = edit {
            self.apply_edit(s, a, st, en);
        }
        if stopped {
            self.commit(s, a);
            grab = None;
        }
        self.grab = grab;

        let (found, mut st, mut en) = {
            let c = &self.samples[s].cells[a];
            (c.found, c.start, c.end)
        };
        let mut changed = false;
        let mut done = false;
        ui.horizontal(|ui| {
            if found {
                ui.label("Start (min)");
                let r1 = ui.add(dv_rt(&mut st).speed(0.01));
                ui.label("End (min)");
                let r2 = ui.add(dv_rt(&mut en).speed(0.01));
                changed = r1.changed() || r2.changed();
                done = r1.drag_stopped() || r1.lost_focus() || r2.drag_stopped() || r2.lost_focus();
            } else if ui.button("No peak found - define manually").clicked() {
                let x = &self.samples[s].xics[a][0];
                let mid = self.analytes[a]
                    .rt
                    .unwrap_or_else(|| x.get(x.len() / 2).map(|p| p[0]).unwrap_or(1.0));
                st = mid - 0.1;
                en = mid + 0.1;
                changed = true;
                done = true;
            }
            ui.label("Reason for manual change:");
            ui.add(egui::TextEdit::singleline(&mut self.reason).desired_width(260.0));
        });
        if changed && st < en {
            self.apply_edit(s, a, st, en);
        }
        if done {
            self.commit(s, a);
        }
        if found && ui.button("Reset to automatic boundaries").clicked() {
            self.push_undo("reset integration");
            self.audit.log(
                "MANUAL_RESET",
                &format!(
                    "file={} analyte={}",
                    self.samples[s].data.name, self.analytes[a].name
                ),
            );
            self.samples[s].cells[a].manual = false;
            self.recompute_sample(s);
            self.refit(a);
        }
        let c = &self.samples[s].cells[a];
        if c.found {
            let mut t = String::new();
            for (i, ar) in c.areas.iter().enumerate() {
                t += &format!(
                    "{} {} (m/z {}): area {}   ",
                    if self.settings.basis == Basis::Ms1 {
                        "precursor"
                    } else {
                        "ion"
                    },
                    i + 1,
                    fmt::mz4(tmz[i]),
                    fmt::sci(*ar)
                );
                if i > 0 && c.areas[0] > 0.0 {
                    t += &format!("[ratio {}]   ", fmt::sci(ar / c.areas[0]));
                }
            }
            ui.monospace(t);
            match self.conc_of(s, a) {
                Some(v) => ui.strong(format!(
                    "Calculated: {} {}   {}",
                    fmt::sci(v),
                    self.settings.units,
                    self.flags(s, a)
                )),
                None => {
                    if self.settings.mode == Mode::Curve {
                        ui.label("No calibration available for this analyte")
                    } else {
                        ui.strong(format!(
                            "Integrated area: {} a.u. (intensity·s)   {}",
                            fmt::sci(c.areas[0]),
                            self.flags(s, a)
                        ))
                    }
                }
            };
        }
    }

    /// Read-only overlays: all targets of the current file, one target / a custom m/z across selected files, or TIC/BPC of selected files.
    pub(crate) fn ui_overlay(&mut self, ui: &mut egui::Ui, cur: Option<usize>) {
        let a = self.sel_a;
        let norm = self.normalize;
        let shown = self.shown();
        let level: u8 = if self.settings.basis == Basis::Ms1 {
            1
        } else {
            2
        };
        let mut series: Vec<Series> = vec![];
        match (self.view, self.scope) {
            (View::Xic, Scope::AllTargets) => {
                if let Some(s) = cur {
                    let sm = &self.samples[s];
                    for (ai, an) in self.analytes.iter().enumerate() {
                        if let Some(x) = sm.xics.get(ai).and_then(|v| v.first()) {
                            let desc =
                                format!("{} · m/z {}", an.name, fmt::mz4(self.trace_mz(ai)[0]));
                            series.push(Series {
                                name: an.name.clone(),
                                data: x.clone(),
                                ci: ai,
                                cur: false,
                                file: s,
                                level,
                                fname: sm.data.name.clone(),
                                desc,
                                scale: 1.0,
                            });
                        }
                    }
                }
            }
            (View::Quick, _) => {
                let desc = format!(
                    "custom m/z {} (MS{})",
                    fmt::mz4(self.quick_mz),
                    self.quick_level
                );
                for &f in &shown {
                    let sm = &self.samples[f];
                    series.push(Series {
                        name: sm.data.name.clone(),
                        data: self.quick_trace(f),
                        ci: f,
                        cur: Some(f) == cur,
                        file: f,
                        level: self.quick_level,
                        fname: sm.data.name.clone(),
                        desc: desc.clone(),
                        scale: 1.0,
                    });
                }
            }
            (View::Xic, _) => {
                let desc = format!(
                    "{} · m/z {}",
                    self.analytes[a].name,
                    fmt::mz4(self.trace_mz(a)[0])
                );
                for &f in &shown {
                    let sm = &self.samples[f];
                    if let Some(x) = sm.xics.get(a).and_then(|v| v.first()) {
                        series.push(Series {
                            name: sm.data.name.clone(),
                            data: x.clone(),
                            ci: f,
                            cur: Some(f) == cur,
                            file: f,
                            level,
                            fname: sm.data.name.clone(),
                            desc: desc.clone(),
                            scale: 1.0,
                        });
                    }
                }
            }
            (v, _) => {
                let bpc = v == View::Bpc;
                for &f in &shown {
                    let sm = &self.samples[f];
                    let sh = sm.rt_shift;
                    let d: Vec<[f64; 2]> = chrom(
                        &sm.data,
                        self.tic_level,
                        if bpc {
                            SummaryTrace::BasePeak
                        } else {
                            SummaryTrace::TotalIon
                        },
                    )
                    .iter()
                    .map(|p| [p[0] + sh, p[1]])
                    .collect();
                    series.push(Series {
                        name: sm.data.name.clone(),
                        data: d,
                        ci: f,
                        cur: Some(f) == cur,
                        file: f,
                        level: self.tic_level,
                        fname: sm.data.name.clone(),
                        desc: format!("{} (MS{})", if bpc { "BPC" } else { "TIC" }, self.tic_level),
                        scale: 1.0,
                    });
                }
            }
        }
        series.retain(|x| !x.data.is_empty());
        if shown.is_empty() {
            ui.label("No file selected.");
            return;
        }
        if series.is_empty() {
            ui.label(if self.view == View::Xic {
                "No chromatogram data."
            } else {
                "The selected file(s) have no scans at this MS level."
            });
            return;
        }
        if norm {
            for sr in series.iter_mut() {
                let m = sr.data.iter().fold(0.0f64, |m, p| m.max(p[1]));
                if m > 0.0 {
                    sr.scale = 100.0 / m;
                }
            }
        }
        let rt_hint = if self.view == View::Xic && !self.analytes.is_empty() {
            self.analytes[a].rt
        } else {
            None
        };
        let id = (self.view as u8, self.scope as u8, self.tic_level);
        let mut hover: Option<Hover> = None;
        let clicked;
        let mut ex: Option<ExportKind> = None;
        let mut export_req: Option<(ExportKind, SvgPlot, egui::Rect, String)> = None;
        let mut csv_req: Option<(String, String)> = None;
        let capturing = self.capture.is_some();
        let reset_now = std::mem::take(&mut self.reset_chrom);
        let alt = ui.input(|i| i.modifiers.alt);
        let mut zoom_c = self.zoom_chrom;
        let mut reset_req = false;
        {
            let scan_of = |f: usize, l: u8, j: usize| -> String {
                plotting::scan_info(&self.samples[f].data, l, j)
            };
            ui.weak("drag a box = zoom · click = pin the nearest MS2 scan · right-click = menu");
            let mut plot = rtplot!(
                Plot::new(id)
                    .height((self.panel_height - 60.0).max(160.0))
                    .allow_drag(false)
                    .allow_boxed_zoom(false)
                    .allow_scroll(!alt)
                    .legend(Legend::default())
                    .x_axis_label("Retention time (min)")
                    .y_axis_label(if norm {
                        "Intensity (% of maximum)"
                    } else {
                        "Intensity"
                    }),
                norm
            );
            if reset_now {
                plot = plot.reset();
            }
            let pr = plot.show(ui, |pu| {
                draw_series!(pu, &series, hover, scan_of, capturing);
                if let Some(h) = rt_hint {
                    pu.vline(VLine::new(h).color(Color32::GRAY).name("expected RT"));
                }
                let r = pu.response().clone();
                let _ = zoom_pan!(pu, r, zoom_c, false);
            });
            clicked = pr.response.clicked() && !capturing;
            let rect = pr.response.rect;
            let (x0, x1, y0, y1) = {
                let b = pr.transform.bounds();
                (b.min()[0], b.max()[0], b.min()[1], b.max()[1])
            };
            let names: Vec<String> = series.iter().map(|s| s.name.clone()).collect();
            export_menu!(pr.response, "chromatogram", ex, &names);
            if let Some(k) = ex {
                // the file name suggestion says what is exported: TIC / BPC / XIC, MS level, target or m/z, file
                let filepart = if shown.len() == 1 {
                    safe_name(&self.samples[shown[0]].data.name)
                } else {
                    format!("{}files", shown.len())
                };
                let (prefix, stem_all, title): (String, String, String) = match self.view {
                    View::Xic => {
                        if self.scope == Scope::AllTargets {
                            (
                                "XIC".into(),
                                format!("XIC_all-targets_{}", filepart),
                                "Extracted ion chromatograms (quantifier ions of all targets)"
                                    .into(),
                            )
                        } else {
                            let t = safe_name(&self.analytes[a].name);
                            (
                                format!("XIC_{}", t),
                                format!("XIC_{}_{}", t, filepart),
                                format!("Extracted ion chromatogram · {}", self.analytes[a].name),
                            )
                        }
                    }
                    View::Tic => (
                        format!("TIC_MS{}", self.tic_level),
                        format!("TIC_MS{}_{}", self.tic_level, filepart),
                        format!("Total ion chromatogram (MS{})", self.tic_level),
                    ),
                    View::Bpc => (
                        format!("BPC_MS{}", self.tic_level),
                        format!("BPC_MS{}_{}", self.tic_level, filepart),
                        format!("Base peak chromatogram (MS{})", self.tic_level),
                    ),
                    View::Quick => {
                        let p = format!(
                            "XIC_custom_mz{}_MS{}",
                            fmt::mz4(self.quick_mz),
                            self.quick_level
                        );
                        (
                            p.clone(),
                            format!("{}_{}", p, filepart),
                            format!(
                                "Extracted ion chromatogram · m/z {} (MS{})",
                                fmt::mz4(self.quick_mz),
                                self.quick_level
                            ),
                        )
                    }
                };
                match k {
                    ExportKind::Reset => {
                        reset_req = true;
                    }
                    ExportKind::Csv(i) => {
                        if let Some(sr) = series.get(i) {
                            let stem = if self.view == View::Xic && self.scope == Scope::AllTargets
                            {
                                format!("XIC_{}_{}", safe_name(&sr.name), safe_name(&sr.fname))
                            } else {
                                format!("{}_{}", prefix, safe_name(&sr.name))
                            };
                            csv_req = Some((
                                stem,
                                csv::two_columns(
                                    "RT (min)",
                                    "Intensity",
                                    sr.data.iter().map(|p| (p[0], p[1])),
                                ),
                            ));
                        }
                    }
                    _ => {
                        let yf: fn(f64) -> String = if norm { fmt::pct0 } else { fmt::sci3 };
                        let vl: Vec<SvgVLine> = rt_hint
                            .map(|h| SvgVLine {
                                x: h,
                                rgb: (128, 128, 128),
                                label: "expected RT".into(),
                            })
                            .into_iter()
                            .collect();
                        export_req = Some((
                            k,
                            SvgPlot {
                                title,
                                x_label: "Retention time (min)".into(),
                                y_label: if norm {
                                    "Intensity (% of maximum)".into()
                                } else {
                                    "Intensity".into()
                                },
                                series: svg_series(&series),
                                vlines: vl,
                                labels: vec![],
                                notes: vec![],
                                bounds: [x0, x1, y0, y1],
                                x_fmt: fmt::rt2,
                                y_fmt: yf,
                            },
                            rect,
                            stem_all,
                        ));
                    }
                }
            }
        }
        if let Some(h) = &hover {
            self.hover = Some((h.file, h.rt));
            if clicked {
                self.pin_scan(h.file, h.rt);
            }
        }
        if let Some((k, plot, rect, stem)) = export_req {
            self.do_export(k, plot, rect, &stem);
        }
        self.zoom_chrom = zoom_c;
        if reset_req {
            self.reset_chrom = true;
            ui.ctx().request_repaint();
        }
        if let Some((stem, text)) = csv_req {
            self.save_csv(&stem, &text);
        }
    }

    /// Retention-time shift control of the current file.
    pub(crate) fn ui_shift(&mut self, ui: &mut egui::Ui, s: usize) {
        let mut v = self.samples[s].shift_ui;
        let mut commit = false;
        ui.horizontal(|ui| {
            ui.label(format!("RT shift for {} (min):", self.samples[s].data.name));
            let r = ui.add(dv_rt(&mut v).speed(0.01));
            commit = r.drag_stopped() || r.lost_focus();
            ui.weak("applies to all chromatograms of this file; resets its manual integrations");
        });
        self.samples[s].shift_ui = v;
        if commit && (v - self.samples[s].rt_shift).abs() > 1e-9 {
            self.push_undo("RT shift");
            self.set_shift(s, v, "manual");
        }
    }

    /// The chromatogram tab: view selection, plot, results table and calibration.
    pub(crate) fn ui_chrom(&mut self, ui: &mut egui::Ui) {
        let cur = self.cur();
        let mut add_q = false;
        ui.horizontal(|ui| {
            ui.label("View:");
            ui.selectable_value(&mut self.view, View::Xic, "Target XICs");
            ui.selectable_value(&mut self.view, View::Tic, "TIC");
            ui.selectable_value(&mut self.view, View::Bpc, "BPC");
            ui.selectable_value(&mut self.view, View::Quick, "Custom m/z (quick XIC)");
            ui.separator();
            if self.view == View::Xic {
                egui::ComboBox::new("scope", "")
                    .width(330.0)
                    .selected_text(match self.scope {
                        Scope::Ions => "Selected target, all ions, current file (edit integration)",
                        Scope::AllTargets => "All targets (quant ions), current file",
                        Scope::Files => "Selected target (quant ion), all selected files",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.scope,
                            Scope::Ions,
                            "Selected target, all ions, current file (edit integration)",
                        );
                        ui.selectable_value(
                            &mut self.scope,
                            Scope::AllTargets,
                            "All targets (quant ions), current file",
                        );
                        ui.selectable_value(
                            &mut self.scope,
                            Scope::Files,
                            "Selected target (quant ion), all selected files",
                        );
                    });
            } else if self.view == View::Quick {
                ui.label("m/z");
                ui.add(dv_mz(&mut self.quick_mz).speed(0.01).range(0.0..=100000.0));
                ui.label(format!(
                    "± {}{}",
                    fmt::sci(self.settings.tol),
                    if self.settings.tol_ppm { " ppm" } else { " Da" }
                ));
                ui.label("level");
                ui.selectable_value(&mut self.quick_level, 1u8, "MS1");
                ui.selectable_value(&mut self.quick_level, 2u8, "MS2");
                if ui.button("Add as target").clicked() {
                    add_q = true;
                }
            } else {
                ui.label("Scans:");
                ui.selectable_value(&mut self.tic_level, 1u8, "MS1");
                ui.selectable_value(&mut self.tic_level, 2u8, "MS2");
            }
            ui.checkbox(&mut self.normalize, "Normalize");
            if !self.analytes.is_empty()
                && self.view == View::Xic
                && self.scope != Scope::AllTargets
            {
                ui.separator();
                ui.label("Target:");
                egui::ComboBox::new("an", "")
                    .selected_text(self.analytes[self.sel_a].name.clone())
                    .show_ui(ui, |ui| {
                        for i in 0..self.analytes.len() {
                            let n = self.analytes[i].name.clone();
                            ui.selectable_value(&mut self.sel_a, i, n);
                        }
                    });
            }
            ui.label(format!(
                "Current file: {}",
                cur.map(|s| self.samples[s].data.name.clone())
                    .unwrap_or_else(|| "none".into())
            ));
        });
        if add_q {
            self.add_quick_target();
        }
        let needs_cur = self.view == View::Xic && self.scope != Scope::Files;
        if self.view == View::Xic && self.analytes.is_empty() {
            ui.label("Load a target table (.csv) to see target XICs. TIC and BPC views work without one.");
        } else if needs_cur && cur.is_none() {
            ui.label("No file selected.");
        } else if self.view == View::Xic && self.scope == Scope::Ions {
            if let Some(s) = cur {
                self.ui_plot(ui, s);
            }
        } else {
            self.ui_overlay(ui, cur);
        }
        if let Some(s) = cur {
            self.ui_shift(ui, s);
        }
    }
}
