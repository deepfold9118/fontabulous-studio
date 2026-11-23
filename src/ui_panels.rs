//! The right dock: a column of collapsible panels plus a vertical strip with an
//! icon for every panel (click to show or hide it).

use egui::{Align2, Color32, CornerRadius, Frame, Panel, Rect, Sense, Stroke, StrokeKind, Vec2};

use crate::app::*;
use crate::font::{Class, Guide, FLAG_COLORS};
use crate::glyphnames;
use crate::icons as I;
use crate::theme::{bold, icon_font};
use crate::widgets::*;

pub fn icon_strip(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    Panel::right("panelstrip").exact_size(40.0).resizable(false).frame(Frame::new().fill(pal.title)).show(ui, |ui| {
        egui::ScrollArea::vertical().id_salt("strip").auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(4.0);
            ui.vertical_centered(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                for p in PanelId::ALL {
                    let on = app.prefs.right_panels.contains(&p) && app.prefs.show_right;
                    let tip = if p.coming_soon().is_some() { format!("{} (coming soon)", p.label()) } else { p.label().to_string() };
                    if icon_button(ui, &pal, p.icon(), &tip, on).clicked() {
                        if !app.prefs.show_right {
                            app.prefs.show_right = true;
                            if !app.prefs.right_panels.contains(&p) {
                                app.prefs.right_panels.insert(0, p);
                            }
                        } else {
                            crate::ui_shell::toggle_panel(app, p);
                        }
                    }
                }
                ui.add_space(6.0);
                if icon_button(ui, &pal, I::SETTINGS, "Preferences", app.dlg.prefs).clicked() {
                    app.dlg.prefs = true;
                }
            });
        });
    });
}

pub fn dock(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    Panel::right("paneldock").default_size(290.0).size_range(230.0..=520.0).resizable(true).frame(Frame::new().fill(pal.panel)).show(ui, |ui| {
        dock_header(ui, &pal, |ui| {
            ui.label(egui::RichText::new(I::PANEL_RIGHT).font(icon_font(14.0)).color(pal.muted));
            ui.label(egui::RichText::new("Panels").font(bold(12.5)).color(pal.text));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icon_button(ui, &pal, I::X, "Hide panels", false).clicked() {
                    app.prefs.show_right = false;
                }
                ui.menu_button(egui::RichText::new(I::PLUS).font(icon_font(14.0)).color(pal.muted), |ui| {
                    for p in PanelId::ALL {
                        let mut on = app.prefs.right_panels.contains(&p);
                        if ui.checkbox(&mut on, format!("{}  {}", p.icon(), p.label())).changed() {
                            crate::ui_shell::toggle_panel(app, p);
                        }
                    }
                });
            });
        });
        egui::ScrollArea::vertical().id_salt("dock_scroll").auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(4.0);
            let panels = app.prefs.right_panels.clone();
            for p in panels {
                panel_section(app, ui, p);
            }
            if app.prefs.right_panels.is_empty() {
                ui.add_space(20.0);
                ui.vertical_centered(|ui| ui.label(egui::RichText::new("Pick panels from the strip on the right.").color(pal.faint)));
            }
            ui.add_space(12.0);
        });
    });
}

fn panel_section(app: &mut App, ui: &mut egui::Ui, p: PanelId) {
    let pal = app.pal;
    let id = ui.make_persistent_id(("panel", p));
    let mut open = ui.data_mut(|d| *d.get_persisted_mut_or(id, true));
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::click());
    ui.painter().rect_filled(rect.shrink2(Vec2::new(4.0, 1.0)), CornerRadius::same(5), if resp.hovered() { pal.hover } else { pal.raised });
    let chev = if open { I::CHEVRON_DOWN } else { I::CHEVRON_RIGHT };
    ui.painter().text(rect.left_center() + Vec2::new(14.0, 0.0), Align2::CENTER_CENTER, chev, icon_font(11.0), pal.muted);
    ui.painter().text(rect.left_center() + Vec2::new(30.0, 0.0), Align2::CENTER_CENTER, p.icon(), icon_font(13.0), pal.accent_text);
    ui.painter().text(rect.left_center() + Vec2::new(44.0, 0.0), Align2::LEFT_CENTER, p.label().to_uppercase(), bold(10.5), pal.text);
    let xr = Rect::from_center_size(rect.right_center() - Vec2::new(16.0, 0.0), Vec2::splat(18.0));
    let close_hover = ui.rect_contains_pointer(xr);
    if resp.hovered() {
        ui.painter().text(xr.center(), Align2::CENTER_CENTER, I::X, icon_font(11.0), if close_hover { pal.text } else { pal.muted });
    }
    if resp.clicked() {
        if close_hover {
            crate::ui_shell::toggle_panel(app, p);
            return;
        }
        open = !open;
        ui.data_mut(|d| d.insert_persisted(id, open));
    }
    if !open {
        ui.add_space(2.0);
        return;
    }
    ui.add_space(4.0);
    egui::Frame::new().inner_margin(egui::Margin { left: 10, right: 10, top: 2, bottom: 8 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        if let Some(msg) = p.coming_soon() {
            ui.label(egui::RichText::new(format!("{}  Coming soon", I::SPARKLES)).font(bold(12.0)).color(pal.accent_text));
            ui.label(egui::RichText::new(msg).size(11.5).color(pal.muted));
            return;
        }
        match p {
            PanelId::Fonts => fonts_panel(app, ui),
            PanelId::Glyph => glyph_panel(app, ui),
            PanelId::LayersMasters => masters_panel(app, ui),
            PanelId::Elements => elements_panel(app, ui),
            PanelId::Transform => transform_panel(app, ui),
            PanelId::Metrics => metrics_panel(app, ui),
            PanelId::Kerning => kerning_panel(app, ui),
            PanelId::Classes => classes_panel(app, ui),
            PanelId::Features => features_panel(app, ui),
            PanelId::FontInfo => font_info_panel(app, ui),
            PanelId::AnchorsPins => anchors_panel(app, ui),
            PanelId::Guide => guide_panel(app, ui),
            PanelId::History => history_panel(app, ui),
            PanelId::Node => node_panel(app, ui),
            PanelId::Notepad => {
                ui.add(egui::TextEdit::multiline(&mut app.prefs.notepad).desired_rows(8).desired_width(f32::INFINITY).hint_text("Notes kept between sessions"));
            }
            PanelId::Output => output_panel(app, ui),
            PanelId::Preview => preview_panel(app, ui),
            PanelId::PairsPhrases => pairs_panel(app, ui),
            PanelId::Measurements => measure_panel(app, ui),
            PanelId::FontMap => font_map_panel(app, ui),
            PanelId::FontAudit => audit_panel(app, ui),
            PanelId::Color => color_panel(app, ui),
            PanelId::Gallery => gallery_panel(app, ui),
            PanelId::Selections => selections_panel(app, ui),
            PanelId::Source => source_panel(app, ui),
            PanelId::Tables => tables_panel(app, ui),
            PanelId::Variations => variations_panel(app, ui),
            PanelId::View => view_panel(app, ui),
            _ => {}
        }
    });
}

fn kv(ui: &mut egui::Ui, pal: &crate::theme::Palette, k: &str, v: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(k).size(11.5).color(pal.muted));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| ui.label(egui::RichText::new(v).size(11.5).color(pal.text)));
    });
}

fn num_row(ui: &mut egui::Ui, label: &str, v: &mut f32) -> egui::Response {
    ui.horizontal(|ui| {
        ui.add_sized([96.0, 20.0], egui::Label::new(egui::RichText::new(label).size(11.5)));
        ui.add(egui::DragValue::new(v).speed(1.0))
    })
    .inner
}

// ------------------------------------------------------------------------------------------

fn fonts_panel(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    let cur = app.doc_idx();
    let mut go = None;
    for (i, d) in app.docs.iter().enumerate() {
        let (r, _) = tree_row(ui, &pal, 0, None, I::BOOK_TYPE, &d.title(), Some(d.font.glyphs.len()), i == cur);
        if r.clicked() {
            go = Some(i);
        }
    }
    if let Some(i) = go {
        if let Some(ti) = app.tabs.iter().position(|t| t.doc == i && t.kind == TabKind::Font) {
            app.active = ti;
        } else {
            app.tabs.push(Tab::font(i));
            app.active = app.tabs.len() - 1;
        }
    }
    ui.horizontal(|ui| {
        if icon_button(ui, &pal, I::FILE_PLUS, "New font", false).clicked() {
            app.dlg.welcome = true;
        }
        if icon_button(ui, &pal, I::FOLDER_OPEN, "Open fonts", false).clicked() {
            app.open_dialog();
        }
        if icon_button(ui, &pal, I::INFO, "Font Info", false).clicked() {
            app.dlg.font_info = true;
        }
    });
}

fn glyph_panel(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    let Some(gi) = app.current_glyph() else {
        ui.label(egui::RichText::new("Select a glyph").color(pal.faint));
        return;
    };
    let di = app.doc_idx();
    let m = app.docs[di].master;
    // Preview.
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 110.0), Sense::hover());
    ui.painter().rect_filled(r, CornerRadius::same(6), pal.card);
    let fg = pal.text;
    let tm = pal.raised;
    crate::ui_fontwin::paint_glyph_in_rect(app, &ui.painter_at(r), di, gi, r.shrink(10.0), fg, tm);
    let (mut name, uni, mut width, lsb, rsb, flag) = {
        let f = &app.docs[di].font;
        let g = &f.glyphs[gi];
        (g.name.clone(), g.unicodes.iter().map(|u| format!("{u:04X}")).collect::<Vec<_>>().join(" "), g.layer(m).width, f.lsb(gi, m), f.rsb(gi, m), g.flag)
    };
    egui::Grid::new("glyph_props").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        ui.label(egui::RichText::new("Name").size(11.5).color(pal.muted));
        let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(150.0));
        if r.lost_focus() && name != app.docs[di].font.glyphs[gi].name {
            app.docs[di].checkpoint_font("Rename glyph");
            let old = app.docs[di].font.glyphs[gi].name.clone();
            if app.docs[di].font.rename_glyph(gi, &name) {
                for t in &mut app.tabs {
                    for n in &mut t.text {
                        if *n == old {
                            *n = name.clone();
                        }
                    }
                }
            }
        }
        ui.end_row();
        ui.label(egui::RichText::new("Unicode").size(11.5).color(pal.muted));
        let mut u = uni.clone();
        let r = ui.add(egui::TextEdit::singleline(&mut u).desired_width(150.0).hint_text("e.g. 0041"));
        if r.lost_focus() && u != uni {
            let us: Vec<u32> = u.split(|c: char| c == ' ' || c == ',').filter_map(|h| u32::from_str_radix(h.trim().trim_start_matches("U+"), 16).ok()).collect();
            app.docs[di].checkpoint_glyph(gi, "Unicode");
            app.docs[di].font.glyphs[gi].unicodes = us;
        }
        ui.end_row();
        ui.label(egui::RichText::new("Width").size(11.5).color(pal.muted));
        if ui.add(egui::DragValue::new(&mut width).speed(1.0)).changed() {
            apply_metrics(&mut app.docs[di].font, gi, m, None, None, Some(width.round()));
            app.docs[di].font.dirty = true;
        }
        ui.end_row();
        ui.label(egui::RichText::new("LSB / RSB").size(11.5).color(pal.muted));
        ui.label(format!("{lsb:.0} / {rsb:.0}"));
        ui.end_row();
        ui.label(egui::RichText::new("Category").size(11.5).color(pal.muted));
        let g = &app.docs[di].font.glyphs[gi];
        ui.label(glyphnames::category(&g.name, g.unicodes.first().copied()).label());
        ui.end_row();
    });
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new(I::FLAG).font(icon_font(12.0)).color(pal.muted));
        if ui.selectable_label(flag.is_none(), "None").clicked() {
            app.set_flag(None);
        }
        for (i, c) in FLAG_COLORS.iter().enumerate() {
            let (r, resp) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::click());
            ui.painter().rect_filled(r.shrink(1.0), CornerRadius::same(3), Color32::from_rgb(c.0, c.1, c.2));
            if flag == Some(i) {
                ui.painter().rect_stroke(r, CornerRadius::same(3), Stroke::new(1.5, pal.text), StrokeKind::Outside);
            }
            if resp.on_hover_text(c.3).clicked() {
                app.set_flag(Some(i));
            }
        }
    });
    let mut note = app.docs[di].font.glyphs[gi].note.clone();
    if ui.add(egui::TextEdit::multiline(&mut note).desired_rows(2).desired_width(f32::INFINITY).hint_text("Glyph note")).changed() {
        app.docs[di].font.glyphs[gi].note = note;
        app.docs[di].font.dirty = true;
    }
}

fn masters_panel(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    let di = app.doc_idx();
    let cur = app.docs[di].master;
    let mut pick = None;
    for (i, ms) in app.docs[di].font.masters.iter().enumerate() {
        let (r, _) = tree_row(ui, &pal, 0, None, I::LAYERS_3, &ms.name, None, i == cur);
        if r.clicked() {
            pick = Some(i);
        }
    }
    if let Some(i) = pick {
        app.docs[di].master = i;
    }
    let mut name = app.docs[di].font.masters[cur].name.clone();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Name").size(11.5).color(pal.muted));
        if ui.add(egui::TextEdit::singleline(&mut name).desired_width(140.0)).changed() {
            app.docs[di].font.masters[cur].name = name;
            app.docs[di].font.dirty = true;
        }
    });
    ui.horizontal(|ui| {
        if icon_button(ui, &pal, I::PLUS, "Add master (copy of current)", false).clicked() {
            app.dlg.add_variation = true;
        }
        let can = app.docs[di].font.masters.len() > 1;
        if icon_button_sized(ui, &pal, I::MINUS, "Remove current master", false, can).clicked() && can {
            app.docs[di].checkpoint_font("Remove master");
            app.docs[di].font.remove_master(cur);
            app.docs[di].master = 0;
        }
        if icon_button(ui, &pal, I::GIT_COMPARE, "Check compatibility", false).clicked() {
            let ctx = ui.ctx().clone();
            let bad = app.docs[di].font.glyphs.iter().filter(|g| g.layers.iter().skip(1).any(|l| !crate::font::compatible(&g.layers[0], l))).count();
            app.toast(&ctx, if bad == 0 { "All glyphs are compatible across masters".to_string() } else { format!("{bad} glyph(s) are incompatible across masters") });
        }
    });
    if let Some(gi) = app.current_glyph() {
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Current glyph in each master").size(11.0).color(pal.faint));
        let n = app.docs[di].font.masters.len();
        for i in 0..n {
            let g = &app.docs[di].font.glyphs[gi];
            let l = g.layer(i);
            let compat = if i == 0 || crate::font::compatible(&g.layers[0], l) { I::CHECK } else { I::TRIANGLE_ALERT };
            let col = if compat == I::CHECK { pal.green } else { pal.orange };
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(compat).font(icon_font(12.0)).color(col));
                ui.label(egui::RichText::new(format!("{} · {} contours · width {:.0}", app.docs[di].font.masters[i].name, l.contours.len(), l.width)).size(11.5));
            });
        }
    }
}

fn elements_panel(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    let Some(gi) = app.current_glyph() else {
        ui.label(egui::RichText::new("Open a glyph to see its elements").color(pal.faint));
        return;
    };
    let di = app.doc_idx();
    let m = app.docs[di].master;
    let l = app.docs[di].font.glyphs[gi].layer(m).clone();
    let mut action: Option<(usize, bool, usize)> = None; // (index, is_component, op) op 0 select, 1 delete, 2 reverse
    for (ci, c) in l.contours.iter().enumerate() {
        let sel = c.points.iter().any(|p| p.sel);
        let dir = if c.signed_area() > 0.0 { "↺" } else { "↻" };
        let label = format!("Contour {} · {} nodes {}{}", ci + 1, c.points.iter().filter(|p| p.t.on_curve()).count(), dir, if c.closed { "" } else { " · open" });
        ui.horizontal(|ui| {
            let (r, _) = tree_row(ui, &pal, 0, None, I::SPLINE, &label, None, sel);
            if r.clicked() {
                action = Some((ci, false, 0));
            }
        });
    }
    for (k, c) in l.components.iter().enumerate() {
        let label = format!("{} (component)", c.base);
        let (r, _) = tree_row(ui, &pal, 0, None, I::COMPONENT, &label, None, c.sel);
        if r.clicked() {
            action = Some((k, true, 0));
        }
    }
    if l.contours.is_empty() && l.components.is_empty() {
        ui.label(egui::RichText::new("Empty glyph — draw with the Pen (P)").size(11.5).color(pal.faint));
    }
    ui.horizontal(|ui| {
        if icon_button(ui, &pal, I::COMPONENT, "Add component…", false).clicked() {
            app.dlg.add_component = true;
        }
        if icon_button(ui, &pal, I::REFRESH_CW, "Reverse selected contours", false).clicked() {
            app.edit_layer("Reverse", op_reverse);
        }
        if icon_button(ui, &pal, I::BOXES, "Decompose components", false).clicked() {
            app.decompose();
        }
        if icon_button(ui, &pal, I::TRASH_2, "Delete selection", false).clicked() {
            app.edit_layer("Delete", delete_selection);
        }
    });
    if let Some((i, comp, _)) = action {
        let g = &mut app.docs[di].font.glyphs[gi];
        let l = g.layer_mut(m);
        l.clear_selection();
        if comp {
            l.components[i].sel = true;
        } else {
            for p in &mut l.contours[i].points {
                p.sel = true;
            }
        }
        g.touch();
    }
}

/// Transform matrix about the selection's (or glyph's) bounding box point.
pub fn apply_transform_about_center(app: &mut App, m: [f32; 6], label: &str) {
    let origin = 4; // centre
    apply_transform_at(app, m, label, origin);
}

fn sel_bounds(l: &crate::font::Layer) -> Option<[f32; 4]> {
    let any = l.has_selection();
    let mut b: Option<[f32; 4]> = None;
    let mut add = |x: f32, y: f32| {
        b = Some(match b {
            None => [x, y, x, y],
            Some(o) => [o[0].min(x), o[1].min(y), o[2].max(x), o[3].max(y)],
        })
    };
    for c in &l.contours {
        for p in &c.points {
            if !any || p.sel {
                add(p.x, p.y);
            }
        }
    }
    for a in &l.anchors {
        if any && a.sel {
            add(a.x, a.y);
        }
    }
    for c in &l.components {
        if !any || c.sel {
            add(c.t[4], c.t[5]);
        }
    }
    b
}

fn apply_transform_at(app: &mut App, m: [f32; 6], label: &str, origin: usize) {
    let in_glyph = app.in_glyph_tab();
    let f = move |l: &mut crate::font::Layer| {
        let b = sel_bounds(l).unwrap_or([0.0, 0.0, l.width, 0.0]);
        let ox = [b[0], (b[0] + b[2]) / 2.0, b[2]][origin % 3];
        let oy = [b[3], (b[1] + b[3]) / 2.0, b[1]][origin / 3];
        // T(o) * M * T(-o)
        let mm = [m[0], m[1], m[2], m[3], ox - (m[0] * ox + m[2] * oy) + m[4], oy - (m[1] * ox + m[3] * oy) + m[5]];
        l.transform(mm, true);
    };
    if in_glyph {
        app.edit_layer(label, f);
    } else {
        app.edit_glyphs(label, |g, mi| f(g.layer_mut(mi)));
    }
}

fn transform_panel(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    ui.horizontal(|ui| {
        // Origin picker (3×3).
        let (r, _) = ui.allocate_exact_size(Vec2::splat(42.0), Sense::hover());
        for k in 0..9 {
            let c = r.min + Vec2::new(7.0 + (k % 3) as f32 * 14.0, 7.0 + (k / 3) as f32 * 14.0);
            let cr = Rect::from_center_size(c, Vec2::splat(12.0));
            let resp = ui.interact(cr, ui.id().with(("origin", k)), Sense::click());
            let on = app.transform.origin == k;
            ui.painter().circle(c, 4.0, if on { pal.accent } else { pal.panel }, Stroke::new(1.0, if on { pal.accent } else { pal.muted }));
            if resp.clicked() {
                app.transform.origin = k;
            }
        }
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let o = app.transform.origin;
                for (icon, tip, mtx) in [
                    (I::FLIP_HORIZONTAL, "Flip horizontally", [-1.0, 0.0, 0.0, 1.0, 0.0, 0.0]),
                    (I::FLIP_VERTICAL, "Flip vertically", [1.0, 0.0, 0.0, -1.0, 0.0, 0.0]),
                    (I::ROTATE_CCW, "Rotate 90° left", [0.0, 1.0, -1.0, 0.0, 0.0, 0.0]),
                    (I::ROTATE_CW, "Rotate 90° right", [0.0, -1.0, 1.0, 0.0, 0.0, 0.0]),
                ] {
                    if icon_button(ui, &pal, icon, tip, false).clicked() {
                        apply_transform_at(app, mtx, tip, o);
                    }
                }
            });
            ui.horizontal(|ui| {
                for (icon, tip, k) in [
                    (I::ALIGN_START_VERTICAL, "Align left", 0),
                    (I::ALIGN_CENTER_VERTICAL, "Align centre", 1),
                    (I::ALIGN_END_VERTICAL, "Align right", 2),
                    (I::ALIGN_START_HORIZONTAL, "Align top", 3),
                    (I::ALIGN_CENTER_HORIZONTAL, "Align middle", 4),
                    (I::ALIGN_END_HORIZONTAL, "Align bottom", 5),
                ] {
                    if icon_button(ui, &pal, icon, tip, false).clicked() {
                        align_contours(app, k);
                    }
                }
            });
        });
    });
    let t = &mut app.transform;
    egui::Grid::new("xf").num_columns(4).spacing([6.0, 4.0]).show(ui, |ui| {
        ui.label("Move");
        ui.add(egui::DragValue::new(&mut t.dx).prefix("x ").speed(1.0));
        ui.add(egui::DragValue::new(&mut t.dy).prefix("y ").speed(1.0));
        ui.end_row();
        ui.label("Scale");
        let r1 = ui.add(egui::DragValue::new(&mut t.sx).suffix("%").speed(0.5));
        let r2 = ui.add(egui::DragValue::new(&mut t.sy).suffix("%").speed(0.5));
        if t.proportional {
            if r1.changed() {
                t.sy = t.sx;
            } else if r2.changed() {
                t.sx = t.sy;
            }
        }
        ui.toggle_value(&mut t.proportional, I::LOCK).on_hover_text("Keep proportions");
        ui.end_row();
        ui.label("Rotate");
        ui.add(egui::DragValue::new(&mut t.rotate).suffix("°").speed(0.5));
        ui.label("Slant");
        ui.add(egui::DragValue::new(&mut t.slant).suffix("°").speed(0.5));
        ui.end_row();
    });
    ui.horizontal(|ui| {
        if icon_text_button(ui, &pal, I::CHECK, "Apply", true).clicked() {
            let t = app.transform.clone();
            let (sx, sy) = (t.sx / 100.0, t.sy / 100.0);
            let a = t.rotate.to_radians();
            let k = t.slant.to_radians().tan();
            // scale → slant → rotate, then move
            let s = [sx, 0.0, k * sy, sy];
            let r = [a.cos(), a.sin(), -a.sin(), a.cos()];
            let mm = [r[0] * s[0] + r[2] * s[1], r[1] * s[0] + r[3] * s[1], r[0] * s[2] + r[2] * s[3], r[1] * s[2] + r[3] * s[3], t.dx, t.dy];
            let o = t.origin;
            apply_transform_at(app, mm, "Transform", o);
        }
        if icon_text_button(ui, &pal, I::ROTATE_CCW, "Reset", false).clicked() {
            app.transform = TransformState::default();
        }
    });
}

fn align_contours(app: &mut App, k: usize) {
    app.edit_layer("Align", |l| {
        let Some(b) = sel_bounds(l) else { return };
        let any = l.has_selection();
        for c in &mut l.contours {
            if any && !c.points.iter().any(|p| p.sel) {
                continue;
            }
            let Some(cb) = crate::geom::contour_bounds(c) else { continue };
            let (dx, dy) = match k {
                0 => (b[0] - cb[0], 0.0),
                1 => ((b[0] + b[2]) / 2.0 - (cb[0] + cb[2]) / 2.0, 0.0),
                2 => (b[2] - cb[2], 0.0),
                3 => (0.0, b[3] - cb[3]),
                4 => (0.0, (b[1] + b[3]) / 2.0 - (cb[1] + cb[3]) / 2.0),
                _ => (0.0, b[1] - cb[1]),
            };
            for p in &mut c.points {
                p.x += dx.round();
                p.y += dy.round();
            }
        }
    });
}

fn metrics_panel(app: &mut App, ui: &mut egui::Ui) {
    let di = app.doc_idx();
    let m = app.docs[di].master;
    let ms = &mut app.docs[di].font.masters[m];
    let mut changed = false;
    changed |= num_row(ui, "Ascender", &mut ms.ascender).changed();
    changed |= num_row(ui, "Cap height", &mut ms.cap_height).changed();
    changed |= num_row(ui, "x-height", &mut ms.x_height).changed();
    changed |= num_row(ui, "Descender", &mut ms.descender).changed();
    changed |= num_row(ui, "Line gap", &mut ms.line_gap).changed();
    changed |= num_row(ui, "Italic angle", &mut ms.italic_angle).changed();
    if changed {
        app.docs[di].font.dirty = true;
    }
    ui.add_space(4.0);
    let pal = app.pal;
    if let Some(gi) = app.current_glyph() {
        let f = &app.docs[di].font;
        let g = &f.glyphs[gi];
        ui.label(egui::RichText::new(format!("{}: LSB {:.0} · width {:.0} · RSB {:.0}", g.name, f.lsb(gi, m), g.layer(m).width, f.rsb(gi, m))).size(11.5).color(pal.muted));
    }
    if icon_text_button(ui, &pal, I::MOVE_HORIZONTAL, "Open Metrics Tab", false).clicked() {
        app.open_text_tab("nnonnoonnn HHOHHOOHHH", Tool::Metrics);
    }
}

fn kerning_panel(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    let di = app.doc_idx();
    let m = app.docs[di].master;
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut app.kern_filter).hint_text(format!("{}  Filter pairs", I::SEARCH)).desired_width(150.0));
        ui.label(egui::RichText::new(format!("{} pairs", app.docs[di].font.masters[m].kerning.len())).size(11.0).color(pal.muted));
    });
    let filt = app.kern_filter.to_lowercase();
    let mut remove = None;
    let mut edit = None;
    let mut open_pair = None;
    egui::ScrollArea::vertical().id_salt("kern_list").max_height(220.0).show(ui, |ui| {
        for (i, k) in app.docs[di].font.masters[m].kerning.iter().enumerate() {
            if !filt.is_empty() && !k.left.to_lowercase().contains(&filt) && !k.right.to_lowercase().contains(&filt) {
                continue;
            }
            ui.horizontal(|ui| {
                let lab = ui.add(egui::Label::new(egui::RichText::new(format!("{}  {}", k.left, k.right)).size(11.5)).sense(Sense::click()).truncate());
                if lab.double_clicked() {
                    open_pair = Some((k.left.clone(), k.right.clone()));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button(I::X).clicked() {
                        remove = Some(i);
                    }
                    let mut v = k.value;
                    if ui.add(egui::DragValue::new(&mut v).speed(1.0)).changed() {
                        edit = Some((i, v));
                    }
                });
            });
        }
    });
    if let Some(i) = remove {
        app.docs[di].checkpoint_font("Remove pair");
        app.docs[di].font.masters[m].kerning.remove(i);
    }
    if let Some((i, v)) = edit {
        app.docs[di].font.masters[m].kerning[i].value = v.round();
        app.docs[di].font.dirty = true;
    }
    if let Some((l, r)) = open_pair {
        let f = &app.docs[di].font;
        let resolve = |n: &str| if let Some(c) = n.strip_prefix('@') { f.classes.iter().find(|x| x.name == c).map(|x| if x.key.is_empty() { x.glyphs.first().cloned().unwrap_or_default() } else { x.key.clone() }).unwrap_or_default() } else { n.to_string() };
        let t = format!("/{} /{} ", resolve(&l), resolve(&r));
        app.open_text_tab(&t, Tool::Kerning);
    }
    ui.horizontal(|ui| {
        if icon_text_button(ui, &pal, I::ARROW_LEFT_RIGHT, "Kerning Tab", false).clicked() {
            app.open_text_tab("AVATAR Toyota WAVE", Tool::Kerning);
        }
        if icon_button(ui, &pal, I::SLIDERS_HORIZONTAL, "Adjust pairs…", false).clicked() {
            app.dlg.kern_adjust = true;
        }
    });
}

fn classes_panel(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    let di = app.doc_idx();
    let mut pick = None;
    egui::ScrollArea::vertical().id_salt("class_list").max_height(150.0).show(ui, |ui| {
        for (i, c) in app.docs[di].font.classes.iter().enumerate() {
            let side = match (c.kern_left, c.kern_right) {
                (true, true) => " · L R",
                (true, false) => " · L",
                (false, true) => " · R",
                _ => "",
            };
            let (r, _) = tree_row(ui, &pal, 0, None, I::BOXES, &format!("@{}{side}", c.name), Some(c.glyphs.len()), app.class_sel == Some(i));
            if r.clicked() {
                pick = Some(i);
            }
        }
    });
    if let Some(i) = pick {
        app.class_sel = Some(i);
    }
    ui.horizontal(|ui| {
        if icon_button(ui, &pal, I::PLUS, "New class from selected glyphs", false).clicked() {
            let d = &app.docs[di];
            let glyphs: Vec<String> = if app.in_glyph_tab() { app.tabs[app.active].text.iter().filter(|n| *n != "\n").cloned().collect() } else { d.selected().iter().map(|&i| d.font.glyphs[i].name.clone()).collect() };
            let n = d.font.classes.len() + 1;
            let key = glyphs.first().cloned().unwrap_or_default();
            app.docs[di].checkpoint_font("New class");
            app.docs[di].font.classes.push(Class { name: format!("class{n}"), glyphs, kern_left: false, kern_right: false, key });
            app.class_sel = Some(app.docs[di].font.classes.len() - 1);
        }
        let has = app.class_sel.is_some_and(|i| i < app.docs[di].font.classes.len());
        if icon_button_sized(ui, &pal, I::MINUS, "Delete class", false, has).clicked() && has {
            app.docs[di].checkpoint_font("Delete class");
            app.docs[di].font.classes.remove(app.class_sel.unwrap());
            app.class_sel = None;
        }
    });
    if let Some(i) = app.class_sel.filter(|&i| i < app.docs[di].font.classes.len()) {
        let c = &mut app.docs[di].font.classes[i];
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label("Name");
            changed |= ui.add(egui::TextEdit::singleline(&mut c.name).desired_width(140.0)).changed();
        });
        ui.horizontal(|ui| {
            changed |= ui.checkbox(&mut c.kern_left, "Kerning left (1st)").changed();
            changed |= ui.checkbox(&mut c.kern_right, "right (2nd)").changed();
        });
        let mut members = c.glyphs.join(" ");
        if ui.add(egui::TextEdit::multiline(&mut members).desired_rows(3).desired_width(f32::INFINITY).hint_text("Glyph names separated by spaces")).changed() {
            c.glyphs = members.split_whitespace().map(|s| s.to_string()).collect();
            changed = true;
        }
        ui.horizontal(|ui| {
            ui.label("Key glyph");
            changed |= ui.add(egui::TextEdit::singleline(&mut c.key).desired_width(100.0)).changed();
        });
        if changed {
            app.docs[di].font.dirty = true;
        }
    }
}

fn features_panel(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    let di = app.doc_idx();
    let r = ui.add(egui::TextEdit::multiline(&mut app.docs[di].font.features).code_editor().desired_rows(12).desired_width(f32::INFINITY).hint_text("# OpenType feature code (.fea)\nfeature liga {\n  sub f i by fi;\n} liga;"));
    if r.changed() {
        app.docs[di].font.dirty = true;
    }
    ui.horizontal(|ui| {
        if icon_text_button(ui, &pal, I::ARROW_LEFT_RIGHT, "Add kern feature", false).clicked() {
            let m = app.docs[di].master;
            let code = crate::io::kern_feature(&app.docs[di].font, m);
            app.docs[di].checkpoint_font("Generate kern feature");
            let f = &mut app.docs[di].font.features;
            if !f.is_empty() && !f.ends_with('\n') {
                f.push('\n');
            }
            f.push_str(&code);
        }
        if icon_button(ui, &pal, I::PLAY, "Compile features (coming soon)", false).clicked() {
            app.dlg.coming_soon = Some("Compiling OpenType features into exported fonts is coming in a later version. Feature code is saved with the font and exported to UFO.".into());
        }
    });
    let _ = pal;
}

fn font_info_panel(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    let di = app.doc_idx();
    let f = &mut app.docs[di].font;
    let mut changed = false;
    egui::Grid::new("fi_panel").num_columns(2).spacing([6.0, 4.0]).show(ui, |ui| {
        ui.label(egui::RichText::new("Family").size(11.5).color(pal.muted));
        changed |= ui.add(egui::TextEdit::singleline(&mut f.info.family).desired_width(150.0)).changed();
        ui.end_row();
        ui.label(egui::RichText::new("Style").size(11.5).color(pal.muted));
        changed |= ui.add(egui::TextEdit::singleline(&mut f.info.style).desired_width(150.0)).changed();
        ui.end_row();
        ui.label(egui::RichText::new("Units per em").size(11.5).color(pal.muted));
        changed |= ui.add(egui::DragValue::new(&mut f.info.upm).range(16.0..=16384.0)).changed();
        ui.end_row();
        ui.label(egui::RichText::new("Designer").size(11.5).color(pal.muted));
        changed |= ui.add(egui::TextEdit::singleline(&mut f.info.designer).desired_width(150.0)).changed();
        ui.end_row();
    });
    if changed {
        f.dirty = true;
    }
    if icon_text_button(ui, &pal, I::INFO, "Open Font Info…", false).clicked() {
        app.dlg.font_info = true;
    }
}

fn anchors_panel(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    let Some(gi) = app.current_glyph() else { return };
    let di = app.doc_idx();
    let m = app.docs[di].master;
    let mut remove = None;
    let mut changed = false;
    {
        let l = app.docs[di].font.glyphs[gi].layer_mut(m);
        for (i, a) in l.anchors.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(I::ANCHOR).font(icon_font(12.0)).color(pal.orange));
                changed |= ui.add(egui::TextEdit::singleline(&mut a.name).desired_width(70.0)).changed();
                changed |= ui.add(egui::DragValue::new(&mut a.x).prefix("x ")).changed();
                changed |= ui.add(egui::DragValue::new(&mut a.y).prefix("y ")).changed();
                if ui.small_button(I::X).clicked() {
                    remove = Some(i);
                }
            });
        }
        if l.anchors.is_empty() {
            ui.label(egui::RichText::new("No anchors. Use the Anchor tool (N) or add one below.").size(11.5).color(pal.faint));
        }
    }
    if let Some(i) = remove {
        app.edit_layer("Remove anchor", |l| {
            l.anchors.remove(i);
        });
    }
    if changed {
        app.docs[di].font.glyphs[gi].touch();
        app.docs[di].font.dirty = true;
    }
    if icon_text_button(ui, &pal, I::PLUS, "Add Anchor…", false).clicked() {
        app.dlg.add_anchor = true;
    }
}

fn guide_panel(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    let Some(gi) = app.current_glyph() else { return };
    let di = app.doc_idx();
    let m = app.docs[di].master;
    let mut remove = None;
    {
        let l = app.docs[di].font.glyphs[gi].layer_mut(m);
        for (i, g) in l.guides.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut g.x).prefix("x "));
                ui.add(egui::DragValue::new(&mut g.y).prefix("y "));
                ui.add(egui::DragValue::new(&mut g.angle).suffix("°"));
                if ui.small_button(I::X).clicked() {
                    remove = Some(i);
                }
            });
        }
    }
    if let Some(i) = remove {
        app.edit_layer("Remove guide", |l| {
            l.guides.remove(i);
        });
    }
    ui.horizontal(|ui| {
        if icon_text_button(ui, &pal, I::MOVE_VERTICAL, "Vertical", false).clicked() {
            app.edit_layer("Add guide", |l| l.guides.push(Guide { name: String::new(), x: (l.width / 2.0).round(), y: 0.0, angle: 90.0, color: None }));
        }
        if icon_text_button(ui, &pal, I::MOVE_HORIZONTAL, "Horizontal", false).clicked() {
            app.edit_layer("Add guide", |l| l.guides.push(Guide { name: String::new(), x: 0.0, y: 250.0, angle: 0.0, color: None }));
        }
    });
}

fn history_panel(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    let d = app.doc();
    let undo: Vec<String> = d.undo.iter().map(|u| u.label.clone()).collect();
    let redo: Vec<String> = d.redo.iter().rev().map(|u| u.label.clone()).collect();
    let mut steps: i64 = 0;
    egui::ScrollArea::vertical().id_salt("hist").max_height(200.0).show(ui, |ui| {
        let (r, _) = tree_row(ui, &pal, 0, None, I::FILE, "Opened / created", None, false);
        if r.clicked() {
            steps = -(undo.len() as i64);
        }
        for (i, l) in undo.iter().enumerate() {
            let (r, _) = tree_row(ui, &pal, 0, None, I::HISTORY, l, None, i + 1 == undo.len());
            if r.clicked() {
                steps = (i as i64 + 1) - undo.len() as i64;
            }
        }
        for (i, l) in redo.iter().enumerate() {
            let (r, _) = tree_row(ui, &pal, 0, None, I::REDO_2, &format!("{l} (undone)"), None, false);
            if r.clicked() {
                steps = i as i64 + 1;
            }
        }
    });
    let ctx = ui.ctx().clone();
    while steps < 0 {
        app.undo(&ctx);
        steps += 1;
    }
    while steps > 0 {
        app.redo(&ctx);
        steps -= 1;
    }
}

fn node_panel(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    let Some(gi) = app.current_glyph() else { return };
    let di = app.doc_idx();
    let m = app.docs[di].master;
    let sel = app.docs[di].font.glyphs[gi].layer(m).selected_points();
    if sel.is_empty() {
        ui.label(egui::RichText::new("Select a node with the Contour tool (A)").size(11.5).color(pal.faint));
        return;
    }
    let (ci, pi) = sel[0];
    let mut p = app.docs[di].font.glyphs[gi].layer(m).contours[ci].points[pi].clone();
    let kind = match p.t {
        crate::font::PtType::Off => "Handle",
        crate::font::PtType::Line => "Line node",
        crate::font::PtType::Curve => "Curve node",
        crate::font::PtType::QCurve => "TrueType node",
        crate::font::PtType::Move => "Start of open contour",
    };
    kv(ui, &pal, "Selected", &format!("{} point(s)", sel.len()));
    kv(ui, &pal, "Type", kind);
    let mut changed = false;
    ui.horizontal(|ui| {
        changed |= ui.add(egui::DragValue::new(&mut p.x).prefix("x ")).changed();
        changed |= ui.add(egui::DragValue::new(&mut p.y).prefix("y ")).changed();
        if p.t.on_curve() {
            changed |= ui.checkbox(&mut p.smooth, "Smooth").changed();
        }
    });
    if changed {
        app.docs[di].checkpoint_glyph(gi, "Edit node");
        let g = &mut app.docs[di].font.glyphs[gi];
        let l = g.layer_mut(m);
        let old = l.contours[ci].points[pi].clone();
        let (dx, dy) = (p.x - old.x, p.y - old.y);
        if dx != 0.0 || dy != 0.0 {
            l.clear_selection();
            l.contours[ci].points[pi].sel = true;
            l.move_selection(dx, dy);
        }
        l.contours[ci].points[pi].smooth = p.smooth;
        g.touch();
    }
    ui.horizontal(|ui| {
        if icon_button(ui, &pal, I::SPLINE, "Line to curve", false).clicked() {
            app.edit_layer("Line to curve", op_to_curve);
        }
        if icon_button(ui, &pal, I::SLASH, "Curve to line", false).clicked() {
            app.edit_layer("Curve to line", op_to_line);
        }
        if icon_button(ui, &pal, I::CIRCLE_DOT, "Toggle smooth", false).clicked() {
            app.edit_layer("Toggle smooth", op_toggle_smooth);
        }
        if icon_button(ui, &pal, I::FLAG, "Set start point", false).clicked() {
            app.edit_layer("Set start point", op_set_start);
        }
    });
}

fn output_panel(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    egui::ScrollArea::vertical().id_salt("out").max_height(160.0).stick_to_bottom(true).show(ui, |ui| {
        for l in &app.output {
            ui.label(egui::RichText::new(l).monospace().size(11.0).color(pal.muted));
        }
    });
    if ui.small_button("Clear").clicked() {
        app.output.clear();
    }
}

fn preview_panel(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut app.prefs.preview_text).desired_width(150.0));
        ui.add(egui::DragValue::new(&mut app.prefs.preview_size).range(8.0..=200.0).suffix(" px"));
    });
    let di = app.doc_idx();
    let names = text_to_names(&app.docs[di].font, &app.prefs.preview_text);
    paint_text_line(app, ui, di, &names, app.prefs.preview_size, None);
    if app.docs[di].font.masters.len() > 1 {
        ui.label(egui::RichText::new("All masters").size(11.0).color(pal.faint));
        let n = app.docs[di].font.masters.len();
        let keep = app.docs[di].master;
        for i in 0..n {
            app.docs[di].master = i;
            paint_text_line(app, ui, di, &names, (app.prefs.preview_size * 0.6).max(12.0), None);
        }
        app.docs[di].master = keep;
    }
}

/// Paint a line of glyphs at a pixel size (Preview, Variations).
pub fn paint_text_line(app: &mut App, ui: &mut egui::Ui, di: usize, names: &[String], px: f32, layer_override: Option<&[(usize, crate::font::Layer)]>) {
    let pal = app.pal;
    let m = app.docs[di].master;
    let (asc, desc, upm) = {
        let f = &app.docs[di].font;
        (f.masters[m].ascender, f.masters[m].descender, f.info.upm)
    };
    let s = px / upm;
    let h = (asc - desc) * s + 8.0;
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
    ui.painter().rect_filled(r, CornerRadius::same(4), pal.card);
    let painter = ui.painter_at(r);
    let mut x = r.left() + 6.0;
    let baseline = r.top() + 4.0 + asc * s;
    let mut prev: Option<String> = None;
    for n in names {
        let Some(gi) = app.docs[di].font.find(n) else { continue };
        if let Some(p) = &prev {
            if let Some((k, _, _)) = app.docs[di].font.kerning(m, p, n) {
                x += k * s;
            }
        }
        let view = crate::render::View { origin: egui::pos2(x, baseline), s };
        let w;
        if let Some(ov) = layer_override.and_then(|o| o.iter().find(|(g, _)| *g == gi)) {
            let t = crate::geom::tessellate(&ov.1.contours, 1.0);
            crate::render::paint_fill(&painter, &std::sync::Arc::new(t), view, pal.text);
            w = ov.1.width;
        } else {
            let t = app.cache.fill(di, &app.docs[di].font, gi, m);
            crate::render::paint_fill(&painter, &t, view, pal.text);
            w = app.docs[di].font.glyphs[gi].layer(m).width;
        }
        x += w * s;
        prev = Some(n.clone());
        if x > r.right() {
            break;
        }
    }
}

fn pairs_panel(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    for s in app.prefs.sample_texts.clone() {
        if task_row(ui, &pal, I::TEXT_QUOTE, None, &s, true).clicked() {
            app.open_text_tab(&s, Tool::Text);
        }
    }
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut app.pairs_text).hint_text("New phrase").desired_width(160.0));
        if icon_button(ui, &pal, I::PLUS, "Add phrase", false).clicked() && !app.pairs_text.trim().is_empty() {
            app.prefs.sample_texts.push(app.pairs_text.trim().to_string());
            app.pairs_text.clear();
            app.prefs_dirty = true;
        }
    });
}

fn measure_panel(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    match app.edit.measure {
        Some((a, b)) => {
            kv(ui, &pal, "Start", &format!("{:.0}, {:.0}", a[0], a[1]));
            kv(ui, &pal, "End", &format!("{:.0}, {:.0}", b[0], b[1]));
            kv(ui, &pal, "Distance", &format!("{:.1}", crate::geom::dist(a, b)));
            kv(ui, &pal, "Angle", &format!("{:.1}°", (b[1] - a[1]).atan2(b[0] - a[0]).to_degrees()));
        }
        None => {
            ui.label(egui::RichText::new("Drag with the Measure tool (U) in a glyph window.").size(11.5).color(pal.faint));
        }
    }
    if let Some(p) = app.edit.hover_font {
        kv(ui, &pal, "Pointer", &format!("{:.0}, {:.0}", p[0], p[1]));
    }
}

fn font_map_panel(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    let d = app.doc();
    let us: std::collections::HashSet<u32> = d.font.glyphs.iter().flat_map(|g| g.unicodes.iter().copied()).collect();
    for (a, b, name) in glyphnames::BLOCKS {
        let total = (a..=b).filter(|u| char::from_u32(*u).map(|c| !c.is_control()).unwrap_or(false)).count().max(1);
        let have = (a..=b).filter(|u| us.contains(u)).count();
        if have == 0 && a > 0x24F {
            continue;
        }
        ui.horizontal(|ui| {
            ui.add_sized([150.0, 16.0], egui::Label::new(egui::RichText::new(name).size(11.0)).truncate());
            let (r, _) = ui.allocate_exact_size(Vec2::new((ui.available_width() - 50.0).max(30.0), 8.0), Sense::hover());
            ui.painter().rect_filled(r, CornerRadius::same(4), pal.deep);
            let frac = have as f32 / total as f32;
            ui.painter().rect_filled(Rect::from_min_size(r.min, Vec2::new(r.width() * frac.min(1.0), r.height())), CornerRadius::same(4), pal.accent);
            ui.label(egui::RichText::new(format!("{have}/{total}")).size(10.5).color(pal.muted));
        });
    }
}

pub fn audit(font: &crate::font::Font, m: usize) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut names = std::collections::HashSet::new();
    let mut unis: std::collections::HashMap<u32, String> = Default::default();
    for (gi, g) in font.glyphs.iter().enumerate() {
        if !names.insert(g.name.clone()) {
            out.push((g.name.clone(), "Duplicate glyph name".into()));
        }
        for u in &g.unicodes {
            if let Some(o) = unis.insert(*u, g.name.clone()) {
                out.push((g.name.clone(), format!("Unicode U+{u:04X} also used by {o}")));
            }
        }
        let l = g.layer(m);
        for (ci, c) in l.contours.iter().enumerate() {
            if !c.closed {
                out.push((g.name.clone(), format!("Contour {} is open", ci + 1)));
            }
            if c.points.iter().filter(|p| p.t.on_curve()).count() < 2 {
                out.push((g.name.clone(), format!("Contour {} has fewer than 2 nodes", ci + 1)));
            }
            let mut tmp = c.clone();
            if crate::geom::clean_contour(&mut tmp) > 0 {
                out.push((g.name.clone(), format!("Contour {} has duplicate nodes", ci + 1)));
            }
            if c.points.iter().any(|p| p.x.fract() != 0.0 || p.y.fract() != 0.0) {
                out.push((g.name.clone(), format!("Contour {} has fractional coordinates", ci + 1)));
            }
            let mut ext = c.clone();
            if crate::geom::add_extremes(&mut ext) > 0 {
                out.push((g.name.clone(), format!("Contour {} is missing nodes at extremes", ci + 1)));
            }
        }
        for c in &l.components {
            if font.find(&c.base).is_none() {
                out.push((g.name.clone(), format!("Component refers to missing glyph '{}'", c.base)));
            }
        }
        for k in 1..g.layers.len() {
            if !crate::font::compatible(&g.layers[0], &g.layers[k]) {
                out.push((g.name.clone(), format!("Not compatible with master {}", font.masters.get(k).map(|m| m.name.as_str()).unwrap_or("?"))));
            }
        }
        if l.width < 0.0 {
            out.push((g.name.clone(), "Negative advance width".into()));
        }
        let _ = gi;
    }
    if font.find(".notdef").is_none() {
        out.push((".notdef".into(), "The font has no .notdef glyph".into()));
    }
    out
}

fn audit_panel(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    let di = app.doc_idx();
    let m = app.docs[di].master;
    let id = ui.make_persistent_id("audit_results");
    let mut results: Vec<(String, String)> = ui.data(|d| d.get_temp(id)).unwrap_or_default();
    if icon_text_button(ui, &pal, I::SHIELD_CHECK, "Run FontAudit", true).clicked() {
        results = audit(&app.docs[di].font, m);
        ui.data_mut(|d| d.insert_temp(id, results.clone()));
        let n = results.len();
        let ctx = ui.ctx().clone();
        app.toast(&ctx, format!("FontAudit found {n} issue(s)"));
    }
    let mut open = None;
    egui::ScrollArea::vertical().id_salt("audit").max_height(220.0).show(ui, |ui| {
        for (g, msg) in &results {
            let r = ui.add(egui::Label::new(egui::RichText::new(format!("{}  {g}: {msg}", I::TRIANGLE_ALERT)).size(11.5).color(pal.orange)).sense(Sense::click()));
            if r.double_clicked() {
                open = Some(g.clone());
            }
        }
    });
    if let Some(n) = open {
        if let Some(gi) = app.docs[di].font.find(&n) {
            app.open_glyph_tab(gi);
        }
    }
    if !results.is_empty() && icon_text_button(ui, &pal, I::WRENCH, "Fix: round, clean, extremes", false).clicked() {
        app.docs[di].checkpoint_font("FontAudit fixes");
        for g in &mut app.docs[di].font.glyphs {
            let l = g.layer_mut(m);
            op_clean(l);
            op_extremes(l);
            op_round(l);
            g.touch();
        }
        ui.data_mut(|d| d.remove::<Vec<(String, String)>>(id));
    }
}

fn color_panel(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    ui.label(egui::RichText::new("Flag the selected glyphs").size(11.5).color(pal.muted));
    ui.horizontal_wrapped(|ui| {
        for (i, c) in FLAG_COLORS.iter().enumerate() {
            let (r, resp) = ui.allocate_exact_size(Vec2::new(28.0, 22.0), Sense::click());
            ui.painter().rect_filled(r, CornerRadius::same(4), Color32::from_rgb(c.0, c.1, c.2));
            if resp.on_hover_text(c.3).clicked() {
                app.set_flag(Some(i));
            }
        }
        if ui.button("Clear").clicked() {
            app.set_flag(None);
        }
    });
}

fn gallery_panel(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    let Some(gi) = app.current_glyph() else { return };
    let di = app.doc_idx();
    let name = app.docs[di].font.glyphs[gi].name.clone();
    let users: Vec<usize> = app.docs[di].font.glyphs.iter().enumerate().filter(|(_, g)| g.layers.iter().any(|l| l.components.iter().any(|c| c.base == name))).map(|(i, _)| i).collect();
    ui.label(egui::RichText::new(format!("Glyphs using {name} as a component: {}", users.len())).size(11.5).color(pal.muted));
    ui.horizontal_wrapped(|ui| {
        for u in users {
            let (r, resp) = ui.allocate_exact_size(Vec2::new(44.0, 52.0), Sense::click());
            ui.painter().rect_filled(r, CornerRadius::same(4), pal.card);
            let fg = pal.text;
            let tm = pal.raised;
            crate::ui_fontwin::paint_glyph_in_rect(app, &ui.painter_at(r), di, u, r.shrink(4.0), fg, tm);
            if resp.on_hover_text(app.docs[di].font.glyphs[u].name.clone()).double_clicked() {
                app.open_glyph_tab(u);
            }
        }
    });
}

fn selections_panel(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    let mut pick = None;
    let mut remove = None;
    for (i, (n, g)) in app.selections.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(n).desired_width(120.0));
            ui.label(egui::RichText::new(format!("{}", g.len())).size(11.0).color(pal.muted));
            if ui.small_button(I::CHECK).on_hover_text("Select these glyphs").clicked() {
                pick = Some(i);
            }
            if ui.small_button(I::X).clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = pick {
        let names = app.selections[i].1.clone();
        let d = app.doc_mut();
        d.sel = names.iter().filter_map(|n| d.font.find(n)).collect();
    }
    if let Some(i) = remove {
        app.selections.remove(i);
    }
    if icon_text_button(ui, &pal, I::PLUS, "Save Selection", false).clicked() {
        let d = app.doc();
        let names: Vec<String> = d.selected().iter().map(|&i| d.font.glyphs[i].name.clone()).collect();
        let n = app.selections.len() + 1;
        app.selections.push((format!("Selection {n}"), names));
    }
}

fn source_panel(app: &mut App, ui: &mut egui::Ui) {
    let Some(gi) = app.current_glyph() else { return };
    let di = app.doc_idx();
    let m = app.docs[di].master;
    let g = &app.docs[di].font.glyphs[gi];
    let mut s = crate::io::glif_string(g, g.layer(m));
    egui::ScrollArea::vertical().id_salt("src").max_height(260.0).show(ui, |ui| {
        ui.add(egui::TextEdit::multiline(&mut s).code_editor().desired_width(f32::INFINITY).interactive(true));
    });
    if ui.small_button(format!("{}  Copy .glif", I::COPY)).clicked() {
        ui.ctx().copy_text(s);
    }
}

fn tables_panel(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    let di = app.doc_idx();
    let m = app.docs[di].master;
    let view: Vec<usize> = if app.docs[di].view_cache.is_empty() { (0..app.docs[di].font.glyphs.len()).collect() } else { app.docs[di].view_cache.clone() };
    ui.horizontal(|ui| {
        for (t, w) in [("Glyph", 80.0), ("Unicode", 54.0), ("Width", 54.0), ("LSB", 44.0), ("RSB", 44.0)] {
            ui.add_sized([w, 16.0], egui::Label::new(egui::RichText::new(t).size(10.5).color(pal.muted)));
        }
    });
    let row_h = 22.0;
    let mut set: Option<(usize, Option<f32>, Option<f32>, Option<f32>)> = None;
    let mut open = None;
    egui::ScrollArea::vertical().id_salt("tables").max_height(260.0).show_rows(ui, row_h, view.len(), |ui, range| {
        for &gi in &view[range] {
            let f = &app.docs[di].font;
            let g = &f.glyphs[gi];
            let (mut w, mut l, mut r) = (g.layer(m).width, f.lsb(gi, m).round(), f.rsb(gi, m).round());
            ui.horizontal(|ui| {
                if ui.add_sized([80.0, 18.0], egui::Label::new(egui::RichText::new(&g.name).size(11.5)).truncate().sense(Sense::click())).double_clicked() {
                    open = Some(gi);
                }
                ui.add_sized([54.0, 18.0], egui::Label::new(egui::RichText::new(g.unicode_str()).size(11.0).color(pal.muted)));
                if ui.add_sized([54.0, 18.0], egui::DragValue::new(&mut w)).changed() {
                    set = Some((gi, None, None, Some(w.round())));
                }
                if ui.add_sized([44.0, 18.0], egui::DragValue::new(&mut l)).changed() {
                    set = Some((gi, Some(l), None, None));
                }
                if ui.add_sized([44.0, 18.0], egui::DragValue::new(&mut r)).changed() {
                    set = Some((gi, None, Some(r), None));
                }
            });
        }
    });
    if let Some((gi, l, r, w)) = set {
        apply_metrics(&mut app.docs[di].font, gi, m, l, r, w);
        app.docs[di].font.dirty = true;
    }
    if let Some(gi) = open {
        app.open_glyph_tab(gi);
    }
}

fn variations_panel(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    let di = app.doc_idx();
    let nm = app.docs[di].font.masters.len();
    if app.docs[di].font.axes.is_empty() {
        ui.label(egui::RichText::new("No axes yet. Add a variation (a second master) to make a variable design.").size(11.5).color(pal.faint));
        if icon_text_button(ui, &pal, I::VARIABLE, "Add Variation…", false).clicked() {
            app.dlg.add_variation = true;
        }
        return;
    }
    let axes = app.docs[di].font.axes.clone();
    if app.var_loc.len() != axes.len() {
        app.var_loc = axes.iter().map(|a| a.default).collect();
    }
    for (i, a) in axes.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("{} ({})", a.name, a.tag)).size(11.5));
            ui.add(egui::Slider::new(&mut app.var_loc[i], a.min..=a.max));
        });
    }
    for (i, ms) in app.docs[di].font.masters.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(I::LAYERS_3).font(icon_font(12.0)).color(pal.muted));
            ui.add_sized([90.0, 18.0], egui::Label::new(egui::RichText::new(&ms.name).size(11.5)).truncate());
            if ms.location.len() < axes.len() {
                ms.location.resize(axes.len(), 0.0);
            }
            for v in ms.location.iter_mut() {
                ui.add(egui::DragValue::new(v).speed(1.0));
            }
            let _ = i;
        });
    }
    if nm >= 2 {
        // Live interpolation preview.
        let loc = app.var_loc.clone();
        let (a, b, t) = interp_masters(&app.docs[di].font, &loc);
        let names = text_to_names(&app.docs[di].font, &app.prefs.preview_text);
        let mut layers = Vec::new();
        let mut bad = 0;
        for n in &names {
            if let Some(gi) = app.docs[di].font.find(n) {
                let g = &app.docs[di].font.glyphs[gi];
                let (la, lb) = (g.layer(a), g.layer(b));
                if crate::font::compatible(la, lb) {
                    let mut l = crate::font::interpolate(la, lb, t);
                    l.contours = {
                        let mut tmp = app.docs[di].font.clone();
                        tmp.glyphs[gi].layers[0] = l.clone();
                        tmp.rebuild_index();
                        tmp.flat_contours(gi, 0)
                    };
                    layers.push((gi, l));
                } else {
                    bad += 1;
                }
            }
        }
        ui.label(egui::RichText::new(format!("Preview at {:?} (blend {:.0}%)", loc.iter().map(|v| v.round()).collect::<Vec<_>>(), t * 100.0)).size(11.0).color(pal.faint));
        let size = app.prefs.preview_size.min(64.0);
        paint_text_line(app, ui, di, &names, size, Some(&layers));
        if bad > 0 {
            ui.label(egui::RichText::new(format!("{bad} glyph(s) are not compatible and show the first master")).size(11.0).color(pal.orange));
        }
        ui.horizontal(|ui| {
            if icon_text_button(ui, &pal, I::SPARKLES, "Generate Instance…", false).clicked() {
                app.dlg.instance_loc = loc.clone();
                app.dlg.instance = true;
            }
            if icon_button(ui, &pal, I::PLUS, "Add instance at this location", false).clicked() {
                let n = app.docs[di].font.instances.len() + 1;
                app.docs[di].font.instances.push(crate::font::Instance { name: format!("Instance {n}"), location: loc, export: true });
            }
        });
        let mut remove = None;
        for (i, inst) in app.docs[di].font.instances.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(I::SPARKLES).font(icon_font(12.0)).color(pal.muted));
                ui.add(egui::TextEdit::singleline(&mut inst.name).desired_width(100.0));
                for v in inst.location.iter_mut() {
                    ui.add(egui::DragValue::new(v));
                }
                if ui.small_button(I::X).clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            app.docs[di].font.instances.remove(i);
        }
    }
}

fn view_panel(app: &mut App, ui: &mut egui::Ui) {
    let p = &mut app.prefs;
    ui.checkbox(&mut p.show_metrics, "Metrics lines");
    ui.checkbox(&mut p.show_zones, "Alignment zones");
    ui.checkbox(&mut p.show_guides, "Guides");
    ui.checkbox(&mut p.show_grid, "Grid");
    ui.checkbox(&mut p.show_nodes, "Nodes & handles");
    ui.checkbox(&mut p.show_coords, "Node coordinates");
    ui.checkbox(&mut p.show_anchors, "Anchors");
    ui.checkbox(&mut p.show_kerning, "Apply kerning");
    ui.checkbox(&mut p.show_fill, "True fill");
    ui.checkbox(&mut p.show_template, "Template glyphs");
    ui.horizontal(|ui| {
        ui.checkbox(&mut p.snap, "Snap to grid every");
        ui.add(egui::DragValue::new(&mut p.grid_step).range(1.0..=500.0));
    });
}
