//! The scientific calculations. Nothing in this module depends on the user interface.

/// Retention-time alignment.
pub mod alignment;
/// Calibration by weighted regression.
pub mod calibration;
/// Peak integration.
pub mod integration;
/// Peak finding.
pub mod peaks;
/// Smoothing, derivatives and noise estimation.
pub mod smoothing;
/// Extracted-ion chromatograms.
pub mod xic;
