//! The command list: every menu action has an id here, and the menus, keyboard and
//! agents (MCP) all run it through [`run`]. Commands that normally ask for a file
//! accept a path instead, so an agent never gets stuck behind a file dialog.

use std::path::{Path, PathBuf};

use crate::app::*;
use crate::font::FLAG_COLORS;
use crate::icons as I;
use crate::theme::ThemeMode;

/// When a command can run.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum When {
    Always,
    /// A glyph is selected (font tab) or being edited (glyph tab).
    Glyph,
    /// A glyph/text tab is active.
    GlyphTab,
    Undo,
    Redo,
    Clip,
    /// The font has two or more masters.
    Masters,
    /// Two or more fonts are open.
    Docs,
    /// The font has been saved to a file.
    Saved,
    /// Two or more tabs are open.
    Tabs,
    AnyGlyphTab,
    /// Not available yet.
    Never,
}

/// What kind of path a command takes. Without one the command opens a file dialog
/// (in the app) or fails with a hint (from an agent).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PathArg {
    None,
    OpenFile,
    OpenFolder,
    SaveFile,
    Folder,
}

impl PathArg {
    pub fn describe(self) -> &'static str {
        match self {
            PathArg::None => "",
            PathArg::OpenFile => "file to open",
            PathArg::OpenFolder => "folder to open",
            PathArg::SaveFile => "file to write",
            PathArg::Folder => "folder to write into",
        }
    }
}

pub struct Cmd {
    pub id: &'static str,
    pub label: &'static str,
    pub icon: &'static str,
    pub keys: &'static str,
    pub when: When,
    pub path: PathArg,
}

const fn c(id: &'static str, label: &'static str, icon: &'static str, keys: &'static str, when: When) -> Cmd {
    Cmd { id, label, icon, keys, when, path: PathArg::None }
}
const fn cp(id: &'static str, label: &'static str, icon: &'static str, keys: &'static str, path: PathArg) -> Cmd {
    Cmd { id, label, icon, keys, when: When::Always, path }
}

use When::*;

pub const COMMANDS: &[Cmd] = &[
    // File
    c("file.welcome", "Welcome…", I::APP_WINDOW, "", Always),
    c("file.new", "New Font", I::FILE_PLUS, "Ctrl+N", Always),
    cp("file.open", "Open Fonts…", I::FOLDER_OPEN, "Ctrl+O", PathArg::OpenFile),
    cp("file.open_ufo", "Open UFO Font Folder…", I::FOLDER, "", PathArg::OpenFolder),
    c("file.open_installed", "Open Installed Font…", I::MONITOR, "", Always),
    c("file.revert", "Revert Font", I::ROTATE_CCW, "Ctrl+F12", Saved),
    c("file.save", "Save Font…", I::SAVE, "Ctrl+S", Always),
    cp("file.save_as", "Save Font As…", "", "Ctrl+Shift+S", PathArg::SaveFile),
    c("file.save_all", "Save All", "", "", Always),
    c("file.export_profiles", "Export Profiles…", I::SLIDERS_HORIZONTAL, "", Always),
    cp("file.export_layer_ttf", "Export Layer As TrueType", I::EXTERNAL_LINK, "", PathArg::Folder),
    c("file.export_as", "Export Font As…", I::FILE_OUTPUT, "Ctrl+E", Always),
    cp("file.export_ttf", "Export TrueType (all masters)", I::FILE_TYPE, "", PathArg::Folder),
    cp("file.export_ufo", "Export UFO", I::FOLDER, "", PathArg::SaveFile),
    cp("file.export_native", "Export Fontabulous Studio File", I::SAVE, "", PathArg::SaveFile),
    cp("file.import_kerning", "Metrics / Kerning…", "", "", PathArg::OpenFile),
    cp("file.import_features", "Features…", "", "", PathArg::OpenFile),
    cp("file.import_classes", "Classes…", "", "", PathArg::OpenFile),
    c("file.import_artwork", "Artwork…", "", "", Always),
    cp("file.export_kerning", "Kerning…", "", "", PathArg::SaveFile),
    cp("file.export_kern_feature", "Kerning as Feature Code…", "", "", PathArg::SaveFile),
    cp("file.export_features", "Features…", "", "", PathArg::SaveFile),
    cp("file.export_classes", "Classes…", "", "", PathArg::SaveFile),
    c("file.font_info", "Font Info…", I::INFO, "Ctrl+Alt+F", Always),
    c("file.print", "Print Font…", I::PRINTER, "Ctrl+P", Always),
    c("file.exit", "Exit", "", "", Always),
    // Edit
    c("edit.undo", "Undo", I::UNDO_2, "Ctrl+Z", Undo),
    c("edit.redo", "Redo", I::REDO_2, "Ctrl+Shift+Z", Redo),
    c("edit.cut", "Cut", I::SCISSORS, "Ctrl+X", Glyph),
    c("edit.copy", "Copy", I::COPY, "Ctrl+C", Glyph),
    c("edit.delete", "Delete", I::TRASH_2, "Del", Glyph),
    c("edit.paste", "Paste", I::CLIPBOARD_PASTE, "Ctrl+V", Clip),
    c("edit.duplicate", "Duplicate", I::COPY_PLUS, "Ctrl+D", Glyph),
    c("edit.copy_metrics_to_masters", "Copy Metrics to Masters", I::LAYERS_2, "Ctrl+Y", Masters),
    c("edit.select_all", "Select All", I::LIST_CHECKS, "Ctrl+A", Always),
    c("edit.invert_selection", "Invert Selection", "", "Ctrl+Shift+I", Always),
    c("edit.find", "Find Glyphs…", I::SEARCH, "Ctrl+F", Always),
    c("edit.more_info", "More Info", I::INFO, "Ctrl+Alt+I", Glyph),
    c("edit.preferences", "Preferences…", I::SETTINGS, "Ctrl+,", Always),
    // Text
    c("text.previous", "Previous in Text", I::CHEVRON_LEFT, "[", GlyphTab),
    c("text.next", "Next in Text", I::CHEVRON_RIGHT, "]", GlyphTab),
    c("text.uppercase", "UPPERCASE", "", "", GlyphTab),
    c("text.lowercase", "lowercase", "", "", GlyphTab),
    c("text.copy_names", "Glyph names (/A/B)", "", "", Always),
    c("text.copy_chars", "Unicode characters", "", "", Always),
    c("text.right_to_left", "Right to Left", I::ARROW_LEFT_RIGHT, "", Never),
    // Font
    c("font.create_glyph", "Create Glyph", I::FILE_PLUS, "Ctrl+Alt+N", Always),
    c("font.build_glyphs", "Build Glyphs", I::COMPONENT, "Ctrl+Alt+G", Glyph),
    c("font.copy_template", "Copy Template Outlines", I::COPY_PLUS, "", Glyph),
    c("font.add_from_catalog", "Add Glyphs from Catalog…", I::LIBRARY_BIG, "", Always),
    c("font.detect_composites", "Detect Composites", I::SCAN_SEARCH, "", Always),
    c("font.add_variation", "Add Variation…", I::VARIABLE, "", Always),
    c("font.merge_masters", "Merge Fonts to Masters", I::MERGE, "", Docs),
    c("font.generate_instance", "Generate Instance…", I::SPARKLES, "", Masters),
    c("font.compare", "Compare Fonts…", I::GIT_COMPARE, "", Docs),
    c("font.overlay", "Overlay Fonts…", I::LAYERS_2, "", Always),
    c("font.kerning_tab", "Open Kerning Tab", I::ARROW_LEFT_RIGHT, "Ctrl+Alt+Shift+K", Always),
    c("font.kern_adjust", "Adjust Pairs…", "", "", Always),
    c("font.kern_remove", "Remove Pairs…", "", "", Always),
    c("font.kern_remove_all", "Remove All Pairs", "", "", Always),
    c("font.metrics_tab", "Open Metrics Tab", I::MOVE_HORIZONTAL, "", Always),
    c("font.link_metrics", "Link Glyph Metrics…", "", "", Glyph),
    c("font.center_glyphs", "Center Glyphs", "", "", Glyph),
    c("font.equalize_sidebearings", "Equalize Sidebearings", "", "", Glyph),
    c("font.sort_name", "By Name", "", "", Always),
    c("font.sort_unicode", "By Unicode", "", "", Always),
    c("font.sort_category", "By Category", "", "", Always),
    c("font.names_from_unicode", "Names from Unicode", "", "", Always),
    c("font.generate_unicodes", "Generate Unicodes", I::BINARY, "", Always),
    c("font.previous", "Previous Font", I::ARROW_UP, "", Docs),
    c("font.next", "Next Font", I::ARROW_DOWN, "", Docs),
    c("font.close", "Close Font", I::X, "", Always),
    // Glyph
    c("glyph.rename", "Rename Glyph…", I::PENCIL, "Ctrl+\\", Glyph),
    c("glyph.duplicate", "Duplicate Glyph…", I::COPY_PLUS, "Ctrl+Shift+D", Glyph),
    c("glyph.next", "Next Glyph", I::ARROW_RIGHT, "Ctrl+]", Always),
    c("glyph.previous", "Previous Glyph", I::ARROW_LEFT, "Ctrl+[", Always),
    c("glyph.flag_none", "No flag", I::X, "", Glyph),
    c("glyph.flag_red", "Red", I::FLAG, "", Glyph),
    c("glyph.flag_orange", "Orange", I::FLAG, "", Glyph),
    c("glyph.flag_yellow", "Yellow", I::FLAG, "", Glyph),
    c("glyph.flag_green", "Green", I::FLAG, "", Glyph),
    c("glyph.flag_teal", "Teal", I::FLAG, "", Glyph),
    c("glyph.flag_blue", "Blue", I::FLAG, "", Glyph),
    c("glyph.flag_purple", "Purple", I::FLAG, "", Glyph),
    c("glyph.flag_grey", "Grey", I::FLAG, "", Glyph),
    c("glyph.add_component", "Add Component…", I::COMPONENT, "Alt+Ins", Glyph),
    c("glyph.decompose", "Decompose", I::BOXES, "", Glyph),
    c("glyph.add_anchor", "Add Named Anchor…", I::ANCHOR, "", Glyph),
    c("glyph.remove_anchors", "Remove Anchors", "", "", Glyph),
    c("glyph.match_masters", "Match Masters", I::LAYERS_3, "Ctrl+Alt+Shift+M", Masters),
    c("glyph.ps_direction", "Set PS Direction", I::REFRESH_CW, "", Glyph),
    c("glyph.add_sticker", "Add Sticker", I::STICKER, "", Never),
    // Element
    c("element.new_rectangle", "Rectangle", I::SQUARE, "", GlyphTab),
    c("element.new_ellipse", "Ellipse", I::CIRCLE, "", GlyphTab),
    c("element.locked_content", "Locked Content", I::LOCK, "", Never),
    c("element.reverse", "Reverse", "", "", Glyph),
    c("element.to_cubic", "Convert to Cubic", "", "", Glyph),
    c("element.brush", "Brush & Stroke", I::PAINTBRUSH, "", Never),
    c("element.image", "Image", I::IMAGE, "", Never),
    // Contour
    c("contour.toggle_smooth", "Toggle Smooth", "", "Alt+Enter", GlyphTab),
    c("contour.delete_nodes", "Delete Nodes", "", "Del", GlyphTab),
    c("contour.line_to_curve", "Line to Curve", "", "", GlyphTab),
    c("contour.curve_to_line", "Curve to Line", "", "", GlyphTab),
    c("contour.round", "Round to Integer", "", "", Glyph),
    c("contour.set_start", "Set Start Point", "", "", GlyphTab),
    c("contour.reverse", "Reverse Contour", I::REFRESH_CW, "Ctrl+Alt+R", Glyph),
    c("contour.break", "Break", I::SCISSORS, "Ctrl+B", GlyphTab),
    c("contour.join", "Join", I::COMBINE, "Ctrl+J", GlyphTab),
    c("contour.align_h", "Align Points Horizontally", "", "", GlyphTab),
    c("contour.align_v", "Align Points Vertically", "", "", GlyphTab),
    c("contour.clean_up", "Clean Up", "", "Ctrl+Alt+Shift+C", Glyph),
    c("contour.extremes", "Nodes at Extremes", "", "Ctrl+Alt+J", Glyph),
    c("contour.simplify", "Simplify", "", "Ctrl+Alt+B", Never),
    c("contour.remove_overlap", "Remove Overlap", I::BLEND, "Ctrl+F10", Glyph),
    // Tools
    c("tools.flip_h", "Flip Horizontally", "", "", Glyph),
    c("tools.flip_v", "Flip Vertically", "", "", Glyph),
    c("tools.rotate_left", "Rotate 90° Left", "", "", Glyph),
    c("tools.rotate_right", "Rotate 90° Right", "", "", Glyph),
    c("tools.guide_center", "Add Vertical Guide at Center", "", "", Glyph),
    c("tools.remove_guides", "Remove Glyph Guides", "", "", Glyph),
    c("tools.actions", "Actions…", I::LIST_CHECKS, "Ctrl+Shift+T", Always),
    c("tools.font_audit", "FontAudit", I::SHIELD_CHECK, "", Always),
    c("tools.shortcuts", "Commands & Shortcuts", I::COMMAND, "Ctrl+Shift+P", Always),
    // Script
    c("script.scripting", "Scripting Panel", I::BRACES, "", Always),
    c("script.output", "Output Panel", I::TERMINAL, "", Always),
    // View
    c("view.zoom_in", "Zoom In", "", "Ctrl+=", GlyphTab),
    c("view.zoom_out", "Zoom Out", "", "Ctrl+-", GlyphTab),
    c("view.fit", "Fit to Window", "", "Ctrl+0", GlyphTab),
    c("view.larger_cells", "Larger Cells", "", "Ctrl+=", Always),
    c("view.smaller_cells", "Smaller Cells", "", "Ctrl+-", Always),
    c("view.toggle_metrics", "Metrics & Font Dimensions", "", "", Always),
    c("view.toggle_zones", "Zones", "", "", Always),
    c("view.toggle_guides", "Guides", "", "", Always),
    c("view.toggle_grid", "Grid", "", "", Always),
    c("view.toggle_nodes", "Nodes & Handles", "", "", Always),
    c("view.toggle_coords", "Node Coordinates", "", "", Always),
    c("view.toggle_anchors", "Anchors", "", "", Always),
    c("view.toggle_kerning", "Kerning", "", "", Always),
    c("view.toggle_fill", "True Fill", "", "Shift+Space", Always),
    c("view.toggle_template", "Template Glyphs", "", "", Always),
    c("view.toggle_snap", "Enable Snapping", "", "", Always),
    c("view.toggle_theme", "Switch Light / Dark Theme", I::SUN, "", Always),
    // Window
    c("window.new_font_tab", "New Font Tab", I::LAYOUT_GRID, "", Always),
    c("window.new_glyph_tab", "New Glyph Tab", I::LETTER_TEXT, "", Glyph),
    c("window.toggle_toolbar", "Toolbar", "", "", Always),
    c("window.toggle_panels", "Panels", "", "", Always),
    c("window.toggle_sidebar", "Sidebar", "", "", Always),
    c("window.next_tab", "Next Tab", "", "Ctrl+Tab", Tabs),
    c("window.previous_tab", "Previous Tab", "", "Ctrl+Shift+Tab", Tabs),
    c("window.close_tab", "Close Tab", "", "Ctrl+W", Always),
    c("window.close_glyph_tabs", "Close All Glyph Tabs", "", "", AnyGlyphTab),
    // Help
    c("help.getting_started", "Getting Started", I::BOOK_OPEN, "", Always),
    c("help.about", "About Fontabulous Studio", I::INFO, "", Always),
];

pub fn find(id: &str) -> Option<&'static Cmd> {
    COMMANDS.iter().find(|c| c.id == id)
}

pub fn enabled(app: &App, when: When) -> bool {
    match when {
        Always => true,
        Glyph => app.current_glyph().is_some(),
        GlyphTab => app.in_glyph_tab(),
        Undo => !app.doc().undo.is_empty(),
        Redo => !app.doc().redo.is_empty(),
        Clip => app.clip.is_some(),
        Masters => app.doc().font.masters.len() > 1,
        Docs => app.docs.len() > 1,
        Saved => app.doc().font.path.is_some(),
        Tabs => app.tabs.len() > 1,
        AnyGlyphTab => app.tabs.iter().any(|t| t.kind == TabKind::Glyph),
        Never => false,
    }
}

/// The label shown in menus; a few change with the context.
pub fn label(app: &App, cmd: &Cmd) -> String {
    match cmd.id {
        "edit.cut" | "edit.copy" => format!("{} {}", cmd.label, if app.in_glyph_tab() { "Elements" } else { "Glyphs" }),
        _ => cmd.label.to_string(),
    }
}

fn soon(app: &mut App, what: &str) {
    app.dlg.coming_soon = Some(format!("{what} is coming in a later version of Fontabulous Studio."));
}

fn show_panel(app: &mut App, p: PanelId, front: bool) {
    if !app.prefs.right_panels.contains(&p) {
        if front {
            app.prefs.right_panels.insert(0, p);
        } else {
            app.prefs.right_panels.push(p);
        }
    }
    app.prefs.show_right = true;
    app.prefs_dirty = true;
}

fn toggle(app: &mut App, f: impl FnOnce(&mut Prefs) -> &mut bool) -> String {
    let v = f(&mut app.prefs);
    *v = !*v;
    let now = *v;
    app.prefs_dirty = true;
    if now { "on".into() } else { "off".into() }
}

/// Run a command from the menus, keyboard or an agent.
///
/// `path` is used by commands that read or write files. From the app it is `None`
/// and those commands show a file dialog; agents (`interactive == false`) must pass
/// one, because nobody may be there to answer a dialog.
pub fn run(app: &mut App, ctx: &egui::Context, id: &str, path: Option<PathBuf>, interactive: bool) -> Result<String, String> {
    let cmd = find(id).ok_or_else(|| format!("Unknown command '{id}'. Use list_commands to see them."))?;
    if !enabled(app, cmd.when) {
        return Err(match cmd.when {
            Never => format!("'{}' is coming in a later version.", cmd.label),
            Glyph => format!("'{}' needs a selected glyph (select_glyphs or open_glyph first).", cmd.label),
            GlyphTab | AnyGlyphTab => format!("'{}' needs a glyph tab (open_glyph or open_text first).", cmd.label),
            _ => format!("'{}' is not available right now.", cmd.label),
        });
    }
    if cmd.path != PathArg::None && path.is_none() && !interactive {
        return Err(format!("'{}' needs a \"path\" argument ({}).", cmd.label, cmd.path.describe()));
    }
    let p = path.as_deref();
    let done = |s: &str| Ok(s.to_string());
    match id {
        // ------------------------------------------------------------------ File
        "file.welcome" => {
            app.dlg.welcome = true;
            done("Opened the Welcome dialog")
        }
        "file.new" => {
            let set = app.prefs.default_glyph_set;
            app.new_font("Unnamed", set);
            Ok(format!("Created a new font with {} glyphs", app.doc().font.glyphs.len()))
        }
        "file.open" | "file.open_ufo" => match p {
            Some(p) => open_checked(app, p),
            None => {
                if id == "file.open" {
                    app.open_dialog();
                } else {
                    app.open_ufo_dialog();
                }
                done("")
            }
        },
        "file.open_installed" => {
            app.dlg.installed = crate::io::installed_fonts();
            app.dlg.open_installed = true;
            Ok(format!("Opened the installed-font list ({} fonts). Agents can open one with file.open and its path.", app.dlg.installed.len()))
        }
        "file.revert" => {
            let p = app.doc().font.path.clone().ok_or("The font has not been saved")?;
            let di = app.doc_idx();
            let f = crate::io::open_any(&p)?;
            app.docs[di] = Doc::new(f);
            app.cache.clear_doc(di);
            Ok(format!("Reverted to {}", p.display()))
        }
        "file.save" => {
            if let Some(path) = app.doc().font.path.clone() {
                app.save_to(ctx, &path)
            } else if interactive {
                app.save(ctx, true);
                done("")
            } else {
                Err("The font has not been saved yet; use file.save_as with a path ending in .fstudio".into())
            }
        }
        "file.save_as" => match p {
            Some(p) => app.save_to(ctx, p),
            None => {
                app.save(ctx, true);
                done("")
            }
        },
        "file.save_all" => {
            if !interactive && app.docs.iter().any(|d| d.font.path.is_none()) {
                return Err("Some fonts were never saved; save each with file.save_as first".into());
            }
            app.save_all(ctx);
            done("Saved all fonts")
        }
        "file.export_profiles" | "file.export_as" => {
            app.dlg.export = true;
            done("Opened the Export dialog (agents can use file.export_ttf, file.export_ufo or file.export_native with a path)")
        }
        "file.export_layer_ttf" | "file.export_ttf" => {
            app.dlg.export_masters = id == "file.export_ttf";
            match p {
                Some(p) => app.export_to(ctx, 0, p),
                None => {
                    app.export(ctx, 0);
                    done("")
                }
            }
        }
        "file.export_ufo" | "file.export_native" => {
            let f = if id == "file.export_ufo" { 1 } else { 2 };
            match p {
                Some(p) => app.export_to(ctx, f, p),
                None => {
                    app.export(ctx, f);
                    done("")
                }
            }
        }
        "file.import_kerning" | "file.import_features" | "file.import_classes" => {
            let what = &id["file.import_".len()..];
            match p {
                Some(p) => app.import_text_from(ctx, what, p),
                None => {
                    app.import_text(ctx, what);
                    done("")
                }
            }
        }
        "file.export_kerning" | "file.export_kern_feature" | "file.export_features" | "file.export_classes" => {
            let what = match id {
                "file.export_kerning" => "kerning",
                "file.export_kern_feature" => "kernfea",
                "file.export_features" => "features",
                _ => "classes",
            };
            match p {
                Some(p) => app.export_text_to(ctx, what, p),
                None => {
                    app.export_text(ctx, what);
                    done("")
                }
            }
        }
        "file.import_artwork" => {
            soon(app, "Artwork import");
            Err("Artwork import is coming in a later version.".into())
        }
        "file.font_info" => {
            app.dlg.font_info = true;
            done("Opened Font Info (agents can use get_font_info / set_font_info)")
        }
        "file.print" => {
            soon(app, "Printing glyph proofs");
            Err("Printing is coming in a later version.".into())
        }
        "file.exit" => {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            done("Closing the app")
        }
        // ------------------------------------------------------------------ Edit
        "edit.undo" => {
            app.undo(ctx);
            done("Undone")
        }
        "edit.redo" => {
            app.redo(ctx);
            done("Redone")
        }
        "edit.cut" => {
            app.cut(ctx);
            done("Cut")
        }
        "edit.copy" => {
            app.copy(ctx);
            done("Copied")
        }
        "edit.delete" => {
            app.delete(ctx);
            done("Deleted")
        }
        "edit.paste" => {
            app.paste(ctx);
            done("Pasted")
        }
        "edit.duplicate" | "glyph.duplicate" => {
            app.duplicate_glyphs(ctx);
            done("Duplicated")
        }
        "edit.copy_metrics_to_masters" => {
            let m = app.doc().master;
            app.edit_glyphs("Copy metrics to masters", |g, _| {
                let w = g.layer(m).width;
                for l in &mut g.layers {
                    l.width = w;
                }
            });
            done("Copied widths to every master")
        }
        "edit.select_all" => {
            app.select_all();
            done("Selected all")
        }
        "edit.invert_selection" => {
            app.invert_selection();
            done("Inverted the selection")
        }
        "edit.find" => {
            app.dlg.find = true;
            done("Opened Find Glyphs")
        }
        "edit.more_info" => {
            show_panel(app, PanelId::Glyph, true);
            done("Showing the Glyph panel")
        }
        "edit.preferences" => {
            app.dlg.prefs = true;
            done("Opened Preferences (agents can use set_preferences)")
        }
        // ------------------------------------------------------------------ Text
        "text.previous" => {
            let t = app.tab_mut();
            t.cur = t.cur.saturating_sub(1);
            done("Moved to the previous glyph in the text")
        }
        "text.next" => {
            let t = app.tab_mut();
            if t.cur + 1 < t.text.len() {
                t.cur += 1;
            }
            done("Moved to the next glyph in the text")
        }
        "text.uppercase" | "text.lowercase" => {
            let di = app.doc_idx();
            let t = app.tab().text_input.clone();
            let nt = if id == "text.uppercase" { t.to_uppercase() } else { t.to_lowercase() };
            let names = text_to_names(&app.docs[di].font, &nt);
            let tab = app.tab_mut();
            tab.text_input = nt.clone();
            tab.text = names;
            tab.cur = tab.cur.min(tab.text.len().saturating_sub(1));
            Ok(format!("Text is now \"{nt}\""))
        }
        "text.copy_names" => {
            let names: Vec<String> = if app.in_glyph_tab() { app.tab().text.clone() } else { app.doc().selected().iter().map(|&i| app.doc().font.glyphs[i].name.clone()).collect() };
            let s: String = names.iter().map(|n| format!("/{n}")).collect();
            ctx.copy_text(s.clone());
            Ok(s)
        }
        "text.copy_chars" => {
            let d = app.doc();
            let ids: Vec<usize> = if app.in_glyph_tab() { app.tab().text.iter().filter_map(|n| d.font.find(n)).collect() } else { d.selected() };
            let s: String = ids.iter().filter_map(|&i| d.font.glyphs[i].char()).collect();
            ctx.copy_text(s.clone());
            Ok(s)
        }
        // ------------------------------------------------------------------ Font
        "font.create_glyph" | "font.add_from_catalog" => {
            app.dlg.add_glyphs = true;
            done("Opened Add Glyphs (agents can use add_glyphs)")
        }
        "font.build_glyphs" => {
            app.build_glyphs(ctx);
            Ok(app.status.as_ref().map(|s| s.0.clone()).unwrap_or_else(|| "Built glyphs".into()))
        }
        "font.copy_template" => {
            app.copy_template(ctx);
            Ok(app.status.as_ref().map(|s| s.0.clone()).unwrap_or_default())
        }
        "font.detect_composites" => {
            let n = app.doc().font.glyphs.iter().filter(|g| crate::glyphnames::composite_recipe(&g.name).is_some() && g.layers[0].is_empty()).count();
            let msg = format!("{n} empty glyph(s) can be built from components (select them and use Build Glyphs)");
            app.toast(ctx, msg.clone());
            Ok(msg)
        }
        "font.add_variation" => {
            app.dlg.add_variation = true;
            done("Opened Add Variation")
        }
        "font.merge_masters" => {
            soon(app, "Merging open fonts into masters");
            Err("Merging fonts into masters is coming in a later version.".into())
        }
        "font.generate_instance" => {
            app.dlg.instance = true;
            done("Opened Generate Instance")
        }
        "font.compare" => {
            soon(app, "Comparing fonts");
            Err("Comparing fonts is coming in a later version.".into())
        }
        "font.overlay" => {
            soon(app, "Overlaying fonts");
            Err("Overlaying fonts is coming in a later version.".into())
        }
        "font.kerning_tab" => {
            app.open_text_tab("AVATAR Toyota WAVE", Tool::Kerning);
            done("Opened a kerning tab")
        }
        "font.kern_adjust" => {
            app.dlg.kern_adjust = true;
            done("Opened Adjust Pairs")
        }
        "font.kern_remove" => {
            app.dlg.kern_remove = true;
            done("Opened Remove Pairs")
        }
        "font.kern_remove_all" => {
            let di = app.doc_idx();
            let m = app.docs[di].master;
            app.docs[di].checkpoint_font("Remove all kerning");
            let n = app.docs[di].font.masters[m].kerning.len();
            app.docs[di].font.masters[m].kerning.clear();
            app.docs[di].font.dirty = true;
            Ok(format!("Removed {n} kerning pairs"))
        }
        "font.metrics_tab" => {
            app.open_text_tab("nnonnoonnn HHOHHOOHHH", Tool::Metrics);
            done("Opened a metrics tab")
        }
        "font.link_metrics" => {
            app.dlg.metrics_link = true;
            done("Opened Link Glyph Metrics (agents can use set_metrics)")
        }
        "font.center_glyphs" | "font.equalize_sidebearings" => {
            center_selected(app);
            done("Equalized sidebearings")
        }
        "font.sort_name" => {
            app.sort_glyphs(SortBy::Name);
            done("Sorted by name")
        }
        "font.sort_unicode" => {
            app.sort_glyphs(SortBy::Unicode);
            done("Sorted by Unicode")
        }
        "font.sort_category" => {
            app.sort_glyphs(SortBy::Category);
            done("Sorted by category")
        }
        "font.names_from_unicode" => {
            let di = app.doc_idx();
            app.docs[di].checkpoint_font("Rename from Unicode");
            let n = app.docs[di].font.glyphs.len();
            let mut k = 0;
            for gi in 0..n {
                if let Some(&u) = app.docs[di].font.glyphs[gi].unicodes.first() {
                    let nm = crate::glyphnames::name_for_unicode(u);
                    if nm != app.docs[di].font.glyphs[gi].name && app.docs[di].font.rename_glyph(gi, &nm) {
                        k += 1;
                    }
                }
            }
            app.docs[di].view_key = u64::MAX;
            Ok(format!("Renamed {k} glyph(s)"))
        }
        "font.generate_unicodes" => {
            app.generate_unicodes(ctx);
            Ok(app.status.as_ref().map(|s| s.0.clone()).unwrap_or_default())
        }
        "font.previous" => {
            cycle_font(app, -1);
            Ok(format!("Now on {}", app.doc().title()))
        }
        "font.next" => {
            cycle_font(app, 1);
            Ok(format!("Now on {}", app.doc().title()))
        }
        "font.close" => {
            let di = app.doc_idx();
            if app.docs[di].font.dirty && interactive {
                app.dlg.confirm_close = Some(di);
                done("")
            } else if app.docs[di].font.dirty {
                Err("The font has unsaved changes; save it first or run font.close again after file.save".into())
            } else {
                app.close_doc(di);
                done("Closed the font")
            }
        }
        // ------------------------------------------------------------------ Glyph
        "glyph.rename" => {
            let gi = app.current_glyph().ok_or("No glyph")?;
            let n = app.doc().font.glyphs[gi].name.clone();
            let u = app.doc().font.glyphs[gi].unicode_str();
            app.dlg.rename = Some((gi, n, u));
            done("Opened Rename Glyph (agents can use set_glyph with new_name)")
        }
        "glyph.next" => {
            app.step_glyph(1);
            Ok(current_name(app))
        }
        "glyph.previous" => {
            app.step_glyph(-1);
            Ok(current_name(app))
        }
        "glyph.flag_none" => {
            app.set_flag(None);
            done("Cleared the flag")
        }
        f if f.starts_with("glyph.flag_") => {
            let name = &f["glyph.flag_".len()..];
            let i = FLAG_COLORS.iter().position(|c| c.3.eq_ignore_ascii_case(name)).ok_or("Unknown flag")?;
            app.set_flag(Some(i));
            Ok(format!("Flagged {}", FLAG_COLORS[i].3))
        }
        "glyph.add_component" => {
            app.dlg.add_component = true;
            done("Opened Add Component (agents can use set_glyph with components)")
        }
        "glyph.decompose" => {
            app.decompose();
            done("Decomposed components")
        }
        "glyph.add_anchor" => {
            app.dlg.add_anchor = true;
            done("Opened Add Named Anchor (agents can use set_glyph with anchors)")
        }
        "glyph.remove_anchors" => {
            app.edit_glyphs("Remove anchors", |g, m| g.layer_mut(m).anchors.clear());
            done("Removed anchors")
        }
        "glyph.match_masters" => Ok(report_compat(app, ctx)),
        "glyph.ps_direction" => {
            app.contour_op("Set PS direction", op_direction);
            done("Set PostScript contour direction")
        }
        // ------------------------------------------------------------------ Element
        "element.new_rectangle" => {
            let upm = app.doc().font.info.upm;
            app.edit_layer("New rectangle", |l| l.contours.push(crate::geom::rect_contour(upm * 0.1, 0.0, upm * 0.4, upm * 0.5, false)));
            done("Added a rectangle")
        }
        "element.new_ellipse" => {
            let upm = app.doc().font.info.upm;
            app.edit_layer("New ellipse", |l| l.contours.push(crate::geom::ellipse_contour(upm * 0.3, upm * 0.25, upm * 0.2, upm * 0.25)));
            done("Added an ellipse")
        }
        "element.reverse" | "contour.reverse" => {
            app.contour_op("Reverse contours", op_reverse);
            done("Reversed contours")
        }
        "element.to_cubic" => {
            app.contour_op("Convert to cubic", op_to_cubic);
            done("Converted to cubic curves")
        }
        // ------------------------------------------------------------------ Contour
        "contour.toggle_smooth" => {
            app.edit_layer("Toggle smooth", op_toggle_smooth);
            done("Toggled smooth on the selected nodes")
        }
        "contour.delete_nodes" => {
            app.delete(ctx);
            done("Deleted the selected nodes")
        }
        "contour.line_to_curve" => {
            app.edit_layer("Line to curve", op_to_curve);
            done("Converted lines to curves")
        }
        "contour.curve_to_line" => {
            app.edit_layer("Curve to line", op_to_line);
            done("Converted curves to lines")
        }
        "contour.round" => {
            app.contour_op("Round coordinates", op_round);
            done("Rounded coordinates")
        }
        "contour.set_start" => {
            app.edit_layer("Set start point", op_set_start);
            done("Set the start point")
        }
        "contour.break" => {
            app.edit_layer("Break contour", op_break);
            done("Broke the contour at the selected node")
        }
        "contour.join" => {
            app.edit_layer("Join contour", op_join);
            done("Joined contours")
        }
        "contour.align_h" => {
            app.edit_layer("Align points", op_align_h);
            done("Aligned points horizontally")
        }
        "contour.align_v" => {
            app.edit_layer("Align points", op_align_v);
            done("Aligned points vertically")
        }
        "contour.clean_up" => {
            app.contour_op("Clean up", op_clean);
            done("Cleaned up contours")
        }
        "contour.extremes" => {
            app.contour_op("Nodes at extremes", op_extremes);
            done("Added nodes at extremes")
        }
        "contour.remove_overlap" => {
            soon(app, "Remove Overlap");
            Err("Remove Overlap is coming in a later version.".into())
        }
        // ------------------------------------------------------------------ Tools
        "tools.flip_h" | "tools.flip_v" | "tools.rotate_left" | "tools.rotate_right" => {
            let m = match id {
                "tools.flip_h" => [-1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                "tools.flip_v" => [1.0, 0.0, 0.0, -1.0, 0.0, 0.0],
                "tools.rotate_left" => [0.0, 1.0, -1.0, 0.0, 0.0, 0.0],
                _ => [0.0, -1.0, 1.0, 0.0, 0.0, 0.0],
            };
            crate::ui_panels::apply_transform_about_center(app, m, cmd.label);
            Ok(cmd.label.to_string())
        }
        "tools.guide_center" => {
            app.edit_layer("Add guide", |l| {
                let x = (l.width / 2.0).round();
                l.guides.push(crate::font::Guide { name: String::new(), x, y: 0.0, angle: 90.0, color: None });
            });
            done("Added a vertical guide")
        }
        "tools.remove_guides" => {
            app.edit_layer("Remove guides", |l| l.guides.clear());
            done("Removed glyph guides")
        }
        "tools.actions" => {
            app.dlg.actions = true;
            done("Opened Actions")
        }
        "tools.font_audit" => {
            show_panel(app, PanelId::FontAudit, true);
            let d = app.doc();
            let issues = crate::ui_panels::audit(&d.font, d.master);
            Ok(if issues.is_empty() { "FontAudit found no problems".into() } else { issues.iter().take(100).map(|(g, w)| format!("{g}: {w}")).collect::<Vec<_>>().join("\n") })
        }
        "tools.shortcuts" => {
            app.dlg.shortcuts = true;
            done("Opened Commands & Shortcuts")
        }
        // ------------------------------------------------------------------ Script
        "script.scripting" => {
            soon(app, "Python scripting");
            Err("Python scripting is coming in a later version.".into())
        }
        "script.output" => {
            show_panel(app, PanelId::Output, false);
            Ok(app.output.iter().rev().take(30).rev().cloned().collect::<Vec<_>>().join("\n"))
        }
        // ------------------------------------------------------------------ View
        "view.zoom_in" => {
            app.tab_mut().zoom *= 1.25;
            done("Zoomed in")
        }
        "view.zoom_out" => {
            app.tab_mut().zoom /= 1.25;
            done("Zoomed out")
        }
        "view.fit" => {
            app.tab_mut().fit = true;
            done("Fit to window")
        }
        "view.larger_cells" => {
            app.prefs.cell_size = (app.prefs.cell_size * 1.15).min(320.0);
            app.prefs_dirty = true;
            done("Larger cells")
        }
        "view.smaller_cells" => {
            app.prefs.cell_size = (app.prefs.cell_size / 1.15).max(40.0);
            app.prefs_dirty = true;
            done("Smaller cells")
        }
        "view.toggle_metrics" => Ok(toggle(app, |p| &mut p.show_metrics)),
        "view.toggle_zones" => Ok(toggle(app, |p| &mut p.show_zones)),
        "view.toggle_guides" => Ok(toggle(app, |p| &mut p.show_guides)),
        "view.toggle_grid" => Ok(toggle(app, |p| &mut p.show_grid)),
        "view.toggle_nodes" => Ok(toggle(app, |p| &mut p.show_nodes)),
        "view.toggle_coords" => Ok(toggle(app, |p| &mut p.show_coords)),
        "view.toggle_anchors" => Ok(toggle(app, |p| &mut p.show_anchors)),
        "view.toggle_kerning" => Ok(toggle(app, |p| &mut p.show_kerning)),
        "view.toggle_fill" => Ok(toggle(app, |p| &mut p.show_fill)),
        "view.toggle_template" => Ok(toggle(app, |p| &mut p.show_template)),
        "view.toggle_snap" => Ok(toggle(app, |p| &mut p.snap)),
        "view.toggle_theme" => {
            let next = if app.prefs.theme == ThemeMode::Dark { ThemeMode::Light } else { ThemeMode::Dark };
            app.set_theme(ctx, next);
            Ok(if next == ThemeMode::Dark { "Dark theme".into() } else { "Light theme".into() })
        }
        // ------------------------------------------------------------------ Window
        "window.new_font_tab" => {
            let di = app.doc_idx();
            app.tabs.push(Tab::font(di));
            app.active = app.tabs.len() - 1;
            done("Opened a font tab")
        }
        "window.new_glyph_tab" => {
            let gi = app.current_glyph().ok_or("No glyph")?;
            let di = app.doc_idx();
            let n = app.docs[di].font.glyphs[gi].name.clone();
            app.tabs.push(Tab::glyph(di, vec![n.clone()], 0));
            app.active = app.tabs.len() - 1;
            Ok(format!("Opened {n}"))
        }
        "window.toggle_toolbar" => Ok(toggle(app, |p| &mut p.show_toolbar)),
        "window.toggle_panels" => Ok(toggle(app, |p| &mut p.show_right)),
        "window.toggle_sidebar" => Ok(toggle(app, |p| &mut p.show_left)),
        "window.next_tab" => {
            app.active = (app.active + 1) % app.tabs.len();
            done("Next tab")
        }
        "window.previous_tab" => {
            app.active = (app.active + app.tabs.len() - 1) % app.tabs.len();
            done("Previous tab")
        }
        "window.close_tab" => {
            let a = app.active;
            app.close_tab(a);
            done("Closed the tab")
        }
        "window.close_glyph_tabs" => {
            app.tabs.retain(|t| t.kind == TabKind::Font);
            app.active = 0;
            done("Closed all glyph tabs")
        }
        // ------------------------------------------------------------------ Help
        "help.getting_started" => {
            app.dlg.message = Some((
                "Getting started".into(),
                "• Double-click a glyph cell to open it for drawing.\n• Pick the Pen (P) to draw: click for corners, drag for curves, click the first node to close.\n• The Contour tool (A) selects and moves nodes; arrows nudge (Shift ×10).\n• Metrics (M) and Kerning (K) tools adjust spacing in a text line.\n• File ▸ Export Font As… writes TrueType or UFO.\n• AI agents can drive the app over MCP: see Preferences ▸ Agents.".into(),
            ));
            done("Opened Getting Started")
        }
        "help.about" => {
            app.dlg.about = true;
            done("Opened About")
        }
        _ => Err(format!("'{}' is coming in a later version.", cmd.label)),
    }
}

fn open_checked(app: &mut App, p: &Path) -> Result<String, String> {
    if !p.exists() {
        return Err(format!("{} does not exist", p.display()));
    }
    let before = app.docs.len();
    let msg_before = app.dlg.message.clone();
    app.open_path(p);
    if app.dlg.message != msg_before {
        if let Some((_, body)) = app.dlg.message.take() {
            return Err(body);
        }
    }
    let _ = before;
    Ok(format!("Opened {} ({} glyphs)", app.doc().title(), app.doc().font.glyphs.len()))
}

fn current_name(app: &App) -> String {
    app.current_glyph().map(|gi| app.doc().font.glyphs[gi].name.clone()).unwrap_or_default()
}

pub fn cycle_font(app: &mut App, d: i64) {
    let n = app.docs.len() as i64;
    let next = (app.doc_idx() as i64 + d).rem_euclid(n) as usize;
    if let Some(ti) = app.tabs.iter().position(|t| t.doc == next && t.kind == TabKind::Font) {
        app.active = ti;
    }
}

fn center_selected(app: &mut App) {
    let di = app.doc_idx();
    let m = app.docs[di].master;
    let targets: Vec<usize> = if app.in_glyph_tab() { app.current_glyph().into_iter().collect() } else { app.docs[di].selected() };
    app.docs[di].checkpoint_font("Center glyphs");
    for gi in targets {
        let lsb = app.docs[di].font.lsb(gi, m);
        let rsb = app.docs[di].font.rsb(gi, m);
        let avg = ((lsb + rsb) / 2.0).round();
        apply_metrics(&mut app.docs[di].font, gi, m, Some(avg), Some(avg), None);
    }
}

fn report_compat(app: &mut App, ctx: &egui::Context) -> String {
    let d = app.doc();
    let mut bad = Vec::new();
    for g in &d.font.glyphs {
        for k in 1..g.layers.len() {
            if !crate::font::compatible(&g.layers[0], &g.layers[k]) {
                bad.push(g.name.clone());
                break;
            }
        }
    }
    let msg = if bad.is_empty() { "All glyphs are compatible across masters.".to_string() } else { format!("{} glyph(s) are not compatible: {}", bad.len(), bad.iter().take(30).cloned().collect::<Vec<_>>().join(" ")) };
    app.toast(ctx, msg.clone());
    msg
}
