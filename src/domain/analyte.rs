//! The target of an analysis.

/// A target: a compound defined by its m/z values and, optionally, its retention time.
#[derive(Clone, Debug, PartialEq)]
pub struct Analyte {
    pub name: String,
    pub rt: Option<f64>,
    /// MS2 product (diagnostic) ions; the first one is the quantifier.
    pub ions: Vec<f64>,
    /// optional MS1 precursor m/z, used when quantifying on the MS1 signal.
    pub precursor: Option<f64>,
}
