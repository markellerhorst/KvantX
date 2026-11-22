//! The frame loop: the menu, the toolbar, the workspaces and the windows, one frame at a time.

use crate::app::state::{App, Msg, Tab};
use crate::formats::project::RawSource;
use eframe::egui;
use std::sync::Arc;

impl eframe::App for App {
    /// Draws one frame: processes messages of the background threads, shortcuts and screenshots, then
    /// lays out the menu, the toolbar, the active workspace and the windows.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::Loaded(d, info) => self.add_sample(d, info),
                Msg::Err(e) => self.status = format!("Load failed: {}", e),
                Msg::Finished => self.loading = self.loading.saturating_sub(1),
                Msg::ProjectSaved {
                    path,
                    result,
                    src_paths,
                    missing,
                } => {
                    self.saving = self.saving.saturating_sub(1);
                    match result {
                        Ok(offs) => {
                            let (mut n, mut bytes) = (0usize, 0u64);
                            for (sp, o) in src_paths.iter().zip(offs.iter()) {
                                if let Some((off, len)) = o {
                                    n += 1;
                                    bytes += *len;
                                    for s in self.samples.iter_mut().filter(|s| &s.src_path == sp) {
                                        s.source = RawSource::Embedded {
                                            project: path.clone(),
                                            offset: *off,
                                            len: *len,
                                        };
                                    }
                                }
                            }
                            self.adopt_project_trail(&path);
                            self.audit.log(
                                "PROJECT_SAVED",
                                &format!(
                                    "{} embedded_raw_files={} embedded_bytes={}",
                                    path.display(),
                                    n,
                                    bytes
                                ),
                            );
                            let mut msg = format!(
                                "Saved project {} ({} raw file(s) embedded, {:.1} MB)",
                                path.display(),
                                n,
                                bytes as f64 / 1e6
                            );
                            if !missing.is_empty() {
                                msg += &format!(
                                    "  ⚠ not embedded (original file not found): {}",
                                    missing.join(", ")
                                );
                            }
                            self.status = msg;
                            self.project_path = Some(path);
                        }
                        Err(e) => self.status = format!("Saving the project failed: {}", e),
                    }
                }
            }
        }
        if self.loading == 0 {
            if let Some(p) = self.pending.take() {
                self.finish_project(p);
            }
        }
        for f in ctx.input(|i| i.raw.dropped_files.clone()) {
            if let Some(p) = f.path {
                self.load_path(ctx, &p.to_string_lossy());
            }
        }
        let shot: Option<Arc<egui::ColorImage>> = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        match self.capture.as_ref().map(|c| (c.sent, c.frames)) {
            Some((true, _)) => {
                if let Some(img) = shot {
                    if let Some(cap) = self.capture.take() {
                        self.save_png(ctx, &img, cap);
                    }
                }
            }
            Some((false, 0)) => {
                if let Some(c) = self.capture.as_mut() {
                    c.sent = true;
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
                ctx.request_repaint();
            }
            Some((false, _)) => {
                if let Some(c) = self.capture.as_mut() {
                    c.frames -= 1;
                }
                ctx.request_repaint();
            }
            None => {}
        }
        self.shortcuts(ctx);
        self.fix_selection();
        if self.sel_a >= self.analytes.len() {
            self.sel_a = 0;
        }
        if self.align_ref >= self.samples.len() {
            self.align_ref = 0;
        }

        let title = format!(
            "msQuant - {}",
            self.project_path
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "untitled project".into())
        );
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
        self.apply_theme(ctx);

        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            self.menu(ui, ctx);
        });
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            self.ui_toolbar(ui);
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            self.ui_status(ui);
        });
        let previous_hover = self.hover;
        self.hover = None;
        egui::CentralPanel::default().show(ctx, |ui| match self.tab {
            Tab::Analyze => self.ui_analyze(ui),
            Tab::Calibration => self.ui_calibration_workspace(ui),
            Tab::Results => self.ui_matrix(ui),
        });
        if self.hover != previous_hover {
            ctx.request_repaint();
        }
        self.ui_settings_window(ctx);
        self.ui_prefs(ctx);
        self.ui_audit_window(ctx);
        self.ui_info_windows(ctx);
        self.ui_new_project_window(ctx);
        self.autosave(ctx);
    }
}
