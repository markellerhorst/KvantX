//! The application layer: state, workflows and the connection to files and background jobs.
//!
//! [`App`] holds everything the user works on. The modules add its behavior in groups:
//! import (`files`), analysis (`analysis_ops`), targets (`targets_ops`), projects (`projects`),
//! export (`exports`) and session handling with undo (`session`). Drawing is done by [`crate::gui`].

pub mod analysis_ops;
pub mod exports;
pub mod files;
pub mod layout;
pub mod projects;
pub mod session;
pub mod settings_io;
pub mod state;
pub mod targets_ops;

pub use crate::domain::settings::AnalysisSettings;
pub use crate::domain::types::{Basis, Kind, Mode, PeakMethod, Theme};
pub use layout::{Layout, WindowState};
pub use state::{App, Cell, LoadInfo, NewProjectForm, Sample};
