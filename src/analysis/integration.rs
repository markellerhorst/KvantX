//! Numerical integration of a chromatographic peak.

/// Intensity of the trace `x` at time `t`, interpolated linearly; 0 outside the trace.
fn y_at(x: &[[f64; 2]], t: f64) -> f64 {
    for w in x.windows(2) {
        if t >= w[0][0] && t <= w[1][0] && w[1][0] > w[0][0] {
            return w[0][1] + (w[1][1] - w[0][1]) * (t - w[0][0]) / (w[1][0] - w[0][0]);
        }
    }
    0.0
}

/// Trapezoidal area over [a,b] in intensity*seconds, boundaries interpolated exactly.
#[must_use]
pub fn integrate(x: &[[f64; 2]], a: f64, b: f64, baseline: bool) -> f64 {
    let mut area = 0.0;
    for w in x.windows(2) {
        let (x0, y0, x1, y1) = (w[0][0], w[0][1], w[1][0], w[1][1]);
        if x1 <= a || x0 >= b || x1 <= x0 {
            continue;
        }
        let (xa, xb) = (x0.max(a), x1.min(b));
        let ya = y0 + (y1 - y0) * (xa - x0) / (x1 - x0);
        let yb = y0 + (y1 - y0) * (xb - x0) / (x1 - x0);
        area += 0.5 * (ya + yb) * (xb - xa);
    }
    if baseline {
        area -= 0.5 * (y_at(x, a) + y_at(x, b)) * (b - a);
    }
    area * 60.0
}

/// Outline of the area under the trace between `a` and `b`, for drawing the integrated region.
#[must_use]
pub fn shade(x: &[[f64; 2]], a: f64, b: f64) -> Vec<[f64; 2]> {
    let mut p = vec![[a, 0.0], [a, y_at(x, a)]];
    for q in x {
        if q[0] > a && q[0] < b {
            p.push(*q);
        }
    }
    p.push([b, y_at(x, b)]);
    p.push([b, 0.0]);
    p
}
