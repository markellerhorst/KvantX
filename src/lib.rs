//! msQuant: targeted quantification of LC-MS/MS data.
//!
//! The crate is split in a user interface ([`app`]) and independent building blocks: readers
//! ([`mzml`], [`rawconv`], [`icon`]), analysis ([`quant`]), storage ([`project`], [`audit`],
//! [`paths`]) and output ([`svg`], [`png`], [`fmt`]).

#![forbid(unsafe_code)]
/// User interface and analysis state.
pub mod app;
/// Hash-chained audit trail.
pub mod audit;
/// CSV output helpers.
pub mod csv;
/// Number formatting for the user interface.
pub mod fmt;
/// Bounded undo and redo history.
pub mod history;
/// Decoder for the application icon.
pub mod icon;
/// Decompression of zlib streams.
pub mod inflate;
/// Reader for mzML files.
pub mod mzml;
/// Locations of the files msQuant writes.
pub mod paths;
/// Geometry and data helpers of the plots.
pub mod plotting;
/// Minimal PNG writer.
pub mod png;
/// Project file format.
pub mod project;
/// Chromatography mathematics: extraction, integration, calibration and alignment.
pub mod quant;
/// Conversion of Thermo `.raw` files with ThermoRawFileParser.
pub mod rawconv;
/// SHA-256 hashing.
pub mod sha256;
/// Vector figure export.
pub mod svg;
/// Synthetic data for the test suites.
pub mod testkit;
