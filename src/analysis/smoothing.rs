//! Smoothing, derivatives and noise estimation of chromatographic traces.
//!
//! The Savitzky-Golay filter (Savitzky and Golay, Anal. Chem. 36 (1964) 1627) fits a quadratic
//! polynomial to a moving window by least squares. Its coefficients are fixed numbers, so the
//! filter is a plain weighted sum. The window has 7 points. The traces are treated as evenly
//! spaced; the spacing of real traces varies slightly with the scan cycle, which this ignores.

/// Half-width of the 7-point window.
const HALF: usize = 3;

/// Smoothing coefficients for a quadratic polynomial over 7 points: `(-2, 3, 6, 7, 6, 3, -2) / 21`.
const SMOOTH: [f64; 7] = [-2.0, 3.0, 6.0, 7.0, 6.0, 3.0, -2.0];

/// First-derivative coefficients for a quadratic polynomial over 7 points: `(-3 ... 3) / 28`
/// per sampling interval.
const DERIVATIVE: [f64; 7] = [-3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0];

/// Savitzky-Golay smoothed copy of the intensities `y`; the first and last three points are
/// copied unchanged.
#[must_use]
pub fn smooth(y: &[f64]) -> Vec<f64> {
    let mut out = y.to_vec();
    if y.len() <= 2 * HALF {
        return out;
    }
    for i in HALF..y.len() - HALF {
        let window = &y[i - HALF..=i + HALF];
        out[i] = window
            .iter()
            .zip(SMOOTH.iter())
            .map(|(v, c)| v * c)
            .sum::<f64>()
            / 21.0;
    }
    out
}

/// Savitzky-Golay first derivative of the trace `x` (`[time, intensity]`) in intensity per
/// minute; zero in the first and last three points.
#[must_use]
pub fn derivative(x: &[[f64; 2]]) -> Vec<f64> {
    let mut out = vec![0.0; x.len()];
    if x.len() <= 2 * HALF {
        return out;
    }
    for i in HALF..x.len() - HALF {
        let dt = (x[i + HALF][0] - x[i - HALF][0]) / (2 * HALF) as f64;
        if dt > 0.0 {
            let slope: f64 = (0..7)
                .map(|j| x[i + j - HALF][1] * DERIVATIVE[j])
                .sum::<f64>()
                / 28.0;
            out[i] = slope / dt;
        }
    }
    out
}

/// Median of `v` (0 for an empty slice).
#[must_use]
pub fn median(v: &[f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    let n = s.len();
    if n % 2 == 1 {
        s[n / 2]
    } else {
        0.5 * (s[n / 2 - 1] + s[n / 2])
    }
}

/// Robust noise level of a trace: `1.4826 * MAD / sqrt(2/3)` of the residuals `y - smooth(y)`.
///
/// `MAD = median(|r - median(r)|)`; the factor 1.4826 makes it equal to the standard deviation
/// for normally distributed values. The residuals of the 7-point filter have only `2/3` of the
/// variance of the noise itself, which the last factor corrects. A peak hardly changes the median,
/// so unlike the standard deviation this estimate is not inflated by the peaks in the trace.
#[must_use]
pub fn noise_sigma(y: &[f64], smoothed: &[f64]) -> f64 {
    let residual: Vec<f64> = y.iter().zip(smoothed.iter()).map(|(a, b)| a - b).collect();
    let m = median(&residual);
    let deviations: Vec<f64> = residual.iter().map(|r| (r - m).abs()).collect();
    1.4826 * median(&deviations) / (2.0f64 / 3.0).sqrt()
}
