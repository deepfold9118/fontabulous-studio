//! Dialog windows: Welcome, Font Info, Preferences, Export, Find, Add Glyphs,
//! Rename, kerning tools, components/anchors, variations, instances and prompts.

use egui::{Align2, CornerRadius, Sense, Vec2};

use crate::app::*;
use crate::font::{Anchor, Axis, Component, Guide};
use crate::glyphnames;
use crate::icons as I;
use crate::theme::{bold, icon_font, ThemeMode};
use crate::widgets::*;

fn window<'a>(title: &'a str, id: &'a str) -> egui::Window<'a> {
    egui::Window::new(title).id(egui::Id::new(id)).collapsible(false).resizable(true)
}

pub fn dialogs(app: &mut App, ctx: &egui::Context) {
    welcome(app, ctx);
    font_info(app, ctx);
    prefs(app, ctx);
    export(app, ctx);
    find(app, ctx);
    add_glyphs(app, ctx);
    rename(app, ctx);
    installed(app, ctx);
    kerning_tools(app, ctx);
    component_anchor(app, ctx);
    variation(app, ctx);
    instance(app, ctx);
    metrics_link(app, ctx);
    actions(app, ctx);
    shortcuts(app, ctx);
    about(app, ctx);
    prompts(app, ctx);
}

fn welcome(app: &mut App, ctx: &egui::Context) {
    if !app.dlg.welcome {
        return;
    }
    let pal = app.pal;
    let mut open = true;
    window("Welcome to Fontabulous Studio", "welcome").open(&mut open).resizable(false).default_width(640.0).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
        ui.columns(2, |cols| {
            let ui = &mut cols[0];
            ui.label(egui::RichText::new("Create New Font").font(bold(20.0)).color(pal.text));
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Family name").size(12.0).color(pal.muted));
            ui.add(egui::TextEdit::singleline(&mut app.dlg.welcome_family).hint_text("Typographic Family Name").desired_width(260.0));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Glyph set").size(12.0).color(pal.muted));
                egui::ComboBox::from_id_salt("welcome_set").selected_text(glyphnames::GLYPH_SETS[app.dlg.welcome_set]).width(170.0).show_index(ui, &mut app.dlg.welcome_set, glyphnames::GLYPH_SETS.len(), |i| glyphnames::GLYPH_SETS[i]);
            });
            ui.add_space(10.0);
            if icon_text_button(ui, &pal, I::FILE_PLUS, "Create New Font", true).clicked() {
                let fam = app.dlg.welcome_family.clone();
                let set = app.dlg.welcome_set;
                app.new_font(&fam, set);
                app.dlg.welcome = false;
            }
            ui.add_space(18.0);
            ui.label(egui::RichText::new("Tip").font(bold(16.0)).color(pal.text));
            ui.label(egui::RichText::new("Double-click any glyph cell to draw it. Empty cells show a faint template letter to trace over. Press P for the Pen, A to edit nodes.").size(12.0).color(pal.muted));
            let ui = &mut cols[1];
            ui.label(egui::RichText::new("Open Font").font(bold(20.0)).color(pal.text));
            ui.add_space(8.0);
            if task_row(ui, &pal, I::FOLDER_OPEN, None, "Open font file…", true).clicked() {
                app.open_dialog();
                app.dlg.welcome = false;
            }
            if task_row(ui, &pal, I::FOLDER, None, "Open UFO folder…", true).clicked() {
                app.open_ufo_dialog();
                app.dlg.welcome = false;
            }
            if task_row(ui, &pal, I::MONITOR, None, "Open installed font…", true).clicked() {
                app.dlg.installed = crate::io::installed_fonts();
                app.dlg.open_installed = true;
                app.dlg.welcome = false;
            }
            ui.add_space(10.0);
            ui.label(egui::RichText::new("Recent").font(bold(12.0)).color(pal.muted));
            let recent: Vec<_> = app.prefs.recent.iter().take(6).cloned().collect();
            if recent.is_empty() {
                ui.label(egui::RichText::new("No recent fonts").size(11.5).color(pal.faint));
            }
            for p in recent {
                let name = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                if task_row(ui, &pal, I::FILE_TYPE, None, &name, true).on_hover_text(p.display().to_string()).clicked() {
                    app.open_path(&p);
                    app.dlg.welcome = false;
                }
            }
        });
        ui.separator();
        ui.horizontal(|ui| {
            if ui.checkbox(&mut app.prefs.welcome_at_startup, "Open this dialog at startup").changed() {
                app.prefs_dirty = true;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Close").clicked() {
                    app.dlg.welcome = false;
                }
            });
        });
    });
    if !open {
        app.dlg.welcome = false;
    }
}

const FI_PAGES: [(&str, &str); 14] = [
    ("Names", "TAG"),
    ("Family Dimensions", "RULER"),
    ("Font Dimensions", "MOVE_VERTICAL"),
    ("Note", "NOTEBOOK_PEN"),
    ("Stems", "GRIP_VERTICAL"),
    ("Zones", "ALIGN_CENTER_HORIZONTAL"),
    ("Guides", "RULER_DIMENSION_LINE"),
    ("Other Values", "ELLIPSIS"),
    ("Creator", "USER"),
    ("Legal", "SHIELD_CHECK"),
    ("Codepages", "LAYOUT_GRID"),
    ("Unicode Ranges", "BRACES"),
    ("Axes", "VARIABLE"),
    ("Masters & Instances", "LAYERS_3"),
];

fn fi_icon(k: &str) -> &'static str {
    match k {
        "TAG" => I::TAG,
        "RULER" => I::RULER,
        "MOVE_VERTICAL" => I::MOVE_VERTICAL,
        "NOTEBOOK_PEN" => I::NOTEBOOK_PEN,
        "GRIP_VERTICAL" => I::GRIP_VERTICAL,
        "ALIGN_CENTER_HORIZONTAL" => I::ALIGN_CENTER_HORIZONTAL,
        "RULER_DIMENSION_LINE" => I::RULER_DIMENSION_LINE,
        "ELLIPSIS" => I::ELLIPSIS,
        "USER" => I::INFO,
        "SHIELD_CHECK" => I::SHIELD_CHECK,
        "LAYOUT_GRID" => I::LAYOUT_GRID,
        "BRACES" => I::BRACES,
        "VARIABLE" => I::VARIABLE,
        _ => I::LAYERS_3,
    }
}

fn field(ui: &mut egui::Ui, label: &str, v: &mut String) -> bool {
    ui.label(egui::RichText::new(label).size(12.0));
    let r = ui.add(egui::TextEdit::singleline(v).desired_width(320.0)).changed();
    ui.end_row();
    r
}

fn numf(ui: &mut egui::Ui, label: &str, v: &mut f32) -> bool {
    ui.label(egui::RichText::new(label).size(12.0));
    let r = ui.add(egui::DragValue::new(v).speed(1.0)).changed();
    ui.end_row();
    r
}

fn font_info(app: &mut App, ctx: &egui::Context) {
    if !app.dlg.font_info {
        return;
    }
    let pal = app.pal;
    let di = app.doc_idx();
    let mut open = true;
    let title = format!("Font Info: {}", app.docs[di].font.info.family);
    window(&title, "fontinfo").open(&mut open).default_size([760.0, 520.0]).show(ctx, |ui| {
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(190.0);
                ui.label(egui::RichText::new("MASTER PROPERTIES").font(bold(10.5)).color(pal.faint));
                for (i, (name, icon)) in FI_PAGES.iter().enumerate() {
                    if i == 8 {
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new("FONT PROPERTIES").font(bold(10.5)).color(pal.faint));
                    }
                    let (r, _) = tree_row(ui, &pal, 0, None, fi_icon(icon), name, None, app.dlg.fi_page == i);
                    if r.clicked() {
                        app.dlg.fi_page = i;
                    }
                }
            });
            ui.separator();
            ui.vertical(|ui| {
                let m = app.docs[di].master;
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("Font: {}   Master:", app.docs[di].font.info.full())).font(bold(12.0)));
                    let names: Vec<String> = app.docs[di].font.masters.iter().map(|m| m.name.clone()).collect();
                    let mut mm = m;
                    egui::ComboBox::from_id_salt("fi_master").selected_text(names[m].clone()).show_ui(ui, |ui| {
                        for (i, n) in names.iter().enumerate() {
                            ui.selectable_value(&mut mm, i, n);
                        }
                    });
                    app.docs[di].master = mm;
                });
                ui.separator();
                egui::ScrollArea::vertical().id_salt("fi_scroll").auto_shrink([false, false]).show(ui, |ui| {
                    let changed = fi_page(app, ui, di);
                    if changed {
                        app.docs[di].font.dirty = true;
                        for g in &mut app.docs[di].font.glyphs {
                            g.touch();
                        }
                    }
                });
            });
        });
    });
    if !open {
        app.dlg.font_info = false;
    }
}

fn fi_page(app: &mut App, ui: &mut egui::Ui, di: usize) -> bool {
    let pal = app.pal;
    let m = app.docs[di].master;
    let page = app.dlg.fi_page;
    let f = &mut app.docs[di].font;
    let mut ch = false;
    match page {
        0 => {
            egui::Grid::new("fi_names").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                ch |= field(ui, "Family name", &mut f.info.family);
                ch |= field(ui, "Style name", &mut f.info.style);
                ui.label("Weight class");
                ch |= ui.add(egui::DragValue::new(&mut f.info.weight_class).range(1..=1000)).changed();
                ui.end_row();
                ui.label("Width class");
                ch |= ui.add(egui::DragValue::new(&mut f.info.width_class).range(1..=9)).changed();
                ui.end_row();
                ui.label("Monospaced");
                ch |= ui.checkbox(&mut f.info.monospaced, "").changed();
                ui.end_row();
                let mut full = f.info.full();
                if field(ui, "Full name", &mut full) {
                    f.info.full_name = full;
                    ch = true;
                }
                let mut ps = f.info.postscript();
                if field(ui, "PostScript name", &mut ps) {
                    f.info.ps_name = ps;
                    ch = true;
                }
                ch |= field(ui, "Style group", &mut f.info.style_group);
                ch |= field(ui, "Master name", &mut f.masters[m].name);
            });
            if ui.button(format!("{}  Build Names", I::REFRESH_CW)).clicked() {
                f.info.full_name = format!("{} {}", f.info.family, f.info.style);
                f.info.ps_name = format!("{}-{}", f.info.family, f.info.style).replace(' ', "");
                ch = true;
            }
        }
        1 => {
            egui::Grid::new("fi_fam").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                ui.label("Units per em");
                ch |= ui.add(egui::DragValue::new(&mut f.info.upm).range(16.0..=16384.0)).changed();
                ui.end_row();
                ui.label("Version");
                ui.horizontal(|ui| {
                    ch |= ui.add(egui::DragValue::new(&mut f.info.version_major)).changed();
                    ui.label(".");
                    ch |= ui.add(egui::DragValue::new(&mut f.info.version_minor).range(0..=999)).changed();
                });
                ui.end_row();
                ch |= field(ui, "Vendor ID", &mut f.info.vendor_id);
            });
        }
        2 => {
            let ms = &mut f.masters[m];
            egui::Grid::new("fi_dims").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                ch |= numf(ui, "Ascender", &mut ms.ascender);
                ch |= numf(ui, "Caps height", &mut ms.cap_height);
                ch |= numf(ui, "x-height", &mut ms.x_height);
                ch |= numf(ui, "Descender", &mut ms.descender);
                ch |= numf(ui, "Line gap", &mut ms.line_gap);
                ch |= numf(ui, "Italic angle", &mut ms.italic_angle);
                ch |= numf(ui, "Underline position", &mut ms.underline_position);
                ch |= numf(ui, "Underline thickness", &mut ms.underline_thickness);
            });
        }
        3 => {
            ch |= ui.add(egui::TextEdit::multiline(&mut f.info.note).desired_rows(14).desired_width(f32::INFINITY).hint_text("Notes about this font")).changed();
        }
        4 => {
            let ms = &mut f.masters[m];
            for (label, v) in [("Horizontal stems (comma separated)", &mut ms.h_stems), ("Vertical stems", &mut ms.v_stems)] {
                ui.label(label);
                let mut s = v.iter().map(|x| format!("{x:.0}")).collect::<Vec<_>>().join(", ");
                if ui.add(egui::TextEdit::singleline(&mut s).desired_width(320.0)).changed() {
                    *v = s.split(',').filter_map(|t| t.trim().parse().ok()).collect();
                    ch = true;
                }
            }
        }
        5 => {
            let ms = &mut f.masters[m];
            ui.label(egui::RichText::new("Alignment zones: position and depth (negative depth = bottom zone)").size(11.5).color(pal.muted));
            let mut remove = None;
            for (i, z) in ms.zones.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ch |= ui.add(egui::DragValue::new(&mut z.0).prefix("position ")).changed();
                    ch |= ui.add(egui::DragValue::new(&mut z.1).prefix("depth ")).changed();
                    if ui.small_button(I::X).clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                ms.zones.remove(i);
                ch = true;
            }
            if ui.button(format!("{}  Add zone", I::PLUS)).clicked() {
                ms.zones.push((ms.x_height, 15.0));
                ch = true;
            }
        }
        6 => {
            let ms = &mut f.masters[m];
            ui.label(egui::RichText::new("Font guides appear in every glyph of this master.").size(11.5).color(pal.muted));
            let mut remove = None;
            for (i, g) in ms.guides.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ch |= ui.add(egui::TextEdit::singleline(&mut g.name).desired_width(90.0)).changed();
                    ch |= ui.add(egui::DragValue::new(&mut g.x).prefix("x ")).changed();
                    ch |= ui.add(egui::DragValue::new(&mut g.y).prefix("y ")).changed();
                    ch |= ui.add(egui::DragValue::new(&mut g.angle).suffix("°")).changed();
                    if ui.small_button(I::X).clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                ms.guides.remove(i);
                ch = true;
            }
            if ui.button(format!("{}  Add guide", I::PLUS)).clicked() {
                ms.guides.push(Guide { name: "guide".into(), x: 0.0, y: ms.x_height / 2.0, angle: 0.0, color: None });
                ch = true;
            }
        }
        7 => {
            egui::Grid::new("fi_other").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                ui.label("Embedding (fsType)");
                let opts = ["Installable (0)", "Restricted (2)", "Preview & print (4)", "Editable (8)"];
                let vals = [0u16, 2, 4, 8];
                let mut idx = vals.iter().position(|v| *v == f.info.fs_type).unwrap_or(0);
                if egui::ComboBox::from_id_salt("fstype").selected_text(opts[idx]).show_index(ui, &mut idx, 4, |i| opts[i]).changed() {
                    f.info.fs_type = vals[idx];
                    ch = true;
                }
                ui.end_row();
                ui.label("Master weight class");
                ch |= ui.add(egui::DragValue::new(&mut f.masters[m].weight_class).range(1..=1000)).changed();
                ui.end_row();
                ui.label("Master width class");
                ch |= ui.add(egui::DragValue::new(&mut f.masters[m].width_class).range(1..=9)).changed();
                ui.end_row();
            });
            ui.add_space(6.0);
            ui.label(egui::RichText::new("Font lib (key = value)").size(11.5).color(pal.muted));
            for (k, v) in f.lib.iter() {
                ui.label(egui::RichText::new(format!("{k} = {v}")).monospace().size(11.0));
            }
        }
        8 => {
            egui::Grid::new("fi_creator").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                ch |= field(ui, "Designer", &mut f.info.designer);
                ch |= field(ui, "Designer URL", &mut f.info.designer_url);
                ch |= field(ui, "Manufacturer", &mut f.info.manufacturer);
                ch |= field(ui, "Manufacturer URL", &mut f.info.manufacturer_url);
            });
            ui.label("Description");
            ch |= ui.add(egui::TextEdit::multiline(&mut f.info.description).desired_rows(4).desired_width(f32::INFINITY)).changed();
        }
        9 => {
            egui::Grid::new("fi_legal").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                ch |= field(ui, "Copyright", &mut f.info.copyright);
                ch |= field(ui, "Trademark", &mut f.info.trademark);
                ch |= field(ui, "License URL", &mut f.info.license_url);
            });
            ui.label("License");
            ch |= ui.add(egui::TextEdit::multiline(&mut f.info.license).desired_rows(6).desired_width(f32::INFINITY)).changed();
            if ui.button("Use SIL Open Font License 1.1").clicked() {
                f.info.license = "This Font Software is licensed under the SIL Open Font License, Version 1.1.".into();
                f.info.license_url = "https://openfontlicense.org".into();
                ch = true;
            }
        }
        10 => {
            let all = ["1252 Latin 1", "1250 Latin 2: Eastern Europe", "1251 Cyrillic", "1253 Greek", "1254 Turkish", "1257 Windows Baltic", "Macintosh Character Set (US Roman)"];
            for c in all {
                let mut on = f.info.codepages.iter().any(|x| x == c);
                if ui.checkbox(&mut on, c).changed() {
                    if on {
                        f.info.codepages.push(c.to_string());
                    } else {
                        f.info.codepages.retain(|x| x != c);
                    }
                    ch = true;
                }
            }
        }
        11 => {
            let us: std::collections::HashSet<u32> = f.glyphs.iter().flat_map(|g| g.unicodes.iter().copied()).collect();
            ui.label(egui::RichText::new("Unicode ranges are set automatically on export from the glyphs in the font.").size(11.5).color(pal.muted));
            for (a, b, name) in glyphnames::BLOCKS {
                let n = (a..=b).filter(|u| us.contains(u)).count();
                if n > 0 {
                    ui.label(format!("{}  {name}  ({n} glyphs)", I::CHECK));
                }
            }
        }
        12 => {
            let mut remove = None;
            for (i, a) in f.axes.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ch |= ui.add(egui::TextEdit::singleline(&mut a.name).desired_width(80.0)).changed();
                    ch |= ui.add(egui::TextEdit::singleline(&mut a.tag).desired_width(40.0)).changed();
                    ch |= ui.add(egui::DragValue::new(&mut a.min).prefix("min ")).changed();
                    ch |= ui.add(egui::DragValue::new(&mut a.default).prefix("default ")).changed();
                    ch |= ui.add(egui::DragValue::new(&mut a.max).prefix("max ")).changed();
                    if ui.small_button(I::X).clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                f.axes.remove(i);
                for ms in &mut f.masters {
                    if i < ms.location.len() {
                        ms.location.remove(i);
                    }
                }
                ch = true;
            }
            if ui.button(format!("{}  Add axis", I::PLUS)).clicked() {
                f.axes.push(Axis { name: "Weight".into(), tag: "wght".into(), min: 100.0, default: 400.0, max: 900.0 });
                for ms in &mut f.masters {
                    ms.location.push(400.0);
                }
                ch = true;
            }
        }
        _ => {
            ui.label(egui::RichText::new("Masters").font(bold(12.0)));
            for ms in &mut f.masters {
                ui.horizontal(|ui| {
                    ch |= ui.add(egui::TextEdit::singleline(&mut ms.name).desired_width(120.0)).changed();
                    for v in ms.location.iter_mut() {
                        ch |= ui.add(egui::DragValue::new(v)).changed();
                    }
                });
            }
            ui.add_space(6.0);
            ui.label(egui::RichText::new("Instances").font(bold(12.0)));
            let mut remove = None;
            for (i, inst) in f.instances.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ch |= ui.add(egui::TextEdit::singleline(&mut inst.name).desired_width(120.0)).changed();
                    for v in inst.location.iter_mut() {
                        ch |= ui.add(egui::DragValue::new(v)).changed();
                    }
                    ch |= ui.checkbox(&mut inst.export, "Export").changed();
                    if ui.small_button(I::X).clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                f.instances.remove(i);
                ch = true;
            }
            if ui.button(format!("{}  Add instance", I::PLUS)).clicked() {
                let loc = f.axes.iter().map(|a| a.default).collect();
                let n = f.instances.len() + 1;
                f.instances.push(crate::font::Instance { name: format!("Instance {n}"), location: loc, export: true });
                ch = true;
            }
        }
    }
    ch
}

fn prefs(app: &mut App, ctx: &egui::Context) {
    if !app.dlg.prefs {
        return;
    }
    let pal = app.pal;
    let mut open = true;
    let pages = ["General", "Font Window", "Glyph Window", "Editing", "Spacing & Kerning", "Texts", "Agents (MCP)"];
    window("Preferences", "prefs").open(&mut open).default_size([620.0, 420.0]).show(ctx, |ui| {
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(150.0);
                for (i, p) in pages.iter().enumerate() {
                    let icon = [I::SETTINGS, I::LAYOUT_GRID, I::PEN_TOOL, I::MOUSE_POINTER_2, I::ARROW_LEFT_RIGHT, I::TEXT_QUOTE, I::BOT][i];
                    let (r, _) = tree_row(ui, &pal, 0, None, icon, p, None, app.dlg.prefs_page == i);
                    if r.clicked() {
                        app.dlg.prefs_page = i;
                    }
                }
            });
            ui.separator();
            ui.vertical(|ui| {
                let p = &mut app.prefs;
                match app.dlg.prefs_page {
                    0 => {
                        let mut dark = p.theme == ThemeMode::Dark;
                        ui.horizontal(|ui| {
                            ui.label("Theme");
                            ui.selectable_value(&mut dark, true, format!("{}  Dark", I::MOON));
                            ui.selectable_value(&mut dark, false, format!("{}  Light", I::SUN));
                        });
                        let want = if dark { ThemeMode::Dark } else { ThemeMode::Light };
                        if want != p.theme {
                            app.set_theme(ctx, want);
                        }
                        let p = &mut app.prefs;
                        ui.checkbox(&mut p.welcome_at_startup, "Show the Welcome dialog at startup");
                        ui.horizontal(|ui| {
                            ui.label("New fonts start with");
                            egui::ComboBox::from_id_salt("pref_set").selected_text(glyphnames::GLYPH_SETS[p.default_glyph_set.min(4)]).show_index(ui, &mut p.default_glyph_set, glyphnames::GLYPH_SETS.len(), |i| glyphnames::GLYPH_SETS[i]);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Undo levels");
                            ui.add(egui::DragValue::new(&mut p.undo_levels).range(10..=1000));
                        });
                    }
                    1 => {
                        ui.horizontal(|ui| {
                            ui.label("Cell size");
                            ui.add(egui::Slider::new(&mut p.cell_size, 40.0..=320.0));
                        });
                        ui.checkbox(&mut p.show_template, "Show template glyphs in empty cells");
                        ui.horizontal(|ui| {
                            ui.label("Caption");
                            ui.selectable_value(&mut p.caption, Caption::Name, "Name");
                            ui.selectable_value(&mut p.caption, Caption::Unicode, "Unicode");
                            ui.selectable_value(&mut p.caption, Caption::Index, "Index");
                            ui.selectable_value(&mut p.caption, Caption::Width, "Width");
                        });
                    }
                    2 => {
                        ui.checkbox(&mut p.show_metrics, "Metrics lines");
                        ui.checkbox(&mut p.show_zones, "Alignment zones");
                        ui.checkbox(&mut p.show_nodes, "Nodes and handles");
                        ui.checkbox(&mut p.show_coords, "Node coordinates");
                        ui.checkbox(&mut p.show_anchors, "Anchors");
                        ui.checkbox(&mut p.show_guides, "Guides");
                        ui.checkbox(&mut p.show_grid, "Grid");
                        ui.horizontal(|ui| {
                            ui.label("Grid step");
                            ui.add(egui::DragValue::new(&mut p.grid_step).range(1.0..=500.0));
                        });
                    }
                    3 => {
                        ui.horizontal(|ui| {
                            ui.label("Arrow nudge");
                            ui.add(egui::DragValue::new(&mut p.nudge).range(0.1..=100.0));
                            ui.label("Shift");
                            ui.add(egui::DragValue::new(&mut p.nudge_shift).range(1.0..=500.0));
                            ui.label("Ctrl");
                            ui.add(egui::DragValue::new(&mut p.nudge_ctrl).range(1.0..=1000.0));
                        });
                        ui.checkbox(&mut p.snap, "Snap points to the grid while dragging");
                    }
                    4 => {
                        ui.horizontal(|ui| {
                            ui.label("Kerning step (arrow keys)");
                            ui.add(egui::DragValue::new(&mut p.kern_step).range(1.0..=100.0));
                        });
                        ui.checkbox(&mut p.show_kerning, "Apply kerning in text views");
                    }
                    6 => {
                        ui.label(egui::RichText::new("Let AI agents use Fontabulous Studio").font(bold(14.0)));
                        ui.label(egui::RichText::new("Agents that speak MCP (Model Context Protocol) can run every menu command, edit outlines, spacing, kerning and font info, and take screenshots. Everything they do appears live, is logged in the Output panel and can be undone.").size(11.5).color(pal.muted));
                        ui.add_space(6.0);
                        if ui.checkbox(&mut p.mcp_http, "Allow agents to connect on this computer").changed() {
                            app.prefs_dirty = true;
                        }
                        let p = &mut app.prefs;
                        ui.horizontal(|ui| {
                            ui.label("Port");
                            if ui.add(egui::DragValue::new(&mut p.mcp_port).range(1024..=65535)).changed() {
                                app.prefs_dirty = true;
                            }
                        });
                        let url = format!("http://127.0.0.1:{}/mcp", app.prefs.mcp_port);
                        if let Some(e) = &app.mcp.http_error {
                            ui.label(egui::RichText::new(format!("{}  {e}", I::TRIANGLE_ALERT)).size(11.5).color(pal.red));
                        } else if app.mcp.http_port().is_some() {
                            ui.label(egui::RichText::new(format!("{}  Listening at {url}", I::CHECK)).size(11.5).color(pal.green));
                        } else {
                            ui.label(egui::RichText::new("Off — agents cannot connect over HTTP.").size(11.5).color(pal.faint));
                        }
                        ui.label(egui::RichText::new("Only programs on this computer can connect; web pages are refused.").size(11.0).color(pal.faint));
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("Connect an agent").font(bold(12.0)));
                        let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_else(|_| "fontabulous_studio".into());
                        let http_cfg = format!("{{
  \"mcpServers\": {{
    \"fontabulous-studio\": {{ \"type\": \"http\", \"url\": \"{url}\" }}
  }}
}}");
                        let stdio_cfg = format!("{{
  \"mcpServers\": {{
    \"fontabulous-studio\": {{ \"command\": {:?}, \"args\": [\"--mcp\"] }}
  }}
}}", exe);
                        ui.label(egui::RichText::new("Attach to this window (HTTP, needs the option above):").size(11.5).color(pal.muted));
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(&url).monospace().size(11.0));
                            if ui.small_button(format!("{}  Copy config", I::COPY)).clicked() {
                                ui.ctx().copy_text(http_cfg.clone());
                            }
                        });
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("Or let the agent start its own window (stdio):").size(11.5).color(pal.muted));
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("fontabulous_studio --mcp").monospace().size(11.0));
                            if ui.small_button(format!("{}  Copy config", I::COPY)).clicked() {
                                ui.ctx().copy_text(stdio_cfg.clone());
                            }
                        });
                        if app.mcp.stdio {
                            ui.label(egui::RichText::new(format!("{}  This window was started by an agent (stdio).", I::BOT)).size(11.5).color(pal.accent_text));
                        }
                        if app.mcp.calls > 0 {
                            ui.label(egui::RichText::new(format!("{} agent call(s) this session", app.mcp.calls)).size(11.0).color(pal.faint));
                        }
                    }
                    _ => {
                        ui.label("Sample texts (one per line)");
                        let mut s = p.sample_texts.join("\n");
                        if ui.add(egui::TextEdit::multiline(&mut s).desired_rows(8).desired_width(f32::INFINITY)).changed() {
                            p.sample_texts = s.lines().map(|l| l.to_string()).filter(|l| !l.trim().is_empty()).collect();
                        }
                    }
                }
            });
        });
    });
    if !open {
        app.dlg.prefs = false;
        app.prefs_dirty = true;
    }
}

fn export(app: &mut App, ctx: &egui::Context) {
    if !app.dlg.export {
        return;
    }
    let pal = app.pal;
    let mut open = true;
    let formats = [
        (I::FILE_TYPE, "TrueType (.ttf)", "Desktop font with TrueType outlines, kerning in the kern table."),
        (I::FOLDER, "UFO 3 (.ufo + .designspace)", "Source format for other font editors and build tools."),
        (I::SAVE, "Fontabulous Studio (.fstudio)", "This app's own format; keeps everything."),
        (I::FILE_CODE, "OpenType PS (.otf)  — coming soon", "CFF outlines."),
        (I::GLOBE, "Web font (.woff2)  — coming soon", "Compressed font for websites."),
    ];
    window("Export Font As", "export").open(&mut open).default_width(520.0).show(ctx, |ui| {
        for (i, (icon, name, desc)) in formats.iter().enumerate() {
            let sel = app.dlg.export_format == i;
            let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 44.0), Sense::click());
            ui.painter().rect_filled(r, CornerRadius::same(6), if sel { pal.accent_soft } else if resp.hovered() { pal.hover } else { pal.raised });
            ui.painter().text(r.left_center() + Vec2::new(18.0, 0.0), Align2::CENTER_CENTER, *icon, icon_font(16.0), if sel { pal.accent_text } else { pal.muted });
            ui.painter().text(r.left_top() + Vec2::new(38.0, 8.0), Align2::LEFT_TOP, *name, bold(12.5), pal.text);
            ui.painter().text(r.left_top() + Vec2::new(38.0, 25.0), Align2::LEFT_TOP, *desc, egui::FontId::proportional(11.0), pal.muted);
            if resp.clicked() {
                app.dlg.export_format = i;
            }
            ui.add_space(3.0);
        }
        ui.add_space(4.0);
        if app.dlg.export_format == 0 {
            ui.checkbox(&mut app.dlg.export_masters, "Export every master (otherwise only the current one)");
            let can = app.doc().font.masters.len() > 1 && !app.doc().font.instances.is_empty();
            ui.add_enabled(can, egui::Checkbox::new(&mut app.dlg.export_instances, "Also export instances (interpolated)"));
        }
        ui.horizontal(|ui| {
            if icon_text_button(ui, &pal, I::FILE_OUTPUT, "Export…", true).clicked() {
                let f = app.dlg.export_format;
                app.dlg.export = false;
                app.export(ctx, f);
            }
            if ui.button("Cancel").clicked() {
                app.dlg.export = false;
            }
        });
    });
    if !open {
        app.dlg.export = false;
    }
}

fn find(app: &mut App, ctx: &egui::Context) {
    if !app.dlg.find {
        return;
    }
    let pal = app.pal;
    let mut open = true;
    window(&format!("{}  Find Glyphs", I::SEARCH), "find").open(&mut open).default_width(360.0).show(ctx, |ui| {
        ui.label(egui::RichText::new("Name, character or Unicode (e.g. 'acute', 'é', 'U+00E9')").size(11.5).color(pal.muted));
        let r = ui.add(egui::TextEdit::singleline(&mut app.dlg.find_text).desired_width(f32::INFINITY));
        r.request_focus();
        let q = app.dlg.find_text.clone();
        let d = app.doc_mut();
        let nf = if q.trim().is_empty() { GlyphFilter::All } else { GlyphFilter::Search(q) };
        if d.filter != nf {
            d.filter = nf;
            d.view_key = u64::MAX;
        }
        ui.horizontal(|ui| {
            if ui.button("Select results").clicked() {
                let d = app.doc_mut();
                d.sel = d.view_cache.iter().copied().collect();
            }
            if ui.button("Open first").clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if let Some(&gi) = app.doc().view_cache.first() {
                    app.open_glyph_tab(gi);
                    app.dlg.find = false;
                }
            }
            ui.label(egui::RichText::new(format!("{} found", app.doc().view_cache.len())).size(11.5).color(pal.muted));
        });
    });
    if !open {
        app.dlg.find = false;
    }
}

fn add_glyphs(app: &mut App, ctx: &egui::Context) {
    if !app.dlg.add_glyphs {
        return;
    }
    let pal = app.pal;
    let mut open = true;
    window(&format!("{}  Add Glyphs", I::FILE_PLUS), "addglyphs").open(&mut open).default_width(440.0).show(ctx, |ui| {
        ui.label(egui::RichText::new("Glyph names, characters or U+XXXX values, separated by spaces").size(11.5).color(pal.muted));
        ui.add(egui::TextEdit::multiline(&mut app.dlg.add_glyphs_text).desired_rows(4).desired_width(f32::INFINITY).hint_text("Aacute ç U+20AC f_f.liga"));
        ui.horizontal(|ui| {
            if icon_text_button(ui, &pal, I::PLUS, "Add Glyphs", true).clicked() {
                let names: Vec<String> = app.dlg.add_glyphs_text.split_whitespace().map(|s| s.to_string()).collect();
                app.add_glyphs_by_names(ctx, &names);
                app.dlg.add_glyphs_text.clear();
            }
        });
        ui.separator();
        ui.label(egui::RichText::new("Or add a whole glyph set from the catalog").size(11.5).color(pal.muted));
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("catalog").selected_text(glyphnames::GLYPH_SETS[app.dlg.add_glyphs_set]).width(180.0).show_index(ui, &mut app.dlg.add_glyphs_set, glyphnames::GLYPH_SETS.len(), |i| glyphnames::GLYPH_SETS[i]);
            let set = glyphnames::glyph_set(app.dlg.add_glyphs_set);
            let missing = set.iter().filter(|n| app.doc().font.find(n).is_none()).count();
            if ui.button(format!("Add {missing} missing")).clicked() {
                app.add_glyphs_by_names(ctx, &set);
            }
        });
    });
    if !open {
        app.dlg.add_glyphs = false;
    }
}

fn rename(app: &mut App, ctx: &egui::Context) {
    let Some((gi, mut name, mut uni)) = app.dlg.rename.clone() else { return };
    let mut open = true;
    let mut done = false;
    window("Rename Glyph", "rename").open(&mut open).resizable(false).default_width(320.0).show(ctx, |ui| {
        egui::Grid::new("rn").num_columns(2).show(ui, |ui| {
            ui.label("Name");
            ui.add(egui::TextEdit::singleline(&mut name).desired_width(200.0));
            ui.end_row();
            ui.label("Unicode");
            ui.add(egui::TextEdit::singleline(&mut uni).desired_width(200.0));
            ui.end_row();
        });
        ui.horizontal(|ui| {
            if ui.button("Name from Unicode").clicked() {
                if let Ok(u) = u32::from_str_radix(uni.trim().trim_start_matches("U+"), 16) {
                    name = glyphnames::name_for_unicode(u);
                }
            }
            if ui.button("Unicode from name").clicked() {
                if let Some(u) = glyphnames::unicode_for_name(&name) {
                    uni = format!("{u:04X}");
                }
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Rename").clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                done = true;
            }
            if ui.button("Cancel").clicked() {
                app.dlg.rename = None;
            }
        });
    });
    if done {
        let di = app.doc_idx();
        app.docs[di].checkpoint_font("Rename glyph");
        let old = app.docs[di].font.glyphs[gi].name.clone();
        if name != old && !app.docs[di].font.rename_glyph(gi, &name) {
            app.toast(ctx, format!("A glyph named '{name}' already exists"));
        } else {
            for t in &mut app.tabs {
                for n in &mut t.text {
                    if *n == old {
                        *n = name.clone();
                    }
                }
            }
        }
        app.docs[di].font.glyphs[gi].unicodes = uni.split_whitespace().filter_map(|h| u32::from_str_radix(h.trim_start_matches("U+"), 16).ok()).collect();
        app.docs[di].view_key = u64::MAX;
        app.dlg.rename = None;
    } else if app.dlg.rename.is_some() {
        app.dlg.rename = if open { Some((gi, name, uni)) } else { None };
    }
}

fn installed(app: &mut App, ctx: &egui::Context) {
    if !app.dlg.open_installed {
        return;
    }
    let pal = app.pal;
    let mut open = true;
    let mut pick = None;
    window(&format!("{}  Open Installed Font", I::MONITOR), "installed").open(&mut open).default_size([460.0, 480.0]).show(ctx, |ui| {
        ui.add(egui::TextEdit::singleline(&mut app.dlg.installed_filter).hint_text(format!("{}  Filter by file name", I::SEARCH)).desired_width(f32::INFINITY));
        let q = app.dlg.installed_filter.to_lowercase();
        let list: Vec<std::path::PathBuf> = app.dlg.installed.iter().filter(|p| q.is_empty() || p.file_name().map(|n| n.to_string_lossy().to_lowercase().contains(&q)).unwrap_or(false)).cloned().collect();
        ui.label(egui::RichText::new(format!("{} fonts", list.len())).size(11.0).color(pal.muted));
        egui::ScrollArea::vertical().id_salt("installed_list").auto_shrink([false, false]).show_rows(ui, 24.0, list.len(), |ui, range| {
            for p in &list[range] {
                let name = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                if task_row(ui, &pal, I::FILE_TYPE, None, &name, true).on_hover_text(p.display().to_string()).clicked() {
                    pick = Some(p.clone());
                }
            }
        });
    });
    if let Some(p) = pick {
        app.open_installed_font(&p);
        app.dlg.open_installed = false;
    }
    if !open {
        app.dlg.open_installed = false;
    }
}

fn kerning_tools(app: &mut App, ctx: &egui::Context) {
    let di = app.doc_idx();
    let m = app.docs[di].master;
    if app.dlg.kern_adjust {
        let mut open = true;
        window("Adjust Kerning Pairs", "kadj").open(&mut open).resizable(false).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Scale all pairs by");
                ui.add(egui::DragValue::new(&mut app.dlg.kern_scale).suffix("%"));
            });
            ui.horizontal(|ui| {
                ui.label("Then add");
                ui.add(egui::DragValue::new(&mut app.dlg.kern_add));
            });
            if ui.button("Apply").clicked() {
                app.docs[di].checkpoint_font("Adjust kerning");
                let (s, a) = (app.dlg.kern_scale / 100.0, app.dlg.kern_add);
                for k in &mut app.docs[di].font.masters[m].kerning {
                    k.value = (k.value * s + a).round();
                }
                app.dlg.kern_adjust = false;
            }
        });
        if !open {
            app.dlg.kern_adjust = false;
        }
    }
    if app.dlg.kern_remove {
        let mut open = true;
        window("Remove Kerning Pairs", "krem").open(&mut open).resizable(false).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Remove pairs smaller than");
                ui.add(egui::DragValue::new(&mut app.dlg.kern_threshold).range(0.0..=500.0));
                ui.label("units");
            });
            if ui.button("Remove").clicked() {
                app.docs[di].checkpoint_font("Remove small pairs");
                let t = app.dlg.kern_threshold;
                let before = app.docs[di].font.masters[m].kerning.len();
                app.docs[di].font.masters[m].kerning.retain(|k| k.value.abs() >= t);
                let n = before - app.docs[di].font.masters[m].kerning.len();
                app.toast(ctx, format!("Removed {n} pair(s)"));
                app.dlg.kern_remove = false;
            }
        });
        if !open {
            app.dlg.kern_remove = false;
        }
    }
}

fn component_anchor(app: &mut App, ctx: &egui::Context) {
    if app.dlg.add_component {
        let mut open = true;
        let mut go = false;
        window("Add Component", "addcomp").open(&mut open).resizable(false).show(ctx, |ui| {
            ui.label("Glyph to use as a component");
            ui.add(egui::TextEdit::singleline(&mut app.dlg.component_name).hint_text("e.g. acutecomb"));
            let q = app.dlg.component_name.to_lowercase();
            if !q.is_empty() {
                let names: Vec<String> = app.doc().font.glyphs.iter().filter(|g| g.name.to_lowercase().starts_with(&q)).take(8).map(|g| g.name.clone()).collect();
                ui.horizontal_wrapped(|ui| {
                    for n in names {
                        if ui.small_button(&n).clicked() {
                            app.dlg.component_name = n;
                        }
                    }
                });
            }
            if ui.button("Add").clicked() {
                go = true;
            }
        });
        if go {
            let n = app.dlg.component_name.trim().to_string();
            if app.doc().font.find(&n).is_some() {
                app.edit_layer("Add component", |l| l.components.push(Component::new(&n, 0.0, 0.0)));
                app.dlg.add_component = false;
            } else {
                app.toast(ctx, format!("No glyph named '{n}'"));
            }
        }
        if !open {
            app.dlg.add_component = false;
        }
    }
    if app.dlg.add_anchor {
        let mut open = true;
        let mut go = false;
        window("Add Named Anchor", "addanchor").open(&mut open).resizable(false).show(ctx, |ui| {
            ui.add(egui::TextEdit::singleline(&mut app.dlg.anchor_name));
            ui.horizontal(|ui| {
                for n in ["top", "bottom", "_top", "_bottom", "ogonek", "center"] {
                    if ui.small_button(n).clicked() {
                        app.dlg.anchor_name = n.into();
                    }
                }
            });
            if ui.button("Add").clicked() {
                go = true;
            }
        });
        if go {
            let name = app.dlg.anchor_name.clone();
            let di = app.doc_idx();
            let m = app.docs[di].master;
            let (xh, caph) = (app.docs[di].font.masters[m].x_height, app.docs[di].font.masters[m].cap_height);
            app.edit_glyphs("Add anchor", |g, mi| {
                let upper = g.char().map(|c| c.is_uppercase()).unwrap_or(false);
                let l = g.layer_mut(mi);
                let x = (l.width / 2.0).round();
                let y = if name.contains("bottom") || name == "ogonek" { 0.0 } else if upper { caph } else { xh };
                l.anchors.retain(|a| a.name != name);
                l.anchors.push(Anchor { name: name.clone(), x, y, sel: false });
            });
            app.dlg.add_anchor = false;
        }
        if !open {
            app.dlg.add_anchor = false;
        }
    }
}

fn variation(app: &mut App, ctx: &egui::Context) {
    if !app.dlg.add_variation {
        return;
    }
    let mut open = true;
    let mut go = false;
    window("Add Variation (Master)", "addvar").open(&mut open).resizable(false).default_width(340.0).show(ctx, |ui| {
        ui.label("A new master is created as a copy of the current one. Edit its outlines to make the variation; keep the same nodes so masters stay compatible.");
        ui.horizontal(|ui| {
            ui.label("Master name");
            ui.add(egui::TextEdit::singleline(&mut app.dlg.variation_name).desired_width(140.0));
        });
        if ui.button("Add Master").clicked() {
            go = true;
        }
    });
    if go {
        let di = app.doc_idx();
        let m = app.docs[di].master;
        app.docs[di].checkpoint_font("Add master");
        let name = app.dlg.variation_name.clone();
        let f = &mut app.docs[di].font;
        if f.axes.is_empty() {
            f.axes.push(Axis { name: "Weight".into(), tag: "wght".into(), min: 400.0, default: 400.0, max: 700.0 });
            for ms in &mut f.masters {
                ms.location = vec![400.0];
            }
        }
        f.add_master(&name, m);
        let last = f.masters.len() - 1;
        let max = f.axes[0].max;
        f.masters[last].location = vec![max];
        if name.to_lowercase().contains("bold") {
            f.masters[last].weight_class = 700;
        }
        app.docs[di].master = last;
        app.toast(ctx, format!("Added master '{name}'"));
        app.dlg.add_variation = false;
    }
    if !open {
        app.dlg.add_variation = false;
    }
}

fn instance(app: &mut App, ctx: &egui::Context) {
    if !app.dlg.instance {
        return;
    }
    let di = app.doc_idx();
    let axes = app.docs[di].font.axes.clone();
    if app.dlg.instance_loc.len() != axes.len() {
        app.dlg.instance_loc = axes.iter().map(|a| (a.min + a.max) / 2.0).collect();
    }
    let mut open = true;
    let mut go = false;
    window("Generate Instance", "instance").open(&mut open).resizable(false).default_width(360.0).show(ctx, |ui| {
        ui.label("Interpolates a new single-master font between the masters.");
        for (i, a) in axes.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(&a.name);
                ui.add(egui::Slider::new(&mut app.dlg.instance_loc[i], a.min..=a.max));
            });
        }
        ui.horizontal(|ui| {
            ui.label("Style name");
            ui.add(egui::TextEdit::singleline(&mut app.dlg.instance_name).desired_width(140.0));
        });
        if ui.button("Generate").clicked() {
            go = true;
        }
    });
    if go {
        let loc = app.dlg.instance_loc.clone();
        let name = app.dlg.instance_name.clone();
        let f = app.instance_font(di, &loc, &name);
        app.docs.push(Doc::new(f));
        let nd = app.docs.len() - 1;
        app.tabs.push(Tab::font(nd));
        app.active = app.tabs.len() - 1;
        app.toast(ctx, format!("Generated instance '{name}'"));
        app.dlg.instance = false;
    }
    if !open {
        app.dlg.instance = false;
    }
}

fn metrics_link(app: &mut App, ctx: &egui::Context) {
    if !app.dlg.metrics_link {
        return;
    }
    let mut open = true;
    let mut go = false;
    window("Link Glyph Metrics", "mlink").open(&mut open).resizable(false).show(ctx, |ui| {
        ui.label("Copy sidebearings from other glyphs (leave empty to keep).");
        egui::Grid::new("ml").num_columns(2).show(ui, |ui| {
            ui.label("LSB like");
            ui.add(egui::TextEdit::singleline(&mut app.dlg.link_lsb).hint_text("e.g. H").desired_width(120.0));
            ui.end_row();
            ui.label("RSB like");
            ui.add(egui::TextEdit::singleline(&mut app.dlg.link_rsb).hint_text("e.g. H").desired_width(120.0));
            ui.end_row();
        });
        if ui.button("Apply").clicked() {
            go = true;
        }
    });
    if go {
        let di = app.doc_idx();
        let m = app.docs[di].master;
        let f = &app.docs[di].font;
        let lsb = f.find(app.dlg.link_lsb.trim()).map(|g| f.lsb(g, m));
        let rsb = f.find(app.dlg.link_rsb.trim()).map(|g| f.rsb(g, m));
        if lsb.is_none() && rsb.is_none() {
            app.toast(ctx, "Those glyphs are not in the font");
        } else {
            app.set_sidebearings(lsb, rsb, None);
            app.dlg.metrics_link = false;
        }
    }
    if !open {
        app.dlg.metrics_link = false;
    }
}

fn actions(app: &mut App, ctx: &egui::Context) {
    if !app.dlg.actions {
        return;
    }
    let mut open = true;
    let mut go = false;
    window(&format!("{}  Actions", I::LIST_CHECKS), "actions").open(&mut open).resizable(false).default_width(360.0).show(ctx, |ui| {
        ui.label("Run several clean-up steps on the selected glyphs (or the current glyph).");
        ui.checkbox(&mut app.dlg.action_clean, "Remove duplicate nodes");
        ui.checkbox(&mut app.dlg.action_extremes, "Add nodes at extremes");
        ui.checkbox(&mut app.dlg.action_direction, "Set PostScript contour direction");
        ui.checkbox(&mut app.dlg.action_round, "Round coordinates to integers");
        ui.horizontal(|ui| {
            ui.label("Add to LSB");
            ui.add(egui::DragValue::new(&mut app.dlg.action_lsb));
            ui.label("RSB");
            ui.add(egui::DragValue::new(&mut app.dlg.action_rsb));
        });
        if ui.button("Run Actions").clicked() {
            go = true;
        }
    });
    if go {
        let d = &app.dlg;
        let (c, e, dir, r, dl, dr) = (d.action_clean, d.action_extremes, d.action_direction, d.action_round, d.action_lsb, d.action_rsb);
        app.edit_glyphs("Actions", |g, m| {
            let l = g.layer_mut(m);
            if c {
                op_clean(l);
            }
            if e {
                op_extremes(l);
            }
            if dir {
                op_direction(l);
            }
            if r {
                op_round(l);
            }
            if dl != 0.0 {
                for ct in &mut l.contours {
                    for p in &mut ct.points {
                        p.x += dl;
                    }
                }
                for cp in &mut l.components {
                    cp.t[4] += dl;
                }
                l.width += dl;
            }
            l.width += dr;
        });
        app.toast(ctx, "Actions finished");
        app.dlg.actions = false;
    }
    if !open {
        app.dlg.actions = false;
    }
}

const SHORTCUTS: [(&str, &str); 44] = [
    ("New Font", "Ctrl+N"),
    ("Open Fonts", "Ctrl+O"),
    ("Save Font", "Ctrl+S"),
    ("Save Font As", "Ctrl+Shift+S"),
    ("Export Font As", "Ctrl+E"),
    ("Font Info", "Ctrl+Alt+F"),
    ("Undo", "Ctrl+Z"),
    ("Redo", "Ctrl+Shift+Z"),
    ("Cut / Copy / Paste", "Ctrl+X / C / V"),
    ("Duplicate", "Ctrl+D"),
    ("Select All", "Ctrl+A"),
    ("Invert Selection", "Ctrl+Shift+I"),
    ("Find Glyphs", "Ctrl+F"),
    ("Preferences", "Ctrl+,"),
    ("Create Glyph", "Ctrl+Alt+N"),
    ("Rename Glyph", "Ctrl+\\"),
    ("Next / Previous Glyph", "Ctrl+] / Ctrl+["),
    ("Next / Previous in Text", "] / ["),
    ("Break Contour", "Ctrl+B"),
    ("Join Contour", "Ctrl+J"),
    ("Reverse Contour", "Ctrl+Alt+R"),
    ("Nodes at Extremes", "Ctrl+Alt+J"),
    ("Toggle Smooth", "Alt+Enter"),
    ("Delete", "Del"),
    ("Nudge", "Arrows (Shift ×10, Ctrl ×100)"),
    ("True Fill", "Shift+Space"),
    ("Pan", "Space+drag / middle drag"),
    ("Zoom", "Mouse wheel, Ctrl+= / Ctrl+-"),
    ("Fit to Window", "Ctrl+0"),
    ("Close Tab", "Ctrl+W"),
    ("Next Tab", "Ctrl+Tab"),
    ("Kerning Tab", "Ctrl+Alt+Shift+K"),
    ("Actions", "Ctrl+Shift+T"),
    ("Contour tool", "A"),
    ("Element tool", "V"),
    ("Metrics tool", "M"),
    ("Kerning tool", "K"),
    ("Text tool", "T"),
    ("Pen tool", "P"),
    ("Pencil tool", "Y"),
    ("Rectangle / Ellipse", "F / E"),
    ("Knife / Eraser", "J / X"),
    ("Anchor / Measure", "N / U"),
    ("Hand / Zoom tools", "H / Z"),
];

fn shortcuts(app: &mut App, ctx: &egui::Context) {
    if !app.dlg.shortcuts {
        return;
    }
    let pal = app.pal;
    let mut open = true;
    window(&format!("{}  Commands & Shortcuts", I::COMMAND), "shortcuts").open(&mut open).default_size([420.0, 480.0]).show(ctx, |ui| {
        ui.add(egui::TextEdit::singleline(&mut app.dlg.shortcuts_filter).hint_text(format!("{}  Filter", I::SEARCH)).desired_width(f32::INFINITY));
        let q = app.dlg.shortcuts_filter.to_lowercase();
        egui::ScrollArea::vertical().id_salt("sc").auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("scg").num_columns(2).striped(true).spacing([20.0, 4.0]).show(ui, |ui| {
                for (c, k) in SHORTCUTS {
                    if !q.is_empty() && !c.to_lowercase().contains(&q) {
                        continue;
                    }
                    ui.label(c);
                    ui.label(egui::RichText::new(k).color(pal.accent_text));
                    ui.end_row();
                }
            });
        });
    });
    if !open {
        app.dlg.shortcuts = false;
    }
}

fn about(app: &mut App, ctx: &egui::Context) {
    if !app.dlg.about {
        return;
    }
    let pal = app.pal;
    let mut open = true;
    window("About Fontabulous Studio", "about").open(&mut open).resizable(false).default_width(380.0).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
        ui.vertical_centered(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(56.0), Sense::hover());
            crate::ui_shell::paint_mark(ui.painter(), r, pal.accent);
            ui.label(egui::RichText::new("Fontabulous Studio").font(bold(18.0)));
            ui.label(egui::RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).color(pal.muted));
            ui.label(egui::RichText::new("A font editor and companion to the Fontabulous font manager.").size(12.0).color(pal.muted));
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Interface font: Inter (SIL OFL 1.1) · Icons: Lucide (ISC)\nBuilt with Rust, egui/eframe, ttf-parser, lyon, roxmltree (MIT/Apache-2.0)").size(11.0).color(pal.faint));
        });
    });
    if !open {
        app.dlg.about = false;
    }
}

fn prompts(app: &mut App, ctx: &egui::Context) {
    let pal = app.pal;
    if let Some((title, body)) = app.dlg.message.clone() {
        let mut open = true;
        let mut close = false;
        window(&title, "message").open(&mut open).resizable(true).default_width(460.0).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                ui.label(body);
            });
            if ui.button("OK").clicked() {
                close = true;
            }
        });
        if !open || close {
            app.dlg.message = None;
        }
    }
    if let Some(msg) = app.dlg.coming_soon.clone() {
        let mut open = true;
        let mut close = false;
        window("Coming soon", "soon").open(&mut open).resizable(false).default_width(360.0).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(I::SPARKLES).font(icon_font(22.0)).color(pal.accent_text));
                ui.label(msg);
            });
            if ui.button("OK").clicked() {
                close = true;
            }
        });
        if !open || close {
            app.dlg.coming_soon = None;
        }
    }
    if let Some(di) = app.dlg.confirm_close {
        let mut open = true;
        let name = app.docs.get(di).map(|d| d.font.title()).unwrap_or_default();
        window("Close font?", "confirmclose").open(&mut open).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.label(format!("{name} has unsaved changes."));
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    if let Some(ti) = app.tabs.iter().position(|t| t.doc == di) {
                        app.active = ti;
                    }
                    app.save(ctx, false);
                    if !app.docs[di].font.dirty {
                        app.close_doc(di);
                    }
                    app.dlg.confirm_close = None;
                }
                if ui.button("Don't Save").clicked() {
                    app.close_doc(di);
                    app.dlg.confirm_close = None;
                }
                if ui.button("Cancel").clicked() {
                    app.dlg.confirm_close = None;
                }
            });
        });
        if !open {
            app.dlg.confirm_close = None;
        }
    }
    if app.dlg.confirm_quit {
        let mut open = true;
        window("Quit Fontabulous Studio?", "confirmquit").open(&mut open).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            let n = app.docs.iter().filter(|d| d.font.dirty).count();
            ui.label(format!("{n} font(s) have unsaved changes."));
            ui.horizontal(|ui| {
                if ui.button("Save All").clicked() {
                    app.save_all(ctx);
                    if app.docs.iter().all(|d| !d.font.dirty) {
                        app.quit_ok = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    app.dlg.confirm_quit = false;
                }
                if ui.button("Quit Without Saving").clicked() {
                    app.quit_ok = true;
                    app.dlg.confirm_quit = false;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                if ui.button("Cancel").clicked() {
                    app.dlg.confirm_quit = false;
                }
            });
        });
        if !open {
            app.dlg.confirm_quit = false;
        }
    }
}
