//! Reading and writing of files: mzML, projects, target tables, converter, CSV, SVG, PNG and the icon.

/// CSV output helpers.
pub mod csv;
/// Decoder for the application icon.
pub mod icon;
/// Reader for mzML files.
pub mod mzml;
/// Minimal PNG writer.
pub mod png;
/// Project file format.
pub mod project;
/// Conversion of Thermo `.raw` files with ThermoRawFileParser.
pub mod rawconv;
/// Vector figure export.
pub mod svg;
/// Parser of the target table.
pub mod targets;
