//! The font window: the glyph-filter sidebar on the left and the glyph cell grid.

use egui::{Align2, Color32, CornerRadius, Frame, Panel, Rect, Sense, Stroke, StrokeKind, Vec2};

use crate::app::*;
use crate::font::FLAG_COLORS;
use crate::glyphnames::{self, Category, Script};
use crate::icons as I;
use crate::render::{self, View};
use crate::theme::{bold, icon_font};
use crate::widgets::*;

fn matches(font: &crate::font::Font, gi: usize, f: &GlyphFilter) -> bool {
    let g = &font.glyphs[gi];
    let u = g.unicodes.first().copied();
    match f {
        GlyphFilter::All => true,
        GlyphFilter::Letters => glyphnames::category(&g.name, u).is_letter(),
        GlyphFilter::Category(c) => glyphnames::category(&g.name, u) == *c,
        GlyphFilter::Script(s) => glyphnames::script(u) == *s,
        GlyphFilter::Flag(i) => g.flag == Some(*i),
        GlyphFilter::Empty => g.layers.iter().all(|l| l.is_empty()),
        GlyphFilter::WithOutlines => g.layers.iter().any(|l| !l.contours.is_empty()),
        GlyphFilter::Composites => g.layers.iter().any(|l| !l.components.is_empty()),
        GlyphFilter::Unencoded => g.unicodes.is_empty(),
        GlyphFilter::Kerned => font.masters.iter().any(|m| m.kerning.iter().any(|k| k.left == g.name || k.right == g.name)),
        GlyphFilter::Search(q) => {
            let q = q.trim();
            let ql = q.to_lowercase();
            g.name.to_lowercase().contains(&ql)
                || (q.chars().count() == 1 && g.char().map(|c| c.to_string() == q).unwrap_or(false))
                || g.unicodes.iter().any(|u| format!("{u:04x}") == ql.trim_start_matches("u+").trim_start_matches("uni"))
        }
    }
}

pub fn rebuild_view(app: &mut App) {
    let sort = app.prefs.sort;
    let di = app.doc_idx();
    let d = &mut app.docs[di];
    let key = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        format!("{:?}", d.filter).hash(&mut h);
        d.font.glyphs.len().hash(&mut h);
        (sort as u8).hash(&mut h);
        d.undo.len().hash(&mut h);
        d.redo.len().hash(&mut h);
        h.finish()
    };
    if key == d.view_key {
        return;
    }
    d.view_key = key;
    let mut v: Vec<usize> = (0..d.font.glyphs.len()).filter(|&i| matches(&d.font, i, &d.filter)).collect();
    match sort {
        SortBy::Font => {}
        SortBy::Name => v.sort_by(|&a, &b| d.font.glyphs[a].name.cmp(&d.font.glyphs[b].name)),
        SortBy::Unicode => v.sort_by_key(|&i| d.font.glyphs[i].unicodes.first().copied().unwrap_or(u32::MAX)),
        SortBy::Category => v.sort_by_key(|&i| (glyphnames::category(&d.font.glyphs[i].name, d.font.glyphs[i].unicodes.first().copied()) as u8, i)),
    }
    d.view_cache = v;
}

fn count(font: &crate::font::Font, f: &GlyphFilter) -> usize {
    (0..font.glyphs.len()).filter(|&i| matches(font, i, f)).count()
}

fn section_label(ui: &mut egui::Ui, pal: &crate::theme::Palette, text: &str) {
    ui.add_space(6.0);
    ui.label(egui::RichText::new(text).font(bold(10.5)).color(pal.faint));
    ui.add_space(1.0);
}

pub fn sidebar(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    Panel::left("fontsidebar").default_size(220.0).size_range(170.0..=380.0).resizable(true).frame(Frame::new().fill(pal.panel)).show(ui, |ui| {
        dock_header(ui, &pal, |ui| {
            ui.label(egui::RichText::new(I::LIST_FILTER).font(icon_font(14.0)).color(pal.muted));
            ui.label(egui::RichText::new("Glyph Filters").font(bold(12.5)).color(pal.text));
        });
        egui::ScrollArea::vertical().id_salt("sidebar_scroll").auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(4.0);
            ui.spacing_mut().item_spacing.y = 1.0;
            let di = app.doc_idx();
            let mut set = None;
            {
                let font = &app.docs[di].font;
                let cur = app.docs[di].filter.clone();
                let mut row = |ui: &mut egui::Ui, depth: usize, icon: &str, label: &str, f: GlyphFilter| {
                    let n = count(font, &f);
                    let (r, _) = tree_row(ui, &pal, depth, None, icon, label, Some(n), cur == f);
                    if r.clicked() {
                        set = Some(f);
                    }
                };
                section_label(ui, &pal, "  BASICS");
                row(ui, 0, I::LAYOUT_GRID, "All Glyphs", GlyphFilter::All);
                row(ui, 0, I::SPLINE, "With Outlines", GlyphFilter::WithOutlines);
                row(ui, 0, I::SQUARE, "Empty", GlyphFilter::Empty);
                row(ui, 0, I::COMPONENT, "Composites", GlyphFilter::Composites);
                row(ui, 0, I::HASH, "Unencoded", GlyphFilter::Unencoded);
                row(ui, 0, I::ARROW_LEFT_RIGHT, "Kerned", GlyphFilter::Kerned);
                section_label(ui, &pal, "  CATEGORIES");
                row(ui, 0, I::CASE_SENSITIVE, "Letter", GlyphFilter::Letters);
                row(ui, 1, I::TYPE, "Uppercase", GlyphFilter::Category(Category::Uppercase));
                row(ui, 1, I::LETTER_TEXT, "Lowercase", GlyphFilter::Category(Category::Lowercase));
                row(ui, 1, I::LANGUAGES, "Other Letter", GlyphFilter::Category(Category::OtherLetter));
                row(ui, 0, I::BINARY, "Figure", GlyphFilter::Category(Category::Figure));
                row(ui, 0, I::PILCROW, "Punctuation", GlyphFilter::Category(Category::Punctuation));
                row(ui, 0, I::SIGMA, "Symbol", GlyphFilter::Category(Category::Symbol));
                row(ui, 0, I::DOT, "Mark", GlyphFilter::Category(Category::Mark));
                row(ui, 0, I::SPACE, "Separator", GlyphFilter::Category(Category::Separator));
                row(ui, 0, I::CIRCLE_DASHED, "Other", GlyphFilter::Category(Category::Other));
                section_label(ui, &pal, "  SCRIPTS");
                row(ui, 0, I::A_LARGE_SMALL, "Latin", GlyphFilter::Script(Script::Latin));
                row(ui, 0, I::A_LARGE_SMALL, "Greek", GlyphFilter::Script(Script::Greek));
                row(ui, 0, I::A_LARGE_SMALL, "Cyrillic", GlyphFilter::Script(Script::Cyrillic));
                row(ui, 0, I::GLOBE, "Common", GlyphFilter::Script(Script::Common));
                section_label(ui, &pal, "  FLAGS");
                for (i, c) in FLAG_COLORS.iter().enumerate() {
                    let f = GlyphFilter::Flag(i);
                    let n = count(font, &f);
                    let (r, _) = tree_row(ui, &pal, 0, None, I::FLAG, c.3, Some(n), cur == f);
                    ui.painter().text(r.rect.left_center() + Vec2::new(28.0, 0.0), Align2::CENTER_CENTER, I::FLAG, icon_font(14.0), Color32::from_rgb(c.0, c.1, c.2));
                    if r.clicked() {
                        set = Some(f);
                    }
                }
            }
            if let Some(f) = set {
                app.docs[di].filter = f;
                app.docs[di].view_key = u64::MAX;
                app.dlg.find_text.clear();
            }
            section_label(ui, &pal, "  SELECTIONS");
            let mut pick = None;
            for (i, (name, names)) in app.selections.iter().enumerate() {
                let (r, _) = tree_row(ui, &pal, 0, None, I::BOOKMARK, name, Some(names.len()), false);
                if r.clicked() {
                    pick = Some(i);
                }
            }
            if let Some(i) = pick {
                let names = app.selections[i].1.clone();
                let d = &mut app.docs[di];
                d.sel = names.iter().filter_map(|n| d.font.find(n)).collect();
                d.filter = GlyphFilter::All;
                d.view_key = u64::MAX;
            }
            if task_row(ui, &pal, I::PLUS, None, "Save current selection", !app.docs[di].sel.is_empty()).clicked() {
                let d = &app.docs[di];
                let names: Vec<String> = d.selected().iter().map(|&i| d.font.glyphs[i].name.clone()).collect();
                let n = app.selections.len() + 1;
                app.selections.push((format!("Selection {n}"), names));
            }
            section_label(ui, &pal, "  HISTORY");
            let hist: Vec<String> = app.docs[di].undo.iter().rev().take(8).map(|u| u.label.clone()).collect();
            if hist.is_empty() {
                ui.label(egui::RichText::new("     No changes yet").size(11.5).color(pal.faint));
            }
            for h in hist {
                tree_row(ui, &pal, 0, None, I::HISTORY, &h, None, false);
            }
            ui.add_space(10.0);
        });
    });
}

/// Draw one glyph fitted into `rect` (font window cells, previews).
pub fn paint_glyph_in_rect(app: &mut App, painter: &egui::Painter, di: usize, gi: usize, rect: Rect, color: Color32, template_color: Color32) {
    let font = &app.docs[di].font;
    let m = app.docs[di].master;
    let ms = &font.masters[m.min(font.masters.len() - 1)];
    let em_top = ms.ascender.max(font.info.upm * 0.75);
    let em_bot = ms.descender.min(-font.info.upm * 0.25);
    let em_h = (em_top - em_bot).max(1.0);
    let s = rect.height() / em_h;
    let g = &font.glyphs[gi];
    let l = g.layer(m);
    let width = l.width;
    let baseline = rect.bottom() + em_bot * s;
    if l.is_empty() {
        if app.prefs.show_template {
            if let Some(u) = g.unicodes.first().copied() {
                let upm = font.info.upm;
                if let Some((t, w)) = app.cache.template(u, upm) {
                    let ox = rect.center().x - w * s / 2.0;
                    let view = View { origin: egui::pos2(ox, baseline), s };
                    render::paint_fill(painter, &t, view, template_color);
                }
            }
        }
        return;
    }
    let ox = rect.center().x - width * s / 2.0;
    let view = View { origin: egui::pos2(ox, baseline), s };
    let t = app.cache.fill(di, &app.docs[di].font, gi, m);
    render::paint_fill(painter, &t, view, color);
}

pub fn grid(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    rebuild_view(app);
    let di = app.doc_idx();
    // Header row: caption + sort.
    let header = Rect::from_min_size(ui.cursor().min, Vec2::new(ui.available_width(), 32.0));
    ui.painter().rect_filled(header, 0.0, pal.panel);
    ui.painter().line_segment([header.left_bottom(), header.right_bottom()], Stroke::new(1.0, pal.border));
    let mut hui = ui.new_child(egui::UiBuilder::new().max_rect(header.shrink2(Vec2::new(8.0, 3.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
    hui.label(egui::RichText::new("Caption").size(11.5).color(pal.muted));
    for (c, l) in [(Caption::Name, "Name"), (Caption::Unicode, "Unicode"), (Caption::Index, "Index"), (Caption::Width, "Width")] {
        if hui.selectable_label(app.prefs.caption == c, l).clicked() {
            app.prefs.caption = c;
            app.prefs_dirty = true;
        }
    }
    vsep(&mut hui, &pal);
    hui.label(egui::RichText::new("Order").size(11.5).color(pal.muted));
    for (s, l) in [(SortBy::Font, "Font"), (SortBy::Name, "Name"), (SortBy::Unicode, "Unicode"), (SortBy::Category, "Category")] {
        if hui.selectable_label(app.prefs.sort == s, l).clicked() {
            app.prefs.sort = s;
            app.docs[di].view_key = u64::MAX;
            app.prefs_dirty = true;
        }
    }
    vsep(&mut hui, &pal);
    let filter_label = match &app.docs[di].filter {
        GlyphFilter::All => "All Glyphs".to_string(),
        GlyphFilter::Search(q) => format!("Search: {q}"),
        f => format!("{f:?}").replace("Category(", "").replace("Script(", "").replace(')', ""),
    };
    hui.label(egui::RichText::new(format!("{}  {}", I::LIST_FILTER, filter_label)).size(12.0).color(pal.accent_text));
    if app.docs[di].filter != GlyphFilter::All && hui.small_button(I::X).on_hover_text("Show all glyphs").clicked() {
        app.docs[di].filter = GlyphFilter::All;
        app.docs[di].view_key = u64::MAX;
        app.dlg.find_text.clear();
    }
    hui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if icon_button(ui, &pal, I::FILE_PLUS, "Add glyphs…", false).clicked() {
            app.dlg.add_glyphs = true;
        }
        if icon_button(ui, &pal, I::COMPONENT, "Build composite glyphs from components", false).clicked() {
            let ctx = ui.ctx().clone();
            app.build_glyphs(&ctx);
        }
    });
    ui.allocate_rect(header, Sense::hover());

    let view = app.docs[di].view_cache.clone();
    let avail = ui.available_width() - 8.0;
    let cell_w = app.prefs.cell_size;
    let cols = ((avail / cell_w).floor() as usize).max(1);
    let cw = (avail / cols as f32).floor();
    let name_h = 20.0;
    let ch = cw + name_h;
    let rows = view.len().div_ceil(cols);
    let mut open = None;
    let mut ctx_target = None;
    egui::ScrollArea::vertical().id_salt(("grid", di)).auto_shrink([false, false]).show_rows(ui, ch, rows, |ui, range| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        for row in range {
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                for col in 0..cols {
                    let k = row * cols + col;
                    if k >= view.len() {
                        break;
                    }
                    let gi = view[k];
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(cw, ch), Sense::click_and_drag());
                    if !ui.is_rect_visible(rect) {
                        continue;
                    }
                    let sel = app.docs[di].sel.contains(&gi);
                    let painter = ui.painter_at(rect);
                    let inner = rect.shrink(1.0);
                    let glyph_r = Rect::from_min_max(inner.min, egui::pos2(inner.max.x, inner.max.y - name_h));
                    let name_r = Rect::from_min_max(egui::pos2(inner.min.x, inner.max.y - name_h), inner.max);
                    let bg = if sel { pal.accent_soft } else if resp.hovered() { pal.hover.gamma_multiply(0.5) } else { pal.card };
                    painter.rect_filled(inner, CornerRadius::same(3), bg);
                    let (flag, name, empty) = {
                        let g = &app.docs[di].font.glyphs[gi];
                        (g.flag, g.name.clone(), g.layers.iter().all(|l| l.is_empty()))
                    };
                    let fg = if pal.dark { Color32::from_rgb(0xe4, 0xe6, 0xea) } else { Color32::from_rgb(0x1c, 0x1f, 0x26) };
                    let tmpl = if pal.dark { Color32::from_rgb(0x34, 0x39, 0x44) } else { Color32::from_rgb(0xd8, 0xdb, 0xe1) };
                    paint_glyph_in_rect(app, &painter, di, gi, glyph_r.shrink2(Vec2::new(4.0, cw * 0.1)), fg, tmpl);
                    let name_bg = if sel { pal.accent } else if let Some(f) = flag { let c = FLAG_COLORS[f]; Color32::from_rgb(c.0, c.1, c.2).gamma_multiply(0.55) } else { pal.raised };
                    painter.rect_filled(name_r, CornerRadius { nw: 0, ne: 0, sw: 3, se: 3 }, name_bg);
                    let caption = {
                        let g = &app.docs[di].font.glyphs[gi];
                        match app.prefs.caption {
                            Caption::Name => name.clone(),
                            Caption::Unicode => g.unicodes.first().map(|u| format!("U+{u:04X}")).unwrap_or_else(|| "—".into()),
                            Caption::Index => gi.to_string(),
                            Caption::Width => format!("{:.0}", g.layer(app.docs[di].master).width),
                        }
                    };
                    let tcol = if sel { Color32::WHITE } else if empty { pal.muted } else { pal.text };
                    painter.with_clip_rect(name_r.shrink2(Vec2::new(3.0, 0.0))).text(name_r.center(), Align2::CENTER_CENTER, caption, egui::FontId::proportional(11.5), tcol);
                    if sel {
                        painter.rect_stroke(inner, CornerRadius::same(3), Stroke::new(1.5, pal.accent), StrokeKind::Inside);
                    }
                    if resp.clicked() {
                        let mods = ui.input(|i| i.modifiers);
                        click_select(app, di, gi, &view, mods);
                    }
                    if resp.double_clicked() {
                        open = Some(gi);
                    }
                    if resp.secondary_clicked() && !app.docs[di].sel.contains(&gi) {
                        app.docs[di].sel.clear();
                        app.docs[di].sel.insert(gi);
                        app.docs[di].focus = Some(gi);
                    }
                    resp.context_menu(|ui| {
                        ctx_target = Some(gi);
                        cell_menu(app, ui);
                    });
                }
            });
        }
    });
    if let Some(gi) = open {
        app.open_glyph_tab(gi);
    }
    let _ = ctx_target;
}

fn click_select(app: &mut App, di: usize, gi: usize, view: &[usize], mods: egui::Modifiers) {
    let d = &mut app.docs[di];
    if mods.shift {
        if let Some(a) = d.anchor {
            let pa = view.iter().position(|&x| x == a).unwrap_or(0);
            let pb = view.iter().position(|&x| x == gi).unwrap_or(0);
            let (lo, hi) = (pa.min(pb), pa.max(pb));
            if !mods.command {
                d.sel.clear();
            }
            for &x in &view[lo..=hi] {
                d.sel.insert(x);
            }
        }
    } else if mods.command {
        if !d.sel.remove(&gi) {
            d.sel.insert(gi);
        }
        d.anchor = Some(gi);
    } else {
        d.sel.clear();
        d.sel.insert(gi);
        d.anchor = Some(gi);
    }
    d.focus = Some(gi);
}

fn cell_menu(app: &mut App, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    let mi = |ui: &mut egui::Ui, icon: &str, label: &str| {
        let r = ui.button(format!("{icon}  {label}")).clicked();
        if r {
            ui.close();
        }
        r
    };
    if mi(ui, I::PEN_TOOL, "Open in Glyph Window") {
        if let Some(gi) = app.current_glyph() {
            app.open_glyph_tab(gi);
        }
    }
    if mi(ui, I::MOVE_HORIZONTAL, "Open in Metrics Window") {
        let d = app.doc();
        let t: String = d.selected().iter().map(|&i| format!("/{} ", d.font.glyphs[i].name)).collect();
        app.open_text_tab(&t, Tool::Metrics);
    }
    ui.separator();
    if mi(ui, I::SCISSORS, "Cut") {
        app.cut(&ctx);
    }
    if mi(ui, I::COPY, "Copy") {
        app.copy(&ctx);
    }
    if mi(ui, I::CLIPBOARD_PASTE, "Paste") {
        app.paste(&ctx);
    }
    if mi(ui, I::COPY_PLUS, "Duplicate") {
        app.duplicate_glyphs(&ctx);
    }
    if mi(ui, I::TRASH_2, "Delete Glyphs") {
        app.delete(&ctx);
    }
    ui.separator();
    if mi(ui, I::PENCIL, "Rename…") {
        if let Some(gi) = app.current_glyph() {
            let n = app.doc().font.glyphs[gi].name.clone();
            let u = app.doc().font.glyphs[gi].unicode_str();
            app.dlg.rename = Some((gi, n, u));
        }
    }
    if mi(ui, I::COMPONENT, "Build Composites") {
        app.build_glyphs(&ctx);
    }
    if mi(ui, I::BOXES, "Decompose") {
        app.decompose();
    }
    if mi(ui, I::TRASH_2, "Clear Outlines") {
        app.edit_glyphs("Clear outlines", |g, m| {
            let l = g.layer_mut(m);
            l.contours.clear();
            l.components.clear();
        });
    }
    ui.menu_button(format!("{}  Flag", I::FLAG), |ui| {
        if ui.button("No flag").clicked() {
            app.set_flag(None);
            ui.close();
        }
        for (i, c) in FLAG_COLORS.iter().enumerate() {
            if ui.button(egui::RichText::new(format!("{}  {}", I::FLAG, c.3)).color(Color32::from_rgb(c.0, c.1, c.2))).clicked() {
                app.set_flag(Some(i));
                ui.close();
            }
        }
    });
    ui.menu_button(format!("{}  Copy Text As", I::COPY), |ui| {
        if ui.button("Glyph names").clicked() {
            let d = app.doc();
            ctx.copy_text(d.selected().iter().map(|&i| format!("/{}", d.font.glyphs[i].name)).collect::<String>());
            ui.close();
        }
        if ui.button("Characters").clicked() {
            let d = app.doc();
            ctx.copy_text(d.selected().iter().filter_map(|&i| d.font.glyphs[i].char()).collect::<String>());
            ui.close();
        }
    });
    ui.separator();
    if mi(ui, I::LIST_CHECKS, "Select All") {
        app.select_all();
    }
}
