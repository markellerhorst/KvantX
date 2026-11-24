//! Peak finding: the legacy method, the adaptive method and the smoothing it is built on.

use kvantx::analysis::integration;
use kvantx::analysis::peaks::{self, AdaptiveFinder, LegacyFinder, PeakFinder};
use kvantx::analysis::smoothing;
use kvantx::domain::types::PeakMethod;

/// Deterministic pseudo-random numbers (xorshift), so that the tests are reproducible.
struct Random(u64);

impl Random {
    /// Uniform in `[0, 1)`.
    fn uniform(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Standard normal (Box-Muller).
    fn normal(&mut self) -> f64 {
        let (u1, u2) = (self.uniform().max(1e-12), self.uniform());
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }
}

/// 1000 points over 10 minutes: Gaussian peaks `(center, height, width)` on a baseline, plus noise.
fn trace(
    peaks: &[(f64, f64, f64)],
    baseline: impl Fn(f64) -> f64,
    sigma: f64,
    seed: u64,
) -> Vec<[f64; 2]> {
    let mut rng = Random(seed);
    (0..1000)
        .map(|i| {
            let t = i as f64 * 0.01;
            let mut y = baseline(t);
            for &(center, height, width) in peaks {
                y += height * (-0.5 * ((t - center) / width).powi(2)).exp();
            }
            [t, (y + sigma * rng.normal()).max(0.0)]
        })
        .collect()
}

#[test]
fn smoothing_leaves_a_quadratic_unchanged_and_the_derivative_of_a_line_is_its_slope() {
    let y: Vec<f64> = (0..30)
        .map(|i| 3.0 + 2.0 * i as f64 - 0.5 * (i * i) as f64)
        .collect();
    let smoothed = smoothing::smooth(&y);
    for i in 3..27 {
        assert!((smoothed[i] - y[i]).abs() < 1e-9, "point {}", i);
    }
    let line: Vec<[f64; 2]> = (0..30)
        .map(|i| [i as f64 * 0.01, 5.0 * i as f64 * 0.01])
        .collect();
    let d = smoothing::derivative(&line);
    for i in 3..27 {
        assert!((d[i] - 5.0).abs() < 1e-9, "point {}", i);
    }
}

#[test]
fn the_noise_estimate_matches_the_true_noise() {
    let mut rng = Random(12345);
    let y: Vec<f64> = (0..3000).map(|_| 100.0 + 10.0 * rng.normal()).collect();
    let sigma = smoothing::noise_sigma(&y, &smoothing::smooth(&y));
    assert!((sigma - 10.0).abs() < 1.0, "estimated {}", sigma);
}

#[test]
fn the_median_of_even_and_odd_lists() {
    assert_eq!(smoothing::median(&[3.0, 1.0, 2.0]), 2.0);
    assert_eq!(smoothing::median(&[4.0, 1.0, 2.0, 3.0]), 2.5);
    assert_eq!(smoothing::median(&[]), 0.0);
}

#[test]
fn the_adaptive_method_finds_a_noisy_peak_and_its_area_is_right() {
    let x = trace(&[(5.0, 1000.0, 0.05)], |_| 0.0, 10.0, 7);
    let peak = AdaptiveFinder { min_snr: 3.0 }
        .find(&x, Some(5.0), 0.3)
        .expect("a peak");
    assert!((peak.apex - 5.0).abs() < 0.03, "apex {}", peak.apex);
    assert!(
        peak.start > 4.6 && peak.start < 4.95,
        "start {}",
        peak.start
    );
    assert!(peak.end > 5.05 && peak.end < 5.4, "end {}", peak.end);
    let found = integration::integrate(&x, peak.start, peak.end, true);
    let reference = integration::integrate(&x, 4.5, 5.5, true);
    assert!(
        (found - reference).abs() / reference < 0.05,
        "{} vs {}",
        found,
        reference
    );
}

#[test]
fn the_adaptive_method_copes_with_a_sloped_baseline() {
    let x = trace(&[(5.0, 1000.0, 0.05)], |t| 100.0 + 20.0 * t, 5.0, 11);
    let peak = AdaptiveFinder { min_snr: 3.0 }
        .find(&x, Some(5.0), 0.3)
        .expect("a peak");
    let found = integration::integrate(&x, peak.start, peak.end, true);
    let reference = integration::integrate(&x, 4.5, 5.5, true);
    assert!(
        (found - reference).abs() / reference < 0.05,
        "{} vs {}",
        found,
        reference
    );
}

#[test]
fn the_adaptive_method_finds_no_peak_in_noise() {
    let x = trace(&[], |_| 50.0, 10.0, 99);
    assert!(
        peaks::finder(PeakMethod::Adaptive)
            .find(&x, None, 0.3)
            .is_none()
    );
}

#[test]
fn the_adaptive_method_prefers_the_peak_near_the_expected_retention_time() {
    let x = trace(&[(3.0, 500.0, 0.05), (7.0, 2000.0, 0.05)], |_| 0.0, 5.0, 3);
    let finder = AdaptiveFinder { min_snr: 3.0 };
    assert!((finder.find(&x, Some(3.0), 0.3).unwrap().apex - 3.0).abs() < 0.05);
    assert!(
        (finder.find(&x, None, 0.3).unwrap().apex - 7.0).abs() < 0.05,
        "strongest peak without a hint"
    );
    assert!(
        finder.find(&x, Some(5.0), 0.3).is_none(),
        "no peak inside the window"
    );
}

#[test]
fn the_adaptive_method_stops_at_the_valley_between_overlapping_peaks() {
    let x = trace(&[(5.0, 1000.0, 0.05), (5.3, 1000.0, 0.05)], |_| 0.0, 2.0, 5);
    let peak = AdaptiveFinder { min_snr: 3.0 }
        .find(&x, Some(5.0), 0.1)
        .expect("a peak");
    assert!(
        peak.end < 5.25,
        "end {} is inside the second peak",
        peak.end
    );
    assert!(peak.start < 5.0);
}

#[test]
fn short_and_flat_traces_have_no_peak() {
    let finder = AdaptiveFinder { min_snr: 3.0 };
    assert!(finder.find(&[[0.0, 1.0]; 5], None, 0.3).is_none());
    let flat: Vec<[f64; 2]> = (0..100).map(|i| [i as f64 * 0.01, 7.0]).collect();
    assert!(finder.find(&flat, None, 0.3).is_none());
}

#[test]
fn the_legacy_finder_reports_the_boundaries_of_the_original_method() {
    let x = trace(&[(5.0, 1000.0, 0.05)], |_| 0.0, 1.0, 21);
    let (start, end) = peaks::auto_peak(&x, Some(5.0), 0.3).expect("a peak");
    let peak = LegacyFinder.find(&x, Some(5.0), 0.3).expect("a peak");
    assert_eq!((peak.start, peak.end), (start, end));
    assert!(peak.apex >= start && peak.apex <= end);
}

#[test]
fn the_methods_are_selected_by_key() {
    for method in PeakMethod::ALL {
        assert_eq!(PeakMethod::from_key(method.key()), Some(method));
        assert!(!peaks::finder(method).name().is_empty());
    }
    assert_eq!(PeakMethod::from_key("unknown"), None);
    assert!(!peaks::finder(PeakMethod::Legacy).subtracts_baseline());
    assert!(peaks::finder(PeakMethod::Adaptive).subtracts_baseline());
}
