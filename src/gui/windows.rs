//! Floating windows and the target table: analysis settings, preferences, audit trail, credits.

use crate::app::layout::WindowState;
use crate::app::state::{App, CREDITS, README, WEIGHTS};
use crate::app::targets_ops::parse_row;
use crate::domain::types::{Basis, Mode, PeakMethod, Theme};
use crate::formats::rawconv;
use crate::gui::widgets::{dv, dv_rt};
use crate::support::paths;
use eframe::egui::{self, Color32, RichText};
use std::path::PathBuf;

/// What the buttons of the settings window asked for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsAction {
    None,
    Apply,
    Reset,
    Save,
    Load,
}

impl App {
    /// The target table: edit name, retention time, precursor and product ions; changes apply when a field loses focus.
    pub(crate) fn ui_targets_panel(&mut self, ui: &mut egui::Ui) {
        if self.rows_dirty || self.rows.len() != self.analytes.len() {
            self.sync_rows();
        }
        let avail = ui.available_width();
        if self.col_w[0] < 1.0 {
            self.fit_cols(avail);
        }
        let (mut commit, mut delete): (Option<usize>, Option<usize>) = (None, None);
        let (mut add, mut save, mut save_as, mut open_table) = (false, false, false, false);
        ui.horizontal_wrapped(|ui| {
            if ui.button("Add target").clicked() {
                add = true;
            }
            if ui.button("Open…").clicked() {
                open_table = true;
            }
            if ui.button("Save").clicked() {
                save = true;
            }
            if ui.button("Save as…").clicked() {
                save_as = true;
            }
            if ui.button("Fit columns").clicked() {
                self.fit_cols(avail);
            }
        });
        ui.weak(match &self.table_path {
            Some(p) => p.display().to_string(),
            None => "(not saved yet)".into(),
        });
        ui.weak("Name, retention time (min or n/a), precursor m/z (MS1) and product ion m/z values separated by ;");
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.add_sized([28.0, 18.0], egui::Label::new(RichText::new("#").strong()));
                    let heads = ["Name", "RT (min)", "Precursor m/z", "Product ion m/z"];
                    for k in 0..4 {
                        ui.add_sized(
                            [self.col_w[k], 18.0],
                            egui::Label::new(RichText::new(heads[k]).strong()),
                        );
                        let (rect, resp) =
                            ui.allocate_exact_size(egui::vec2(6.0, 18.0), egui::Sense::drag());
                        ui.painter().vline(
                            rect.center().x,
                            rect.y_range(),
                            ui.visuals().widgets.noninteractive.bg_stroke,
                        );
                        if resp.hovered() || resp.dragged() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                        }
                        if resp.dragged() {
                            self.col_w[k] = (self.col_w[k] + resp.drag_delta().x).max(40.0);
                        }
                    }
                });
                for i in 0..self.rows.len() {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        ui.add_sized([28.0, 20.0], egui::Label::new((i + 1).to_string()));
                        for k in 0..4 {
                            let w = self.col_w[k];
                            let r = ui.add_sized(
                                [w, 20.0],
                                egui::TextEdit::singleline(&mut self.rows[i][k]),
                            );
                            if r.lost_focus() {
                                commit = Some(i);
                            }
                            ui.add_space(6.0);
                        }
                        if parse_row(&self.rows[i]).is_none() {
                            ui.colored_label(Color32::RED, "invalid");
                        }
                        ui.add_space(8.0);
                        if ui.small_button("Delete").clicked() {
                            delete = Some(i);
                        }
                    });
                }
            });
        if let Some(i) = commit {
            self.apply_row(i);
        }
        if let Some(i) = delete {
            self.delete_target(i);
        }
        if add {
            self.add_target();
        }
        if open_table {
            self.open_targets_dialog();
        }
        if save_as {
            self.save_targets_as();
        }
        if save {
            match self.table_path.clone() {
                Some(p) => self.write_targets(&p),
                None => self.save_targets_as(),
            }
        }
    }

    /// Distributes the editable columns of the target editor over `avail` pixels.
    pub(crate) fn fit_cols(&mut self, avail: f32) {
        let w = (avail - 28.0 - 4.0 * 6.0 - 130.0).max(260.0);
        self.col_w = [w * 0.22, w * 0.10, w * 0.14, w * 0.54];
    }

    /// The window with the analysis settings; its position and size belong to the layout.
    pub(crate) fn ui_settings_window(&mut self, ctx: &egui::Context) {
        if !self.layout.settings.open {
            return;
        }
        let mut open = true;
        let placement = self.layout.settings.clone();
        let mut action = SettingsAction::None;
        let mut window = egui::Window::new("Analysis settings")
            .id(egui::Id::new(("analysis_settings", self.layout_gen)))
            .open(&mut open)
            .resizable(true);
        window = if placement.w > 0.0 {
            window
                .default_pos([placement.x, placement.y])
                .default_size([placement.w, placement.h])
        } else {
            window.default_size([470.0, 600.0])
        };
        let shown = window.show(ctx, |ui| {
            let mut changed = false;
            ui.strong("Extraction");
            egui::Grid::new("settings_extraction").num_columns(2).spacing([16.0, 6.0]).show(ui, |ui| {
                ui.label("m/z tolerance unit");
                ui.horizontal(|ui| {
                    changed |= ui.selectable_value(&mut self.settings.tol_ppm, true, "ppm").changed();
                    changed |= ui.selectable_value(&mut self.settings.tol_ppm, false, "Da").changed();
                });
                ui.end_row();
                ui.label("m/z tolerance ±");
                changed |= ui.add(dv(&mut self.settings.tol).speed(0.1).range(0.001..=1000.0)).changed();
                ui.end_row();
                ui.label("RT window ± (min)");
                changed |= ui.add(dv_rt(&mut self.settings.rt_win).speed(0.01).range(0.01..=60.0)).changed();
                ui.end_row();
            });
            ui.separator();
            ui.strong("Signal");
            let mut basis = self.settings.basis;
            ui.horizontal(|ui| {
                ui.selectable_value(&mut basis, Basis::Ms2, "MS2 product-ion area");
                ui.selectable_value(&mut basis, Basis::Ms1, "MS1 precursor area");
            });
            if basis != self.settings.basis {
                self.set_basis(basis);
            }
            ui.separator();
            ui.strong("Peak finding and integration");
            ui.horizontal(|ui| {
                ui.label("Method");
                egui::ComboBox::new("peak_method", "").selected_text(self.settings.peak_method.label()).show_ui(ui, |ui| {
                    for method in PeakMethod::ALL {
                        changed |= ui.selectable_value(&mut self.settings.peak_method, method, method.label()).changed();
                    }
                });
            });
            let legacy = self.settings.peak_method == PeakMethod::Legacy;
            changed |= ui
                .add_enabled(legacy, egui::Checkbox::new(&mut self.settings.baseline, "Subtract a straight baseline between the boundaries"))
                .changed();
            ui.weak("The adaptive method always subtracts the baseline. Boundaries set by hand (with a reason) override the method for that file.");
            ui.separator();
            ui.strong("Quantification");
            let before = self.settings.mode;
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.settings.mode, Mode::Curve, "Standard curve");
                ui.selectable_value(&mut self.settings.mode, Mode::Areas, "Integrated areas only");
            });
            if self.settings.mode != before {
                let text = if self.settings.mode == Mode::Areas { "integrated areas only" } else { "standard curve" };
                self.audit.log("QUANT_MODE", text);
            }
            egui::Grid::new("settings_quant").num_columns(2).spacing([16.0, 6.0]).show(ui, |ui| {
                ui.label("Regression weighting");
                egui::ComboBox::new("weighting", "").selected_text(WEIGHTS[self.settings.weighting as usize]).show_ui(ui, |ui| {
                    for (i, w) in WEIGHTS.iter().enumerate() {
                        changed |= ui.selectable_value(&mut self.settings.weighting, i as u8, *w).changed();
                    }
                });
                ui.end_row();
                ui.label("Concentration unit");
                ui.add(egui::TextEdit::singleline(&mut self.settings.units).desired_width(90.0));
                ui.end_row();
                ui.label("Ion-ratio tolerance ± %");
                changed |= ui.add(dv(&mut self.settings.ratio_tol).range(1.0..=100.0)).changed();
                ui.end_row();
            });
            ui.separator();
            ui.strong("Retention-time alignment");
            ui.horizontal(|ui| {
                if ui.checkbox(&mut self.settings.align_on, "Align on import").changed() {
                    let text = format!("align_on_import={}", self.settings.align_on);
                    self.audit.log("ALIGN_SETTING", &text);
                }
                ui.label("max shift ± (min)");
                ui.add(dv_rt(&mut self.settings.align_max).speed(0.05).range(0.1..=10.0));
            });
            ui.horizontal(|ui| {
                let reference = self.samples.get(self.align_ref).map(|s| s.data.name.clone()).unwrap_or_else(|| "-".into());
                ui.label("reference");
                egui::ComboBox::new("align_reference", "").selected_text(reference).show_ui(ui, |ui| {
                    for i in 0..self.samples.len() {
                        ui.selectable_value(&mut self.align_ref, i, &self.samples[i].data.name);
                    }
                });
                if ui.button("Align now").clicked() {
                    self.align_all();
                }
                if ui.button("Clear").clicked() {
                    self.clear_alignment();
                }
            });
            ui.separator();
            ui.strong("Interface");
            ui.horizontal(|ui| {
                ui.label("Zoom");
                ui.add(
                    egui::DragValue::new(&mut self.settings.zoom)
                        .speed(0.01)
                        .range(0.5..=3.0)
                        .custom_formatter(|n, _| format!("{:.0} %", n * 100.0)),
                );
                ui.label("Theme");
                for (theme, name) in [(Theme::System, "System"), (Theme::Light, "Light"), (Theme::Dark, "Dark")] {
                    ui.selectable_value(&mut self.theme, theme, name);
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.add_enabled(self.settings_dirty, egui::Button::new("Apply & re-integrate")).clicked() {
                    action = SettingsAction::Apply;
                }
                if ui.button("Reset to defaults").clicked() {
                    action = SettingsAction::Reset;
                }
            });
            ui.horizontal(|ui| {
                if ui.button("Save to file…").clicked() {
                    action = SettingsAction::Save;
                }
                if ui.button("Load from file…").clicked() {
                    action = SettingsAction::Load;
                }
            });
            if self.settings_dirty {
                ui.colored_label(Color32::YELLOW, "Changed - press 'Apply & re-integrate' to use the new extraction and peak settings.");
            }
            if changed {
                self.settings_dirty = true;
            }
        });
        if let Some(response) = shown {
            let rect = response.response.rect;
            self.layout.settings = WindowState {
                open: true,
                x: rect.min.x,
                y: rect.min.y,
                w: rect.width(),
                h: rect.height(),
            };
        }
        if !open {
            self.layout.settings.open = false;
        }
        match action {
            SettingsAction::None => {}
            SettingsAction::Apply => self.apply_settings(),
            SettingsAction::Reset => self.reset_settings(),
            SettingsAction::Save => self.export_settings_dialog(),
            SettingsAction::Load => self.import_settings_dialog(),
        }
    }

    /// The window with the audit trail; its position and size belong to the layout.
    pub(crate) fn ui_audit_window(&mut self, ctx: &egui::Context) {
        if !self.layout.audit.open {
            return;
        }
        let mut open = true;
        let placement = self.layout.audit.clone();
        let mut window = egui::Window::new("Audit trail (hash-chained)")
            .id(egui::Id::new(("audit_window", self.layout_gen)))
            .open(&mut open)
            .resizable(true);
        window = if placement.w > 0.0 {
            window
                .default_pos([placement.x, placement.y])
                .default_size([placement.w, placement.h])
        } else {
            window.default_size([900.0, 400.0])
        };
        let shown = window.show(ctx, |ui| {
            ui.weak(self.audit.path.clone());
            egui::ScrollArea::both()
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for line in self.audit.recent.iter().rev().take(300).rev() {
                        ui.monospace(line);
                    }
                });
        });
        if let Some(response) = shown {
            let rect = response.response.rect;
            self.layout.audit = WindowState {
                open: true,
                x: rect.min.x,
                y: rect.min.y,
                w: rect.width(),
                h: rect.height(),
            };
        }
        if !open {
            self.layout.audit.open = false;
        }
    }

    /// The credits and the offline README.
    pub(crate) fn ui_info_windows(&mut self, ctx: &egui::Context) {
        if self.show_credits {
            let mut open = true;
            egui::Window::new("Credits & third-party software")
                .open(&mut open)
                .default_width(640.0)
                .show(ctx, |ui| {
                    ui.label(CREDITS);
                });
            self.show_credits = open;
        }
        if self.show_readme {
            let mut open = true;
            egui::Window::new("README (offline)")
                .open(&mut open)
                .default_size([760.0, 560.0])
                .show(ctx, |ui| {
                    egui::ScrollArea::both().show(ui, |ui| {
                        ui.monospace(README);
                    });
                });
            self.show_readme = open;
        }
    }

    /// Preferences window: converter location, project options and folders.
    pub(crate) fn ui_prefs(&mut self, ctx: &egui::Context) {
        if !self.show_prefs {
            return;
        }
        let mut open = true;
        let (mut browse, mut test, mut save) = (false, false, false);
        let (mut browse_dir, mut reset_dir, mut save_dirs) = (false, false, false);
        let default_dir_text = paths::default_dir_path().to_string_lossy().to_string();
        egui::Window::new("Preferences").open(&mut open).default_width(660.0).show(ctx, |ui| {
            ui.heading("User");
            let user_box = ui.add(egui::TextEdit::singleline(&mut self.audit.user).desired_width(220.0));
            if user_box.lost_focus() {
                if self.audit.user.trim().is_empty() {
                    self.audit.user = "N/A".into();
                }
                if self.audit.user != self.user_logged {
                    let change = format!("{} -> {}", self.user_logged, self.audit.user);
                    self.audit.log("USER_CHANGED", &change);
                    self.user_logged = self.audit.user.clone();
                    self.write_settings();
                }
            }
            ui.weak("Written into every audit-trail entry.");
            ui.separator();
            ui.heading("Thermo .raw reader");
            ui.label("ThermoRawFileParser is only needed for Thermo .raw files; mzML files are read directly.");
            ui.horizontal(|ui| {
                let r = ui.add(egui::TextEdit::singleline(&mut self.trfp_path).desired_width(430.0)
                    .hint_text("path to ThermoRawFileParser.exe (empty = search automatically)"));
                if r.lost_focus() { save = true; }
                if ui.button("Browse…").clicked() { browse = true; }
                if ui.button("Test").clicked() { test = true; }
            });
            match rawconv::find_trfp(&self.trfp_path) {
                Some(p) => { ui.colored_label(Color32::GREEN, format!("found: {}", p.display())); }
                None => { ui.colored_label(Color32::RED, "not found"); }
            }
            ui.separator();
            ui.heading("Projects");
            ui.checkbox(&mut self.embed_raw, "Embed raw files when saving projects");
            ui.separator();
            ui.heading("Folders");
            ui.label("Standard folder for projects, results and figures (file dialogs start here):");
            ui.horizontal(|ui| {
                let mut text = self.data_dir.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
                let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(430.0).hint_text(default_dir_text.clone()));
                if r.changed() {
                    self.data_dir = if text.trim().is_empty() { None } else { Some(PathBuf::from(text.trim())) };
                }
                if r.lost_focus() { save_dirs = true; }
                if ui.button("Browse…").clicked() { browse_dir = true; }
                if ui.button("Use default").clicked() { reset_dir = true; }
            });
            ui.weak(format!("Default when empty: {}", default_dir_text));
            ui.weak("Settings, the audit trail and temporary files are always kept in the default folder; nothing else is written unless you save or export.");
        });
        let was_open = self.show_prefs;
        self.show_prefs = open;
        if browse {
            if let Some(p) = rfd::FileDialog::new()
                .set_title("Locate ThermoRawFileParser")
                .add_filter("ThermoRawFileParser", &["exe", "dll"])
                .pick_file()
            {
                self.trfp_path = p.to_string_lossy().to_string();
                save = true;
            }
        }
        if browse_dir {
            if let Some(p) = rfd::FileDialog::new()
                .set_title("Choose the standard folder")
                .set_directory(self.standard_dir_path())
                .pick_folder()
            {
                self.data_dir = Some(p);
                save_dirs = true;
            }
        }
        if reset_dir {
            self.data_dir = None;
            save_dirs = true;
        }
        if save_dirs {
            self.write_settings();
            self.audit.log(
                "STANDARD_FOLDER",
                &self.standard_dir_path().display().to_string(),
            );
        }
        if save {
            self.save_trfp();
        }
        if test {
            self.status = match rawconv::find_trfp(&self.trfp_path) {
                Some(p) => match rawconv::converter_version(&p) {
                    Ok(v) => format!("ThermoRawFileParser works: {}", v),
                    Err(e) => format!("ThermoRawFileParser test failed: {}", e),
                },
                None => "ThermoRawFileParser not found".into(),
            };
        }
        if was_open && !open {
            self.write_settings();
        }
    }

    /// The "New project" dialog; it is also shown at the very first start.
    pub(crate) fn ui_new_project_window(&mut self, ctx: &egui::Context) {
        let Some(mut form) = self.new_project_form.take() else {
            return;
        };
        let mut answer = 0u8; // 1 = OK, 2 = cancel
        let title = if form.welcome {
            "Welcome to msQuant"
        } else {
            "New project"
        };
        egui::Window::new(title).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            if form.welcome {
                ui.label("Please check these settings. You can change them later under Settings.");
            } else {
                ui.label("The current project is cleared. Check the settings of the new project:");
            }
            ui.add_space(6.0);
            egui::Grid::new("new_project_grid").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                ui.label("m/z window ±");
                ui.horizontal(|ui| {
                    ui.add(dv(&mut form.tol).speed(0.1).range(0.001..=1000.0));
                    ui.selectable_value(&mut form.tol_ppm, true, "ppm");
                    ui.selectable_value(&mut form.tol_ppm, false, "Da");
                });
                ui.end_row();
                ui.label("Default saving folder");
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut form.folder).desired_width(260.0));
                    if ui.button("Browse…").clicked() {
                        if let Some(dir) = self.dialog().set_title("Default saving folder").pick_folder() {
                            form.folder = dir.display().to_string();
                        }
                    }
                });
                ui.end_row();
                ui.label("Integration method");
                egui::ComboBox::new("new_project_method", "").selected_text(form.method.label()).show_ui(ui, |ui| {
                    for method in PeakMethod::ALL {
                        ui.selectable_value(&mut form.method, method, method.label());
                    }
                });
                ui.end_row();
                ui.label("User name");
                ui.add(egui::TextEdit::singleline(&mut form.user).desired_width(200.0));
                ui.end_row();
                ui.label("Raw files");
                ui.checkbox(&mut form.embed, "Save the raw files inside the .quant file");
                ui.end_row();
                if form.ask_trfp {
                    ui.label("ThermoRawFileParser");
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut form.trfp).desired_width(260.0));
                        if ui.button("Browse…").clicked() {
                            if let Some(file) = self.dialog().set_title("Locate ThermoRawFileParser").pick_file() {
                                form.trfp = file.display().to_string();
                            }
                        }
                    });
                    ui.end_row();
                }
            });
            if form.ask_trfp {
                ui.weak("Needed for Thermo .raw files only (mzML files need no converter). Leave empty to set it later under Settings → Preferences.");
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(if form.welcome { "Start" } else { "Create project" }).clicked() {
                    answer = 1;
                }
                if ui.button("Cancel").clicked() {
                    answer = 2;
                }
            });
        });
        match answer {
            1 => self.apply_new_project_form(&form),
            2 => {}
            _ => self.new_project_form = Some(form),
        }
    }
}
