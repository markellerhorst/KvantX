//! The panel with the imported files.

use crate::app::state::App;
use crate::domain::types::Kind;
use eframe::egui::{self, RichText};

impl App {
    /// The panel with the imported files: selection (click, Ctrl, Shift), type and the context menu.
    pub(crate) fn ui_files_panel(&mut self, ui: &mut egui::Ui) {
        if self.samples.is_empty() {
            ui.weak("Drop Thermo .raw or mzML files here, or use File → Import.");
        }
        let mut align = false;
        ui.horizontal(|ui| {
            if ui.small_button("Select all").clicked() { for s in self.samples.iter_mut() { s.show = true; } }
            if ui.small_button("Select none").clicked() { for s in self.samples.iter_mut() { s.show = false; } }
            if ui.small_button("Align RT").on_hover_text("Align the retention times of all files to the reference file (Settings → Analysis settings)").clicked() { align = true; }
        });
        ui.weak("Click = select · Ctrl+click = add/remove · Shift+click = range · right-click = menu · changing the type of a selected file changes all selected files");
        let mut kind_set: Option<(usize, Kind)> = None;
        let mut click: Option<(usize, bool, bool)> = None;
        let mut remove: Option<usize> = None;
        let mut set_ref: Option<usize> = None;
        let selected_count = self.samples.iter().filter(|s| s.show).count();
        let mut sort = false;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                for (i, s) in self.samples.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut s.show, "");
                        let label = if i == self.sel_s && s.show {
                            RichText::new(&s.data.name).strong()
                        } else {
                            RichText::new(&s.data.name)
                        };
                        let resp = ui.selectable_label(s.show, label);
                        if resp.clicked() {
                            let m = ui.input(|inp| inp.modifiers);
                            click = Some((i, m.command, m.shift));
                        }
                        resp.context_menu(|ui| {
                            if ui
                                .button("Remove from project (file stays on disk)")
                                .clicked()
                            {
                                remove = Some(i);
                                ui.close_menu();
                            }
                            if ui.button("Select only this file").clicked() {
                                click = Some((i, false, false));
                                ui.close_menu();
                            }
                            if ui.button("Use as alignment reference").clicked() {
                                set_ref = Some(i);
                                ui.close_menu();
                            }
                            if ui
                                .add_enabled(
                                    selected_count > 1,
                                    egui::Button::new("Sort selected files alphabetically"),
                                )
                                .clicked()
                            {
                                sort = true;
                                ui.close_menu();
                            }
                        });
                        let mut kk = s.kind;
                        egui::ComboBox::new(("kind", i), "")
                            .selected_text(s.kind.label())
                            .show_ui(ui, |ui| {
                                for k in Kind::ALL {
                                    ui.selectable_value(&mut kk, k, k.label());
                                }
                            });
                        if kk != s.kind {
                            kind_set = Some((i, kk));
                        }
                        if s.rt_shift != 0.0 {
                            ui.weak(format!("Δ{:+.2} min", s.rt_shift));
                        }
                    });
                }
            });
        if let Some((i, k)) = kind_set {
            self.set_type(i, k);
        }
        if let Some((i, ctrl, shift)) = click {
            self.apply_click(i, ctrl, shift);
        }
        if let Some(i) = set_ref {
            self.align_ref = i;
            self.audit
                .log("ALIGN_REFERENCE", &self.samples[i].data.name);
        }
        if let Some(i) = remove {
            self.remove_sample(i);
        }
        if sort {
            self.sort_selected();
        }
        if align {
            self.align_all();
        }
    }
}
