//! Macros that build the plots. They are macros because the plot types of egui_plot cannot be named in a function signature.

/// Scientific-notation styling for every plot (axes + hover text).
macro_rules! sciplot {
    ($p:expr) => {
        $p.x_axis_formatter(|m, _| fmt::sci(m.value))
            .y_axis_formatter(|m, _| fmt::sci3(m.value))
            .y_axis_min_width(85.0)
            .label_formatter(|name, v| {
                let head = if name.is_empty() {
                    String::new()
                } else {
                    format!("{}\n", name)
                };
                format!("{}x = {}\ny = {}", head, fmt::sci(v.x), fmt::sci(v.y))
            })
    };
}

/// Chromatogram plots: retention-time axis with two decimals; y axis in compact scientific notation, or 0-100 % when normalised.
/// The y axis gets a minimum width so that the axis title never collides with the tick labels.
macro_rules! rtplot {
    ($p:expr, $norm:expr) => {{
        let mut p = $p
            .x_axis_formatter(|m, _| format!("{:.2}", m.value))
            .y_axis_formatter(move |m, _| {
                if $norm {
                    fmt::pct0(m.value)
                } else {
                    fmt::sci3(m.value)
                }
            })
            .y_axis_min_width(85.0)
            .show_x(false)
            .show_y(false);
        if $norm {
            p = p
                .include_y(0.0)
                .include_y(100.0)
                .set_margin_fraction(egui::vec2(0.0, 0.05));
        }
        p
    }};
}

/// Spectrum plots: m/z with four decimals; no default cursor label (the snapped peak is drawn by hand).
macro_rules! mzplot {
    ($p:expr, $norm:expr) => {{
        let mut p = $p
            .x_axis_formatter(|m, _| fmt::axis_mz(m.value, m.step_size))
            .y_axis_formatter(move |m, _| {
                if $norm {
                    fmt::pct0(m.value)
                } else {
                    fmt::sci3(m.value)
                }
            })
            .y_axis_min_width(85.0)
            .show_x(false)
            .show_y(false);
        if $norm {
            p = p
                .include_y(0.0)
                .include_y(100.0)
                .set_margin_fraction(egui::vec2(0.0, 0.05));
        }
        p
    }};
}

/// Draws the traces, finds the point nearest to the cursor and annotates it with m/z, RT, scan (and precursor) and intensity.
/// `$cap`: true while a PNG capture is pending (no hover annotation in the picture).
macro_rules! draw_series {
    ($pu:ident, $series:expr, $hover:ident, $scan_of:ident, $cap:expr) => {{
        for s in $series.iter() {
            let d: Vec<[f64; 2]> = s.data.iter().map(|p| [p[0], p[1] * s.scale]).collect();
            let mut l = Line::new(PlotPoints::from(d))
                .name(s.name.clone())
                .color(pal(s.ci));
            if s.cur {
                l = l.width(2.5_f32);
            }
            $pu.line(l);
        }
        if !$cap {
            if let Some(p) = $pu.pointer_coordinate() {
                let b = $pu.plot_bounds();
                if $pu.response().hovered() && plotting::in_view(&p, &b) {
                    $hover = plotting::nearest_hover($series, p.x, p.y, b.max()[0] - b.min()[0]);
                }
            }
        }
        if let Some(h) = &$hover {
            let b = $pu.plot_bounds();
            let right = h.rt > (b.min()[0] + b.max()[0]) / 2.0;
            let pct = if h.scale != 1.0 {
                format!(" ({:.1}%)", h.y * h.scale)
            } else {
                String::new()
            };
            let txt = format!(
                "{}\n{}\nRT {:.2} min · {}\nintensity {}{}",
                h.fname,
                h.desc,
                h.rt,
                $scan_of(h.file, h.level, h.idx),
                fmt::sci(h.y),
                pct
            );
            $pu.points(
                Points::new(vec![[h.rt, h.y * h.scale]])
                    .radius(5.0_f32)
                    .color(Color32::WHITE),
            );
            $pu.text(
                Text::new(
                    PlotPoint::new(h.rt, h.y * h.scale),
                    RichText::new(txt)
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
    }};
}

/// Mouse navigation of a plot: drag a box with the left button to zoom to it, Shift+left-drag or middle-drag pans,
/// Alt+wheel scales the y axis and Shift+Alt+wheel the x axis (about the cursor). Everything only reacts while the pointer is
/// inside the data area of the plot. `$blocked`: true while the left button is busy with something else (moving a boundary).
/// Evaluates to true when the view was zoomed by a box.
macro_rules! zoom_pan {
    ($pu:ident, $r:ident, $zoom:ident, $blocked:expr) => {{
        let (alt, shift) = $r.ctx.input(|i| (i.modifiers.alt, i.modifiers.shift));
        let mut zoomed = false;
        let inside = $r.hovered()
            && $pu
                .pointer_coordinate()
                .map(|p| plotting::in_view(&p, &$pu.plot_bounds()))
                .unwrap_or(false);
        if alt && inside {
            let raw = $r.ctx.input(|i| i.raw_scroll_delta);
            let d = (if raw.y != 0.0 { raw.y } else { raw.x }) as f64;
            if d != 0.0 {
                if let Some(p) = $pu.pointer_coordinate() {
                    let b = $pu.plot_bounds();
                    let f = (-d * 0.003).exp();
                    let (mut x0, mut x1, mut y0, mut y1) =
                        (b.min()[0], b.max()[0], b.min()[1], b.max()[1]);
                    if shift {
                        let (n0, n1) = plotting::scale_range(x0, x1, p.x, f);
                        x0 = n0;
                        x1 = n1;
                    } else {
                        let (n0, n1) = plotting::scale_range(y0, y1, p.y, f);
                        y0 = n0;
                        y1 = n1;
                    }
                    $pu.set_plot_bounds(PlotBounds::from_min_max([x0, y0], [x1, y1]));
                }
            }
        }
        if $r.dragged_by(PointerButton::Middle)
            || ($r.dragged_by(PointerButton::Primary) && shift && !alt)
        {
            let d = $pu.pointer_coordinate_drag_delta();
            $pu.translate_bounds(-d);
        } else if let Some(p) = $pu.pointer_coordinate() {
            let b0 = $pu.plot_bounds();
            let cp = [
                p.x.max(b0.min()[0]).min(b0.max()[0]),
                p.y.max(b0.min()[1]).min(b0.max()[1]),
            ];
            if $r.drag_started_by(PointerButton::Primary) && !$blocked && plotting::in_view(&p, &b0)
            {
                $zoom = Some([p.x, p.y]);
            }
            if $r.dragged_by(PointerButton::Primary) {
                if let Some(s0) = $zoom {
                    let (x0, x1) = (s0[0].min(cp[0]), s0[0].max(cp[0]));
                    let (y0, y1) = (s0[1].min(cp[1]), s0[1].max(cp[1]));
                    $pu.polygon(
                        Polygon::new(PlotPoints::from(vec![
                            [x0, y0],
                            [x1, y0],
                            [x1, y1],
                            [x0, y1],
                        ]))
                        .fill_color(Color32::from_rgba_unmultiplied(100, 150, 255, 45))
                        .stroke(egui::Stroke::new(1.5_f32, Color32::from_rgb(100, 150, 255))),
                    );
                }
            }
            if $r.drag_stopped() {
                if let Some(s0) = $zoom.take() {
                    let (xw, yw) = (b0.max()[0] - b0.min()[0], b0.max()[1] - b0.min()[1]);
                    if let Some((lo, hi)) = plotting::zoom_box(s0, cp, xw, yw) {
                        $pu.set_plot_bounds(PlotBounds::from_min_max(lo, hi));
                        zoomed = true;
                    }
                }
            }
        }
        zoomed
    }};
}

/// Right-click menu of a plot: reset zoom, save as PNG / SVG, or one trace as a two-column CSV.
macro_rules! export_menu {
    ($resp:expr, $what:expr, $out:ident, $names:expr) => {
        $resp.context_menu(|ui| {
            if ui.button("Reset zoom").clicked() {
                $out = Some(ExportKind::Reset);
                ui.close_menu();
            }
            ui.weak("drag a box = zoom · Shift/middle-drag = pan");
            ui.weak("Alt+wheel = scale y · Shift+Alt+wheel = scale x");
            ui.separator();
            if ui.button(format!("Save {} as PNG…", $what)).clicked() {
                $out = Some(ExportKind::Png);
                ui.close_menu();
            }
            if ui.button(format!("Save {} as SVG…", $what)).clicked() {
                $out = Some(ExportKind::Svg);
                ui.close_menu();
            }
            ui.separator();
            for (i, n) in $names.iter().enumerate().take(20) {
                if ui
                    .button(format!("Save '{}' as CSV (2 columns)…", n))
                    .clicked()
                {
                    $out = Some(ExportKind::Csv(i));
                    ui.close_menu();
                }
            }
        })
    };
}
