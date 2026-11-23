//! Window chrome: title bar with the menus, the document tab strip with the tool
//! row, the status bar, window resize edges and keyboard shortcuts.

use egui::{Align, Align2, Color32, CornerRadius, Frame, Key, Layout, Margin, Panel, Sense, Stroke, StrokeKind, Vec2};

use crate::app::*;
use crate::commands;
use crate::font::FLAG_COLORS;
use crate::icons as I;
use crate::theme::{bold, icon_font, ThemeMode};
use crate::widgets::*;

pub fn paint_mark(p: &egui::Painter, rect: egui::Rect, accent: Color32) {
    // Studio mark: a pen nib with a breather hole and slit (same geometry as the app icon).
    use crate::logo;
    let c = rect.center();
    let s = rect.height() / 18.0;
    let at = |q: [f32; 2]| c + Vec2::new(q[0] * s, q[1] * s);
    let dark = Color32::from_rgb(logo::DARK[0], logo::DARK[1], logo::DARK[2]);
    p.add(egui::Shape::convex_polygon(logo::NIB.iter().map(|q| at(*q)).collect(), accent, Stroke::NONE));
    p.circle_filled(at(logo::HOLE_C), logo::HOLE_R * s, dark);
    p.line_segment([at(logo::HOLE_C), at([0.0, logo::SLIT_END_Y])], Stroke::new(logo::SLIT_W * s, dark));
}

fn window_button(ui: &mut egui::Ui, pal: &crate::theme::Palette, icon: &str, close: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(40.0, 30.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(4), if close { pal.red } else { pal.hover });
    }
    let col = if resp.hovered() && close { Color32::WHITE } else { pal.muted };
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, icon, icon_font(14.0), col);
    resp
}

pub fn title_bar(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    Panel::top("titlebar")
        .exact_size(36.0)
        .frame(Frame::new().fill(pal.title).inner_margin(Margin::symmetric(8, 0)))
        .show_separator_line(false)
        .show(ui, |ui| {
            let bar_rect = ui.max_rect();
            let bg = ui.interact(bar_rect, ui.id().with("title_drag"), Sense::click_and_drag());
            if bg.drag_started() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            if bg.double_clicked() {
                let max = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
            }
            ui.horizontal_centered(|ui| {
                ui.add_space(2.0);
                let (r, _) = ui.allocate_exact_size(Vec2::new(16.0, 18.0), Sense::hover());
                paint_mark(ui.painter(), r, pal.accent);
                ui.add_space(8.0);
                egui::MenuBar::new().ui(ui, |ui| menus(app, ui));
            });
            let title = format!("Fontabulous Studio — {}", app.doc().title());
            ui.painter().text(egui::pos2(bar_rect.center().x + 120.0, bar_rect.center().y), Align2::CENTER_CENTER, title, bold(12.5), pal.muted);
            let right = egui::Rect::from_min_max(egui::pos2(bar_rect.right() - 230.0, bar_rect.top()), bar_rect.max);
            ui.scope_builder(egui::UiBuilder::new().max_rect(right).layout(Layout::right_to_left(Align::Center)), |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                if window_button(ui, &pal, I::X, true).clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
                let max = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
                if window_button(ui, &pal, if max { I::MINIMIZE_2 } else { I::SQUARE }, false).clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
                }
                if window_button(ui, &pal, I::MINUS, false).clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
                ui.add_space(8.0);
                let (icon, tip, next) = if pal.dark { (I::SUN, "Switch to light theme", ThemeMode::Light) } else { (I::MOON, "Switch to dark theme", ThemeMode::Dark) };
                if icon_button(ui, &pal, icon, tip, false).clicked() {
                    let ctx = ui.ctx().clone();
                    app.set_theme(&ctx, next);
                }
                if icon_button(ui, &pal, I::SEARCH, "Find Glyphs (Ctrl+F)", app.dlg.find).clicked() {
                    app.dlg.find = !app.dlg.find;
                }
            });
        });
}

/// Menu item with icon, label and shortcut text.
fn item(ui: &mut egui::Ui, icon: &str, label: &str, shortcut: &str, enabled: bool) -> bool {
    let text = if icon.is_empty() { format!("      {label}") } else { format!("{icon}  {label}") };
    let mut b = egui::Button::new(text);
    if !shortcut.is_empty() {
        b = b.shortcut_text(shortcut);
    }
    let r = ui.add_enabled(enabled, b);
    if r.clicked() {
        ui.close();
        true
    } else {
        false
    }
}

fn check(ui: &mut egui::Ui, v: &mut bool, label: &str) -> bool {
    let r = ui.checkbox(v, label);
    r.changed()
}

/// A menu item that runs a command from the shared command list.
fn cmd(ui: &mut egui::Ui, app: &mut App, ctx: &egui::Context, id: &str) {
    let Some(c) = commands::find(id) else { return };
    let enabled = commands::enabled(app, c.when);
    if item(ui, c.icon, &commands::label(app, c), c.keys, enabled) {
        run_cmd(app, ctx, id);
    }
}

/// Run a command from the app (menus, keyboard); failures show in the status bar.
pub fn run_cmd(app: &mut App, ctx: &egui::Context, id: &str) {
    if let Err(e) = commands::run(app, ctx, id, None, true) {
        if !e.is_empty() && app.dlg.coming_soon.is_none() {
            app.toast(ctx, e);
        }
    }
}

fn menus(app: &mut App, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    let ctx = &ctx;
    ui.menu_button("File", |ui| {
        cmd(ui, app, ctx, "file.welcome");
        cmd(ui, app, ctx, "file.new");
        ui.separator();
        cmd(ui, app, ctx, "file.open");
        ui.menu_button(format!("{}  Open", I::FOLDER), |ui| {
            cmd(ui, app, ctx, "file.open_ufo");
            cmd(ui, app, ctx, "file.open_installed");
        });
        ui.menu_button(format!("{}  Recent Fonts", I::HISTORY), |ui| {
            let recent = app.prefs.recent.clone();
            if recent.is_empty() {
                ui.label("No recent fonts");
            }
            for p in recent {
                if item(ui, I::FILE_TYPE, &p.display().to_string(), "", true) {
                    app.open_path(&p);
                }
            }
        });
        cmd(ui, app, ctx, "file.revert");
        ui.separator();
        cmd(ui, app, ctx, "file.save");
        cmd(ui, app, ctx, "file.save_as");
        cmd(ui, app, ctx, "file.save_all");
        ui.separator();
        cmd(ui, app, ctx, "file.export_profiles");
        cmd(ui, app, ctx, "file.export_layer_ttf");
        cmd(ui, app, ctx, "file.export_as");
        ui.separator();
        ui.menu_button(format!("{}  Import", I::FILE_INPUT), |ui| {
            cmd(ui, app, ctx, "file.import_kerning");
            cmd(ui, app, ctx, "file.import_features");
            cmd(ui, app, ctx, "file.import_classes");
            cmd(ui, app, ctx, "file.import_artwork");
        });
        ui.menu_button(format!("{}  Export", I::FILE_OUTPUT), |ui| {
            cmd(ui, app, ctx, "file.export_kerning");
            cmd(ui, app, ctx, "file.export_kern_feature");
            cmd(ui, app, ctx, "file.export_features");
            cmd(ui, app, ctx, "file.export_classes");
        });
        ui.separator();
        cmd(ui, app, ctx, "file.font_info");
        cmd(ui, app, ctx, "file.print");
        ui.separator();
        cmd(ui, app, ctx, "file.exit");
    });
    ui.menu_button("Edit", |ui| {
        cmd(ui, app, ctx, "edit.undo");
        cmd(ui, app, ctx, "edit.redo");
        ui.separator();
        cmd(ui, app, ctx, "edit.cut");
        cmd(ui, app, ctx, "edit.copy");
        cmd(ui, app, ctx, "edit.delete");
        cmd(ui, app, ctx, "edit.paste");
        cmd(ui, app, ctx, "edit.duplicate");
        ui.separator();
        cmd(ui, app, ctx, "edit.copy_metrics_to_masters");
        ui.separator();
        cmd(ui, app, ctx, "edit.select_all");
        cmd(ui, app, ctx, "edit.invert_selection");
        ui.separator();
        cmd(ui, app, ctx, "edit.find");
        cmd(ui, app, ctx, "edit.more_info");
        ui.separator();
        cmd(ui, app, ctx, "edit.preferences");
    });
    ui.menu_button("Text", |ui| {
        cmd(ui, app, ctx, "text.previous");
        cmd(ui, app, ctx, "text.next");
        ui.separator();
        ui.menu_button(format!("{}  Change Case", I::CASE_SENSITIVE), |ui| {
            cmd(ui, app, ctx, "text.uppercase");
            cmd(ui, app, ctx, "text.lowercase");
        });
        ui.menu_button(format!("{}  Copy Text As", I::COPY), |ui| {
            cmd(ui, app, ctx, "text.copy_names");
            cmd(ui, app, ctx, "text.copy_chars");
        });
        ui.separator();
        ui.menu_button(format!("{}  Sample Texts", I::TEXT_QUOTE), |ui| {
            for s in app.prefs.sample_texts.clone() {
                if item(ui, "", &s, "", true) {
                    app.open_text_tab(&s, Tool::Text);
                }
            }
        });
        cmd(ui, app, ctx, "text.right_to_left");
    });
    ui.menu_button("Font", |ui| {
        cmd(ui, app, ctx, "font.create_glyph");
        cmd(ui, app, ctx, "font.build_glyphs");
        cmd(ui, app, ctx, "font.copy_template");
        cmd(ui, app, ctx, "font.add_from_catalog");
        cmd(ui, app, ctx, "font.detect_composites");
        ui.separator();
        cmd(ui, app, ctx, "font.add_variation");
        cmd(ui, app, ctx, "font.merge_masters");
        cmd(ui, app, ctx, "font.generate_instance");
        cmd(ui, app, ctx, "font.compare");
        cmd(ui, app, ctx, "font.overlay");
        ui.separator();
        ui.menu_button(format!("{}  Kerning", I::ARROW_LEFT_RIGHT), |ui| {
            cmd(ui, app, ctx, "font.kerning_tab");
            cmd(ui, app, ctx, "font.kern_adjust");
            cmd(ui, app, ctx, "font.kern_remove");
            cmd(ui, app, ctx, "font.kern_remove_all");
        });
        ui.menu_button(format!("{}  Metrics", I::MOVE_HORIZONTAL), |ui| {
            cmd(ui, app, ctx, "font.metrics_tab");
            cmd(ui, app, ctx, "font.link_metrics");
            cmd(ui, app, ctx, "font.center_glyphs");
            cmd(ui, app, ctx, "font.equalize_sidebearings");
        });
        ui.separator();
        ui.menu_button(format!("{}  Sort Glyphs", I::ARROW_DOWN_A_Z), |ui| {
            cmd(ui, app, ctx, "font.sort_name");
            cmd(ui, app, ctx, "font.sort_unicode");
            cmd(ui, app, ctx, "font.sort_category");
        });
        ui.menu_button(format!("{}  Rename Glyphs", I::PENCIL), |ui| {
            cmd(ui, app, ctx, "glyph.rename");
            cmd(ui, app, ctx, "font.names_from_unicode");
        });
        cmd(ui, app, ctx, "font.generate_unicodes");
        ui.separator();
        cmd(ui, app, ctx, "font.previous");
        cmd(ui, app, ctx, "font.next");
        ui.separator();
        cmd(ui, app, ctx, "font.close");
    });
    ui.menu_button("Glyph", |ui| {
        cmd(ui, app, ctx, "glyph.rename");
        cmd(ui, app, ctx, "glyph.duplicate");
        cmd(ui, app, ctx, "glyph.next");
        cmd(ui, app, ctx, "glyph.previous");
        ui.menu_button(format!("{}  Flag", I::FLAG), |ui| {
            cmd(ui, app, ctx, "glyph.flag_none");
            for c in FLAG_COLORS.iter() {
                let text = egui::RichText::new(format!("{}  {}", I::FLAG, c.3)).color(Color32::from_rgb(c.0, c.1, c.2));
                if ui.button(text).clicked() {
                    run_cmd(app, ctx, &format!("glyph.flag_{}", c.3.to_lowercase()));
                    ui.close();
                }
            }
        });
        ui.separator();
        cmd(ui, app, ctx, "glyph.add_component");
        cmd(ui, app, ctx, "glyph.decompose");
        ui.separator();
        cmd(ui, app, ctx, "glyph.add_anchor");
        cmd(ui, app, ctx, "glyph.remove_anchors");
        ui.separator();
        cmd(ui, app, ctx, "font.add_variation");
        cmd(ui, app, ctx, "glyph.match_masters");
        cmd(ui, app, ctx, "glyph.ps_direction");
        ui.separator();
        cmd(ui, app, ctx, "glyph.add_sticker");
    });
    ui.menu_button("Element", |ui| {
        ui.menu_button(format!("{}  New", I::FILE_PLUS), |ui| {
            cmd(ui, app, ctx, "element.new_rectangle");
            cmd(ui, app, ctx, "element.new_ellipse");
        });
        cmd(ui, app, ctx, "element.locked_content");
        ui.separator();
        ui.menu_button(format!("{}  Align", I::ALIGN_START_VERTICAL), |ui| {
            cmd(ui, app, ctx, "contour.align_h");
            cmd(ui, app, ctx, "contour.align_v");
        });
        ui.separator();
        ui.menu_button(format!("{}  Contours", I::SPLINE), |ui| {
            cmd(ui, app, ctx, "element.reverse");
            cmd(ui, app, ctx, "glyph.ps_direction");
            cmd(ui, app, ctx, "element.to_cubic");
        });
        cmd(ui, app, ctx, "glyph.decompose");
        ui.separator();
        cmd(ui, app, ctx, "element.brush");
        cmd(ui, app, ctx, "element.image");
    });
    ui.menu_button("Contour", |ui| {
        ui.menu_button(format!("{}  Nodes", I::CIRCLE_DOT), |ui| {
            cmd(ui, app, ctx, "contour.toggle_smooth");
            cmd(ui, app, ctx, "contour.delete_nodes");
        });
        ui.menu_button(format!("{}  Convert", I::REPLACE), |ui| {
            cmd(ui, app, ctx, "contour.line_to_curve");
            cmd(ui, app, ctx, "contour.curve_to_line");
            cmd(ui, app, ctx, "element.to_cubic");
        });
        ui.menu_button(format!("{}  Coordinates", I::HASH), |ui| {
            cmd(ui, app, ctx, "contour.round");
        });
        ui.separator();
        cmd(ui, app, ctx, "contour.set_start");
        cmd(ui, app, ctx, "contour.reverse");
        cmd(ui, app, ctx, "contour.break");
        cmd(ui, app, ctx, "contour.join");
        ui.separator();
        ui.menu_button(format!("{}  Align Points", I::ALIGN_CENTER_HORIZONTAL), |ui| {
            cmd(ui, app, ctx, "contour.align_h");
            cmd(ui, app, ctx, "contour.align_v");
        });
        ui.separator();
        cmd(ui, app, ctx, "contour.clean_up");
        cmd(ui, app, ctx, "contour.extremes");
        cmd(ui, app, ctx, "contour.simplify");
        ui.separator();
        cmd(ui, app, ctx, "contour.remove_overlap");
    });
    ui.menu_button("Tools", |ui| {
        ui.menu_button(format!("{}  Transform", I::SCALING), |ui| {
            cmd(ui, app, ctx, "tools.flip_h");
            cmd(ui, app, ctx, "tools.flip_v");
            cmd(ui, app, ctx, "tools.rotate_left");
            cmd(ui, app, ctx, "tools.rotate_right");
        });
        ui.menu_button(format!("{}  Guides", I::RULER_DIMENSION_LINE), |ui| {
            cmd(ui, app, ctx, "tools.guide_center");
            cmd(ui, app, ctx, "tools.remove_guides");
        });
        ui.separator();
        cmd(ui, app, ctx, "tools.actions");
        ui.separator();
        cmd(ui, app, ctx, "tools.font_audit");
        ui.menu_button(format!("{}  Tool", I::PEN_TOOL), |ui| {
            for t in Tool::ALL {
                if item(ui, t.icon(), t.label(), t.key(), true) {
                    app.tool = t;
                }
            }
        });
        cmd(ui, app, ctx, "tools.shortcuts");
    });
    ui.menu_button("Script", |ui| {
        cmd(ui, app, ctx, "script.scripting");
        cmd(ui, app, ctx, "script.output");
    });
    ui.menu_button("View", |ui| {
        let glyph_tab = app.in_glyph_tab();
        ui.menu_button(format!("{}  Zoom", I::ZOOM_IN), |ui| {
            cmd(ui, app, ctx, "view.zoom_in");
            cmd(ui, app, ctx, "view.zoom_out");
            cmd(ui, app, ctx, "view.fit");
            if !glyph_tab {
                cmd(ui, app, ctx, "view.larger_cells");
                cmd(ui, app, ctx, "view.smaller_cells");
            }
        });
        ui.separator();
        let mut changed = false;
        changed |= check(ui, &mut app.prefs.show_metrics, "Metrics & Font Dimensions");
        changed |= check(ui, &mut app.prefs.show_zones, "Zones");
        changed |= check(ui, &mut app.prefs.show_guides, "Guides");
        changed |= check(ui, &mut app.prefs.show_grid, "Grid");
        ui.separator();
        changed |= check(ui, &mut app.prefs.show_nodes, "Nodes & Handles");
        changed |= check(ui, &mut app.prefs.show_coords, "Node Coordinates");
        changed |= check(ui, &mut app.prefs.show_anchors, "Anchors");
        changed |= check(ui, &mut app.prefs.show_kerning, "Kerning");
        changed |= check(ui, &mut app.prefs.show_fill, "True Fill   (Shift+Space)");
        changed |= check(ui, &mut app.prefs.show_template, "Template Glyphs");
        ui.separator();
        changed |= check(ui, &mut app.prefs.snap, "Enable Snapping");
        if changed {
            app.prefs_dirty = true;
        }
    });
    ui.menu_button("Window", |ui| {
        cmd(ui, app, ctx, "window.new_font_tab");
        cmd(ui, app, ctx, "window.new_glyph_tab");
        if item(ui, I::MOVE_HORIZONTAL, "New Metrics Tab", "", true) {
            run_cmd(app, ctx, "font.metrics_tab");
        }
        if item(ui, I::ARROW_LEFT_RIGHT, "New Kerning Tab", "Ctrl+Alt+Shift+K", true) {
            run_cmd(app, ctx, "font.kerning_tab");
        }
        ui.separator();
        if check(ui, &mut app.prefs.show_toolbar, "Toolbar") {
            app.prefs_dirty = true;
        }
        if check(ui, &mut app.prefs.show_right, "Panels") {
            app.prefs_dirty = true;
        }
        if check(ui, &mut app.prefs.show_left, "Sidebar") {
            app.prefs_dirty = true;
        }
        ui.menu_button(format!("{}  Panels", I::PANEL_RIGHT), |ui| {
            for p in PanelId::ALL {
                let mut on = app.prefs.right_panels.contains(&p);
                if ui.checkbox(&mut on, format!("{}  {}", p.icon(), p.label())).changed() {
                    toggle_panel(app, p);
                }
            }
        });
        ui.separator();
        cmd(ui, app, ctx, "window.next_tab");
        cmd(ui, app, ctx, "window.previous_tab");
        cmd(ui, app, ctx, "window.close_tab");
        cmd(ui, app, ctx, "window.close_glyph_tabs");
    });
    ui.menu_button("Help", |ui| {
        cmd(ui, app, ctx, "tools.shortcuts");
        cmd(ui, app, ctx, "help.getting_started");
        ui.separator();
        cmd(ui, app, ctx, "help.about");
    });
}

pub fn toggle_panel(app: &mut App, p: PanelId) {
    if let Some(i) = app.prefs.right_panels.iter().position(|x| *x == p) {
        app.prefs.right_panels.remove(i);
    } else {
        app.prefs.right_panels.insert(0, p);
        app.prefs.show_right = true;
    }
    app.prefs_dirty = true;
}

// ------------------------------------------------------------------------------------------
// Tab strip + tool row

pub fn tab_bar(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    Panel::top("tabbar").exact_size(38.0).frame(Frame::new().fill(pal.deep).inner_margin(Margin::symmetric(6, 0))).show_separator_line(false).show(ui, |ui| {
        let rect = ui.max_rect();
        ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, pal.border));
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let mut close = None;
            let mut activate = None;
            let n = app.tabs.len();
            for ti in 0..n {
                let t = &app.tabs[ti];
                let d = &app.docs[t.doc.min(app.docs.len() - 1)];
                let (icon, label) = match t.kind {
                    TabKind::Font => (I::LAYOUT_GRID, d.title()),
                    TabKind::Glyph => {
                        let s = if t.text.len() == 1 { t.text[0].clone() } else { t.text_input.chars().take(18).collect::<String>() };
                        (I::LETTER_TEXT, s)
                    }
                };
                let sel = ti == app.active;
                let galley = ui.painter().layout_no_wrap(label.clone(), egui::FontId::proportional(12.5), pal.text);
                let w = (galley.size().x + 56.0).min(240.0);
                let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 28.0), Sense::click());
                let bg = if sel { pal.panel } else if resp.hovered() { pal.hover.gamma_multiply(0.6) } else { Color32::TRANSPARENT };
                ui.painter().rect_filled(r, CornerRadius::same(5), bg);
                if sel {
                    ui.painter().line_segment([r.left_bottom() + Vec2::new(6.0, 0.0), r.right_bottom() - Vec2::new(6.0, 0.0)], Stroke::new(2.0, pal.accent));
                }
                ui.painter().text(r.left_center() + Vec2::new(14.0, 0.0), Align2::CENTER_CENTER, icon, icon_font(13.0), if sel { pal.accent_text } else { pal.muted });
                let clip = egui::Rect::from_min_max(r.min + Vec2::new(26.0, 0.0), r.max - Vec2::new(24.0, 0.0));
                ui.painter().with_clip_rect(clip).text(r.left_center() + Vec2::new(26.0, 0.0), Align2::LEFT_CENTER, &label, egui::FontId::proportional(12.5), if sel { pal.text } else { pal.muted });
                let xr = egui::Rect::from_center_size(r.right_center() - Vec2::new(13.0, 0.0), Vec2::splat(18.0));
                let xh = ui.rect_contains_pointer(xr);
                if resp.hovered() || sel {
                    if xh {
                        ui.painter().rect_filled(xr, CornerRadius::same(4), pal.hover);
                    }
                    ui.painter().text(xr.center(), Align2::CENTER_CENTER, I::X, icon_font(11.0), pal.muted);
                }
                if resp.clicked() {
                    if xh {
                        close = Some(ti);
                    } else {
                        activate = Some(ti);
                    }
                }
                if resp.middle_clicked() {
                    close = Some(ti);
                }
            }
            if let Some(ti) = activate {
                app.active = ti;
            }
            if let Some(ti) = close {
                app.close_tab(ti);
            }
            if icon_button(ui, &pal, I::PLUS, "New font", false).clicked() {
                app.dlg.welcome = true;
            }
            // Right side: tool row, flag colours, glyph search.
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                let r = ui.add(egui::TextEdit::singleline(&mut app.dlg.find_text).hint_text(format!("{}  Search glyphs", I::SEARCH)).desired_width(160.0));
                if r.changed() {
                    let q = app.dlg.find_text.clone();
                    let d = app.doc_mut();
                    d.filter = if q.trim().is_empty() { GlyphFilter::All } else { GlyphFilter::Search(q) };
                    d.view_key = u64::MAX;
                    if app.in_glyph_tab() {
                        if let Some(ti) = app.tabs.iter().position(|t| t.doc == app.doc_idx() && t.kind == TabKind::Font) {
                            app.active = ti;
                        }
                    }
                }
                vsep(ui, &pal);
                for i in (0..FLAG_COLORS.len()).rev() {
                    let c = FLAG_COLORS[i];
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(16.0, 16.0), Sense::click());
                    ui.painter().rect_filled(r.shrink(1.0), CornerRadius::same(3), Color32::from_rgb(c.0, c.1, c.2));
                    if resp.hovered() {
                        ui.painter().rect_stroke(r, CornerRadius::same(3), Stroke::new(1.5, pal.text), StrokeKind::Outside);
                    }
                    if resp.on_hover_text(format!("Flag selected glyphs {}", c.3)).clicked() {
                        app.set_flag(Some(i));
                    }
                }
                if icon_button(ui, &pal, I::X, "Remove flag", false).clicked() {
                    app.set_flag(None);
                }
                ui.label(egui::RichText::new(I::FLAG).font(icon_font(13.0)).color(pal.muted));
                vsep(ui, &pal);
                if app.prefs.show_toolbar {
                    for t in [Tool::Measure, Tool::Ellipse, Tool::Rectangle, Tool::Eraser, Tool::Knife, Tool::Rapid, Tool::Pen, Tool::Text, Tool::Kerning, Tool::Metrics, Tool::Element, Tool::Contour] {
                        if icon_button(ui, &pal, t.icon(), &format!("{} tool ({})", t.label(), t.key()), app.tool == t).clicked() {
                            set_tool(app, t);
                        }
                    }
                }
            });
        });
    });
}

/// Choosing a tool from the font window opens the focused glyph to use it.
pub fn set_tool(app: &mut App, t: Tool) {
    app.tool = t;
    app.edit.pen_contour = None;
    if !app.in_glyph_tab() {
        if let Some(gi) = app.current_glyph() {
            match t {
                Tool::Metrics | Tool::Kerning | Tool::Text => {
                    let d = app.doc();
                    let sel: Vec<String> = d.selected().iter().map(|&i| d.font.glyphs[i].name.clone()).collect();
                    let names = if sel.len() > 1 { sel } else { vec![d.font.glyphs[gi].name.clone()] };
                    let di = app.doc_idx();
                    app.tabs.push(Tab::glyph(di, names, 0));
                    app.active = app.tabs.len() - 1;
                }
                _ => app.open_glyph_tab(gi),
            }
            app.tool = t;
        }
    }
}

// ------------------------------------------------------------------------------------------
// Status bar

pub fn status_bar(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    Panel::bottom("statusbar").exact_size(26.0).frame(Frame::new().fill(pal.title).inner_margin(Margin::symmetric(10, 0))).show_separator_line(false).show(ui, |ui| {
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            let now = ui.ctx().input(|i| i.time);
            let msg = match &app.status {
                Some((s, until)) if *until > now => s.clone(),
                _ => {
                    if app.in_glyph_tab() {
                        format!("{} tool · Space+drag pans · wheel zooms · Ctrl+] next glyph", app.tool.label())
                    } else {
                        "Double-click a glyph to edit it · Ctrl+Click adds to the selection · Shift+Click selects a range".to_string()
                    }
                }
            };
            // Agent (MCP) indicator: shown while an agent can reach the app; lit briefly after each call.
            let listening = app.mcp.stdio || app.mcp.http_port().is_some();
            if listening {
                let active = app.mcp.last_call.as_ref().map(|(_, t)| now - *t < 1.5).unwrap_or(false);
                let col = if active { pal.accent_text } else { pal.muted };
                let tip = match (&app.mcp.last_call, app.mcp.http_port()) {
                    (Some((n, _)), Some(p)) => format!("Agents can connect at http://127.0.0.1:{p}/mcp · {} call(s) · last: {n}", app.mcp.calls),
                    (Some((n, _)), None) => format!("An agent is connected over stdio · {} call(s) · last: {n}", app.mcp.calls),
                    (None, Some(p)) => format!("Agents can connect at http://127.0.0.1:{p}/mcp"),
                    (None, None) => "An agent is connected over stdio".to_string(),
                };
                let r = ui.label(egui::RichText::new(format!("{}  Agent", I::BOT)).size(11.5).color(col)).on_hover_text(tip);
                if r.clicked() {
                    app.dlg.prefs = true;
                    app.dlg.prefs_page = 6;
                }
                if active {
                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(300));
                }
            }
            ui.label(egui::RichText::new(I::INFO).font(icon_font(12.0)).color(pal.muted));
            ui.label(egui::RichText::new(msg).size(11.5).color(pal.muted));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let d = app.doc();
                let m = d.master;
                if app.in_glyph_tab() {
                    let t = app.tab();
                    ui.label(egui::RichText::new(format!("{:.0}%", t.zoom * 100.0)).size(11.5).color(pal.muted));
                    if let Some(p) = app.edit.hover_font {
                        ui.label(egui::RichText::new(format!("x {:.0}  y {:.0}", p[0], p[1])).size(11.5).color(pal.muted));
                    }
                    if let Some(gi) = app.current_glyph() {
                        let g = &d.font.glyphs[gi];
                        let l = g.layer(m);
                        ui.label(egui::RichText::new(format!("{} · width {:.0} · LSB {:.0} · RSB {:.0} · {} nodes", g.name, l.width, d.font.lsb(gi, m), d.font.rsb(gi, m), l.node_count())).size(11.5).color(pal.text));
                    }
                } else {
                    let total = d.font.glyphs.len();
                    let shown = d.view_cache.len();
                    let sel = d.sel.len();
                    let mname = d.font.masters[m].name.clone();
                    let mut size = app.prefs.cell_size;
                    if ui.add(egui::Slider::new(&mut size, 40.0..=320.0).show_value(false)).on_hover_text("Cell size").changed() {
                        app.prefs.cell_size = size;
                        app.prefs_dirty = true;
                    }
                    ui.label(egui::RichText::new(I::LAYOUT_GRID).font(icon_font(12.0)).color(pal.muted));
                    ui.label(egui::RichText::new(format!("{sel}/{shown} selected · {total} glyphs · master {mname}")).size(11.5).color(pal.text));
                }
            });
        });
    });
}

pub fn resize_edges(ctx: &egui::Context) {
    if ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
        return;
    }
    let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) else { return };
    let rect = ctx.content_rect();
    let m = 5.0;
    let l = pos.x - rect.left() < m;
    let r = rect.right() - pos.x < m;
    let t = pos.y - rect.top() < m;
    let b = rect.bottom() - pos.y < m;
    use egui::viewport::ResizeDirection as D;
    let dir = match (l, r, t, b) {
        (true, _, true, _) => Some(D::NorthWest),
        (_, true, true, _) => Some(D::NorthEast),
        (true, _, _, true) => Some(D::SouthWest),
        (_, true, _, true) => Some(D::SouthEast),
        (true, ..) => Some(D::West),
        (_, true, ..) => Some(D::East),
        (_, _, true, _) => Some(D::North),
        (.., true) => Some(D::South),
        _ => None,
    };
    if let Some(d) = dir {
        let icon = match d {
            D::North | D::South => egui::CursorIcon::ResizeVertical,
            D::East | D::West => egui::CursorIcon::ResizeHorizontal,
            D::NorthWest | D::SouthEast => egui::CursorIcon::ResizeNwSe,
            _ => egui::CursorIcon::ResizeNeSw,
        };
        ctx.set_cursor_icon(icon);
        if ctx.input(|i| i.pointer.primary_pressed()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(d));
        }
    }
}

// ------------------------------------------------------------------------------------------
// Shortcuts

pub fn shortcuts(app: &mut App, ctx: &egui::Context) {
    let typing = ctx.memory(|m| m.focused().is_some()) || (app.in_glyph_tab() && app.tool == Tool::Text);
    let (cmd, shift, alt) = ctx.input(|i| (i.modifiers.command, i.modifiers.shift, i.modifiers.alt));
    let pressed = |k: Key| ctx.input(|i| i.key_pressed(k));
    if cmd {
        if pressed(Key::N) && !alt {
            app.new_font("Unnamed", app.prefs.default_glyph_set);
        }
        if pressed(Key::N) && alt {
            app.dlg.add_glyphs = true;
        }
        if pressed(Key::O) {
            app.open_dialog();
        }
        if pressed(Key::S) {
            app.save(ctx, shift);
        }
        if pressed(Key::E) {
            app.dlg.export = true;
        }
        if pressed(Key::F) && alt {
            app.dlg.font_info = true;
        } else if pressed(Key::F) {
            app.dlg.find = true;
        }
        if pressed(Key::Comma) {
            app.dlg.prefs = true;
        }
        if pressed(Key::W) {
            let a = app.active;
            app.close_tab(a);
        }
        if pressed(Key::Tab) && app.tabs.len() > 1 {
            app.active = if shift { (app.active + app.tabs.len() - 1) % app.tabs.len() } else { (app.active + 1) % app.tabs.len() };
        }
        if !typing {
            if pressed(Key::Z) {
                if shift {
                    app.redo(ctx);
                } else {
                    app.undo(ctx);
                }
            }
            if pressed(Key::Y) && !app.in_glyph_tab() {
                app.redo(ctx);
            }
            if pressed(Key::C) && !alt && !shift {
                app.copy(ctx);
            }
            if pressed(Key::X) {
                app.cut(ctx);
            }
            if pressed(Key::V) {
                app.paste(ctx);
            }
            if pressed(Key::D) {
                app.duplicate_glyphs(ctx);
            }
            if pressed(Key::A) {
                app.select_all();
            }
            if pressed(Key::I) && shift {
                app.invert_selection();
            }
            if pressed(Key::CloseBracket) {
                app.step_glyph(1);
            }
            if pressed(Key::OpenBracket) {
                app.step_glyph(-1);
            }
            if pressed(Key::B) {
                app.edit_layer("Break contour", op_break);
            }
            if pressed(Key::J) && !alt {
                app.edit_layer("Join contour", op_join);
            }
            if pressed(Key::J) && alt {
                app.contour_op("Nodes at extremes", op_extremes);
            }
            if pressed(Key::R) && alt {
                app.contour_op("Reverse contours", op_reverse);
            }
            if pressed(Key::Backslash) {
                if let Some(gi) = app.current_glyph() {
                    let n = app.doc().font.glyphs[gi].name.clone();
                    let u = app.doc().font.glyphs[gi].unicode_str();
                    app.dlg.rename = Some((gi, n, u));
                }
            }
            if pressed(Key::P) && shift {
                app.dlg.shortcuts = true;
            }
            if pressed(Key::T) && shift {
                app.dlg.actions = true;
            }
            if pressed(Key::K) && alt && shift {
                app.open_text_tab("AVATAR Toyota WAVE", Tool::Kerning);
            }
            if pressed(Key::Equals) || pressed(Key::Plus) {
                if app.in_glyph_tab() {
                    app.tab_mut().zoom *= 1.25;
                } else {
                    app.prefs.cell_size = (app.prefs.cell_size * 1.15).min(320.0);
                }
            }
            if pressed(Key::Minus) {
                if app.in_glyph_tab() {
                    app.tab_mut().zoom /= 1.25;
                } else {
                    app.prefs.cell_size = (app.prefs.cell_size / 1.15).max(40.0);
                }
            }
            if pressed(Key::Num0) {
                app.tab_mut().fit = true;
            }
        }
        return;
    }
    if typing {
        return;
    }
    if pressed(Key::Delete) || pressed(Key::Backspace) {
        app.delete(ctx);
    }
    if pressed(Key::Enter) && alt && app.in_glyph_tab() {
        app.edit_layer("Toggle smooth", op_toggle_smooth);
    }
    if pressed(Key::Space) && shift {
        app.prefs.show_fill = !app.prefs.show_fill;
    }
    if !app.in_glyph_tab() {
        if pressed(Key::Enter) {
            if let Some(gi) = app.current_glyph() {
                app.open_glyph_tab(gi);
            }
        }
        for (k, d) in [(Key::ArrowRight, 1i64), (Key::ArrowLeft, -1)] {
            if pressed(k) {
                app.step_glyph(d);
            }
        }
        return;
    }
    if !alt && !shift {
        let keymap = [
            (Key::A, Tool::Contour),
            (Key::V, Tool::Element),
            (Key::M, Tool::Metrics),
            (Key::K, Tool::Kerning),
            (Key::T, Tool::Text),
            (Key::P, Tool::Pen),
            (Key::Y, Tool::Rapid),
            (Key::F, Tool::Rectangle),
            (Key::E, Tool::Ellipse),
            (Key::J, Tool::Knife),
            (Key::X, Tool::Eraser),
            (Key::H, Tool::Hand),
            (Key::Z, Tool::Zoom),
            (Key::U, Tool::Measure),
            (Key::N, Tool::Anchor),
        ];
        for (k, t) in keymap {
            if pressed(k) {
                set_tool(app, t);
            }
        }
        if pressed(Key::OpenBracket) {
            let t = app.tab_mut();
            t.cur = t.cur.saturating_sub(1);
        }
        if pressed(Key::CloseBracket) {
            let t = app.tab_mut();
            if t.cur + 1 < t.text.len() {
                t.cur += 1;
            }
        }
    }
    if pressed(Key::Escape) {
        if app.edit.pen_contour.is_some() {
            app.edit.pen_contour = None;
        } else if let Some(gi) = app.current_glyph() {
            let di = app.doc_idx();
            let m = app.docs[di].master;
            app.docs[di].font.glyphs[gi].layer_mut(m).clear_selection();
        }
    }
}
