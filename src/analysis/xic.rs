//! Extraction of ion chromatograms from spectra.

use crate::domain::Spectrum;

/// Summed intensity within +-tol of mz for every MS2 scan -> (rt, intensity)
#[must_use]
pub fn extract(spectra: &[Spectrum], mz: f64, tol: f64) -> Vec<[f64; 2]> {
    spectra
        .iter()
        .map(|s| {
            let mut i = s.mz.partition_point(|&x| x < mz - tol);
            let mut sum = 0.0f64;
            while i < s.mz.len() && s.mz[i] <= mz + tol {
                sum += s.inten[i] as f64;
                i += 1;
            }
            [s.rt, sum]
        })
        .collect()
}

/// Summed intensity of all peaks within +-tol of `target` in one spectrum (m/z must be ascending).
#[must_use]
pub fn intensity_near(mz: &[f64], inten: &[f32], target: f64, tol: f64) -> f64 {
    let mut i = mz.partition_point(|&x| x < target - tol);
    let mut sum = 0.0f64;
    while i < mz.len() && mz[i] <= target + tol {
        sum += inten[i] as f64;
        i += 1;
    }
    sum
}
