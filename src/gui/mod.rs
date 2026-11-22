//! The user interface, drawn with egui.
//!
//! Every file here adds drawing methods to [`crate::app::App`]; nothing in this module changes
//! the analysis except through the methods of the application layer. The window is a menu bar,
//! a toolbar with three workspaces (analysis, calibration, results), floating windows and a
//! status bar; see `dock` for the panels.

#[macro_use]
mod macros;

mod calibration_view;
mod chromatogram;
mod dock;
mod files_panel;
mod frame;
mod menu;
pub mod plotting;
mod spectrum_panel;
mod theme;
mod toolbar;
mod widgets;
mod windows;
