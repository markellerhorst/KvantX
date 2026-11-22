//! The panels of the analysis workspace and of the calibration workspace.
//!
//! Both workspaces are divided by draggable splitters. The positions are stored as fractions of
//! the window (see [`crate::app::layout`]), so that the panes keep their proportions when the
//! window is resized.

use crate::app::state::App;
use crate::domain::types::Mode;
use eframe::egui;

/// One of the four panels of the analysis workspace.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PanelId {
    /// The imported files.
    Files,
    /// Results and calibration of the selected target.
    Compound,
    /// The chromatogram.
    Chromatogram,
    /// The MS2 spectrum.
    Spectrum,
}

/// Width of the draggable strip between two panes.
const SPLIT: f32 = 6.0;

/// Splits `rect` into a left and a right part at the fraction `frac` and lets the user drag the
/// splitter. `min` is the smallest width of a part in points.
fn splitter_x(
    ui: &mut egui::Ui,
    id: egui::Id,
    rect: egui::Rect,
    frac: &mut f32,
    min: f32,
) -> (egui::Rect, egui::Rect) {
    let total = rect.width().max(1.0);
    let min = min.min(total / 2.0 - SPLIT);
    let x = rect.left() + (*frac * total).clamp(min, (total - min).max(min));
    let handle = egui::Rect::from_min_max(
        egui::pos2(x - SPLIT / 2.0, rect.top()),
        egui::pos2(x + SPLIT / 2.0, rect.bottom()),
    );
    let response = ui.interact(handle, id, egui::Sense::drag());
    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    if response.dragged() {
        *frac = ((x + response.drag_delta().x - rect.left()) / total).clamp(0.1, 0.9);
    }
    ui.painter().vline(
        x,
        rect.y_range(),
        ui.visuals().widgets.noninteractive.bg_stroke,
    );
    (
        egui::Rect::from_min_max(rect.min, egui::pos2(x - SPLIT / 2.0, rect.bottom())),
        egui::Rect::from_min_max(egui::pos2(x + SPLIT / 2.0, rect.top()), rect.max),
    )
}

/// Splits `rect` into an upper and a lower part at the fraction `frac` and lets the user drag the
/// splitter. `min` is the smallest height of a part in points.
fn splitter_y(
    ui: &mut egui::Ui,
    id: egui::Id,
    rect: egui::Rect,
    frac: &mut f32,
    min: f32,
) -> (egui::Rect, egui::Rect) {
    let total = rect.height().max(1.0);
    let min = min.min(total / 2.0 - SPLIT);
    let y = rect.top() + (*frac * total).clamp(min, (total - min).max(min));
    let handle = egui::Rect::from_min_max(
        egui::pos2(rect.left(), y - SPLIT / 2.0),
        egui::pos2(rect.right(), y + SPLIT / 2.0),
    );
    let response = ui.interact(handle, id, egui::Sense::drag());
    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    if response.dragged() {
        *frac = ((y + response.drag_delta().y - rect.top()) / total).clamp(0.1, 0.9);
    }
    ui.painter().hline(
        rect.x_range(),
        y,
        ui.visuals().widgets.noninteractive.bg_stroke,
    );
    (
        egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), y - SPLIT / 2.0)),
        egui::Rect::from_min_max(egui::pos2(rect.left(), y + SPLIT / 2.0), rect.max),
    )
}

impl App {
    /// Whether a panel is open.
    fn panel_open(&self, panel: PanelId) -> bool {
        match panel {
            PanelId::Files => self.layout.files,
            PanelId::Compound => self.layout.compound,
            PanelId::Chromatogram => self.layout.chromatogram,
            PanelId::Spectrum => self.layout.spectrum,
        }
    }

    /// Opens or closes a panel.
    fn set_panel_open(&mut self, panel: PanelId, open: bool) {
        match panel {
            PanelId::Files => self.layout.files = open,
            PanelId::Compound => self.layout.compound = open,
            PanelId::Chromatogram => self.layout.chromatogram = open,
            PanelId::Spectrum => self.layout.spectrum = open,
        }
    }

    /// The analysis workspace: a left and a right column with two panels each.
    pub(crate) fn ui_analyze(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        let left = self.layout.files || self.layout.compound;
        let right = self.layout.chromatogram || self.layout.spectrum;
        match (left, right) {
            (true, true) => {
                let id = egui::Id::new("analysis_split");
                let (l, r) = splitter_x(ui, id, rect, &mut self.layout.left_frac, 220.0);
                self.ui_column(ui, l, PanelId::Files, PanelId::Compound);
                self.ui_column(ui, r, PanelId::Chromatogram, PanelId::Spectrum);
            }
            (true, false) => self.ui_column(ui, rect, PanelId::Files, PanelId::Compound),
            (false, true) => self.ui_column(ui, rect, PanelId::Chromatogram, PanelId::Spectrum),
            (false, false) => {
                ui.centered_and_justified(|ui| {
                    ui.label("All panels are closed. Open them under View → Panels, or use View → Reset layout.");
                });
            }
        }
    }

    /// One column: `top` above `bottom`, separated by a draggable splitter if both are open.
    fn ui_column(&mut self, ui: &mut egui::Ui, rect: egui::Rect, top: PanelId, bottom: PanelId) {
        let (top_open, bottom_open) = (self.panel_open(top), self.panel_open(bottom));
        if top_open && bottom_open {
            let frac = if top == PanelId::Files {
                &mut self.layout.left_top_frac
            } else {
                &mut self.layout.right_top_frac
            };
            let (upper, lower) = splitter_y(
                ui,
                egui::Id::new(("column_split", top as u8)),
                rect,
                frac,
                110.0,
            );
            self.ui_panel_in(ui, upper, top);
            self.ui_panel_in(ui, lower, bottom);
        } else if top_open {
            self.ui_panel_in(ui, rect, top);
        } else if bottom_open {
            self.ui_panel_in(ui, rect, bottom);
        }
    }

    /// Draws a panel inside `rect`: a thin frame, then the title bar and the content.
    fn ui_panel_in(&mut self, ui: &mut egui::Ui, rect: egui::Rect, panel: PanelId) {
        ui.painter()
            .rect_stroke(rect, 3.0, ui.visuals().widgets.noninteractive.bg_stroke);
        let inner = rect.shrink(6.0);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner));
        child.set_clip_rect(inner.intersect(ui.clip_rect()));
        self.ui_panel(&mut child, panel);
    }

    /// A panel: title bar with a close button, then the content.
    fn ui_panel(&mut self, ui: &mut egui::Ui, panel: PanelId) {
        let title = match panel {
            PanelId::Files => "Project files",
            PanelId::Compound => "Compound information",
            PanelId::Chromatogram => "Chromatogram",
            PanelId::Spectrum => "MS2 spectrum",
        };
        let mut close = false;
        ui.horizontal(|ui| {
            ui.strong(title);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("×")
                    .on_hover_text("Close this panel (View → Panels opens it again)")
                    .clicked()
                {
                    close = true;
                }
            });
        });
        ui.separator();
        self.panel_height = ui.available_height();
        match panel {
            PanelId::Files => self.ui_files_panel(ui),
            PanelId::Compound => self.ui_compound_panel(ui),
            // The chromatogram has more controls than fit in a small pane: it scrolls instead of
            // growing, which keeps the other panes at their size.
            PanelId::Chromatogram => {
                egui::ScrollArea::vertical()
                    .id_salt("chromatogram_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.ui_chrom(ui);
                    });
            }
            PanelId::Spectrum => self.ui_spectrum(ui),
        }
        if close {
            self.set_panel_open(panel, false);
        }
    }

    /// Results of the current file and the calibration of the selected target.
    pub(crate) fn ui_compound_panel(&mut self, ui: &mut egui::Ui) {
        if self.analytes.is_empty() {
            ui.weak("No targets yet: open a target table (File menu) or add targets in the Calibration workspace.");
            return;
        }
        let cur = self.cur();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Target:");
                    let name = self.analytes[self.sel_a.min(self.analytes.len() - 1)]
                        .name
                        .clone();
                    egui::ComboBox::new("compound_target", "")
                        .selected_text(name)
                        .show_ui(ui, |ui| {
                            for i in 0..self.analytes.len() {
                                let n = self.analytes[i].name.clone();
                                ui.selectable_value(&mut self.sel_a, i, n);
                            }
                        });
                });
                self.ui_results(ui, cur);
                if self.settings.mode == Mode::Curve {
                    ui.separator();
                    self.ui_cal(ui);
                }
            });
    }

    /// The calibration workspace: target table above calibration setup on the left, the calibration
    /// curve on the right; both halves are 50 % wide by default.
    pub(crate) fn ui_calibration_workspace(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        let (left, right) = splitter_x(
            ui,
            egui::Id::new("calibration_split_x"),
            rect,
            &mut self.layout.cal_left_frac,
            320.0,
        );
        let (top, bottom) = splitter_y(
            ui,
            egui::Id::new("calibration_split_y"),
            left,
            &mut self.layout.cal_left_top_frac,
            150.0,
        );
        self.ui_box(ui, top, "Target table", false, App::ui_targets_panel);
        self.ui_box(ui, bottom, "Calibration setup", true, App::ui_setup);
        self.ui_box(
            ui,
            right,
            "Calibration curve",
            true,
            App::ui_calibration_curve,
        );
    }

    /// A titled pane inside `rect`; `scroll` wraps the content in a scroll area.
    fn ui_box(
        &mut self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        title: &str,
        scroll: bool,
        body: fn(&mut App, &mut egui::Ui),
    ) {
        ui.painter()
            .rect_stroke(rect, 3.0, ui.visuals().widgets.noninteractive.bg_stroke);
        let inner = rect.shrink(6.0);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner));
        child.set_clip_rect(inner.intersect(ui.clip_rect()));
        child.strong(title);
        child.separator();
        self.panel_height = child.available_height();
        if scroll {
            egui::ScrollArea::both()
                .id_salt(title)
                .auto_shrink([false, false])
                .show(&mut child, |ui| {
                    body(self, ui);
                });
        } else {
            body(self, &mut child);
        }
    }
}
