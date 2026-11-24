//! The MS2 spectrum panel.
use crate::gui::widgets::{color_rgb, enlarge_plot_fonts, pal, plot_outer_rect, safe_name};

use crate::app::state::{App, ExportKind};
use crate::formats::csv;
use crate::formats::svg::{SvgLabel, SvgPlot, SvgSeries, SvgVLine};
use crate::gui::plotting;
use crate::support::fmt;
use eframe::egui::{self, Color32, PointerButton, RichText};
use egui_plot::{Line, Plot, PlotBounds, PlotPoint, PlotPoints, Points, Polygon, Text, VLine};

impl App {
    /// Second graph: MS2 spectrum (stems; target product ions highlighted). The cursor snaps to the nearest peak.
    pub(crate) fn ui_spectrum(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.strong("MS2 spectrum");
            ui.checkbox(&mut self.follow_hover, "follow chromatogram hover");
            ui.checkbox(&mut self.spec_autofit, "auto-fit when the scan changes");
            if self.pinned_scan.is_some() && ui.button("Unpin scan").clicked() { self.pinned_scan = None; }
            ui.weak("drag a box = zoom · Shift/middle-drag = pan · Alt+wheel = scale y · Shift+Alt+wheel = scale x · right-click = reset zoom / save");
        });
        let (f, i, why) = match self.spectrum_pick() {
            Some(x) => x,
            None => {
                ui.label("No MS2 scan to show: select a file and a target with a detected peak, or click the chromatogram to pin a scan.");
                return;
            }
        };
        let mut export_req: Option<(ExportKind, SvgPlot, egui::Rect, String)> = None;
        let mut csv_req: Option<(String, String)> = None;
        // the view is re-fitted when the scan changes (if "auto-fit" is on) or when "Reset zoom" was chosen; otherwise a zoom window stays
        let reset_now = std::mem::take(&mut self.reset_spec)
            || (self.spec_autofit && self.spec_last != Some((f, i)));
        let alt = ui.input(|i| i.modifiers.alt);
        self.spec_last = Some((f, i));
        let mut zoom_s = self.zoom_spec;
        let mut zoomed_spec = false;
        let mut reset_req = false;
        {
            let sm = &self.samples[f];
            let sp = &sm.data.spectra[i];
            let norm = self.normalize;
            let capturing = self.capture.is_some();
            let mx = sp.inten.iter().cloned().fold(0.0f32, f32::max) as f64;
            let scale = if norm && mx > 0.0 { 100.0 / mx } else { 1.0 };
            let rt_scan = sp.rt + sm.rt_shift;
            let head = format!(
                "{} · MS2 scan {} · RT {:.2} min · precursor m/z {} · {} peaks · shown: {}",
                sm.data.name,
                sp.scan,
                rt_scan,
                plotting::prec_str(sp.precursor),
                sp.mz.len(),
                why
            );
            ui.label(&head);
            let mut all: Vec<[f64; 2]> = Vec::with_capacity(sp.mz.len() * 3);
            for (m, y) in sp.mz.iter().zip(sp.inten.iter()) {
                let y = *y as f64 * scale;
                all.push([*m, 0.0]);
                all.push([*m, y]);
                all.push([*m, 0.0]);
            }
            let mut hl: Vec<(usize, f64, Vec<[f64; 2]>)> = vec![];
            if self.sel_a < self.analytes.len() {
                for (k, &ion) in self.analytes[self.sel_a].ions.iter().enumerate() {
                    let tol = self.tol_da(ion);
                    let mut v = vec![];
                    for (m, y) in sp.mz.iter().zip(sp.inten.iter()) {
                        if (*m - ion).abs() <= tol {
                            let y = *y as f64 * scale;
                            v.push([*m, 0.0]);
                            v.push([*m, y]);
                            v.push([*m, 0.0]);
                        }
                    }
                    hl.push((k, ion, v));
                }
            }
            let top10 = plotting::top_peaks(&sp.mz, &sp.inten, 10);
            if !hl.is_empty() {
                ui.horizontal(|ui| {
                    for (k, ion, v) in &hl {
                        ui.colored_label(
                            pal(*k),
                            format!(
                                "■ target ion m/z {}{}",
                                fmt::mz4(*ion),
                                if v.is_empty() {
                                    " (not in this scan)"
                                } else {
                                    ""
                                }
                            ),
                        );
                    }
                });
            }
            let plot_style = enlarge_plot_fonts(ui);
            let mut plot = mzplot!(
                Plot::new("spec")
                    .height((ui.available_height() - 36.0).max(100.0))
                    .allow_drag(false)
                    .allow_boxed_zoom(false)
                    .allow_scroll(!alt)
                    .x_axis_label("m/z")
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
                pu.line(Line::new(PlotPoints::from(all.clone())).color(Color32::GRAY));
                for (k, ion, v) in &hl {
                    pu.vline(VLine::new(*ion).color(pal(*k).gamma_multiply(0.5)));
                    if !v.is_empty() {
                        pu.line(
                            Line::new(PlotPoints::from(v.clone()))
                                .color(pal(*k))
                                .width(2.5_f32),
                        );
                    }
                }
                if capturing {
                    // figure export: label the 10 most intense peaks with their m/z
                    for (m, y) in &top10 {
                        pu.text(
                            Text::new(
                                PlotPoint::new(*m, *y * scale),
                                RichText::new(fmt::mz4(*m))
                                    .size(11.0)
                                    .color(Color32::BLACK)
                                    .background_color(Color32::from_rgba_unmultiplied(
                                        255, 255, 255, 215,
                                    )),
                            )
                            .anchor(egui::Align2::CENTER_BOTTOM),
                        );
                    }
                } else if let Some(p) = pu
                    .pointer_coordinate()
                    .filter(|p| pu.response().hovered() && plotting::in_view(p, &pu.plot_bounds()))
                {
                    // snap to the peak closest to the cursor and show only its RT and m/z
                    let b = pu.plot_bounds();
                    let (xw, yw) = (b.max()[0] - b.min()[0], b.max()[1] - b.min()[1]);
                    if let Some((m, y)) =
                        plotting::nearest_peak(&sp.mz, &sp.inten, scale, p.x, p.y, xw, yw)
                    {
                        let right = m > (b.min()[0] + b.max()[0]) / 2.0;
                        pu.line(
                            Line::new(PlotPoints::from(vec![[m, 0.0], [m, y]]))
                                .color(Color32::WHITE)
                                .width(3.0_f32),
                        );
                        pu.points(
                            Points::new(vec![[m, y]])
                                .radius(5.0_f32)
                                .color(Color32::WHITE),
                        );
                        pu.text(
                            Text::new(
                                PlotPoint::new(m, y),
                                RichText::new(format!(
                                    "RT {:.2} min\nm/z {}",
                                    rt_scan,
                                    fmt::mz4(m)
                                ))
                                .color(Color32::WHITE)
                                .background_color(Color32::from_black_alpha(200)),
                            )
                            .anchor(if right {
                                egui::Align2::RIGHT_BOTTOM
                            } else {
                                egui::Align2::LEFT_BOTTOM
                            }),
                        );
                    }
                }
                let r = pu.response().clone();
                if zoom_pan!(pu, r, zoom_s, false) {
                    zoomed_spec = true;
                }
            });
            let rect = plot_outer_rect(ui, pr.response.rect);
            ui.set_style(plot_style);
            let (x0, x1, y0, y1) = {
                let b = pr.transform.bounds();
                (b.min()[0], b.max()[0], b.min()[1], b.max()[1])
            };
            let names = vec!["MS2 spectrum (m/z, Intensity)".to_string()];
            let mut ex: Option<ExportKind> = None;
            export_menu!(pr.response, "MS2 spectrum", ex, &names);
            if let Some(k) = ex {
                let stem = format!("MS2_scan_{}_{}", sp.scan, safe_name(&sm.data.name));
                match k {
                    ExportKind::Reset => {
                        reset_req = true;
                    }
                    ExportKind::Csv(_) => {
                        csv_req = Some((
                            stem,
                            csv::two_columns(
                                "m/z",
                                "Intensity",
                                sp.mz
                                    .iter()
                                    .cloned()
                                    .zip(sp.inten.iter().map(|&v| v as f64)),
                            ),
                        ));
                    }
                    _ => {
                        let mut ss = vec![SvgSeries {
                            name: "peaks".into(),
                            rgb: (128, 128, 128),
                            width: 1.0,
                            points: all.clone(),
                            marker: false,
                        }];
                        let mut vl = vec![];
                        for (ci, ion, v) in &hl {
                            if !v.is_empty() {
                                ss.push(SvgSeries {
                                    name: format!("target ion m/z {}", fmt::mz4(*ion)),
                                    rgb: color_rgb(pal(*ci)),
                                    width: 2.0,
                                    points: v.clone(),
                                    marker: false,
                                });
                            }
                            vl.push(SvgVLine {
                                x: *ion,
                                rgb: color_rgb(pal(*ci)),
                                label: String::new(),
                            });
                        }
                        let labels: Vec<SvgLabel> = top10
                            .iter()
                            .map(|(m, y)| SvgLabel {
                                x: *m,
                                y: *y * scale,
                                text: fmt::mz4(*m),
                                rgb: (0, 0, 0),
                            })
                            .collect();
                        let yf: fn(f64) -> String = if norm { fmt::pct0 } else { fmt::sci3 };
                        export_req = Some((
                            k,
                            SvgPlot {
                                title: head.clone(),
                                x_label: "m/z".into(),
                                y_label: if norm {
                                    "Intensity (% of maximum)".into()
                                } else {
                                    "Intensity".into()
                                },
                                series: ss,
                                vlines: vl,
                                labels,
                                notes: vec![],
                                bounds: [x0, x1, y0, y1],
                                x_fmt: fmt::mz4,
                                y_fmt: yf,
                            },
                            rect,
                            stem,
                        ));
                    }
                }
            }
        }
        if let Some((k, plot, rect, stem)) = export_req {
            self.do_export(k, plot, rect, &stem);
        }
        self.zoom_spec = zoom_s;
        if zoomed_spec {
            self.spec_autofit = false;
        }
        if reset_req {
            self.reset_spec = true;
            ui.ctx().request_repaint();
        }
        if let Some((stem, text)) = csv_req {
            self.save_csv(&stem, &text);
        }
    }
}
