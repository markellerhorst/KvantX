//! Geometry and data helpers of the plots.
//!
//! Finding the data point or peak nearest to the cursor, the zoom window, the mouse-wheel
//! scaling and the text of a scan. None of this draws anything, so it can be tested without a
//! window; the plots in [`crate::app`] call these functions.

use crate::domain::MzData;
pub use crate::domain::types::Boundary;
use crate::support::fmt;
use egui_plot::{PlotBounds, PlotPoint};

/// One trace of a chromatogram plot.
pub struct Series {
    /// Legend text.
    pub name: String,
    /// Points `[retention time, intensity]`, sorted by time.
    pub data: Vec<[f64; 2]>,
    /// Index into the color palette.
    pub ci: usize,
    /// Whether this is the trace of the current file (drawn thicker).
    pub cur: bool,
    /// Index of the file the trace belongs to.
    pub file: usize,
    /// 1 if the points index the MS1 scan list of the file, 2 for the MS2 scan list.
    pub level: u8,
    /// Name of the file, for the read-out.
    pub fname: String,
    /// Description of the trace (target and m/z), for the read-out.
    pub desc: String,
    /// Factor applied for display (100/maximum when normalized, otherwise 1).
    pub scale: f64,
}

/// The data point nearest to the cursor in a chromatogram plot.
pub struct Hover {
    /// Index of the file.
    pub file: usize,
    /// Retention time of the point.
    pub rt: f64,
    /// Intensity of the point (unscaled).
    pub y: f64,
    /// Display scale of its trace.
    pub scale: f64,
    /// Index of the point in the trace, which is also the index of the scan.
    pub idx: usize,
    /// 1 for an MS1 trace, 2 for an MS2 trace.
    pub level: u8,
    /// Name of the file.
    pub fname: String,
    /// Description of the trace.
    pub desc: String,
}

/// Point of any series closest to the cursor: nearest in time, then the trace whose intensity is closest to the cursor.
pub fn nearest_hover(series: &[Series], px: f64, py: f64, width: f64) -> Option<Hover> {
    let mut best: Option<(f64, Hover)> = None;
    for s in series {
        if s.data.is_empty() {
            continue;
        }
        let j = s.data.partition_point(|p| p[0] < px);
        let (c0, c1) = (j.saturating_sub(1), j.min(s.data.len() - 1));
        let bj = if (s.data[c1][0] - px).abs() < (s.data[c0][0] - px).abs() {
            c1
        } else {
            c0
        };
        let p = s.data[bj];
        if width > 0.0 && (p[0] - px).abs() > 0.05 * width {
            continue;
        }
        let dy = (p[1] * s.scale - py).abs();
        if best.as_ref().map(|b| dy < b.0).unwrap_or(true) {
            best = Some((
                dy,
                Hover {
                    file: s.file,
                    rt: p[0],
                    y: p[1],
                    scale: s.scale,
                    idx: bj,
                    level: s.level,
                    fname: s.fname.clone(),
                    desc: s.desc.clone(),
                },
            ));
        }
    }
    best.map(|b| b.1)
}

/// Precursor m/z for display: four decimals or "n/a".
pub fn prec_str(p: Option<f64>) -> String {
    p.map(fmt::mz4).unwrap_or_else(|| "n/a".into())
}

/// The `n` most intense peaks (m/z, intensity), strongest first.
pub fn top_peaks(mz: &[f64], inten: &[f32], n: usize) -> Vec<(f64, f64)> {
    let mut v: Vec<(f64, f64)> = mz
        .iter()
        .cloned()
        .zip(inten.iter().map(|&i| i as f64))
        .collect();
    v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    v.truncate(n);
    v
}

/// Peak (m/z, scaled intensity) closest to the cursor: plot-normalised distance from the cursor to the peak's stem.
pub fn nearest_peak(
    mz: &[f64],
    inten: &[f32],
    scale: f64,
    px: f64,
    py: f64,
    xw: f64,
    yw: f64,
) -> Option<(f64, f64)> {
    if mz.is_empty() || xw <= 0.0 || yw <= 0.0 {
        return None;
    }
    let lo = mz.partition_point(|&m| m < px - 0.08 * xw);
    let mut best: Option<(f64, f64, f64)> = None;
    for i in lo..mz.len() {
        if mz[i] > px + 0.08 * xw {
            break;
        }
        let y = inten[i] as f64 * scale;
        let dx = (mz[i] - px).abs() / xw;
        let dy0 = if py > y {
            py - y
        } else if py < 0.0 {
            -py
        } else {
            0.0
        };
        let dy = dy0 / yw;
        let d = (dx * dx + dy * dy).sqrt();
        if best.map(|b| d < b.0).unwrap_or(true) {
            best = Some((d, mz[i], y));
        }
    }
    best.map(|b| (b.1, b.2))
}

/// "MS2 scan 123 · precursor m/z 286.1438" (or "n/a"), "MS1 scan 45".
pub fn scan_info(d: &MzData, l: u8, j: usize) -> String {
    if l == 1 {
        format!("MS1 scan {}", d.ms1.get(j).map(|x| x.scan).unwrap_or(0))
    } else {
        match d.spectra.get(j) {
            Some(x) => format!(
                "MS2 scan {} · precursor m/z {}",
                x.scan,
                prec_str(x.precursor)
            ),
            None => "MS2 scan ?".into(),
        }
    }
}

/// Integration boundary closest to the pointer, if one is within `max_px` pixels.
///
/// `px_per_unit` converts the distance in plot units (minutes) to screen pixels.
pub fn nearest_boundary(
    px: f64,
    start: f64,
    end: f64,
    px_per_unit: f64,
    max_px: f64,
) -> Option<Boundary> {
    let (ds, de) = (
        (px - start).abs() * px_per_unit,
        (px - end).abs() * px_per_unit,
    );
    if ds.min(de) > max_px {
        None
    } else if ds <= de {
        Some(Boundary::Start)
    } else {
        Some(Boundary::End)
    }
}

/// Zoom window: the rectangle between the drag start and the pointer, if it is large enough to be intended (>1 % of the view in both directions).
pub fn zoom_box(start: [f64; 2], end: [f64; 2], xw: f64, yw: f64) -> Option<([f64; 2], [f64; 2])> {
    let (x0, x1) = (start[0].min(end[0]), start[0].max(end[0]));
    let (y0, y1) = (start[1].min(end[1]), start[1].max(end[1]));
    if x1 - x0 > 0.01 * xw && y1 - y0 > 0.01 * yw {
        Some(([x0, y0], [x1, y1]))
    } else {
        None
    }
}

/// True if the point lies inside the visible data area of a plot (not on the axes or outside the plot).
pub fn in_view(p: &PlotPoint, b: &PlotBounds) -> bool {
    p.x >= b.min()[0] && p.x <= b.max()[0] && p.y >= b.min()[1] && p.y <= b.max()[1]
}

/// Scales the interval [lo, hi] by `factor` around `pivot` (factor < 1 zooms in); the pivot keeps its relative position.
pub fn scale_range(lo: f64, hi: f64, pivot: f64, factor: f64) -> (f64, f64) {
    (pivot - (pivot - lo) * factor, pivot + (hi - pivot) * factor)
}
