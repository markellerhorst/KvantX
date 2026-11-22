//! Calibration by weighted linear regression.

/// Result of a calibration regression: `area = m * conc + b`.
#[derive(Clone, Copy)]
pub struct Fit {
    pub m: f64,
    pub b: f64,
    pub r2: f64,
    pub n: usize,
    pub lo: f64,
    pub hi: f64,
}

/// Weighted linear regression area = m*conc + b. weighting: 0 none, 1 = 1/x, 2 = 1/x^2. pts = (conc, area)
pub fn fit(pts: &[(f64, f64)], weighting: u8) -> Option<Fit> {
    let mut lv: Vec<f64> = pts.iter().map(|p| p.0).collect();
    lv.sort_by(|a, b| a.total_cmp(b));
    lv.dedup();
    if lv.len() < 2 {
        return None;
    }
    let (mut sw, mut swx, mut swy, mut swxx, mut swxy) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for &(x, y) in pts {
        let w = match weighting {
            1 => 1.0 / x,
            2 => 1.0 / (x * x),
            _ => 1.0,
        };
        sw += w;
        swx += w * x;
        swy += w * y;
        swxx += w * x * x;
        swxy += w * x * y;
    }
    let den = sw * swxx - swx * swx;
    if den.abs() < 1e-300 {
        return None;
    }
    let m = (sw * swxy - swx * swy) / den;
    let b = (swy - m * swx) / sw;
    let ybar = pts.iter().map(|p| p.1).sum::<f64>() / pts.len() as f64;
    let ssr: f64 = pts.iter().map(|p| (p.1 - (m * p.0 + b)).powi(2)).sum();
    let sst: f64 = pts.iter().map(|p| (p.1 - ybar).powi(2)).sum();
    Some(Fit {
        m,
        b,
        r2: if sst > 0.0 { 1.0 - ssr / sst } else { 1.0 },
        n: pts.len(),
        lo: lv[0],
        hi: lv[lv.len() - 1],
    })
}
