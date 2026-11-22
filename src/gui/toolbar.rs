//! The toolbar below the menu and the status bar.

use crate::app::state::{App, Tab};
use eframe::egui;

impl App {
    /// The toolbar: the three workspaces and the quick actions.
    pub(crate) fn ui_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.tab, Tab::Analyze, "Analyze");
            ui.selectable_value(&mut self.tab, Tab::Calibration, "Calibration");
            ui.selectable_value(&mut self.tab, Tab::Results, "Results");
            ui.separator();
            if ui
                .button("Export")
                .on_hover_text("Export the results table as CSV")
                .clicked()
            {
                self.export_results_dialog();
            }
            let settings_open = self.layout.settings.open;
            if ui.selectable_label(settings_open, "Settings").clicked() {
                self.layout.settings.open = !settings_open;
            }
        });
    }

    /// The status bar: the last message on the left, the state of the project on the right.
    pub(crate) fn ui_status(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(&self.status);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let project = self
                    .project_path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unsaved".into());
                ui.weak(format!(
                    "Project: {}   Files: {}   Targets: {}   User: {}",
                    project,
                    self.samples.len(),
                    self.analytes.len(),
                    self.audit.user
                ));
                if self.loading > 0 || self.saving > 0 {
                    ui.spinner();
                }
            });
        });
    }
}
