//! The menu bar and the keyboard shortcuts.

use crate::app::state::{App, MAX_UNDO, Tab, VERSION};
use crate::domain::types::Theme;
use crate::support::audit;
use eframe::egui::{self, Key, Modifiers};

impl App {
    /// The menu bar.
    pub(crate) fn menu(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui
                    .add(egui::Button::new("New project").shortcut_text("Ctrl+N"))
                    .clicked()
                {
                    self.request_new_project();
                    ui.close_menu();
                }
                if ui
                    .add(egui::Button::new("Open project…").shortcut_text("Ctrl+O"))
                    .clicked()
                {
                    self.open_project_dialog(ctx);
                    ui.close_menu();
                }
                if ui
                    .add(egui::Button::new("Save project").shortcut_text("Ctrl+S"))
                    .clicked()
                {
                    self.save_project(ctx, false);
                    ui.close_menu();
                }
                if ui.button("Save project as…").clicked() {
                    self.save_project(ctx, true);
                    ui.close_menu();
                }
                ui.checkbox(&mut self.embed_raw, "Embed raw files when saving projects");
                ui.separator();
                if ui.button("Import .raw / .mzML files…").clicked() {
                    if let Some(files) = self
                        .dialog()
                        .set_title("Import Thermo .raw or mzML files")
                        .add_filter("Thermo RAW / mzML", &["raw", "mzML", "mzml"])
                        .pick_files()
                    {
                        for f in files {
                            self.start_load_any(ctx, &f.to_string_lossy());
                        }
                    }
                    ui.close_menu();
                }
                if ui.button("Open target table…").clicked() {
                    self.open_targets_dialog();
                    ui.close_menu();
                }
                if ui.button("Save target table as…").clicked() {
                    self.save_targets_as();
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Export analysis results…").clicked() {
                    self.export_results_dialog();
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Exit").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("Edit", |ui| {
                let (undo_steps, redo_steps) = self.history.lens();
                let undo_text = self
                    .history
                    .peek_undo()
                    .map(|s| format!("Undo {}", s.label))
                    .unwrap_or_else(|| "Undo".into());
                let redo_text = self
                    .history
                    .peek_redo()
                    .map(|s| format!("Redo {}", s.label))
                    .unwrap_or_else(|| "Redo".into());
                if ui
                    .add_enabled(
                        undo_steps > 0,
                        egui::Button::new(undo_text).shortcut_text("Ctrl+Z"),
                    )
                    .clicked()
                {
                    self.undo();
                    ui.close_menu();
                }
                if ui
                    .add_enabled(
                        redo_steps > 0,
                        egui::Button::new(redo_text).shortcut_text("Ctrl+Y"),
                    )
                    .clicked()
                {
                    self.redo();
                    ui.close_menu();
                }
                ui.weak(format!(
                    "history: {} back / {} forward (max {})",
                    undo_steps, redo_steps, MAX_UNDO
                ));
                ui.separator();
                if ui.button("Edit target table").clicked() {
                    self.tab = Tab::Calibration;
                    ui.close_menu();
                }
            });
            ui.menu_button("View", |ui| {
                ui.label("Workspace");
                for (tab, name) in [
                    (Tab::Analyze, "Analyze"),
                    (Tab::Calibration, "Calibration"),
                    (Tab::Results, "Results"),
                ] {
                    if ui.selectable_label(self.tab == tab, name).clicked() {
                        self.tab = tab;
                        ui.close_menu();
                    }
                }
                ui.separator();
                ui.label("Panels");
                ui.checkbox(&mut self.layout.files, "Project files");
                ui.checkbox(&mut self.layout.compound, "Compound information");
                ui.checkbox(&mut self.layout.chromatogram, "Chromatogram");
                ui.checkbox(&mut self.layout.spectrum, "MS2 spectrum");
                ui.separator();
                if ui
                    .add(egui::Button::new("Zoom in").shortcut_text("Ctrl++"))
                    .clicked()
                {
                    self.settings.zoom = (self.settings.zoom + 0.1).min(3.0);
                }
                if ui
                    .add(egui::Button::new("Zoom out").shortcut_text("Ctrl+-"))
                    .clicked()
                {
                    self.settings.zoom = (self.settings.zoom - 0.1).max(0.5);
                }
                if ui
                    .add(egui::Button::new("Reset zoom").shortcut_text("Ctrl+0"))
                    .clicked()
                {
                    self.settings.zoom = 1.0;
                }
                ui.separator();
                ui.label("Theme");
                for (theme, name) in [
                    (Theme::System, "System"),
                    (Theme::Light, "Light"),
                    (Theme::Dark, "Dark"),
                ] {
                    if ui.selectable_label(self.theme == theme, name).clicked() {
                        self.theme = theme;
                    }
                }
                ui.separator();
                if ui.button("Save layout…").clicked() {
                    self.save_layout_dialog();
                    ui.close_menu();
                }
                if ui.button("Load layout…").clicked() {
                    self.load_layout_dialog();
                    ui.close_menu();
                }
                if ui.button("Reset layout").clicked() {
                    self.reset_layout();
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Audit trail").clicked() {
                    self.layout.audit.open = true;
                    ui.close_menu();
                }
            });
            ui.menu_button("Settings", |ui| {
                if ui.button("Analysis settings…").clicked() {
                    self.layout.settings.open = true;
                    ui.close_menu();
                }
                if ui.button("Preferences…").clicked() {
                    self.show_prefs = true;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Save settings to file…").clicked() {
                    self.export_settings_dialog();
                    ui.close_menu();
                }
                if ui.button("Load settings from file…").clicked() {
                    self.import_settings_dialog();
                    ui.close_menu();
                }
                if ui.button("Reset settings to defaults").clicked() {
                    self.reset_settings();
                    ui.close_menu();
                }
            });
            ui.menu_button("Help", |ui| {
                if ui.button("README (offline)").clicked() {
                    self.show_readme = true;
                    ui.close_menu();
                }
                if ui.button("About KvantX").clicked() {
                    self.show_about = true;
                    ui.close_menu();
                }
                if ui.button("Credits & licenses").clicked() {
                    self.show_credits = true;
                    ui.close_menu();
                }
                if ui.button("Verify audit trail integrity").clicked() {
                    self.status = match audit::verify(&self.audit.path) {
                        Ok(n) => format!("Audit trail intact: {} entries, hash chain verified", n),
                        Err(e) => format!("AUDIT TRAIL PROBLEM: {}", e),
                    };
                    ui.close_menu();
                }
                ui.separator();
                ui.label(format!("KvantX v{}", VERSION));
            });
        });
    }

    /// Handles keyboard shortcuts: zoom, undo, redo and the project commands.
    pub(crate) fn shortcuts(&mut self, ctx: &egui::Context) {
        ctx.options_mut(|o| o.zoom_with_keyboard = false);
        let (zin, zout, zreset) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::COMMAND, Key::Plus)
                    || i.consume_key(Modifiers::COMMAND, Key::Equals),
                i.consume_key(Modifiers::COMMAND, Key::Minus),
                i.consume_key(Modifiers::COMMAND, Key::Num0),
            )
        });
        if zin {
            self.settings.zoom = (self.settings.zoom + 0.1).min(3.0);
        }
        if zout {
            self.settings.zoom = (self.settings.zoom - 0.1).max(0.5);
        }
        if zreset {
            self.settings.zoom = 1.0;
        }
        if (self.settings.zoom - ctx.zoom_factor()).abs() > 1e-4 {
            ctx.set_zoom_factor(self.settings.zoom);
            self.write_settings();
        }
        if ctx.wants_keyboard_input() {
            return;
        }
        let cs = Modifiers::COMMAND.plus(Modifiers::SHIFT);
        let (redo, undo, save, open, new) = ctx.input_mut(|i| {
            (
                i.consume_key(cs, Key::Z) || i.consume_key(Modifiers::COMMAND, Key::Y),
                i.consume_key(Modifiers::COMMAND, Key::Z),
                i.consume_key(Modifiers::COMMAND, Key::S),
                i.consume_key(Modifiers::COMMAND, Key::O),
                i.consume_key(Modifiers::COMMAND, Key::N),
            )
        });
        if redo {
            self.redo();
        }
        if undo {
            self.undo();
        }
        if save {
            self.save_project(ctx, false);
        }
        if open {
            self.open_project_dialog(ctx);
        }
        if new {
            self.request_new_project();
        }
    }
}
