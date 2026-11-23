//! The glyph window: tool strip, text bar and the editing canvas with every
//! drawing tool (contour, element, pen, pencil, shapes, knife, eraser, anchors,
//! metrics, kerning, text, measure, hand and zoom).

use egui::{Align2, Color32, CornerRadius, Frame, Key, Panel, PointerButton, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Vec2};

use crate::app::*;
use crate::font::{Anchor, Contour, Point, PtType};
use crate::geom::{self, P2};
use crate::icons as I;
use crate::render::{self, View};
use crate::theme::{bold, icon_font};
use crate::widgets::*;

pub fn tool_strip(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    Panel::left("toolstrip").exact_size(46.0).frame(Frame::new().fill(pal.panel)).resizable(false).show(ui, |ui| {
        ui.add_space(6.0);
        ui.vertical_centered(|ui| {
            ui.spacing_mut().item_spacing.y = 3.0;
            let groups: [&[Tool]; 5] = [
                &[Tool::Contour, Tool::Element],
                &[Tool::Metrics, Tool::Kerning, Tool::Text],
                &[Tool::Pen, Tool::Rapid, Tool::Rectangle, Tool::Ellipse],
                &[Tool::Knife, Tool::Eraser, Tool::Anchor, Tool::Measure],
                &[Tool::Hand, Tool::Zoom],
            ];
            for (gi, g) in groups.iter().enumerate() {
                if gi > 0 {
                    let (r, _) = ui.allocate_exact_size(Vec2::new(28.0, 7.0), Sense::hover());
                    ui.painter().line_segment([r.left_center(), r.right_center()], Stroke::new(1.0, pal.border));
                }
                for &t in g.iter() {
                    let r = icon_button(ui, &pal, t.icon(), &format!("{} tool ({})", t.label(), t.key()), app.tool == t);
                    if r.clicked() {
                        app.tool = t;
                        app.edit.pen_contour = None;
                    }
                }
            }
            ui.add_space(10.0);
            for (icon, tip, v) in [
                (I::EYE, "True fill (Shift+Space)", 0usize),
                (I::CIRCLE_DOT, "Show nodes", 1),
                (I::HASH, "Node coordinates", 2),
                (I::RULER_DIMENSION_LINE, "Metrics lines", 3),
                (I::MAGNET, "Snap to grid", 4),
            ] {
                let on = match v {
                    0 => app.prefs.show_fill,
                    1 => app.prefs.show_nodes,
                    2 => app.prefs.show_coords,
                    3 => app.prefs.show_metrics,
                    _ => app.prefs.snap,
                };
                if icon_button(ui, &pal, icon, tip, on).clicked() {
                    match v {
                        0 => app.prefs.show_fill = !on,
                        1 => app.prefs.show_nodes = !on,
                        2 => app.prefs.show_coords = !on,
                        3 => app.prefs.show_metrics = !on,
                        _ => app.prefs.snap = !on,
                    }
                }
            }
        });
    });
}

fn text_bar(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    let bar = Rect::from_min_size(ui.cursor().min, Vec2::new(ui.available_width(), 34.0));
    ui.painter().rect_filled(bar, 0.0, pal.panel);
    ui.painter().line_segment([bar.left_bottom(), bar.right_bottom()], Stroke::new(1.0, pal.border));
    let mut bui = ui.new_child(egui::UiBuilder::new().max_rect(bar.shrink2(Vec2::new(8.0, 4.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
    let ui2 = &mut bui;
    if icon_button(ui2, &pal, I::CHEVRON_LEFT, "Previous glyph (Ctrl+[)", false).clicked() {
        app.step_glyph(-1);
    }
    if icon_button(ui2, &pal, I::CHEVRON_RIGHT, "Next glyph (Ctrl+])", false).clicked() {
        app.step_glyph(1);
    }
    let di = app.doc_idx();
    let ti = app.active;
    let r = ui2.add(egui::TextEdit::singleline(&mut app.tabs[ti].text_input).hint_text("Type text or /glyphname").desired_width((ui2.available_width() - 360.0).max(160.0)));
    if r.changed() {
        let names = text_to_names(&app.docs[di].font, &app.tabs[ti].text_input);
        let t = &mut app.tabs[ti];
        if !names.is_empty() {
            t.text = names;
            t.cur = t.cur.min(t.text.len() - 1);
        }
    }
    egui::ComboBox::from_id_salt("sample_texts").selected_text(format!("{}  Samples", I::TEXT_QUOTE)).width(110.0).show_ui(ui2, |ui| {
        for s in app.prefs.sample_texts.clone() {
            if ui.selectable_label(false, &s).clicked() {
                let names = text_to_names(&app.docs[di].font, &s);
                let t = &mut app.tabs[ti];
                t.text_input = s.clone();
                t.text = names;
                t.cur = 0;
                t.fit = true;
            }
        }
    });
    let masters: Vec<String> = app.docs[di].font.masters.iter().map(|m| m.name.clone()).collect();
    let mut m = app.docs[di].master;
    egui::ComboBox::from_id_salt("master_pick").selected_text(format!("{}  {}", I::LAYERS_3, masters[m.min(masters.len() - 1)])).width(120.0).show_ui(ui2, |ui| {
        for (i, n) in masters.iter().enumerate() {
            ui.selectable_value(&mut m, i, n);
        }
    });
    app.docs[di].master = m;
    if icon_button(ui2, &pal, I::FOCUS, "Fit to window (Ctrl+0)", false).clicked() {
        app.tabs[ti].fit = true;
    }
    ui.allocate_rect(bar, Sense::hover());
}

struct Layout {
    /// (glyph index or None for missing, x offset, y offset, name)
    items: Vec<(Option<usize>, f32, f32, String)>,
    width: f32,
    line_h: f32,
    lines: usize,
}

fn layout_text(app: &App, di: usize, names: &[String]) -> Layout {
    let font = &app.docs[di].font;
    let m = app.docs[di].master;
    let ms = &font.masters[m.min(font.masters.len() - 1)];
    let line_h = (ms.ascender - ms.descender).max(font.info.upm) * 1.25;
    let mut items = Vec::new();
    let (mut x, mut y) = (0.0f32, 0.0f32);
    let mut maxw = 0.0f32;
    let mut lines = 1;
    let mut prev: Option<String> = None;
    for n in names {
        if n == "\n" {
            maxw = maxw.max(x);
            x = 0.0;
            y -= line_h;
            lines += 1;
            prev = None;
            items.push((None, x, y, n.clone()));
            continue;
        }
        let gi = font.find(n);
        if app.prefs.show_kerning {
            if let (Some(p), Some(_)) = (&prev, gi) {
                if let Some((v, _, _)) = font.kerning(m, p, n) {
                    x += v;
                }
            }
        }
        items.push((gi, x, y, n.clone()));
        x += gi.map(|g| font.glyphs[g].layer(m).width).unwrap_or(font.info.upm * 0.5);
        prev = gi.map(|_| n.clone());
    }
    Layout { items, width: maxw.max(x), line_h, lines }
}

fn snap(app: &App, v: f32) -> f32 {
    if app.prefs.snap && app.prefs.grid_step > 0.0 {
        (v / app.prefs.grid_step).round() * app.prefs.grid_step
    } else {
        v.round()
    }
}

pub fn canvas(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    text_bar(app, ui);
    let di = app.doc_idx();
    let ti = app.active;
    let rect = ui.available_rect_before_wrap();
    let resp = ui.allocate_rect(rect, Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    let light_canvas = !pal.dark;
    let bg = if light_canvas { Color32::from_rgb(0xfc, 0xfc, 0xfd) } else { Color32::from_rgb(0x15, 0x18, 0x1e) };
    painter.rect_filled(rect, 0.0, bg);
    let ink = if light_canvas { Color32::from_rgb(0x1c, 0x1f, 0x26) } else { Color32::from_rgb(0xe4, 0xe6, 0xea) };
    let line_col = if light_canvas { Color32::from_rgb(0xc9, 0xcd, 0xd4) } else { Color32::from_rgb(0x34, 0x3a, 0x46) };

    let names = app.tabs[ti].text.clone();
    if names.is_empty() {
        return;
    }
    let lay = layout_text(app, di, &names);
    let m = app.docs[di].master;
    let (asc, desc, xh, caph, upm, italic) = {
        let f = &app.docs[di].font;
        let ms = &f.masters[m.min(f.masters.len() - 1)];
        (ms.ascender, ms.descender, ms.x_height, ms.cap_height, f.info.upm, ms.italic_angle)
    };
    let em_h = (asc - desc).max(upm);
    // Fit the text line(s) into the canvas.
    if app.tabs[ti].fit {
        let total_h = em_h + lay.line_h * (lay.lines as f32 - 1.0);
        let s = (rect.height() * 0.72 / total_h).min(rect.width() * 0.86 / lay.width.max(upm * 0.5)).clamp(0.01, 40.0);
        let ox = rect.center().x - lay.width * s / 2.0;
        let oy = rect.top() + (rect.height() - total_h * s) / 2.0 + asc * s;
        let t = &mut app.tabs[ti];
        t.zoom = s;
        t.pan = Vec2::new(ox - rect.left(), oy - rect.top());
        t.fit = false;
    }
    let (s, pan) = (app.tabs[ti].zoom, app.tabs[ti].pan);
    let base = View { origin: rect.min + pan, s };

    // ------------------------------------------------------------------ zoom & pan input
    let hover = resp.hover_pos();
    if resp.hovered() {
        let (scroll, zoom_d, shift) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.modifiers.shift));
        let mut factor = zoom_d;
        if !shift && scroll.y != 0.0 {
            factor *= (scroll.y * 0.0018).exp();
        }
        if (factor - 1.0).abs() > 1e-4 {
            if let Some(hp) = hover {
                let new_s = (s * factor).clamp(0.01, 60.0);
                let f = base.to_font(hp);
                let t = &mut app.tabs[ti];
                t.zoom = new_s;
                t.pan = Vec2::new(hp.x - f[0] * new_s - rect.left(), hp.y + f[1] * new_s - rect.top());
            }
        } else if shift && (scroll.y != 0.0 || scroll.x != 0.0) {
            app.tabs[ti].pan += Vec2::new(scroll.y + scroll.x, 0.0);
        }
    }
    let space = ui.input(|i| i.key_down(Key::Space)) && !ui.ctx().memory(|mm| mm.focused().is_some());
    let (s, pan) = (app.tabs[ti].zoom, app.tabs[ti].pan);
    let base = View { origin: rect.min + pan, s };
    app.edit.hover_font = hover.map(|h| {
        let cur = app.tabs[ti].cur.min(lay.items.len() - 1);
        let (_, ox, oy, _) = &lay.items[cur];
        let p = base.to_font(h);
        [p[0] - ox, p[1] - oy]
    });

    // ------------------------------------------------------------------ backdrop: metrics & zones
    let cur = app.tabs[ti].cur.min(lay.items.len() - 1);
    let (cur_gi, cur_x, cur_y, _) = lay.items[cur].clone();
    let ev = View { origin: Pos2::new(base.origin.x + cur_x * s, base.origin.y - cur_y * s), s };
    let mut line_ys: Vec<f32> = vec![0.0];
    for it in &lay.items {
        if !line_ys.contains(&it.2) {
            line_ys.push(it.2);
        }
    }
    if app.prefs.show_zones {
        let zones = app.docs[di].font.masters[m].zones.clone();
        for &ly in &line_ys {
            for (pos, w) in &zones {
                let y0 = base.origin.y - (ly + pos) * s;
                let y1 = base.origin.y - (ly + pos + w) * s;
                let zr = Rect::from_min_max(Pos2::new(rect.left(), y0.min(y1)), Pos2::new(rect.right(), y0.max(y1)));
                painter.rect_filled(zr, 0.0, if light_canvas { Color32::from_rgba_unmultiplied(0x3b, 0x82, 0xf6, 18) } else { Color32::from_rgba_unmultiplied(0x3b, 0x82, 0xf6, 22) });
            }
        }
    }
    if app.prefs.show_metrics {
        for &ly in &line_ys {
            let mut labelled: Vec<f32> = Vec::new();
            for (v, label) in [(0.0, "baseline"), (xh, "x-height"), (caph, "caps"), (asc, "ascender"), (desc, "descender")] {
                let y = base.origin.y - (ly + v) * s;
                painter.line_segment([Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)], Stroke::new(if v == 0.0 { 1.2 } else { 1.0 }, line_col));
                // Skip a label that would overlap one already drawn (small zoom).
                if ly == cur_y && labelled.iter().all(|o| (o - y).abs() >= 12.0) {
                    labelled.push(y);
                    painter.text(Pos2::new(rect.left() + 6.0, y - 2.0), Align2::LEFT_BOTTOM, format!("{label} {v:.0}"), egui::FontId::proportional(10.0), pal.faint);
                }
            }
        }
    }
    if app.prefs.show_grid {
        let step = app.prefs.grid_step.max(1.0);
        if step * s > 6.0 {
            let f0 = base.to_font(rect.left_top());
            let f1 = base.to_font(rect.right_bottom());
            let gc = if light_canvas { Color32::from_rgb(0xee, 0xef, 0xf2) } else { Color32::from_rgb(0x1e, 0x22, 0x2a) };
            let mut x = (f0[0] / step).floor() * step;
            while x < f1[0] {
                let sx = base.to_screen([x, 0.0]).x;
                painter.line_segment([Pos2::new(sx, rect.top()), Pos2::new(sx, rect.bottom())], Stroke::new(1.0, gc));
                x += step;
            }
            let mut y = (f1[1] / step).floor() * step;
            while y < f0[1] {
                let sy = base.to_screen([0.0, y]).y;
                painter.line_segment([Pos2::new(rect.left(), sy), Pos2::new(rect.right(), sy)], Stroke::new(1.0, gc));
                y += step;
            }
        }
    }

    // ------------------------------------------------------------------ glyphs in the text line
    let editing_tool = !matches!(app.tool, Tool::Metrics | Tool::Kerning | Tool::Text | Tool::Hand | Tool::Zoom);
    let tmpl_col = if light_canvas { Color32::from_rgb(0xe6, 0xe8, 0xec) } else { Color32::from_rgb(0x27, 0x2c, 0x35) };
    for (k, (gi, x, y, name)) in lay.items.iter().enumerate() {
        if name == "\n" {
            continue;
        }
        let v = View { origin: Pos2::new(base.origin.x + x * s, base.origin.y - y * s), s };
        let Some(gi) = *gi else {
            // Missing glyph: dashed box with its name.
            let w = upm * 0.5;
            let r = Rect::from_min_max(v.to_screen([0.0, asc]), v.to_screen([w, desc]));
            painter.rect_stroke(r.shrink(2.0), CornerRadius::same(2), Stroke::new(1.0, pal.faint), StrokeKind::Inside);
            painter.text(r.center(), Align2::CENTER_CENTER, name, egui::FontId::proportional(11.0), pal.faint);
            continue;
        };
        let is_cur = k == cur;
        let empty = app.docs[di].font.glyphs[gi].layer(m).is_empty();
        if empty && app.prefs.show_template {
            if let Some(u) = app.docs[di].font.glyphs[gi].unicodes.first().copied() {
                if let Some((t, tw)) = app.cache.template(u, upm) {
                    let w = app.docs[di].font.glyphs[gi].layer(m).width;
                    let vv = v.shifted((w - tw) / 2.0);
                    render::paint_fill(&painter, &t, vv, tmpl_col);
                }
            }
        }
        if is_cur && editing_tool && !app.prefs.show_fill {
            // Edited glyph: translucent fill + outline; nodes drawn later.
            let t = app.cache.fill(di, &app.docs[di].font, gi, m);
            render::paint_fill(&painter, &t, v, ink.gamma_multiply(0.07));
            let font = &app.docs[di].font;
            let l = font.glyphs[gi].layer(m);
            for c in &l.contours {
                render::stroke_contour(&painter, c, v, Stroke::new(1.3, ink));
            }
            // Components in a distinct colour.
            for comp in &l.components {
                if let Some(bi) = font.find(&comp.base) {
                    let mut cs = font.flat_contours(bi, m);
                    for c in &mut cs {
                        for p in &mut c.points {
                            let q = comp.apply([p.x, p.y]);
                            p.x = q[0];
                            p.y = q[1];
                        }
                    }
                    let col = if comp.sel { pal.accent } else { Color32::from_rgb(0x2f, 0xb5, 0xb0) };
                    for c in &cs {
                        render::stroke_contour(&painter, c, v, Stroke::new(1.3, col));
                    }
                }
            }
        } else {
            let t = app.cache.fill(di, &app.docs[di].font, gi, m);
            let col = if is_cur || !editing_tool { ink } else { ink.gamma_multiply(0.85) };
            render::paint_fill(&painter, &t, v, col);
        }
        // Sidebearings.
        let w = app.docs[di].font.glyphs[gi].layer(m).width;
        let show_sb = is_cur || matches!(app.tool, Tool::Metrics | Tool::Kerning);
        if show_sb && app.prefs.show_metrics {
            let top = v.to_screen([0.0, asc + upm * 0.05]).y;
            let bot = v.to_screen([0.0, desc - upm * 0.05]).y;
            let sc = if is_cur { pal.accent.gamma_multiply(0.7) } else { line_col };
            let x0 = v.to_screen([0.0, 0.0]).x;
            let x1 = v.to_screen([w, 0.0]).x;
            painter.line_segment([Pos2::new(x0, top), Pos2::new(x0, bot)], Stroke::new(1.0, sc));
            painter.line_segment([Pos2::new(x1, top), Pos2::new(x1, bot)], Stroke::new(1.0, sc));
            if italic != 0.0 {
                let dxs = (asc - desc) * italic.to_radians().tan() * s;
                painter.line_segment([Pos2::new(x0 + dxs * (-desc / (asc - desc)), bot), Pos2::new(x0 + dxs * (asc / (asc - desc)), top)], Stroke::new(1.0, line_col));
            }
            if matches!(app.tool, Tool::Metrics) || is_cur {
                let font = &app.docs[di].font;
                let (lsb, rsb) = (font.lsb(gi, m), font.rsb(gi, m));
                let ly = v.to_screen([0.0, desc]).y + 4.0;
                painter.text(Pos2::new(x0 + 3.0, ly), Align2::LEFT_TOP, format!("{lsb:.0}"), egui::FontId::proportional(10.5), pal.muted);
                painter.text(Pos2::new(x1 - 3.0, ly), Align2::RIGHT_TOP, format!("{rsb:.0}"), egui::FontId::proportional(10.5), pal.muted);
                painter.text(Pos2::new((x0 + x1) / 2.0, ly + 13.0), Align2::CENTER_TOP, format!("{w:.0}"), egui::FontId::proportional(10.5), pal.faint);
            }
        }
        // Kerning values between pairs.
        if app.tool == Tool::Kerning && k > 0 {
            if let (Some(_), Some(pn)) = (lay.items[k - 1].0, lay.items.get(k - 1).map(|i| i.3.clone())) {
                if let Some((val, l, r)) = app.docs[di].font.kerning(m, &pn, name) {
                    let xk = v.to_screen([0.0, 0.0]).x;
                    let yk = v.to_screen([0.0, desc - upm * 0.12]).y;
                    let cls = l.starts_with('@') || r.starts_with('@');
                    let col = if val < 0.0 { pal.red } else { pal.green };
                    painter.text(Pos2::new(xk, yk), Align2::CENTER_TOP, format!("{}{val:.0}", if cls { "@" } else { "" }), bold(11.0), col);
                }
            }
        }
        if is_cur && matches!(app.tool, Tool::Kerning | Tool::Metrics | Tool::Text) {
            let a = v.to_screen([0.0, desc - upm * 0.02]);
            let b = v.to_screen([w, desc - upm * 0.02]);
            painter.line_segment([a, b], Stroke::new(2.5, pal.accent));
        }
    }

    // ------------------------------------------------------------------ guides, anchors, nodes
    if let Some(gi) = cur_gi {
        if app.prefs.show_guides {
            let gs: Vec<crate::font::Guide> = app.docs[di].font.glyphs[gi].layer(m).guides.iter().cloned().chain(app.docs[di].font.masters[m].guides.iter().cloned()).collect();
            for g in gs {
                let p = ev.to_screen([g.x, g.y]);
                let a = (-g.angle).to_radians();
                let d = Vec2::new(a.cos(), a.sin()) * 4000.0;
                painter.line_segment([p - d, p + d], Stroke::new(1.0, Color32::from_rgb(0x9b, 0x59, 0xd0)));
                if !g.name.is_empty() {
                    painter.text(p + Vec2::new(4.0, -4.0), Align2::LEFT_BOTTOM, &g.name, egui::FontId::proportional(10.0), Color32::from_rgb(0x9b, 0x59, 0xd0));
                }
            }
        }
        if app.prefs.show_anchors {
            let anchors = app.docs[di].font.glyphs[gi].layer(m).anchors.clone();
            for a in anchors {
                let p = ev.to_screen([a.x, a.y]);
                let col = if a.sel { pal.accent } else { Color32::from_rgb(0xe0, 0x8a, 0x1e) };
                let r = 5.0;
                painter.add(Shape::convex_polygon(vec![p + Vec2::new(0.0, -r), p + Vec2::new(r, 0.0), p + Vec2::new(0.0, r), p + Vec2::new(-r, 0.0)], col, Stroke::new(1.0, bg)));
                painter.text(p + Vec2::new(7.0, -6.0), Align2::LEFT_BOTTOM, &a.name, egui::FontId::proportional(10.5), col);
            }
        }
        if editing_tool && app.prefs.show_nodes {
            draw_nodes(app, &painter, di, gi, m, ev, bg);
        }
    }

    // ------------------------------------------------------------------ interaction
    let pointer = resp.interact_pointer_pos().or(hover);
    let pan_drag = (space || app.tool == Tool::Hand) && resp.dragged_by(PointerButton::Primary) || resp.dragged_by(PointerButton::Middle);
    if pan_drag {
        app.tabs[ti].pan += resp.drag_delta();
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    } else if !space {
        match app.tool {
            Tool::Zoom => {
                if resp.clicked() {
                    if let Some(p) = pointer {
                        let alt = ui.input(|i| i.modifiers.alt);
                        let f = if alt { 0.8 } else { 1.25 };
                        let fp = base.to_font(p);
                        let ns = (s * f).clamp(0.01, 60.0);
                        let t = &mut app.tabs[ti];
                        t.zoom = ns;
                        t.pan = Vec2::new(p.x - fp[0] * ns - rect.left(), p.y + fp[1] * ns - rect.top());
                    }
                }
            }
            Tool::Metrics | Tool::Kerning | Tool::Text => text_tools(app, ui, &resp, &lay, base, di, ti, m),
            _ => {
                if let Some(gi) = cur_gi {
                    edit_tools(app, ui, &resp, ev, di, gi, m, &painter);
                } else if resp.clicked() {
                    app.toast(ui.ctx(), "This glyph is not in the font yet — use Font ▸ Create Glyph");
                }
                // Clicking another glyph in the line makes it current.
                if resp.double_clicked() {
                    if let Some(p) = pointer {
                        if let Some(k) = hit_item(&lay, base, p, &app.docs[di].font, m) {
                            if k != cur {
                                app.tabs[ti].cur = k;
                            }
                        }
                    }
                }
            }
        }
    }
    keyboard(app, ui, di, ti, m, &lay);

    // Overlays: rubber band, shape preview, measure, knife, pencil.
    match &app.edit.drag {
        Some(Drag::Rubber { start, end, .. }) => {
            let r = Rect::from_two_pos(ev.to_screen(*start), ev.to_screen(*end));
            painter.rect_filled(r, 0.0, pal.accent.gamma_multiply(0.08));
            painter.rect_stroke(r, 0.0, Stroke::new(1.0, pal.accent), StrokeKind::Inside);
        }
        Some(Drag::Shape { start, end }) => {
            let r = Rect::from_two_pos(ev.to_screen(*start), ev.to_screen(*end));
            if app.tool == Tool::Ellipse {
                painter.add(egui::epaint::EllipseShape::stroke(r.center(), r.size() / 2.0, Stroke::new(1.5, pal.accent)));
            } else {
                painter.rect_stroke(r, 0.0, Stroke::new(1.5, pal.accent), StrokeKind::Inside);
            }
        }
        _ => {}
    }
    if let Some((a, b)) = app.edit.measure {
        let (pa, pb) = (ev.to_screen(a), ev.to_screen(b));
        painter.line_segment([pa, pb], Stroke::new(1.5, pal.orange));
        let d = geom::dist(a, b);
        let ang = (b[1] - a[1]).atan2(b[0] - a[0]).to_degrees();
        painter.text(pb + Vec2::new(8.0, -8.0), Align2::LEFT_BOTTOM, format!("{d:.1} u  ·  {ang:.1}°  ·  Δx {:.0}  Δy {:.0}", b[0] - a[0], b[1] - a[1]), bold(11.5), pal.orange);
    }
    if let Some((a, b)) = app.edit.knife {
        painter.line_segment([ev.to_screen(a), ev.to_screen(b)], Stroke::new(1.5, pal.red));
    }
    if app.edit.rapid.len() > 1 {
        let pts: Vec<Pos2> = app.edit.rapid.iter().map(|p| ev.to_screen(*p)).collect();
        painter.add(Shape::line(pts, Stroke::new(1.5, pal.accent)));
    }

    // Floating metrics widget under the canvas.
    if let Some(gi) = cur_gi {
        metrics_widget(app, ui, rect, di, gi, m);
    }
}

fn hit_item(lay: &Layout, base: View, p: Pos2, font: &crate::font::Font, m: usize) -> Option<usize> {
    let f = base.to_font(p);
    let ms = &font.masters[m.min(font.masters.len() - 1)];
    for (k, (gi, x, y, n)) in lay.items.iter().enumerate() {
        if n == "\n" {
            continue;
        }
        let w = gi.map(|g| font.glyphs[g].layer(m).width).unwrap_or(font.info.upm * 0.5);
        if f[0] >= *x && f[0] < x + w.max(10.0) && f[1] <= y + ms.ascender + 100.0 && f[1] >= y + ms.descender - 100.0 {
            return Some(k);
        }
    }
    None
}

fn draw_nodes(app: &App, painter: &egui::Painter, di: usize, gi: usize, m: usize, ev: View, bg: Color32) {
    let pal = app.pal;
    let l = app.docs[di].font.glyphs[gi].layer(m);
    let handle_col = if pal.dark { Color32::from_rgb(0x91, 0x96, 0x9f) } else { Color32::from_rgb(0x8a, 0x90, 0x9a) };
    let node_col = if pal.dark { Color32::from_rgb(0xe4, 0xe6, 0xea) } else { Color32::from_rgb(0x1c, 0x1f, 0x26) };
    for c in &l.contours {
        let n = c.points.len();
        // Handle lines.
        for i in 0..n {
            let p = &c.points[i];
            if p.t != PtType::Off {
                continue;
            }
            let prev = (i + n - 1) % n;
            let next = (i + 1) % n;
            for j in [prev, next] {
                if c.points[j].t.on_curve() && (c.closed || (j as isize - i as isize).abs() == 1) {
                    painter.line_segment([ev.to_screen(p.pos()), ev.to_screen(c.points[j].pos())], Stroke::new(1.0, handle_col));
                }
            }
        }
        for (i, p) in c.points.iter().enumerate() {
            let sp = ev.to_screen(p.pos());
            let fill = if p.sel { pal.accent } else { bg };
            if p.t == PtType::Off {
                painter.circle(sp, 3.2, fill, Stroke::new(1.2, handle_col));
            } else if p.smooth {
                painter.circle(sp, 4.2, fill, Stroke::new(1.4, if p.sel { pal.accent } else { Color32::from_rgb(0x3f, 0xb9, 0x6a) }));
            } else {
                let r = Rect::from_center_size(sp, Vec2::splat(7.5));
                painter.rect(r, CornerRadius::ZERO, fill, Stroke::new(1.4, if p.sel { pal.accent } else { node_col }), StrokeKind::Inside);
            }
            if i == 0 && c.points.len() > 1 {
                // Start point marker: a small triangle pointing along the contour.
                let nxt = c.points[1].pos();
                let d = Vec2::new(nxt[0] - p.x, -(nxt[1] - p.y));
                if d.length() > 0.01 {
                    let d = d.normalized();
                    let nrm = Vec2::new(-d.y, d.x);
                    let tip = sp + d * 13.0;
                    painter.add(Shape::convex_polygon(vec![tip, sp + d * 6.0 + nrm * 4.0, sp + d * 6.0 - nrm * 4.0], pal.accent.gamma_multiply(0.8), Stroke::NONE));
                }
            }
            if app.prefs.show_coords && (p.sel || p.t.on_curve() && app.prefs.show_coords && l.node_count() < 60) {
                painter.text(sp + Vec2::new(6.0, 6.0), Align2::LEFT_TOP, format!("{:.0}, {:.0}", p.x, p.y), egui::FontId::proportional(10.0), pal.muted);
            }
        }
    }
}

/// Nearest point within `tol` screen px: (contour, point).
fn hit_point(l: &crate::font::Layer, ev: View, p: Pos2, tol: f32) -> Option<(usize, usize)> {
    let mut best: Option<(usize, usize, f32)> = None;
    for (ci, c) in l.contours.iter().enumerate() {
        for (pi, q) in c.points.iter().enumerate() {
            let d = ev.to_screen(q.pos()).distance(p);
            // Prefer on-curve points slightly.
            let d = if q.t.on_curve() { d - 0.5 } else { d };
            if d < tol && best.map(|b| d < b.2).unwrap_or(true) {
                best = Some((ci, pi, d));
            }
        }
    }
    best.map(|b| (b.0, b.1))
}

fn hit_anchor(l: &crate::font::Layer, ev: View, p: Pos2) -> Option<usize> {
    l.anchors.iter().position(|a| ev.to_screen([a.x, a.y]).distance(p) < 7.0)
}

fn hit_segment(l: &crate::font::Layer, ev: View, p: Pos2, tol: f32) -> Option<(usize, usize, f32)> {
    let f = ev.to_font(p);
    let mut best: Option<(usize, usize, f32, f32)> = None;
    for (ci, c) in l.contours.iter().enumerate() {
        if let Some((si, t, d)) = geom::nearest_segment(c, f) {
            let ds = d * ev.s;
            if ds < tol && best.map(|b| ds < b.3).unwrap_or(true) {
                best = Some((ci, si, t, ds));
            }
        }
    }
    best.map(|b| (b.0, b.1, b.2))
}

fn hit_component(app: &App, di: usize, gi: usize, m: usize, f: P2) -> Option<usize> {
    let font = &app.docs[di].font;
    let l = font.glyphs[gi].layer(m);
    for (k, comp) in l.components.iter().enumerate().rev() {
        if let Some(bi) = font.find(&comp.base) {
            let mut cs = font.flat_contours(bi, m);
            for c in &mut cs {
                for p in &mut c.points {
                    let q = comp.apply([p.x, p.y]);
                    p.x = q[0];
                    p.y = q[1];
                }
            }
            if geom::winding(&cs, f) != 0 {
                return Some(k);
            }
        }
    }
    None
}

fn hit_contour_inside(l: &crate::font::Layer, f: P2) -> Option<usize> {
    // Smallest closed contour containing the point.
    let mut best: Option<(usize, f32)> = None;
    for (ci, c) in l.contours.iter().enumerate() {
        if geom::winding(std::slice::from_ref(c), f) != 0 {
            let a = c.signed_area().abs();
            if best.map(|b| a < b.1).unwrap_or(true) {
                best = Some((ci, a));
            }
        }
    }
    best.map(|b| b.0)
}

#[allow(clippy::too_many_arguments)]
fn edit_tools(app: &mut App, ui: &mut egui::Ui, resp: &egui::Response, ev: View, di: usize, gi: usize, m: usize, _painter: &egui::Painter) {
    let mods = ui.input(|i| i.modifiers);
    let Some(pp) = resp.interact_pointer_pos().or(resp.hover_pos()) else { return };
    let f = ev.to_font(pp);
    let fs = [snap(app, f[0]), snap(app, f[1])];
    let tool = app.tool;
    match tool {
        Tool::Contour | Tool::Element => {
            if resp.drag_started_by(PointerButton::Primary) || (resp.clicked() && app.edit.drag.is_none()) {
                let start_drag = resp.drag_started_by(PointerButton::Primary);
                // Press: decide what is under the pointer.
                let layer = app.docs[di].font.glyphs[gi].layer(m).clone();
                let pick_point = if tool == Tool::Contour { hit_point(&layer, ev, pp, 7.0) } else { None };
                let pick_anchor = hit_anchor(&layer, ev, pp);
                let l = app.docs[di].font.glyphs[gi].layer_mut(m);
                if let Some((ci, pi)) = pick_point {
                    let is_off = layer.contours[ci].points[pi].t == PtType::Off;
                    if !layer.contours[ci].points[pi].sel {
                        if !mods.shift {
                            l.clear_selection();
                        }
                        l.contours[ci].points[pi].sel = true;
                    } else if mods.shift && !start_drag {
                        l.contours[ci].points[pi].sel = false;
                    }
                    if start_drag {
                        let nsel = l.selected_points().len();
                        app.edit.drag = Some(if is_off && nsel == 1 { Drag::Handle { ci, pi, moved: false } } else { Drag::Points { last: fs, moved: false } });
                    }
                } else if let Some(ai) = pick_anchor {
                    if !layer.anchors[ai].sel && !mods.shift {
                        l.clear_selection();
                    }
                    l.anchors[ai].sel = true;
                    if start_drag {
                        app.edit.drag = Some(Drag::Points { last: fs, moved: false });
                    }
                } else if let (Tool::Contour, Some((ci, si, t))) = (tool, hit_segment(&layer, ev, pp, 6.0)) {
                    if resp.double_clicked() || (mods.alt && !start_drag) {
                        app.docs[di].checkpoint_glyph(gi, "Add node");
                        let l = app.docs[di].font.glyphs[gi].layer_mut(m);
                        l.clear_selection();
                        geom::insert_node(&mut l.contours[ci], si, t);
                        app.docs[di].font.glyphs[gi].touch();
                    } else {
                        let seg = geom::segments(&layer.contours[ci])[si].clone();
                        if !mods.shift {
                            l.clear_selection();
                        }
                        l.contours[ci].points[seg.start].sel = true;
                        l.contours[ci].points[seg.end].sel = true;
                        if start_drag {
                            app.edit.drag = Some(Drag::Points { last: fs, moved: false });
                        }
                    }
                } else {
                    let comp = hit_component(app, di, gi, m, f);
                    let inside = if tool == Tool::Element { hit_contour_inside(&layer, f) } else { None };
                    let l = app.docs[di].font.glyphs[gi].layer_mut(m);
                    if let Some(k) = comp.filter(|_| inside.is_none()) {
                        if !l.components[k].sel && !mods.shift {
                            l.clear_selection();
                        }
                        l.components[k].sel = true;
                        if start_drag {
                            app.edit.drag = Some(Drag::Points { last: fs, moved: false });
                        }
                    } else if let Some(ci) = inside {
                        let already = l.contours[ci].points.iter().all(|p| p.sel);
                        if !already && !mods.shift {
                            l.clear_selection();
                        }
                        for p in &mut l.contours[ci].points {
                            p.sel = true;
                        }
                        if resp.double_clicked() {
                            app.tool = Tool::Contour;
                        }
                        if start_drag {
                            app.edit.drag = Some(Drag::Points { last: fs, moved: false });
                        }
                    } else {
                        if !mods.shift {
                            l.clear_selection();
                        }
                        if start_drag {
                            app.edit.drag = Some(Drag::Rubber { start: f, end: f, add: mods.shift });
                        }
                    }
                }
                app.docs[di].font.glyphs[gi].touch();
            }
            if resp.dragged_by(PointerButton::Primary) {
                match app.edit.drag.clone() {
                    Some(Drag::Points { last, moved }) => {
                        let mut dx = fs[0] - last[0];
                        let mut dy = fs[1] - last[1];
                        if mods.shift {
                            // Constrain to the dominant axis relative to the press point.
                            if dx.abs() > dy.abs() {
                                dy = 0.0;
                            } else {
                                dx = 0.0;
                            }
                        }
                        if dx != 0.0 || dy != 0.0 {
                            if !moved {
                                app.docs[di].checkpoint_glyph(gi, "Move");
                            }
                            let g = &mut app.docs[di].font.glyphs[gi];
                            g.layer_mut(m).move_selection(dx, dy);
                            g.touch();
                            app.edit.drag = Some(Drag::Points { last: [last[0] + dx, last[1] + dy], moved: true });
                        }
                    }
                    Some(Drag::Handle { ci, pi, moved }) => {
                        if !moved {
                            app.docs[di].checkpoint_glyph(gi, "Move handle");
                        }
                        let g = &mut app.docs[di].font.glyphs[gi];
                        let c = &mut g.layer_mut(m).contours[ci];
                        c.points[pi].x = fs[0];
                        c.points[pi].y = fs[1];
                        keep_smooth(c, pi);
                        g.touch();
                        app.edit.drag = Some(Drag::Handle { ci, pi, moved: true });
                    }
                    Some(Drag::Rubber { start, add, .. }) => {
                        app.edit.drag = Some(Drag::Rubber { start, end: f, add });
                        let r = [start[0].min(f[0]), start[1].min(f[1]), start[0].max(f[0]), start[1].max(f[1])];
                        let inside = |x: f32, y: f32| x >= r[0] && x <= r[2] && y >= r[1] && y <= r[3];
                        let l = app.docs[di].font.glyphs[gi].layer_mut(m);
                        for c in &mut l.contours {
                            let any = c.points.iter().any(|p| inside(p.x, p.y));
                            for p in &mut c.points {
                                let hit = if tool == Tool::Element { any } else { inside(p.x, p.y) };
                                if hit {
                                    p.sel = true;
                                } else if !add {
                                    p.sel = false;
                                }
                            }
                        }
                        for a in &mut l.anchors {
                            a.sel = inside(a.x, a.y) || (add && a.sel);
                        }
                    }
                    _ => {}
                }
            }
            if resp.drag_stopped() {
                app.edit.drag = None;
            }
        }
        Tool::Pen => pen_tool(app, resp, ev, di, gi, m, pp, fs),
        Tool::Rapid => {
            if resp.drag_started_by(PointerButton::Primary) {
                app.edit.rapid = vec![f];
            }
            if resp.dragged_by(PointerButton::Primary) {
                if app.edit.rapid.last().map(|l| geom::dist(*l, f) * ev.s > 2.0).unwrap_or(true) {
                    app.edit.rapid.push(f);
                }
            }
            if resp.drag_stopped() && app.edit.rapid.len() > 2 {
                let pts = std::mem::take(&mut app.edit.rapid);
                let closed = geom::dist(pts[0], *pts.last().unwrap()) * ev.s < 14.0;
                let mut c = geom::fit_freehand(&pts, closed);
                geom::round_contour(&mut c);
                app.edit_layer("Pencil", |l| l.contours.push(c));
            }
        }
        Tool::Rectangle | Tool::Ellipse => {
            if resp.drag_started_by(PointerButton::Primary) {
                app.edit.drag = Some(Drag::Shape { start: fs, end: fs });
            }
            if resp.dragged_by(PointerButton::Primary) {
                if let Some(Drag::Shape { start, .. }) = app.edit.drag.clone() {
                    let mut end = fs;
                    if mods.shift {
                        let d = (end[0] - start[0]).abs().max((end[1] - start[1]).abs());
                        end = [start[0] + d * (end[0] - start[0]).signum(), start[1] + d * (end[1] - start[1]).signum()];
                    }
                    app.edit.drag = Some(Drag::Shape { start, end });
                }
            }
            if resp.drag_stopped() {
                if let Some(Drag::Shape { start, end }) = app.edit.drag.take() {
                    let (x0, y0, x1, y1) = (start[0].min(end[0]), start[1].min(end[1]), start[0].max(end[0]), start[1].max(end[1]));
                    if x1 - x0 > 1.0 && y1 - y0 > 1.0 {
                        let c = if tool == Tool::Rectangle {
                            geom::rect_contour(x0, y0, x1 - x0, y1 - y0, false)
                        } else {
                            let mut c = geom::ellipse_contour((x0 + x1) / 2.0, (y0 + y1) / 2.0, (x1 - x0) / 2.0, (y1 - y0) / 2.0);
                            geom::round_contour(&mut c);
                            c
                        };
                        app.edit_layer(if tool == Tool::Rectangle { "Rectangle" } else { "Ellipse" }, |l| {
                            l.clear_selection();
                            let mut c = c;
                            for p in &mut c.points {
                                p.sel = true;
                            }
                            l.contours.push(c);
                        });
                    }
                }
            }
        }
        Tool::Knife => {
            if resp.drag_started_by(PointerButton::Primary) {
                app.edit.knife = Some((f, f));
            }
            if resp.dragged_by(PointerButton::Primary) {
                if let Some((a, _)) = app.edit.knife {
                    app.edit.knife = Some((a, f));
                }
            }
            if resp.drag_stopped() {
                if let Some((a, b)) = app.edit.knife.take() {
                    app.edit_layer("Knife", |l| knife_cut(l, a, b));
                }
            }
        }
        Tool::Eraser => {
            if resp.clicked() {
                let layer = app.docs[di].font.glyphs[gi].layer(m).clone();
                if let Some((ci, pi)) = hit_point(&layer, ev, pp, 7.0) {
                    app.edit_layer("Erase node", |l| {
                        if l.contours[ci].points[pi].t.on_curve() {
                            remove_node(&mut l.contours[ci], pi);
                            if l.contours[ci].points.iter().filter(|p| p.t.on_curve()).count() < 2 {
                                l.contours.remove(ci);
                            }
                        } else {
                            l.clear_selection();
                            l.contours[ci].points[pi].sel = true;
                            delete_selection(l);
                        }
                    });
                } else if let Some(ci) = hit_contour_inside(&layer, f) {
                    app.edit_layer("Erase contour", |l| {
                        l.contours.remove(ci);
                    });
                } else if let Some(k) = hit_component(app, di, gi, m, f) {
                    app.edit_layer("Erase component", |l| {
                        l.components.remove(k);
                    });
                }
            }
        }
        Tool::Anchor => {
            let layer = app.docs[di].font.glyphs[gi].layer(m).clone();
            if resp.drag_started_by(PointerButton::Primary) {
                if let Some(ai) = hit_anchor(&layer, ev, pp) {
                    let l = app.docs[di].font.glyphs[gi].layer_mut(m);
                    l.clear_selection();
                    l.anchors[ai].sel = true;
                    app.edit.drag = Some(Drag::Points { last: fs, moved: false });
                }
            }
            if resp.dragged_by(PointerButton::Primary) {
                if let Some(Drag::Points { last, moved }) = app.edit.drag.clone() {
                    let (dx, dy) = (fs[0] - last[0], fs[1] - last[1]);
                    if dx != 0.0 || dy != 0.0 {
                        if !moved {
                            app.docs[di].checkpoint_glyph(gi, "Move anchor");
                        }
                        let g = &mut app.docs[di].font.glyphs[gi];
                        g.layer_mut(m).move_selection(dx, dy);
                        g.touch();
                        app.edit.drag = Some(Drag::Points { last: fs, moved: true });
                    }
                }
            }
            if resp.drag_stopped() {
                app.edit.drag = None;
            }
            if resp.clicked() && hit_anchor(&layer, ev, pp).is_none() {
                let used: Vec<String> = layer.anchors.iter().map(|a| a.name.clone()).collect();
                let name = ["top", "bottom", "ogonek", "center", "_top", "_bottom"].iter().find(|n| !used.contains(&n.to_string())).map(|s| s.to_string()).unwrap_or_else(|| format!("anchor{}", used.len() + 1));
                app.edit_layer("Add anchor", |l| {
                    l.clear_selection();
                    l.anchors.push(Anchor { name, x: fs[0], y: fs[1], sel: true });
                });
            }
        }
        Tool::Measure => {
            if resp.drag_started_by(PointerButton::Primary) {
                app.edit.measure = Some((fs, fs));
            }
            if resp.dragged_by(PointerButton::Primary) {
                if let Some((a, _)) = app.edit.measure {
                    app.edit.measure = Some((a, fs));
                }
            }
            if resp.clicked() {
                app.edit.measure = None;
            }
        }
        _ => {}
    }
}

/// Keep the opposite handle of a smooth node collinear when one handle moves.
fn keep_smooth(c: &mut Contour, pi: usize) {
    let n = c.points.len();
    for (node, opp) in [((pi + n - 1) % n, (pi + n - 2) % n), ((pi + 1) % n, (pi + 2) % n)] {
        if n < 3 || node == pi || opp == pi {
            continue;
        }
        if !c.closed && (node >= n || opp >= n || (pi as isize - opp as isize).abs() != 2) {
            continue;
        }
        let nd = &c.points[node];
        if nd.t.on_curve() && nd.smooth && c.points[opp].t == PtType::Off {
            let h = [c.points[pi].x - nd.x, c.points[pi].y - nd.y];
            let len = h[0].hypot(h[1]);
            if len < 1e-3 {
                continue;
            }
            let o = [c.points[opp].x - nd.x, c.points[opp].y - nd.y];
            let olen = o[0].hypot(o[1]);
            let (nx, ny) = (nd.x, nd.y);
            c.points[opp].x = (nx - h[0] / len * olen).round();
            c.points[opp].y = (ny - h[1] / len * olen).round();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn pen_tool(app: &mut App, resp: &egui::Response, ev: View, di: usize, gi: usize, m: usize, pp: Pos2, fs: P2) {
    let pressed = resp.drag_started_by(PointerButton::Primary) || (resp.clicked() && !matches!(app.edit.drag, Some(Drag::PenHandle { .. })));
    if pressed {
        let layer = app.docs[di].font.glyphs[gi].layer(m).clone();
        let active = app.edit.pen_contour.filter(|&ci| ci < layer.contours.len() && !layer.contours[ci].closed);
        app.docs[di].checkpoint_glyph(gi, "Pen");
        let g = &mut app.docs[di].font.glyphs[gi];
        let l = g.layer_mut(m);
        match active {
            Some(ci) => {
                let c = &mut l.contours[ci];
                let first = c.points[0].pos();
                let on_count = c.points.iter().filter(|p| p.t.on_curve()).count();
                if on_count >= 2 && ev.to_screen(first).distance(pp) < 8.0 {
                    // Close the contour.
                    let last_is_off = c.points.last().map(|p| p.t == PtType::Off).unwrap_or(false);
                    let first_has_out = c.points.get(1).map(|p| p.t == PtType::Off).unwrap_or(false);
                    let p0 = c.points[0].pos();
                    if last_is_off || first_has_out {
                        let last_on = c.points.iter().rev().find(|p| p.t.on_curve()).map(|p| p.pos()).unwrap_or(p0);
                        if !last_is_off {
                            let c1 = geom::lerp(last_on, p0, 1.0 / 3.0);
                            c.points.push(Point::new(c1[0].round(), c1[1].round(), PtType::Off));
                        }
                        let c2 = if first_has_out { let h = c.points[1].pos(); [2.0 * p0[0] - h[0], 2.0 * p0[1] - h[1]] } else { geom::lerp(last_on, p0, 2.0 / 3.0) };
                        c.points.push(Point::new(c2[0].round(), c2[1].round(), PtType::Off));
                        c.points[0].t = PtType::Curve;
                        c.points[0].smooth = first_has_out;
                    } else {
                        c.points[0].t = PtType::Line;
                    }
                    c.closed = true;
                    app.edit.pen_contour = None;
                    app.edit.drag = None;
                } else {
                    let has_out = c.points.last().map(|p| p.t == PtType::Off).unwrap_or(false);
                    if has_out {
                        c.points.push(Point::new(fs[0], fs[1], PtType::Off));
                        c.points.push(Point::new(fs[0], fs[1], PtType::Curve));
                    } else {
                        c.points.push(Point::new(fs[0], fs[1], PtType::Line));
                    }
                    let pi = c.points.len() - 1;
                    app.edit.drag = Some(Drag::PenHandle { ci, pi });
                }
            }
            None => {
                // Continue an open contour from one of its ends, or start a new one.
                let mut cont = None;
                for (ci, c) in l.contours.iter_mut().enumerate() {
                    if c.closed || c.points.len() < 2 {
                        continue;
                    }
                    let last = c.points.last().unwrap().pos();
                    if ev.to_screen(last).distance(pp) < 8.0 {
                        cont = Some(ci);
                        break;
                    }
                    if ev.to_screen(c.points[0].pos()).distance(pp) < 8.0 {
                        c.reverse();
                        cont = Some(ci);
                        break;
                    }
                }
                if let Some(ci) = cont {
                    app.edit.pen_contour = Some(ci);
                } else {
                    l.clear_selection();
                    let mut p = Point::new(fs[0], fs[1], PtType::Move);
                    p.sel = true;
                    l.contours.push(Contour { points: vec![p], closed: false });
                    let ci = l.contours.len() - 1;
                    app.edit.pen_contour = Some(ci);
                    app.edit.drag = Some(Drag::PenHandle { ci, pi: 0 });
                }
            }
        }
        g.touch();
    }
    if resp.dragged_by(PointerButton::Primary) {
        if let Some(Drag::PenHandle { ci, mut pi }) = app.edit.drag.clone() {
            let g = &mut app.docs[di].font.glyphs[gi];
            let l = g.layer_mut(m);
            if ci < l.contours.len() && pi < l.contours[ci].points.len() {
                let c = &mut l.contours[ci];
                let node = c.points[pi].pos();
                if geom::dist(node, fs) * ev.s > 3.0 {
                    if c.points[pi].t == PtType::Line {
                        let prev = c.points[pi - 1].pos();
                        let c1 = geom::lerp(prev, node, 1.0 / 3.0);
                        c.points.insert(pi, Point::new(node[0], node[1], PtType::Off));
                        c.points.insert(pi, Point::new(c1[0].round(), c1[1].round(), PtType::Off));
                        pi += 2;
                        c.points[pi].t = PtType::Curve;
                    }
                    if c.points[pi].t == PtType::Curve && pi >= 1 {
                        c.points[pi - 1].x = (2.0 * node[0] - fs[0]).round();
                        c.points[pi - 1].y = (2.0 * node[1] - fs[1]).round();
                    }
                    if pi + 1 < c.points.len() && c.points[pi + 1].t == PtType::Off {
                        c.points[pi + 1].x = fs[0];
                        c.points[pi + 1].y = fs[1];
                    } else {
                        c.points.push(Point::new(fs[0], fs[1], PtType::Off));
                    }
                    c.points[pi].smooth = true;
                    app.edit.drag = Some(Drag::PenHandle { ci, pi });
                }
            }
            g.touch();
        }
    }
    if resp.drag_stopped() {
        app.edit.drag = None;
    }
}

/// Cut closed contours along the line a–b (each contour crossed exactly twice).
pub fn knife_cut(l: &mut crate::font::Layer, a: P2, b: P2) {
    fn seg_x(p: P2, q: P2, a: P2, b: P2) -> Option<f32> {
        let r = [q[0] - p[0], q[1] - p[1]];
        let s = [b[0] - a[0], b[1] - a[1]];
        let den = r[0] * s[1] - r[1] * s[0];
        if den.abs() < 1e-9 {
            return None;
        }
        let t = ((a[0] - p[0]) * s[1] - (a[1] - p[1]) * s[0]) / den;
        let u = ((a[0] - p[0]) * r[1] - (a[1] - p[1]) * r[0]) / den;
        if (0.0..1.0).contains(&t) && (0.0..=1.0).contains(&u) {
            Some(t)
        } else {
            None
        }
    }
    let mut out = Vec::new();
    for c in std::mem::take(&mut l.contours) {
        if !c.closed {
            out.push(c);
            continue;
        }
        let segs = geom::segments(&c);
        let mut hits: Vec<(usize, f32)> = Vec::new();
        for (si, s) in segs.iter().enumerate() {
            let pieces = geom::segment_pieces(&c, s);
            if pieces.len() != 1 {
                continue;
            }
            let steps = 32;
            let fpt = |t: f32| match &pieces[0] {
                geom::Piece::Line(p, q) => geom::lerp(*p, *q, t),
                geom::Piece::Quad(p, q, r) => geom::quad_at(*p, *q, *r, t),
                geom::Piece::Cubic(p, q, r, w) => geom::cubic_at(*p, *q, *r, *w, t),
            };
            for i in 0..steps {
                let (t0, t1) = (i as f32 / steps as f32, (i + 1) as f32 / steps as f32);
                if let Some(t) = seg_x(fpt(t0), fpt(t1), a, b) {
                    hits.push((si, t0 + t * (t1 - t0)));
                }
            }
        }
        if hits.len() != 2 || matches!(geom::segment_pieces(&c, &segs[hits[0].0])[0], geom::Piece::Quad(..)) {
            out.push(c);
            continue;
        }
        let mut c = c;
        hits.sort_by(|x, y| (x.0, x.1).partial_cmp(&(y.0, y.1)).unwrap());
        let (h_lo, h_hi) = (hits[0], hits[1]);
        let len0 = c.points.len();
        let Some(mut idx_hi) = geom::insert_node(&mut c, h_hi.0, h_hi.1.clamp(0.01, 0.99)) else {
            out.push(c);
            continue;
        };
        let t_lo = if h_lo.0 == h_hi.0 { (h_lo.1 / h_hi.1).clamp(0.01, 0.99) } else { h_lo.1.clamp(0.01, 0.99) };
        let len1 = c.points.len();
        let Some(idx_lo) = geom::insert_node(&mut c, h_lo.0, t_lo) else {
            out.push(c);
            continue;
        };
        let added = c.points.len() - len1;
        if idx_lo <= idx_hi {
            idx_hi += added;
        }
        let _ = len0;
        let n = c.points.len();
        let take = |from: usize, to: usize| -> Contour {
            let mut pts = Vec::new();
            let mut i = from;
            loop {
                let mut p = c.points[i].clone();
                p.sel = false;
                pts.push(p);
                if i == to {
                    break;
                }
                i = (i + 1) % n;
            }
            pts[0].t = PtType::Line;
            pts[0].smooth = false;
            Contour { points: pts, closed: true }
        };
        let a1 = take(idx_lo, idx_hi);
        let a2 = take(idx_hi, idx_lo);
        out.push(a1);
        out.push(a2);
    }
    l.contours = out;
}

#[allow(clippy::too_many_arguments)]
fn text_tools(app: &mut App, ui: &mut egui::Ui, resp: &egui::Response, lay: &Layout, base: View, di: usize, ti: usize, m: usize) {
    let Some(pp) = resp.interact_pointer_pos().or(resp.hover_pos()) else { return };
    let f = base.to_font(pp);
    let s = base.s;
    // Hover cursor over sidebearings in Metrics mode.
    let near_sb = |app: &App| -> Option<(usize, bool)> {
        for (k, (gi, x, y, _)) in lay.items.iter().enumerate() {
            let Some(gi) = gi else { continue };
            let w = app.docs[di].font.glyphs[*gi].layer(m).width;
            let ms = &app.docs[di].font.masters[m];
            if f[1] < y + ms.descender - 50.0 || f[1] > y + ms.ascender + 50.0 {
                continue;
            }
            if ((f[0] - x) * s).abs() < 6.0 {
                return Some((k, true));
            }
            if ((f[0] - (x + w)) * s).abs() < 6.0 {
                return Some((k, false));
            }
        }
        None
    };
    if app.tool == Tool::Metrics && near_sb(app).is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    if resp.drag_started_by(PointerButton::Primary) || resp.clicked() {
        if let Some(k) = hit_item(lay, base, pp, &app.docs[di].font, m) {
            app.tabs[ti].cur = k;
        }
        if resp.drag_started_by(PointerButton::Primary) {
            match app.tool {
                Tool::Metrics => {
                    if let Some((k, left)) = near_sb(app) {
                        if let Some(gi) = lay.items[k].0 {
                            app.tabs[ti].cur = k;
                            let font = &app.docs[di].font;
                            let orig = if left { font.lsb(gi, m) } else { font.glyphs[gi].layer(m).width };
                            app.edit.drag = Some(Drag::Sidebearing { left, glyph: gi, start_x: f[0], orig });
                            app.docs[di].checkpoint_glyph(gi, "Spacing");
                        }
                    }
                }
                Tool::Kerning => {
                    if let Some(k) = hit_item(lay, base, pp, &app.docs[di].font, m) {
                        if k > 0 && lay.items[k].0.is_some() && lay.items[k - 1].0.is_some() {
                            let (l, r) = (lay.items[k - 1].3.clone(), lay.items[k].3.clone());
                            let (orig, kl, kr) = app.docs[di].font.kerning(m, &l, &r).unwrap_or((0.0, l.clone(), r.clone()));
                            app.docs[di].checkpoint_font("Kerning");
                            app.edit.drag = Some(Drag::Kern { left: kl, right: kr, start_x: f[0], orig });
                        }
                    }
                }
                _ => {}
            }
        }
    }
    if resp.dragged_by(PointerButton::Primary) {
        match app.edit.drag.clone() {
            Some(Drag::Sidebearing { left, glyph, start_x, orig }) => {
                let dx = (f[0] - start_x).round();
                if left {
                    apply_metrics(&mut app.docs[di].font, glyph, m, Some(orig - dx), None, None);
                } else {
                    apply_metrics(&mut app.docs[di].font, glyph, m, None, None, Some(orig + dx));
                }
                app.docs[di].font.dirty = true;
            }
            Some(Drag::Kern { left, right, start_x, orig }) => {
                let dx = ((f[0] - start_x) / 2.0).round() * 2.0;
                app.docs[di].font.set_kerning(m, &left, &right, orig + dx);
            }
            _ => {}
        }
    }
    if resp.drag_stopped() {
        app.edit.drag = None;
    }
    // Text tool: typing edits the line at the current position.
    if app.tool == Tool::Text && !ui.ctx().memory(|mm| mm.focused().is_some()) {
        let events = ui.input(|i| i.events.clone());
        for e in events {
            match e {
                egui::Event::Text(t) => {
                    let names = text_to_names(&app.docs[di].font, &t);
                    let tab = &mut app.tabs[ti];
                    let at = (tab.cur + 1).min(tab.text.len());
                    for (k, n) in names.into_iter().enumerate() {
                        tab.text.insert(at + k, n);
                    }
                    tab.cur = (at + t.chars().count()).saturating_sub(1).min(tab.text.len() - 1);
                    tab.text_input = names_to_text(&tab.text);
                }
                egui::Event::Key { key: Key::Backspace, pressed: true, .. } => {
                    let tab = &mut app.tabs[ti];
                    if tab.text.len() > 1 {
                        tab.text.remove(tab.cur);
                        tab.cur = tab.cur.saturating_sub(1);
                        tab.text_input = names_to_text(&tab.text);
                    }
                }
                egui::Event::Key { key: Key::Enter, pressed: true, .. } => {
                    let tab = &mut app.tabs[ti];
                    let at = (tab.cur + 1).min(tab.text.len());
                    tab.text.insert(at, "\n".into());
                    tab.cur = at;
                    tab.text_input = names_to_text(&tab.text);
                }
                egui::Event::Key { key: Key::ArrowLeft, pressed: true, .. } => {
                    let tab = &mut app.tabs[ti];
                    tab.cur = tab.cur.saturating_sub(1);
                }
                egui::Event::Key { key: Key::ArrowRight, pressed: true, .. } => {
                    let tab = &mut app.tabs[ti];
                    if tab.cur + 1 < tab.text.len() {
                        tab.cur += 1;
                    }
                }
                _ => {}
            }
        }
    }
}

fn keyboard(app: &mut App, ui: &mut egui::Ui, di: usize, ti: usize, m: usize, lay: &Layout) {
    if ui.ctx().memory(|mm| mm.focused().is_some()) || app.tool == Tool::Text {
        return;
    }
    let (l, r, u, d, shift, cmd, alt) = ui.input(|i| {
        (i.key_pressed(Key::ArrowLeft), i.key_pressed(Key::ArrowRight), i.key_pressed(Key::ArrowUp), i.key_pressed(Key::ArrowDown), i.modifiers.shift, i.modifiers.command, i.modifiers.alt)
    });
    if !(l || r || u || d) {
        return;
    }
    let step = if cmd { app.prefs.nudge_ctrl } else if shift { app.prefs.nudge_shift } else { app.prefs.nudge };
    let dx = if l { -step } else if r { step } else { 0.0 };
    let dy = if d { -step } else if u { step } else { 0.0 };
    let cur = app.tabs[ti].cur.min(lay.items.len().saturating_sub(1));
    match app.tool {
        Tool::Kerning => {
            if cur > 0 && dx != 0.0 {
                let (ln, rn) = (lay.items[cur - 1].3.clone(), lay.items[cur].3.clone());
                if lay.items[cur - 1].0.is_some() && lay.items[cur].0.is_some() {
                    let ks = if shift { 1.0 } else { app.prefs.kern_step };
                    let (v, kl, kr) = app.docs[di].font.kerning(m, &ln, &rn).unwrap_or((0.0, ln, rn));
                    app.docs[di].checkpoint_font("Kerning");
                    app.docs[di].font.set_kerning(m, &kl, &kr, v + dx.signum() * ks);
                }
            }
        }
        Tool::Metrics => {
            if let Some(gi) = lay.items.get(cur).and_then(|i| i.0) {
                app.docs[di].checkpoint_glyph(gi, "Spacing");
                let font = &mut app.docs[di].font;
                if alt {
                    let lsb = font.lsb(gi, m);
                    apply_metrics(font, gi, m, Some(lsb + dx), None, None);
                } else {
                    let w = font.glyphs[gi].layer(m).width;
                    apply_metrics(font, gi, m, None, None, Some(w + dx));
                }
            }
        }
        _ => {
            if let Some(gi) = lay.items.get(cur).and_then(|i| i.0) {
                if app.docs[di].font.glyphs[gi].layer(m).has_selection() {
                    app.docs[di].checkpoint_glyph(gi, "Nudge");
                    let g = &mut app.docs[di].font.glyphs[gi];
                    g.layer_mut(m).move_selection(dx, dy);
                    g.touch();
                }
            }
        }
    }
}

fn metrics_widget(app: &mut App, ui: &mut egui::Ui, canvas: Rect, di: usize, gi: usize, m: usize) {
    let pal = app.pal;
    let w = 360.0;
    let r = Rect::from_min_size(Pos2::new(canvas.center().x - w / 2.0, canvas.bottom() - 46.0), Vec2::new(w, 38.0));
    ui.painter().rect(r, CornerRadius::same(8), pal.panel, Stroke::new(1.0, pal.border), StrokeKind::Inside);
    let (mut lsb, mut width, mut rsb, name) = {
        let f = &app.docs[di].font;
        (f.lsb(gi, m).round(), f.glyphs[gi].layer(m).width.round(), f.rsb(gi, m).round(), f.glyphs[gi].name.clone())
    };
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r.shrink2(Vec2::new(10.0, 5.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
    child.label(egui::RichText::new(name).font(bold(12.0)).color(pal.text));
    child.add_space(6.0);
    child.label(egui::RichText::new(I::ARROW_RIGHT).font(icon_font(12.0)).color(pal.muted)).on_hover_text("Left sidebearing");
    let a = child.add(egui::DragValue::new(&mut lsb).speed(1.0).range(-5000.0..=5000.0));
    child.label(egui::RichText::new(I::MOVE_HORIZONTAL).font(icon_font(12.0)).color(pal.muted)).on_hover_text("Advance width");
    let b = child.add(egui::DragValue::new(&mut width).speed(1.0).range(0.0..=10000.0));
    child.label(egui::RichText::new(I::ARROW_LEFT).font(icon_font(12.0)).color(pal.muted)).on_hover_text("Right sidebearing");
    let c = child.add(egui::DragValue::new(&mut rsb).speed(1.0).range(-5000.0..=5000.0));
    if a.drag_started() || b.drag_started() || c.drag_started() || ((a.changed() || b.changed() || c.changed()) && !(a.dragged() || b.dragged() || c.dragged())) {
        app.docs[di].checkpoint_glyph(gi, "Metrics");
    }
    if a.changed() {
        apply_metrics(&mut app.docs[di].font, gi, m, Some(lsb), None, None);
    } else if b.changed() {
        apply_metrics(&mut app.docs[di].font, gi, m, None, None, Some(width));
    } else if c.changed() {
        apply_metrics(&mut app.docs[di].font, gi, m, None, Some(rsb), None);
    }
    if a.changed() || b.changed() || c.changed() {
        app.docs[di].font.dirty = true;
    }
}
