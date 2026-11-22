//! The data of an analysis, independent of how it is shown or stored.

/// The target of an analysis.
pub mod analyte;
/// The parameters of an analysis.
pub mod settings;
/// Spectra and the data of one file.
pub mod spectrum;
/// Small enumerations that describe a file, the quantification and the signal.
pub mod types;

pub use analyte::Analyte;
pub use spectrum::{MzData, Spectrum};
