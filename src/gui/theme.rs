//! Color theme and the minimalist look of the interface.

use crate::app::state::App;
use crate::domain::types::Theme;
use eframe::egui;

impl App {
    /// Hands the theme preference to egui and applies the minimalist style once per light/dark style.
    pub(crate) fn apply_theme(&mut self, ctx: &egui::Context) {
        if self.applied_theme != Some(self.theme) {
            ctx.set_theme(match self.theme {
                Theme::System => egui::ThemePreference::System,
                Theme::Light => egui::ThemePreference::Light,
                Theme::Dark => egui::ThemePreference::Dark,
            });
            self.applied_theme = Some(self.theme);
        }
        let dark = ctx.style().visuals.dark_mode;
        if self.styled_dark != Some(dark) {
            ctx.style_mut(|style| {
                style.spacing.item_spacing = egui::vec2(8.0, 5.0);
                style.spacing.button_padding = egui::vec2(8.0, 3.0);
                let rounding = egui::Rounding::same(3.0);
                style.visuals.window_rounding = egui::Rounding::same(4.0);
                style.visuals.menu_rounding = egui::Rounding::same(4.0);
                style.visuals.window_shadow = egui::epaint::Shadow::NONE;
                style.visuals.popup_shadow = egui::epaint::Shadow::NONE;
                let widgets = &mut style.visuals.widgets;
                for w in [
                    &mut widgets.noninteractive,
                    &mut widgets.inactive,
                    &mut widgets.hovered,
                    &mut widgets.active,
                    &mut widgets.open,
                ] {
                    w.rounding = rounding;
                }
            });
            self.styled_dark = Some(dark);
        }
    }
}
