//! The measured data of one file.

/// A centroided or profile spectrum.
pub struct Spectrum {
    /// Retention time in minutes.
    pub rt: f64,
    /// Scan number from the spectrum id (the position in the file if the id has none).
    pub scan: u32,
    /// Peak m/z values in ascending order.
    pub mz: Vec<f64>,
    /// Peak intensities, parallel to `mz`.
    pub inten: Vec<f32>,
    /// m/z of the fragmented precursor ion (MS2 only): selected ion m/z, else the isolation window target; None if not recorded.
    pub precursor: Option<f64>,
}

/// The MS1 and MS2 content of one mzML file, held in memory.
pub struct MzData {
    /// File name, for display.
    pub name: String,
    /// SHA-256 of the mzML text as it was parsed.
    pub sha256: String,
    /// MS2 spectra, sorted by retention time.
    pub spectra: Vec<Spectrum>,
    /// MS1 spectra, sorted by retention time (needed for quantification on the MS1 precursor signal)
    pub ms1: Vec<Spectrum>,
    /// Number of spectra in the file, of any MS level.
    pub n_total: usize,
    /// (rt, value) total-ion and base-peak chromatograms of MS1 and MS2 scans, same order as `ms1` / `spectra`
    pub tic1: Vec<[f64; 2]>,
    pub bpc1: Vec<[f64; 2]>,
    pub tic2: Vec<[f64; 2]>,
    pub bpc2: Vec<[f64; 2]>,
}
