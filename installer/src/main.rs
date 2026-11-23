//! Fontabulous Studio Setup: installs Fontabulous Studio for the current Windows user (no admin
//! rights needed), adds Start menu / desktop shortcuts and an entry in Windows'
//! "Installed apps" list. Run with `--uninstall` to remove it, `--silent` to skip the window.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[path = "../../src/logo.rs"]
mod logo;

use egui::{Color32, CornerRadius, RichText, Vec2};
use std::path::{Path, PathBuf};

const APP_EXE: &[u8] = include_bytes!("../../target/release/fontabulous_studio.exe");
const LICENSE_LUCIDE: &[u8] = include_bytes!("../../assets/LICENSE-lucide.txt");
const LICENSE_INTER: &[u8] = include_bytes!("../../assets/OFL-Inter.txt");
const VERSION: &str = env!("CARGO_PKG_VERSION");
const UNINSTALL_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Fontabulous Studio";

// ArtCraft dark palette (same as the app).
const BG: Color32 = Color32::from_rgb(0x13, 0x16, 0x1c);
const PANEL: Color32 = Color32::from_rgb(0x18, 0x1b, 0x22);
const BORDER: Color32 = Color32::from_rgb(0x23, 0x28, 0x32);
const RAISED: Color32 = Color32::from_rgb(0x28, 0x2d, 0x37);
const HOVER: Color32 = Color32::from_rgb(0x31, 0x36, 0x41);
const ACCENT: Color32 = Color32::from_rgb(0xe8, 0x57, 0x3f);
const ACCENT_HI: Color32 = Color32::from_rgb(0xf0, 0x7a, 0x63);
const TEXT: Color32 = Color32::from_rgb(0xe4, 0xe6, 0xea);
const MUTED: Color32 = Color32::from_rgb(0x91, 0x96, 0x9f);
const GREEN: Color32 = Color32::from_rgb(0x3f, 0xb9, 0x6a);
const RED: Color32 = Color32::from_rgb(0xe5, 0x48, 0x4d);

fn default_dir() -> PathBuf {
    let local = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into());
    PathBuf::from(local).join("Programs").join("Fontabulous Studio")
}

fn start_menu_lnk() -> PathBuf {
    let appdata = std::env::var("APPDATA").unwrap_or_default();
    PathBuf::from(appdata).join("Microsoft\\Windows\\Start Menu\\Programs\\Fontabulous Studio.lnk")
}

fn desktop_lnk() -> PathBuf {
    let profile = std::env::var("USERPROFILE").unwrap_or_default();
    PathBuf::from(profile).join("Desktop").join("Fontabulous Studio.lnk")
}

/// Write a file, moving a locked (running) copy aside first.
fn write_replacing(path: &Path, data: &[u8]) -> Result<(), String> {
    if path.exists() && std::fs::write(path, data).is_ok() {
        return Ok(());
    }
    if path.exists() {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let aside = path.with_extension(format!("old-{stamp}"));
        std::fs::rename(path, &aside).map_err(|e| format!("{} is in use and could not be replaced: {e}", path.display()))?;
    }
    std::fs::write(path, data).map_err(|e| format!("Could not write {}: {e}", path.display()))
}

fn shortcut(target: &Path, lnk: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        if let Some(d) = lnk.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let mut sl = mslnk::ShellLink::new(target).map_err(|e| e.to_string())?;
        sl.set_working_dir(target.parent().map(|p| p.display().to_string()));
        sl.set_icon_location(Some(target.display().to_string()));
        sl.set_name(Some("Design and edit fonts".into()));
        sl.create_lnk(lnk).map_err(|e| e.to_string())?;
    }
    let _ = (target, lnk);
    Ok(())
}

fn install(dir: &Path, desktop: bool) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    // Clear copies left behind by earlier upgrades.
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            if e.file_name().to_string_lossy().contains(".old-") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let exe = dir.join("Fontabulous Studio.exe");
    write_replacing(&exe, APP_EXE)?;
    write_replacing(&dir.join("LICENSE-lucide.txt"), LICENSE_LUCIDE)?;
    write_replacing(&dir.join("OFL-Inter.txt"), LICENSE_INTER)?;
    let uninstaller = dir.join("Uninstall Fontabulous Studio.exe");
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    if me != uninstaller {
        let bytes = std::fs::read(&me).map_err(|e| e.to_string())?;
        write_replacing(&uninstaller, &bytes)?;
    }
    shortcut(&exe, &start_menu_lnk())?;
    if desktop {
        shortcut(&exe, &desktop_lnk())?;
    } else {
        let _ = std::fs::remove_file(desktop_lnk());
    }
    #[cfg(windows)]
    {
        use winreg::enums::HKEY_CURRENT_USER;
        let (key, _) = winreg::RegKey::predef(HKEY_CURRENT_USER).create_subkey(UNINSTALL_KEY).map_err(|e| e.to_string())?;
        let un = format!("\"{}\" --uninstall", uninstaller.display());
        let size_kb = ((APP_EXE.len() + LICENSE_INTER.len() + LICENSE_LUCIDE.len()) / 1024) as u32 + 2048;
        key.set_value("DisplayName", &"Fontabulous Studio").map_err(|e| e.to_string())?;
        key.set_value("DisplayVersion", &VERSION).map_err(|e| e.to_string())?;
        key.set_value("Publisher", &"Fontabulous").map_err(|e| e.to_string())?;
        key.set_value("DisplayIcon", &exe.display().to_string()).map_err(|e| e.to_string())?;
        key.set_value("InstallLocation", &dir.display().to_string()).map_err(|e| e.to_string())?;
        key.set_value("UninstallString", &un).map_err(|e| e.to_string())?;
        key.set_value("QuietUninstallString", &format!("{un} --silent")).map_err(|e| e.to_string())?;
        key.set_value("NoModify", &1u32).map_err(|e| e.to_string())?;
        key.set_value("NoRepair", &1u32).map_err(|e| e.to_string())?;
        key.set_value("EstimatedSize", &size_kb).map_err(|e| e.to_string())?;
    }
    Ok(exe)
}

fn installed_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        use winreg::enums::HKEY_CURRENT_USER;
        let key = winreg::RegKey::predef(HKEY_CURRENT_USER).open_subkey(UNINSTALL_KEY).ok()?;
        let s: String = key.get_value("InstallLocation").ok()?;
        return Some(PathBuf::from(s));
    }
    #[allow(unreachable_code)]
    None
}

fn uninstall() -> Result<(), String> {
    let dir = installed_dir().unwrap_or_else(default_dir);
    let _ = std::fs::remove_file(start_menu_lnk());
    let _ = std::fs::remove_file(desktop_lnk());
    #[cfg(windows)]
    {
        use winreg::enums::HKEY_CURRENT_USER;
        let _ = winreg::RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(UNINSTALL_KEY);
    }
    let me = std::env::current_exe().ok();
    let mut busy = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let p = e.path();
            if Some(&p) == me.as_ref() {
                continue;
            }
            if std::fs::remove_file(&p).is_err() && p.extension().map(|x| x == "exe").unwrap_or(false) {
                busy.push(p);
            }
        }
    }
    if !busy.is_empty() {
        return Err("Fontabulous Studio is still running. Close it and run the uninstaller again.".into());
    }
    // The uninstaller cannot delete itself while running: remove the folder just after it exits.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let cmd = format!("ping -n 3 127.0.0.1 > nul & rmdir /s /q \"{}\"", dir.display());
        let _ = std::process::Command::new("cmd").args(["/C", &cmd]).creation_flags(0x0800_0000).spawn();
    }
    Ok(())
}

// ------------------------------------------------------------------------------------------
// Window

#[derive(PartialEq)]
enum Stage {
    Ready,
    Done(PathBuf),
    Removed,
    Failed(String),
}

struct Setup {
    uninstall_mode: bool,
    dir: String,
    desktop: bool,
    launch: bool,
    stage: Stage,
    logo: Option<egui::TextureHandle>,
    shot: Option<PathBuf>,
    frames: u32,
}

impl Setup {
    fn logo(&mut self, ctx: &egui::Context, h_pts: f32) -> egui::TextureHandle {
        if let Some(t) = &self.logo {
            return t.clone();
        }
        // The mark is drawn in its own colours (accent nib, dark hole and slit).
        let px = (h_pts * ctx.pixels_per_point()).round() as u32;
        let rgba = logo::rasterize(px);
        let img = egui::ColorImage::from_rgba_unmultiplied([px as usize, px as usize], &rgba);
        let t = ctx.load_texture("logo", img, egui::TextureOptions::LINEAR);
        self.logo = Some(t.clone());
        t
    }
}

fn button(ui: &mut egui::Ui, label: &str, primary: bool) -> bool {
    let fill = if primary { ACCENT } else { RAISED };
    let text = RichText::new(label).color(if primary { Color32::WHITE } else { TEXT }).size(13.5);
    ui.add(egui::Button::new(text).fill(fill).corner_radius(CornerRadius::same(6)).min_size(Vec2::new(110.0, 32.0))).clicked()
}

impl eframe::App for Setup {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.screenshot_step(&ctx);
        let tex = self.logo(&ctx, 64.0);
        egui::CentralPanel::default().frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin::same(24))).show(ui, |ui| {
            ui.horizontal(|ui| {
                let size = tex.size_vec2() / ctx.pixels_per_point();
                ui.add(egui::Image::new((tex.id(), size)));
                ui.add_space(14.0);
                ui.vertical(|ui| {
                    ui.add_space(6.0);
                    ui.label(RichText::new("Fontabulous Studio").size(24.0).strong().color(TEXT));
                    let sub = if self.uninstall_mode { "Uninstall" } else { "Setup" };
                    ui.label(RichText::new(format!("{sub} · version {VERSION}")).size(13.0).color(MUTED));
                });
            });
            ui.add_space(18.0);
            egui::Frame::new().fill(PANEL).stroke(egui::Stroke::new(1.0, BORDER)).corner_radius(CornerRadius::same(8)).inner_margin(egui::Margin::same(16)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                match &self.stage {
                    Stage::Ready if self.uninstall_mode => {
                        ui.label(RichText::new("Remove Fontabulous Studio from this computer?").color(TEXT).size(14.0));
                        ui.add_space(4.0);
                        ui.label(RichText::new("Your preferences are kept in case you install it again. Your font files are not touched.").color(MUTED));
                    }
                    Stage::Ready => {
                        ui.label(RichText::new("Install location").color(MUTED));
                        ui.add(egui::TextEdit::singleline(&mut self.dir).desired_width(f32::INFINITY));
                        ui.label(RichText::new("Installs for your Windows account only. No administrator rights needed.").size(11.5).color(MUTED));
                        ui.add_space(8.0);
                        ui.checkbox(&mut self.desktop, RichText::new("Create a desktop shortcut").color(TEXT));
                        ui.checkbox(&mut self.launch, RichText::new("Start Fontabulous Studio when setup finishes").color(TEXT));
                    }
                    Stage::Done(dir) => {
                        ui.label(RichText::new("✔  Fontabulous Studio is installed").color(GREEN).size(15.0));
                        ui.add_space(4.0);
                        ui.label(RichText::new(format!("Installed in {}. You'll find it in the Start menu.", dir.display())).color(MUTED));
                    }
                    Stage::Removed => {
                        ui.label(RichText::new("✔  Fontabulous Studio has been removed").color(GREEN).size(15.0));
                    }
                    Stage::Failed(e) => {
                        ui.label(RichText::new("Something went wrong").color(RED).size(15.0));
                        ui.add_space(4.0);
                        ui.label(RichText::new(e).color(TEXT));
                    }
                }
            });
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Max), |ui| {
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| match &self.stage {
                        Stage::Ready => {
                            if self.uninstall_mode {
                                if button(ui, "Remove", true) {
                                    self.stage = match uninstall() {
                                        Ok(()) => Stage::Removed,
                                        Err(e) => Stage::Failed(e),
                                    };
                                }
                            } else if button(ui, "Install", true) {
                                self.stage = match install(Path::new(self.dir.trim()), self.desktop) {
                                    Ok(exe) => {
                                        if self.launch {
                                            let _ = std::process::Command::new(&exe).spawn();
                                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                        }
                                        Stage::Done(PathBuf::from(self.dir.trim()))
                                    }
                                    Err(e) => Stage::Failed(e),
                                };
                            }
                            if button(ui, "Cancel", false) {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        }
                        Stage::Done(dir) => {
                            let exe = dir.join("Fontabulous Studio.exe");
                            if button(ui, "Start Fontabulous Studio", true) {
                                let _ = std::process::Command::new(&exe).spawn();
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                            if button(ui, "Close", false) {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        }
                        _ => {
                            if button(ui, "Close", false) {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        }
                    });
                });
            });
        });
    }
}

impl Setup {
    /// `--screenshot <file.png>`: draw a few frames, save the window picture, exit.
    fn screenshot_step(&mut self, ctx: &egui::Context) {
        let Some(path) = self.shot.clone() else { return };
        ctx.request_repaint();
        self.frames += 1;
        if self.frames == 60 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        let img = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(img) = img {
            let [w, h] = img.size;
            let mut out = Vec::new();
            {
                let mut enc = png::Encoder::new(&mut out, w as u32, h as u32);
                enc.set_color(png::ColorType::Rgba);
                enc.set_depth(png::BitDepth::Eight);
                if let Ok(mut wr) = enc.write_header() {
                    let buf: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
                    let _ = wr.write_image_data(&buf);
                }
            }
            let _ = std::fs::write(&path, out);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

fn icon() -> egui::IconData {
    egui::IconData { rgba: logo::rasterize(128), width: 128, height: 128 }
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    let uninstall_mode = args.iter().any(|a| a == "--uninstall");
    let silent = args.iter().any(|a| a == "--silent");
    let shot = args.iter().position(|a| a == "--screenshot").and_then(|i| args.get(i + 1)).map(PathBuf::from);
    if silent {
        let res = if uninstall_mode { uninstall() } else { install(&installed_dir().unwrap_or_else(default_dir), true).map(|_| ()) };
        if let Err(e) = res {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return Ok(());
    }
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG;
    visuals.widgets.inactive.weak_bg_fill = RAISED;
    visuals.widgets.hovered.weak_bg_fill = HOVER;
    visuals.selection.bg_fill = ACCENT;
    visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT_HI);
    visuals.extreme_bg_color = Color32::from_rgb(0x0f, 0x11, 0x16);
    let mut viewport = egui::ViewportBuilder::default();
    if shot.is_some() {
        viewport = viewport.with_active(false).with_window_level(egui::WindowLevel::AlwaysOnBottom);
    }
    let options = eframe::NativeOptions {
        viewport: viewport
            .with_title(if uninstall_mode { "Uninstall Fontabulous Studio" } else { "Fontabulous Studio Setup" })
            .with_inner_size([540.0, 380.0])
            .with_resizable(false)
            .with_icon(icon()),
        ..Default::default()
    };
    eframe::run_native(
        "Fontabulous Studio Setup",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(visuals);
            Ok(Box::new(Setup {
                uninstall_mode,
                dir: installed_dir().unwrap_or_else(default_dir).display().to_string(),
                desktop: true,
                launch: true,
                stage: Stage::Ready,
                logo: None,
                shot,
                frames: 0,
            }))
        }),
    )
}
