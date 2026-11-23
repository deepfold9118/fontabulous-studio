#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod commands;
mod font;
mod geom;
mod glyphnames;
mod icons;
mod io;
mod logo;
mod mcp;
mod render;
mod theme;
mod ui_dialogs;
mod ui_fontwin;
mod ui_glyph;
mod ui_panels;
mod ui_shell;
mod widgets;

use std::path::PathBuf;

/// Window and taskbar icon: the same pen-nib mark as the title bar.
fn app_icon() -> egui::IconData {
    let n = 256u32;
    egui::IconData { rgba: logo::rasterize(n), width: n, height: n }
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    let mut screenshot: Option<PathBuf> = None;
    let mut mcp_stdio = false;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--mcp" {
            mcp_stdio = true;
        } else if args[i] == "--screenshot" && i + 1 < args.len() {
            screenshot = Some(PathBuf::from(&args[i + 1]));
            i += 1;
        }
        i += 1;
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Fontabulous Studio")
        .with_app_id("fontabulous_studio")
        .with_inner_size([1440.0, 900.0])
        .with_min_inner_size([960.0, 600.0])
        .with_decorations(false)
        .with_icon(app_icon());
    if screenshot.is_some() {
        viewport = viewport.with_active(false);
        #[cfg(windows)]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }

    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions { viewport, ..Default::default() };
    #[cfg(target_os = "macos")]
    if screenshot.is_some() {
        options.event_loop_builder = Some(Box::new(|b| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            b.with_activate_ignoring_other_apps(false);
        }));
    }

    eframe::run_native(
        "Fontabulous Studio",
        options,
        Box::new(move |cc| {
            let mut a = app::App::new(cc, screenshot.clone());
            if mcp_stdio {
                // An agent's MCP client launched us: serve it over stdin/stdout.
                a.mcp.start_stdio();
                a.dlg.welcome = false;
                a.log("MCP: serving an agent over stdin/stdout.");
            }
            Ok(Box::new(a))
        }),
    )
}

impl eframe::App for app::App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.frame_no += 1;

        // Ask before closing the window with unsaved fonts.
        if ctx.input(|i| i.viewport().close_requested()) && !self.quit_ok && self.screenshot.is_none() && self.docs.iter().any(|d| d.font.dirty) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.dlg.confirm_quit = true;
        }

        // Agent (MCP) calls run first, through the same code as menu clicks.
        if self.screenshot.is_none() {
            self.mcp.sync(self.prefs.mcp_http, self.prefs.mcp_port);
            self.mcp.close_when_finished(&ctx);
        }
        mcp::pump(self, &ctx);

        ui_fontwin::rebuild_view(self);
        ui_shell::shortcuts(self, &ctx);
        ui_shell::title_bar(self, ui);
        ui_shell::tab_bar(self, ui);
        ui_shell::status_bar(self, ui);
        if self.in_glyph_tab() {
            ui_glyph::tool_strip(self, ui);
        } else if self.prefs.show_left {
            ui_fontwin::sidebar(self, ui);
        }
        ui_panels::icon_strip(self, ui);
        if self.prefs.show_right {
            ui_panels::dock(self, ui);
        }
        let fill = self.pal.canvas;
        egui::CentralPanel::default().frame(egui::Frame::new().fill(fill)).show(ui, |ui| {
            if self.in_glyph_tab() {
                ui_glyph::canvas(self, ui);
            } else {
                ui_fontwin::grid(self, ui);
            }
        });
        ui_dialogs::dialogs(self, &ctx);
        ui_shell::resize_edges(&ctx);

        if self.prefs_dirty && self.screenshot.is_none() {
            self.prefs_dirty = false;
            self.prefs.save();
        }
        mcp::deliver_screenshots(self, &ctx);
        self.screenshot_step(&ctx);
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        if self.screenshot.is_none() {
            self.prefs.save();
        }
    }
}

impl app::App {
    fn screenshot_step(&mut self, ctx: &egui::Context) {
        let Some(path) = self.screenshot.clone() else { return };
        ctx.request_repaint();
        // With an agent attached over stdio, wait until it disconnects so the picture shows its work.
        let agent_busy = self.mcp.stdio && !self.mcp.stdio_finished() && self.frame_no < 36_000;
        if !self.shot_requested && self.frame_no >= 120 && !agent_busy {
            self.shot_requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        if !self.shot_requested {
            return;
        }
        let shot = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(img) = shot {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let [w, h] = img.size;
            let mut buf = Vec::with_capacity(w * h * 4);
            for c in &img.pixels {
                buf.extend_from_slice(&c.to_srgba_unmultiplied());
            }
            if let Some(rgba) = image::RgbaImage::from_raw(w as u32, h as u32, buf) {
                let _ = rgba.save(&path);
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}
