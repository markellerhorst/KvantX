//! Storing, loading and resetting the analysis settings, and saving the session state.

use crate::app::state::App;
use crate::domain::settings::AnalysisSettings;
use eframe::egui;

impl App {
    /// Writes settings and layout to the default folder whenever they changed, but not while a
    /// mouse button is held (a splitter or window is being dragged).
    pub(crate) fn autosave(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.pointer.any_down()) {
            return;
        }
        if self.settings != self.saved_settings
            || self.theme != self.saved_theme
            || self.prefs_dirty
        {
            self.write_settings();
            self.prefs_dirty = false;
            self.saved_settings = self.settings.clone();
            self.saved_theme = self.theme;
        }
        if self.layout != self.saved_layout {
            self.write_layout_file();
            self.saved_layout = self.layout.clone();
        }
    }

    /// Writes the analysis settings to a file chosen by the user.
    pub(crate) fn export_settings_dialog(&mut self) {
        let Some(path) = self
            .dialog()
            .set_title("Save the settings")
            .add_filter("msQuant settings", &["settings"])
            .set_file_name("msQuant.settings")
            .save_file()
        else {
            return;
        };
        self.status = match std::fs::write(&path, self.settings.to_text()) {
            Ok(()) => {
                self.audit
                    .log("SETTINGS_EXPORTED", &path.display().to_string());
                format!("Saved the settings to {}", path.display())
            }
            Err(e) => format!("Saving the settings failed: {}", e),
        };
    }

    /// Loads analysis settings from a file chosen by the user and re-integrates.
    pub(crate) fn import_settings_dialog(&mut self) {
        let Some(path) = self
            .dialog()
            .set_title("Load settings")
            .add_filter("msQuant settings", &["settings"])
            .pick_file()
        else {
            return;
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                self.settings = AnalysisSettings::from_text(&text);
                self.audit
                    .log("SETTINGS_LOADED", &path.display().to_string());
                self.apply_settings();
                self.status = format!("Loaded the settings from {}", path.display());
            }
            Err(e) => self.status = format!("{}: {}", path.display(), e),
        }
    }

    /// Sets all analysis settings back to their defaults and re-integrates.
    pub fn reset_settings(&mut self) {
        self.settings = AnalysisSettings::default();
        self.audit.log("SETTINGS_RESET", "");
        self.apply_settings();
        self.status = "Settings reset to the defaults".into();
    }
}
