//! Retention-time alignment of runs by cross-correlation of their total-ion chromatograms.

/// Resamples a trace on a regular time grid by linear interpolation; zero outside the trace.
fn resample(x: &[[f64; 2]], t0: f64, step: f64, n: usize) -> Vec<f64> {
    let mut out = vec![0.0; n];
    let mut j = 0usize;
    for i in 0..n {
        let t = t0 + i as f64 * step;
        if t < x[0][0] || t > x[x.len() - 1][0] {
            continue;
        }
        while j + 1 < x.len() && x[j + 1][0] < t {
            j += 1;
        }
        if j + 1 >= x.len() {
            out[i] = x[j][1];
            continue;
        }
        let (x0, y0, x1, y1) = (x[j][0], x[j][1], x[j + 1][0], x[j + 1][1]);
        out[i] = if x1 > x0 {
            y0 + (y1 - y0) * (t - x0) / (x1 - x0)
        } else {
            y0
        };
    }
    out
}

/// Removes the slowly varying baseline: subtracts a moving average of half-width `w` points.
fn highpass(v: &mut Vec<f64>, w: usize) {
    let n = v.len();
    let mut pre = vec![0.0; n + 1];
    for i in 0..n {
        pre[i + 1] = pre[i] + v[i];
    }
    let orig = v.clone();
    for i in 0..n {
        let lo = i.saturating_sub(w);
        let hi = (i + w + 1).min(n);
        v[i] = orig[i] - (pre[hi] - pre[lo]) / (hi - lo) as f64;
    }
}

/// Global RT shift (minutes) to ADD to x's retention times so that it best overlays r.
/// Cross-correlation of baseline-removed traces on a 0.01 min grid, lags within +-max.
#[must_use]
pub fn estimate_shift(r: &[[f64; 2]], x: &[[f64; 2]], max: f64) -> f64 {
    if r.len() < 5 || x.len() < 5 {
        return 0.0;
    }
    if r.iter()
        .chain(x.iter())
        .any(|p| !p[0].is_finite() || !p[1].is_finite())
    {
        return 0.0;
    }
    let step = 0.01;
    let t0 = r[0][0].min(x[0][0]) - max;
    let t1 = r[r.len() - 1][0].max(x[x.len() - 1][0]) + max;
    // Retention times come from a file that is not trusted: refuse absurd ranges before allocating.
    let span = (t1 - t0) / step;
    if !span.is_finite() || span < 9.0 || span > 200_000.0 {
        return 0.0;
    }
    let n = span as usize + 1;
    let mut a = resample(r, t0, step, n);
    let mut b = resample(x, t0, step, n);
    highpass(&mut a, 50);
    highpass(&mut b, 50);
    let lags = (max / step).round() as i64;
    let (mut best, mut best_lag) = (f64::MIN, 0i64);
    for lag in -lags..=lags {
        let mut s = 0.0;
        for i in 0..n {
            let k = i as i64 - lag;
            if k < 0 || k >= n as i64 {
                continue;
            }
            s += a[i] * b[k as usize];
        }
        if s > best {
            best = s;
            best_lag = lag;
        }
    }
    best_lag as f64 * step
}
