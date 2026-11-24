//! KvantX: targeted quantification of LC-MS/MS data.
//!
//! The crate is layered; each layer only uses the layers above it in this list:
//! [`support`] (infrastructure), [`domain`] (data), [`analysis`] (calculations),
//! [`formats`] (files), [`app`] (application state and workflows), [`gui`] (egui).

#![forbid(unsafe_code)]

/// The scientific calculations.
pub mod analysis;
/// Application state and workflows.
pub mod app;
/// The data of an analysis.
pub mod domain;
/// Reading and writing of files.
pub mod formats;
/// The user interface.
pub mod gui;
/// Infrastructure without domain knowledge.
pub mod support;
