//! Small widgets, the color palette of the plots and the conversion of plot data for export.

use crate::app::state::ExportKind;
use crate::formats::svg::{SvgPlot, SvgSeries};
use crate::gui::plotting::Series;
use crate::support::fmt;
use eframe::egui::{self, Color32};

/// Returns the `i`-th color of the plot palette, starting over after the last one.
pub(crate) fn pal(i: usize) -> Color32 {
    const P: [(u8, u8, u8); 10] = [
        (31, 119, 180),
        (214, 39, 40),
        (44, 160, 44),
        (255, 127, 14),
        (148, 103, 189),
        (140, 86, 75),
        (227, 119, 194),
        (127, 127, 127),
        (188, 189, 34),
        (23, 190, 207),
    ];
    let c = P[i % 10];
    Color32::from_rgb(c.0, c.1, c.2)
}

/// DragValue that shows and parses scientific notation.
pub(crate) fn dv(v: &mut f64) -> egui::DragValue<'_> {
    egui::DragValue::new(v)
        .custom_formatter(|n, _| fmt::sci(n))
        .custom_parser(fmt::parse_sci)
}

/// DragValue for retention times / durations in minutes: two decimals ("7.20").
pub(crate) fn dv_rt(v: &mut f64) -> egui::DragValue<'_> {
    egui::DragValue::new(v)
        .custom_formatter(|n, _| format!("{:.2}", n))
        .custom_parser(fmt::parse_sci)
}

/// DragValue for m/z values: four decimals.
pub(crate) fn dv_mz(v: &mut f64) -> egui::DragValue<'_> {
    egui::DragValue::new(v)
        .custom_formatter(|n, _| fmt::mz4(n))
        .custom_parser(fmt::parse_sci)
}

/// DragValue for standard concentrations: "–" means "not a standard for this target".
pub(crate) fn dv_conc(v: &mut f64) -> egui::DragValue<'_> {
    egui::DragValue::new(v)
        .custom_formatter(|n, _| {
            if n == 0.0 {
                "–".to_string()
            } else {
                fmt::sci(n)
            }
        })
        .custom_parser(|s| {
            if s.trim().is_empty() || s.trim() == "–" || s.trim() == "-" {
                Some(0.0)
            } else {
                fmt::parse_sci(s)
            }
        })
}

/// Stores a calibration-plot export request (kind, figure, plot area, suggested file name) in `slot`.
pub(crate) fn export_req_cal(
    slot: &mut Option<(ExportKind, SvgPlot, egui::Rect, String)>,
    k: ExportKind,
    plot: SvgPlot,
    rect: egui::Rect,
    stem: String,
) {
    *slot = Some((k, plot, rect, stem));
}

/// Converts an egui color to an RGB triple for SVG output.
pub(crate) fn color_rgb(c: Color32) -> (u8, u8, u8) {
    (c.r(), c.g(), c.b())
}

/// Replaces everything except ASCII letters, digits, `-` and `.` by `_`, so that the text can be used in a file name.
pub(crate) fn safe_name(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Converts plot series to SVG series, applying the display scale (for example 100/max when normalized).
pub(crate) fn svg_series(series: &[Series]) -> Vec<SvgSeries> {
    series
        .iter()
        .map(|s| SvgSeries {
            name: s.name.clone(),
            rgb: color_rgb(pal(s.ci)),
            width: if s.cur { 2.5 } else { 1.5 },
            points: s.data.iter().map(|p| [p[0], p[1] * s.scale]).collect(),
            marker: false,
        })
        .collect()
}
