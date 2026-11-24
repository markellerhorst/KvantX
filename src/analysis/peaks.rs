//! Peak finding: where does the peak of a target start and end?
//!
//! A [`PeakFinder`] gets the chromatogram of the quantifier ion and the expected retention time
//! and returns the boundaries of one peak. Finding and integrating are separate steps: the area
//! between the boundaries is computed by [`crate::analysis::integration`], so a finder only
//! decides about the boundaries. A new method implements the trait and is added to
//! [`finder`]; nothing else has to change.
//!
//! # Methods
//!
//! * **Legacy** ([`auto_peak`]): the apex is the highest point (within the retention-time
//!   window); the boundaries are where the signal falls below 5 % of the apex height or rises
//!   again on two consecutive points.
//! * **Adaptive** ([`AdaptiveFinder`]): Savitzky-Golay smoothing, a robust noise estimate,
//!   candidate peaks ranked by prominence, boundaries where the smoothed first derivative falls
//!   back to the noise level, a signal-to-noise check and a straight baseline between the
//!   boundaries. See the README for the formulas.

use super::smoothing;
use crate::domain::types::PeakMethod;

/// The boundaries of a peak found by a [`PeakFinder`], in minutes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Peak {
    /// Retention time where the integration starts.
    pub start: f64,
    /// Retention time where the integration ends.
    pub end: f64,
    /// Retention time of the highest point.
    pub apex: f64,
}

/// A method that finds the boundaries of one chromatographic peak.
pub trait PeakFinder: Sync {
    /// Name of the method for menus and the audit trail.
    fn name(&self) -> &'static str;

    /// Whether the area is integrated above a straight baseline between the boundaries.
    fn subtracts_baseline(&self) -> bool;

    /// Finds the peak of a target in `trace` (`[time, intensity]`, sorted by time).
    ///
    /// With `expected_rt` only a peak within `rt_window` minutes of it is considered, otherwise
    /// the strongest peak of the trace.
    fn find(&self, trace: &[[f64; 2]], expected_rt: Option<f64>, rt_window: f64) -> Option<Peak>;
}

/// The peak finder that belongs to a method.
#[must_use]
pub fn finder(method: PeakMethod) -> &'static dyn PeakFinder {
    static LEGACY: LegacyFinder = LegacyFinder;
    static ADAPTIVE: AdaptiveFinder = AdaptiveFinder { min_snr: 5.0 };
    match method {
        PeakMethod::Legacy => &LEGACY,
        PeakMethod::Adaptive => &ADAPTIVE,
    }
}

/// The original method: boundaries at 5 % of the apex height.
pub struct LegacyFinder;

impl PeakFinder for LegacyFinder {
    fn name(&self) -> &'static str {
        "Legacy (5 % of the apex height)"
    }

    fn subtracts_baseline(&self) -> bool {
        false
    }

    fn find(&self, trace: &[[f64; 2]], expected_rt: Option<f64>, rt_window: f64) -> Option<Peak> {
        let (start, end) = auto_peak(trace, expected_rt, rt_window)?;
        let apex = trace
            .iter()
            .filter(|p| p[0] >= start && p[0] <= end)
            .max_by(|a, b| a[1].total_cmp(&b[1]))
            .map(|p| p[0])?;
        Some(Peak { start, end, apex })
    }
}

/// Smoothing, noise and derivative based peak finder.
pub struct AdaptiveFinder {
    /// Smallest accepted signal-to-noise ratio of the peak height above the baseline. The usual
    /// limit of detection is 3 and of quantification 10 (ICH Q2). The method uses 5: it searches
    /// a whole run without a retention time when none is given, and among hundreds of noise bumps
    /// a few reach three standard deviations by chance.
    pub min_snr: f64,
}

/// Prominence of the local maximum `k` of `ys`: its height above the higher of the two lowest
/// points that lie between it and the next higher point on each side.
fn prominence(ys: &[f64], k: usize) -> f64 {
    let h = ys[k];
    let mut left_min = h;
    for &v in ys[..k].iter().rev() {
        if v > h {
            break;
        }
        left_min = left_min.min(v);
    }
    let mut right_min = h;
    for &v in &ys[k + 1..] {
        if v > h {
            break;
        }
        right_min = right_min.min(v);
    }
    h - left_min.max(right_min)
}

impl PeakFinder for AdaptiveFinder {
    fn name(&self) -> &'static str {
        "Adaptive (smoothing, noise and derivative)"
    }

    fn subtracts_baseline(&self) -> bool {
        true
    }

    fn find(&self, trace: &[[f64; 2]], expected_rt: Option<f64>, rt_window: f64) -> Option<Peak> {
        if trace.len() < 15 {
            return None;
        }
        let y: Vec<f64> = trace.iter().map(|p| p[1]).collect();
        let ys = smoothing::smooth(&y);
        let d = smoothing::derivative(trace);
        let y_max = ys.iter().cloned().fold(0.0f64, f64::max);
        // Noise-free data would give a noise level of zero; keep it above a tiny fraction of the signal.
        let sigma = smoothing::noise_sigma(&y, &ys).max(1e-6 * y_max);

        // Candidates: local maxima of the smoothed trace inside the retention-time window,
        // strongest prominence first.
        let mut candidates: Vec<(usize, f64)> = (4..trace.len() - 4)
            .filter(|&i| ys[i] > ys[i - 1] && ys[i] >= ys[i + 1])
            .filter(|&i| {
                expected_rt
                    .map(|rt| (trace[i][0] - rt).abs() <= rt_window)
                    .unwrap_or(true)
            })
            .map(|i| (i, prominence(&ys, i)))
            .filter(|&(_, p)| p >= self.min_snr * sigma)
            .collect();
        candidates.sort_by(|a, b| b.1.total_cmp(&a.1));

        for &(k, _) in candidates.iter().take(10) {
            if let Some(peak) = self.boundaries(trace, &ys, &d, k, sigma) {
                return Some(peak);
            }
        }
        None
    }
}

impl AdaptiveFinder {
    /// Boundaries of the peak around the local maximum `k`, or `None` if its signal-to-noise
    /// ratio above the baseline between the boundaries is too low.
    fn boundaries(
        &self,
        trace: &[[f64; 2]],
        ys: &[f64],
        d: &[f64],
        k: usize,
        sigma: f64,
    ) -> Option<Peak> {
        // Slope threshold: three times the noise of the derivative, but at least 1 % of the
        // steepest flank. The derivative coefficients have a squared sum of 1/28, so the
        // derivative of white noise has the standard deviation sigma / (sqrt(28) * dt).
        let dt = (trace[trace.len() - 1][0] - trace[0][0]) / (trace.len() - 1) as f64;
        let sigma_slope = sigma / (28f64.sqrt() * dt.max(1e-12));
        let steepest = d.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        let threshold = (3.0 * sigma_slope).max(0.01 * steepest);

        // Walk away from the apex on each side. First the slope has to become steeper than the
        // threshold (the flank), then the boundary is the first point where it is flat again. If the
        // signal climbs again before a flank was seen (a neighbouring peak), the boundary is the lowest
        // point reached. Two equal points at the apex do not matter.
        let rise = (3.0 * sigma).max(0.005 * ys[k]);
        let mut left = k;
        let (mut flank, mut lowest) = (false, k);
        while left > 0 {
            let next = left - 1;
            if flank {
                if d[next] <= threshold {
                    left = next;
                    break;
                }
            } else if d[next] > threshold {
                flank = true;
            } else if ys[next] > ys[lowest] + rise {
                left = lowest;
                break;
            }
            if ys[next] < ys[lowest] {
                lowest = next;
            }
            left = next;
        }
        let mut right = k;
        let (mut flank, mut lowest) = (false, k);
        while right + 1 < trace.len() {
            let next = right + 1;
            if flank {
                if d[next] >= -threshold {
                    right = next;
                    break;
                }
            } else if d[next] < -threshold {
                flank = true;
            } else if ys[next] > ys[lowest] + rise {
                right = lowest;
                break;
            }
            if ys[next] < ys[lowest] {
                lowest = next;
            }
            right = next;
        }
        if right <= left {
            return None;
        }
        // Height above the straight line between the boundary points.
        let t = (trace[k][0] - trace[left][0]) / (trace[right][0] - trace[left][0]);
        let baseline = ys[left] + t * (ys[right] - ys[left]);
        if (ys[k] - baseline) < self.min_snr * sigma {
            return None;
        }
        Some(Peak {
            start: trace[left][0],
            end: trace[right][0],
            apex: trace[k][0],
        })
    }
}

/// Initial automatic boundaries around the tallest point (within hint +- win if RT known).
#[must_use]
pub fn auto_peak(x: &[[f64; 2]], hint: Option<f64>, win: f64) -> Option<(f64, f64)> {
    let (mut ai, mut best) = (None, 0.0);
    for (i, p) in x.iter().enumerate() {
        if let Some(h) = hint {
            if (p[0] - h).abs() > win {
                continue;
            }
        }
        if p[1] > best {
            best = p[1];
            ai = Some(i);
        }
    }
    let a = ai?;
    let mut l = a;
    let mut rises = 0;
    while l > 0 {
        let prev = x[l - 1][1];
        if prev < 0.05 * best {
            l -= 1;
            break;
        }
        if prev > x[l][1] {
            rises += 1;
            if rises >= 2 {
                l += 1;
                break;
            }
        } else {
            rises = 0;
        }
        l -= 1;
    }
    let mut r = a;
    rises = 0;
    while r + 1 < x.len() {
        let next = x[r + 1][1];
        if next < 0.05 * best {
            r += 1;
            break;
        }
        if next > x[r][1] {
            rises += 1;
            if rises >= 2 {
                r -= 1;
                break;
            }
        } else {
            rises = 0;
        }
        r += 1;
    }
    if r <= l {
        return None;
    }
    Some((x[l][0], x[r][0]))
}
