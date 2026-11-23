//! Application state: open fonts (documents), tabs, tools, panels, preferences,
//! undo/redo, clipboard and the commands behind every menu item and shortcut.

#![allow(dead_code)]

use egui::Vec2;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::font::*;
use crate::geom;
use crate::glyphnames::{self, Category, Script};
use crate::io;
use crate::render::Cache;
use crate::theme::{self, Palette, ThemeMode};

// ------------------------------------------------------------------------------------------
// Documents and undo

pub enum UndoKind {
    Glyph(usize, Glyph),
    Font(Box<Font>),
}

pub struct Undo {
    pub label: String,
    pub kind: UndoKind,
}

pub struct Doc {
    pub font: Font,
    pub master: usize,
    pub sel: BTreeSet<usize>,
    pub focus: Option<usize>,
    pub anchor: Option<usize>,
    pub undo: Vec<Undo>,
    pub redo: Vec<Undo>,
    pub filter: GlyphFilter,
    pub view_cache: Vec<usize>,
    pub view_key: u64,
}

impl Doc {
    pub fn new(font: Font) -> Self {
        Doc {
            font,
            master: 0,
            sel: BTreeSet::new(),
            focus: None,
            anchor: None,
            undo: Vec::new(),
            redo: Vec::new(),
            filter: GlyphFilter::All,
            view_cache: Vec::new(),
            view_key: u64::MAX,
        }
    }

    /// Remember a glyph before changing it.
    pub fn checkpoint_glyph(&mut self, gi: usize, label: &str) {
        if gi >= self.font.glyphs.len() {
            return;
        }
        self.undo.push(Undo { label: label.to_string(), kind: UndoKind::Glyph(gi, self.font.glyphs[gi].clone()) });
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.font.dirty = true;
    }

    /// Remember the whole font before a font-wide change.
    pub fn checkpoint_font(&mut self, label: &str) {
        self.undo.push(Undo { label: label.to_string(), kind: UndoKind::Font(Box::new(self.font.clone())) });
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.font.dirty = true;
    }

    fn swap(&mut self, u: Undo) -> Undo {
        match u.kind {
            UndoKind::Glyph(gi, g) => {
                if gi < self.font.glyphs.len() {
                    let cur = std::mem::replace(&mut self.font.glyphs[gi], g);
                    self.font.glyphs[gi].touch();
                    self.font.rebuild_index();
                    Undo { label: u.label, kind: UndoKind::Glyph(gi, cur) }
                } else {
                    Undo { label: u.label, kind: UndoKind::Glyph(gi, g) }
                }
            }
            UndoKind::Font(f) => {
                let path = self.font.path.clone();
                let cur = std::mem::replace(&mut self.font, *f);
                self.font.path = path;
                self.font.rebuild_index();
                for g in &mut self.font.glyphs {
                    g.touch();
                }
                self.master = self.master.min(self.font.masters.len() - 1);
                self.sel.retain(|&i| i < self.font.glyphs.len());
                Undo { label: u.label, kind: UndoKind::Font(Box::new(cur)) }
            }
        }
    }

    pub fn undo(&mut self) -> Option<String> {
        let u = self.undo.pop()?;
        let label = u.label.clone();
        let r = self.swap(u);
        self.redo.push(r);
        self.font.dirty = true;
        self.view_key = u64::MAX;
        Some(label)
    }

    pub fn redo(&mut self) -> Option<String> {
        let u = self.redo.pop()?;
        let label = u.label.clone();
        let r = self.swap(u);
        self.undo.push(r);
        self.font.dirty = true;
        self.view_key = u64::MAX;
        Some(label)
    }

    pub fn selected(&self) -> Vec<usize> {
        self.sel.iter().copied().filter(|&i| i < self.font.glyphs.len()).collect()
    }

    pub fn title(&self) -> String {
        format!("{}{}", self.font.title(), if self.font.dirty { " •" } else { "" })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum GlyphFilter {
    All,
    Letters,
    Category(Category),
    Script(Script),
    Flag(usize),
    Empty,
    WithOutlines,
    Composites,
    Unencoded,
    Kerned,
    Search(String),
}

// ------------------------------------------------------------------------------------------
// Tabs

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TabKind {
    Font,
    Glyph,
}

pub struct Tab {
    pub doc: usize,
    pub kind: TabKind,
    /// Glyph names shown in the glyph window's text line.
    pub text: Vec<String>,
    /// Index into `text` of the glyph being edited.
    pub cur: usize,
    pub zoom: f32,
    pub pan: Vec2,
    pub fit: bool,
    pub text_input: String,
    pub caret: usize,
}

impl Tab {
    pub fn font(doc: usize) -> Self {
        Tab { doc, kind: TabKind::Font, text: Vec::new(), cur: 0, zoom: 0.5, pan: Vec2::ZERO, fit: true, text_input: String::new(), caret: 0 }
    }
    pub fn glyph(doc: usize, names: Vec<String>, cur: usize) -> Self {
        let text_input = names_to_text(&names);
        Tab { doc, kind: TabKind::Glyph, caret: cur + 1, text: names, cur, zoom: 0.5, pan: Vec2::ZERO, fit: true, text_input }
    }
    pub fn current_name(&self) -> Option<&str> {
        self.text.get(self.cur).map(|s| s.as_str())
    }
}

/// Glyph names → "/A/B" text, using plain characters for simple names.
pub fn names_to_text(names: &[String]) -> String {
    let mut s = String::new();
    for n in names {
        if n.chars().count() == 1 && n.chars().all(|c| c.is_ascii_alphanumeric()) {
            s.push_str(n);
        } else if n == "space" {
            s.push(' ');
        } else {
            s.push('/');
            s.push_str(n);
            s.push(' ');
        }
    }
    s
}

/// Parse "Hello /A.alt /space" text into glyph names using the font's encoding.
pub fn text_to_names(font: &Font, text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '/' {
            let mut j = i + 1;
            let mut name = String::new();
            while j < chars.len() && !chars[j].is_whitespace() && chars[j] != '/' {
                name.push(chars[j]);
                j += 1;
            }
            if !name.is_empty() {
                out.push(name);
                if j < chars.len() && chars[j] == ' ' {
                    j += 1;
                }
                i = j;
                continue;
            }
            i += 1;
            out.push("slash".into());
            continue;
        }
        if c == '\\' && i + 1 < chars.len() && chars[i + 1] == 'n' {
            out.push("\n".into());
            i += 2;
            continue;
        }
        if c == '\n' {
            out.push("\n".into());
            i += 1;
            continue;
        }
        let u = c as u32;
        let name = font.find_unicode(u).map(|gi| font.glyphs[gi].name.clone()).unwrap_or_else(|| glyphnames::name_for_unicode(u));
        out.push(name);
        i += 1;
    }
    out
}

// ------------------------------------------------------------------------------------------
// Tools

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Tool {
    Contour,
    Element,
    Metrics,
    Kerning,
    Text,
    Pen,
    Rapid,
    Rectangle,
    Ellipse,
    Knife,
    Eraser,
    Hand,
    Zoom,
    Measure,
    Anchor,
}

impl Tool {
    pub const ALL: [Tool; 15] = [
        Tool::Contour,
        Tool::Element,
        Tool::Metrics,
        Tool::Kerning,
        Tool::Text,
        Tool::Pen,
        Tool::Rapid,
        Tool::Rectangle,
        Tool::Ellipse,
        Tool::Knife,
        Tool::Eraser,
        Tool::Anchor,
        Tool::Measure,
        Tool::Hand,
        Tool::Zoom,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Tool::Contour => "Contour",
            Tool::Element => "Element",
            Tool::Metrics => "Metrics",
            Tool::Kerning => "Kerning",
            Tool::Text => "Text",
            Tool::Pen => "Pen",
            Tool::Rapid => "Pencil",
            Tool::Rectangle => "Rectangle",
            Tool::Ellipse => "Ellipse",
            Tool::Knife => "Knife",
            Tool::Eraser => "Eraser",
            Tool::Hand => "Hand",
            Tool::Zoom => "Zoom",
            Tool::Measure => "Measure",
            Tool::Anchor => "Anchor",
        }
    }
    pub fn icon(self) -> &'static str {
        use crate::icons as I;
        match self {
            Tool::Contour => I::MOUSE_POINTER_2,
            Tool::Element => I::MOUSE_POINTER,
            Tool::Metrics => I::MOVE_HORIZONTAL,
            Tool::Kerning => I::ARROW_LEFT_RIGHT,
            Tool::Text => I::TYPE,
            Tool::Pen => I::PEN_TOOL,
            Tool::Rapid => I::PENCIL,
            Tool::Rectangle => I::SQUARE,
            Tool::Ellipse => I::CIRCLE,
            Tool::Knife => I::SLASH,
            Tool::Eraser => I::ERASER,
            Tool::Hand => I::HAND,
            Tool::Zoom => I::ZOOM_IN,
            Tool::Measure => I::RULER,
            Tool::Anchor => I::ANCHOR,
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Tool::Contour => "A",
            Tool::Element => "V",
            Tool::Metrics => "M",
            Tool::Kerning => "K",
            Tool::Text => "T",
            Tool::Pen => "P",
            Tool::Rapid => "Y",
            Tool::Rectangle => "F",
            Tool::Ellipse => "E",
            Tool::Knife => "J",
            Tool::Eraser => "X",
            Tool::Hand => "H",
            Tool::Zoom => "Z",
            Tool::Measure => "U",
            Tool::Anchor => "N",
        }
    }
}

// ------------------------------------------------------------------------------------------
// Panels

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Hash)]
pub enum PanelId {
    AnchorsPins,
    Brush,
    Chat,
    Classes,
    Color,
    Elements,
    Features,
    FontInfo,
    FontMap,
    FontAudit,
    Fonts,
    Gallery,
    Glyph,
    Guide,
    History,
    Image,
    Kerning,
    LayersMasters,
    Lookups,
    Measurements,
    Metrics,
    Node,
    Notepad,
    Output,
    PairsPhrases,
    Preview,
    Scripting,
    Selections,
    Source,
    Stroke,
    Superkerning,
    Tables,
    Transform,
    TtHinting,
    Variations,
    View,
}

impl PanelId {
    pub const ALL: [PanelId; 36] = [
        PanelId::AnchorsPins,
        PanelId::Brush,
        PanelId::Chat,
        PanelId::Classes,
        PanelId::Color,
        PanelId::Elements,
        PanelId::Features,
        PanelId::FontInfo,
        PanelId::FontMap,
        PanelId::FontAudit,
        PanelId::Fonts,
        PanelId::Gallery,
        PanelId::Glyph,
        PanelId::Guide,
        PanelId::History,
        PanelId::Image,
        PanelId::Kerning,
        PanelId::LayersMasters,
        PanelId::Lookups,
        PanelId::Measurements,
        PanelId::Metrics,
        PanelId::Node,
        PanelId::Notepad,
        PanelId::Output,
        PanelId::PairsPhrases,
        PanelId::Preview,
        PanelId::Scripting,
        PanelId::Selections,
        PanelId::Source,
        PanelId::Stroke,
        PanelId::Superkerning,
        PanelId::Tables,
        PanelId::Transform,
        PanelId::TtHinting,
        PanelId::Variations,
        PanelId::View,
    ];
    pub fn label(self) -> &'static str {
        match self {
            PanelId::AnchorsPins => "Anchors & Pins",
            PanelId::Brush => "Brush",
            PanelId::Chat => "Chat",
            PanelId::Classes => "Classes",
            PanelId::Color => "Color",
            PanelId::Elements => "Elements",
            PanelId::Features => "Features",
            PanelId::FontInfo => "Font Info",
            PanelId::FontMap => "Font Map",
            PanelId::FontAudit => "FontAudit",
            PanelId::Fonts => "Fonts",
            PanelId::Gallery => "Gallery",
            PanelId::Glyph => "Glyph",
            PanelId::Guide => "Guide",
            PanelId::History => "History",
            PanelId::Image => "Image",
            PanelId::Kerning => "Kerning",
            PanelId::LayersMasters => "Layers & Masters",
            PanelId::Lookups => "Lookups",
            PanelId::Measurements => "Measurements",
            PanelId::Metrics => "Metrics",
            PanelId::Node => "Node",
            PanelId::Notepad => "Notepad",
            PanelId::Output => "Output",
            PanelId::PairsPhrases => "Pairs & Phrases",
            PanelId::Preview => "Preview",
            PanelId::Scripting => "Scripting",
            PanelId::Selections => "Selections",
            PanelId::Source => "Source",
            PanelId::Stroke => "Stroke",
            PanelId::Superkerning => "Superkerning",
            PanelId::Tables => "Tables",
            PanelId::Transform => "Transform",
            PanelId::TtHinting => "TT Hinting",
            PanelId::Variations => "Variations",
            PanelId::View => "View",
        }
    }
    pub fn icon(self) -> &'static str {
        use crate::icons as I;
        match self {
            PanelId::AnchorsPins => I::ANCHOR,
            PanelId::Brush => I::PAINTBRUSH,
            PanelId::Chat => I::MESSAGE_CIRCLE,
            PanelId::Classes => I::BOXES,
            PanelId::Color => I::PALETTE,
            PanelId::Elements => I::SHAPES,
            PanelId::Features => I::LIGATURE,
            PanelId::FontInfo => I::INFO,
            PanelId::FontMap => I::TABLE_2,
            PanelId::FontAudit => I::SHIELD_CHECK,
            PanelId::Fonts => I::BOOK_TYPE,
            PanelId::Gallery => I::LAYOUT_GRID,
            PanelId::Glyph => I::LETTER_TEXT,
            PanelId::Guide => I::RULER_DIMENSION_LINE,
            PanelId::History => I::HISTORY,
            PanelId::Image => I::IMAGE,
            PanelId::Kerning => I::ARROW_LEFT_RIGHT,
            PanelId::LayersMasters => I::LAYERS_3,
            PanelId::Lookups => I::LIST_TREE,
            PanelId::Measurements => I::RULER,
            PanelId::Metrics => I::MOVE_HORIZONTAL,
            PanelId::Node => I::CIRCLE_DOT,
            PanelId::Notepad => I::NOTEBOOK_PEN,
            PanelId::Output => I::TERMINAL,
            PanelId::PairsPhrases => I::TEXT_QUOTE,
            PanelId::Preview => I::EYE,
            PanelId::Scripting => I::BRACES,
            PanelId::Selections => I::LASSO_SELECT,
            PanelId::Source => I::FILE_TEXT,
            PanelId::Stroke => I::SPLINE,
            PanelId::Superkerning => I::SPARKLES,
            PanelId::Tables => I::TABLE,
            PanelId::Transform => I::SCALING,
            PanelId::TtHinting => I::GRIP_VERTICAL,
            PanelId::Variations => I::VARIABLE,
            PanelId::View => I::SLIDERS_HORIZONTAL,
        }
    }
    /// Panels whose full feature set is still to come.
    pub fn coming_soon(self) -> Option<&'static str> {
        match self {
            PanelId::Brush => Some("Brush strokes turn open contours into outlines with a chosen nib. Coming in a later version."),
            PanelId::Chat => Some("Chat lets you discuss glyphs with collaborators. Coming in a later version."),
            PanelId::Image => Some("Place a scanned drawing behind a glyph and trace it. Coming in a later version."),
            PanelId::Lookups => Some("Visual OpenType lookup editor. For now, write feature code in the Features panel."),
            PanelId::Scripting => Some("Python scripting console. Coming in a later version."),
            PanelId::Stroke => Some("Expand open strokes to outlines with width and cap settings. Coming in a later version."),
            PanelId::Superkerning => Some("Automatic kerning suggestions. Coming in a later version."),
            PanelId::TtHinting => Some("TrueType hinting instructions. Coming in a later version."),
            _ => None,
        }
    }
}

// ------------------------------------------------------------------------------------------
// Preferences

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Caption {
    Name,
    Unicode,
    Index,
    Width,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum SortBy {
    Font,
    Name,
    Unicode,
    Category,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub theme: ThemeMode,
    pub welcome_at_startup: bool,
    pub cell_size: f32,
    pub caption: Caption,
    pub sort: SortBy,
    pub show_template: bool,
    pub nudge: f32,
    pub nudge_shift: f32,
    pub nudge_ctrl: f32,
    pub show_fill: bool,
    pub show_nodes: bool,
    pub show_coords: bool,
    pub show_metrics: bool,
    pub show_zones: bool,
    pub show_guides: bool,
    pub show_grid: bool,
    pub show_anchors: bool,
    pub show_kerning: bool,
    pub grid_step: f32,
    pub snap: bool,
    pub default_glyph_set: usize,
    pub recent: Vec<PathBuf>,
    pub right_panels: Vec<PanelId>,
    pub show_left: bool,
    pub show_right: bool,
    pub show_toolbar: bool,
    pub sample_texts: Vec<String>,
    pub preview_text: String,
    pub preview_size: f32,
    pub kern_step: f32,
    pub undo_levels: usize,
    pub save_format: usize,
    pub notepad: String,
    /// Let AI agents connect over MCP on a local HTTP port.
    pub mcp_http: bool,
    pub mcp_port: u16,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            theme: ThemeMode::Dark,
            welcome_at_startup: true,
            cell_size: 92.0,
            caption: Caption::Name,
            sort: SortBy::Font,
            show_template: true,
            nudge: 1.0,
            nudge_shift: 10.0,
            nudge_ctrl: 100.0,
            show_fill: false,
            show_nodes: true,
            show_coords: false,
            show_metrics: true,
            show_zones: true,
            show_guides: true,
            show_grid: false,
            show_anchors: true,
            show_kerning: true,
            grid_step: 50.0,
            snap: false,
            default_glyph_set: 1,
            recent: Vec::new(),
            right_panels: vec![PanelId::Fonts, PanelId::Glyph, PanelId::LayersMasters, PanelId::Elements, PanelId::Transform],
            show_left: true,
            show_right: true,
            show_toolbar: true,
            sample_texts: vec![
                "HAMBURGEFONTSIV hamburgefontsiv".into(),
                "nnonnoonnnoooo HHOHHOOHHHOOOO".into(),
                "The quick brown fox jumps over the lazy dog".into(),
                "AVATAR Toyota WAVE Yogurt LTA".into(),
                "0123456789 $€£ (12.5%)".into(),
            ],
            preview_text: "Hamburgefontsiv".into(),
            preview_size: 48.0,
            kern_step: 10.0,
            undo_levels: 200,
            save_format: 0,
            notepad: String::new(),
            mcp_http: false,
            mcp_port: crate::mcp::DEFAULT_PORT,
        }
    }
}

fn prefs_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("fontabulous_studio").join("prefs.json"))
}

impl Prefs {
    pub fn load() -> Self {
        prefs_path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }
    pub fn save(&self) {
        if let Some(p) = prefs_path() {
            if let Some(d) = p.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            if let Ok(s) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(p, s);
            }
        }
    }
}

// ------------------------------------------------------------------------------------------
// Dialog state

pub struct Dialogs {
    pub welcome: bool,
    pub welcome_family: String,
    pub welcome_set: usize,
    pub font_info: bool,
    pub fi_page: usize,
    pub prefs: bool,
    pub prefs_page: usize,
    pub find: bool,
    pub find_text: String,
    pub add_glyphs: bool,
    pub add_glyphs_text: String,
    pub add_glyphs_set: usize,
    pub rename: Option<(usize, String, String)>,
    pub about: bool,
    pub shortcuts: bool,
    pub shortcuts_filter: String,
    pub export: bool,
    pub export_format: usize,
    pub export_masters: bool,
    pub export_instances: bool,
    pub open_installed: bool,
    pub installed: Vec<PathBuf>,
    pub installed_filter: String,
    pub kern_adjust: bool,
    pub kern_scale: f32,
    pub kern_add: f32,
    pub kern_remove: bool,
    pub kern_threshold: f32,
    pub add_component: bool,
    pub component_name: String,
    pub add_anchor: bool,
    pub anchor_name: String,
    pub add_variation: bool,
    pub variation_name: String,
    pub instance: bool,
    pub instance_loc: Vec<f32>,
    pub instance_name: String,
    pub message: Option<(String, String)>,
    pub confirm_close: Option<usize>,
    pub confirm_quit: bool,
    pub coming_soon: Option<String>,
    pub print: bool,
    pub metrics_link: bool,
    pub link_lsb: String,
    pub link_rsb: String,
    pub actions: bool,
    pub action_lsb: f32,
    pub action_rsb: f32,
    pub action_round: bool,
    pub action_extremes: bool,
    pub action_direction: bool,
    pub action_clean: bool,
}

impl Default for Dialogs {
    fn default() -> Self {
        Dialogs {
            welcome: false,
            welcome_family: String::new(),
            welcome_set: 1,
            font_info: false,
            fi_page: 0,
            prefs: false,
            prefs_page: 0,
            find: false,
            find_text: String::new(),
            add_glyphs: false,
            add_glyphs_text: String::new(),
            add_glyphs_set: 2,
            rename: None,
            about: false,
            shortcuts: false,
            shortcuts_filter: String::new(),
            export: false,
            export_format: 0,
            export_masters: true,
            export_instances: false,
            open_installed: false,
            installed: Vec::new(),
            installed_filter: String::new(),
            kern_adjust: false,
            kern_scale: 100.0,
            kern_add: 0.0,
            kern_remove: false,
            kern_threshold: 5.0,
            add_component: false,
            component_name: String::new(),
            add_anchor: false,
            anchor_name: "top".into(),
            add_variation: false,
            variation_name: "Bold".into(),
            instance: false,
            instance_loc: Vec::new(),
            instance_name: "Medium".into(),
            message: None,
            confirm_close: None,
            confirm_quit: false,
            coming_soon: None,
            print: false,
            metrics_link: false,
            link_lsb: String::new(),
            link_rsb: String::new(),
            actions: false,
            action_lsb: 0.0,
            action_rsb: 0.0,
            action_round: true,
            action_extremes: false,
            action_direction: true,
            action_clean: true,
        }
    }
}

pub enum Clip {
    Glyphs(Vec<Glyph>),
    Elements(Layer),
}

// ------------------------------------------------------------------------------------------
// Editor interaction state

#[derive(Default)]
pub struct EditState {
    pub drag: Option<Drag>,
    pub pen_contour: Option<usize>,
    pub rapid: Vec<[f32; 2]>,
    pub measure: Option<([f32; 2], [f32; 2])>,
    pub hover_font: Option<[f32; 2]>,
    pub space_pan: bool,
    pub knife: Option<([f32; 2], [f32; 2])>,
}

#[derive(Clone, Debug)]
pub enum Drag {
    Points { last: [f32; 2], moved: bool },
    Handle { ci: usize, pi: usize, moved: bool },
    Rubber { start: [f32; 2], end: [f32; 2], add: bool },
    Shape { start: [f32; 2], end: [f32; 2] },
    PenHandle { ci: usize, pi: usize },
    Pan,
    Sidebearing { left: bool, glyph: usize, start_x: f32, orig: f32 },
    Kern { left: String, right: String, start_x: f32, orig: f32 },
    Measure,
    Knife,
}

// ------------------------------------------------------------------------------------------
// The app

pub struct App {
    pub docs: Vec<Doc>,
    pub tabs: Vec<Tab>,
    pub active: usize,
    pub tool: Tool,
    pub prev_tool: Option<Tool>,
    pub prefs: Prefs,
    pub pal: Palette,
    pub dlg: Dialogs,
    pub cache: Cache,
    pub clip: Option<Clip>,
    pub edit: EditState,
    pub output: Vec<String>,
    pub status: Option<(String, f64)>,
    pub screenshot: Option<PathBuf>,
    pub frame_no: u64,
    pub shot_requested: bool,
    pub prefs_dirty: bool,
    pub transform: TransformState,
    pub selections: Vec<(String, Vec<String>)>,
    pub kern_filter: String,
    pub class_sel: Option<usize>,
    pub pairs_text: String,
    pub var_loc: Vec<f32>,
    pub quit_ok: bool,
    pub fonts_search: String,
    /// MCP bridge: agent tool calls arrive here and run on the UI thread.
    pub mcp: crate::mcp::Bridge,
}

#[derive(Clone)]
pub struct TransformState {
    pub dx: f32,
    pub dy: f32,
    pub sx: f32,
    pub sy: f32,
    pub rotate: f32,
    pub slant: f32,
    pub proportional: bool,
    pub origin: usize,
}

impl Default for TransformState {
    fn default() -> Self {
        TransformState { dx: 0.0, dy: 0.0, sx: 100.0, sy: 100.0, rotate: 0.0, slant: 0.0, proportional: true, origin: 6 }
    }
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<PathBuf>) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        let mut prefs = Prefs::load();
        if screenshot.is_some() {
            // Checks always see the same, default-looking window.
            prefs = Prefs { recent: prefs.recent.clone(), ..Prefs::default() };
        }
        theme::apply(&cc.egui_ctx, prefs.theme);
        let pal = theme::palette(prefs.theme);
        let set = glyphnames::glyph_set(prefs.default_glyph_set.min(glyphnames::GLYPH_SETS.len() - 1));
        let font = Font::new("Unnamed", &set);
        let mut app = App {
            docs: vec![Doc::new(font)],
            tabs: vec![Tab::font(0)],
            active: 0,
            tool: Tool::Contour,
            prev_tool: None,
            dlg: Dialogs { welcome: prefs.welcome_at_startup && screenshot.is_none(), ..Default::default() },
            prefs,
            pal,
            cache: Cache::default(),
            clip: None,
            edit: EditState::default(),
            output: vec!["Fontabulous Studio started.".into()],
            status: None,
            screenshot,
            frame_no: 0,
            shot_requested: false,
            prefs_dirty: false,
            transform: TransformState::default(),
            selections: Vec::new(),
            kern_filter: String::new(),
            class_sel: None,
            pairs_text: String::new(),
            var_loc: Vec::new(),
            quit_ok: false,
            fonts_search: String::new(),
            mcp: crate::mcp::Bridge::new(&cc.egui_ctx),
        };
        let first = 1.min(app.docs[0].font.glyphs.len().saturating_sub(1));
        app.docs[0].sel.insert(first);
        app.docs[0].focus = app.docs[0].sel.iter().next().copied();
        app.docs[0].font.dirty = false;
        // Open files given on the command line.
        let args: Vec<String> = std::env::args().skip(1).collect();
        let mut skip = false;
        for a in args {
            if skip {
                skip = false;
                continue;
            }
            if a == "--mcp" {
                continue;
            }
            if a == "--screenshot" {
                skip = true;
                continue;
            }
            let p = PathBuf::from(&a);
            if p.exists() {
                app.open_path(&p);
                app.dlg.welcome = false;
            }
        }
        app
    }

    pub fn set_theme(&mut self, ctx: &egui::Context, mode: ThemeMode) {
        self.prefs.theme = mode;
        theme::apply(ctx, mode);
        self.pal = theme::palette(mode);
        self.prefs_dirty = true;
    }

    pub fn log(&mut self, s: impl Into<String>) {
        let s = s.into();
        self.output.push(s.clone());
        if self.output.len() > 500 {
            self.output.remove(0);
        }
    }

    pub fn toast(&mut self, ctx: &egui::Context, s: impl Into<String>) {
        let s = s.into();
        self.log(s.clone());
        self.status = Some((s, ctx.input(|i| i.time) + 4.0));
    }

    pub fn tab(&self) -> &Tab {
        &self.tabs[self.active.min(self.tabs.len() - 1)]
    }
    pub fn tab_mut(&mut self) -> &mut Tab {
        let i = self.active.min(self.tabs.len() - 1);
        &mut self.tabs[i]
    }
    pub fn doc_idx(&self) -> usize {
        self.tab().doc.min(self.docs.len() - 1)
    }
    pub fn doc(&self) -> &Doc {
        &self.docs[self.doc_idx()]
    }
    pub fn doc_mut(&mut self) -> &mut Doc {
        let i = self.doc_idx();
        &mut self.docs[i]
    }
    pub fn in_glyph_tab(&self) -> bool {
        self.tab().kind == TabKind::Glyph
    }

    /// The glyph being edited (glyph tab) or the focused glyph (font tab).
    pub fn current_glyph(&self) -> Option<usize> {
        let d = self.doc();
        if self.in_glyph_tab() {
            self.tab().current_name().and_then(|n| d.font.find(n))
        } else {
            d.focus.filter(|&i| i < d.font.glyphs.len()).or_else(|| d.sel.iter().next().copied())
        }
    }

    // ---------------------------------------------------------------------------------- files

    pub fn new_font(&mut self, family: &str, set: usize) {
        let names = glyphnames::glyph_set(set);
        let fam = if family.trim().is_empty() { "Unnamed" } else { family.trim() };
        let mut f = Font::new(fam, &names);
        f.dirty = false;
        self.add_doc(f);
    }

    pub fn add_doc(&mut self, font: Font) {
        // Replace an untouched starter font.
        if self.docs.len() == 1 && !self.docs[0].font.dirty && self.docs[0].font.path.is_none() && self.docs[0].undo.is_empty() && self.docs[0].font.info.family == "Unnamed" {
            self.docs[0] = Doc::new(font);
            self.tabs.retain(|t| t.doc == 0 && t.kind == TabKind::Font);
            if self.tabs.is_empty() {
                self.tabs.push(Tab::font(0));
            }
            self.active = 0;
            self.cache.clear();
        } else {
            self.docs.push(Doc::new(font));
            let di = self.docs.len() - 1;
            self.tabs.push(Tab::font(di));
            self.active = self.tabs.len() - 1;
        }
        let d = self.doc_mut();
        if !d.font.glyphs.is_empty() {
            d.sel.insert(0);
            d.focus = Some(0);
        }
        self.var_loc.clear();
    }

    pub fn open_path(&mut self, p: &std::path::Path) {
        match io::open_any(p) {
            Ok(f) => {
                let n = f.glyphs.len();
                let name = f.title();
                self.add_doc(f);
                self.prefs.recent.retain(|r| r != p);
                self.prefs.recent.insert(0, p.to_path_buf());
                self.prefs.recent.truncate(12);
                self.prefs_dirty = true;
                self.log(format!("Opened {} ({n} glyphs) from {}", name, p.display()));
            }
            Err(e) => {
                self.dlg.message = Some(("Could not open font".into(), format!("{}\n\n{e}", p.display())));
                self.log(format!("Open failed: {e}"));
            }
        }
    }

    pub fn open_dialog(&mut self) {
        if let Some(files) = rfd::FileDialog::new()
            .add_filter("Fonts", &["fstudio", "ttf", "otf", "ttc", "otc", "designspace"])
            .add_filter("Fontabulous Studio", &["fstudio"])
            .add_filter("OpenType / TrueType", &["ttf", "otf", "ttc", "otc"])
            .add_filter("Designspace", &["designspace"])
            .pick_files()
        {
            for f in files {
                self.open_path(&f);
            }
        }
    }

    pub fn open_ufo_dialog(&mut self) {
        if let Some(d) = rfd::FileDialog::new().set_title("Open UFO font folder").pick_folder() {
            self.open_path(&d);
        }
    }

    pub fn save(&mut self, ctx: &egui::Context, save_as: bool) {
        let di = self.doc_idx();
        let path = self.docs[di].font.path.clone();
        let path = match (path, save_as) {
            (Some(p), false) => Some(p),
            _ => rfd::FileDialog::new()
                .add_filter("Fontabulous Studio", &[io::NATIVE_EXT])
                .set_file_name(format!("{}.{}", self.docs[di].font.title().replace(' ', "-"), io::NATIVE_EXT))
                .save_file(),
        };
        let Some(p) = path else { return };
        if let Err(e) = self.save_to(ctx, &p) {
            self.dlg.message = Some(("Could not save".into(), e));
        }
    }

    /// Save the current font in the native format to `p` (no dialog).
    pub fn save_to(&mut self, ctx: &egui::Context, p: &std::path::Path) -> Result<String, String> {
        let di = self.doc_idx();
        io::save_native(&self.docs[di].font, p)?;
        let p = p.to_path_buf();
        self.docs[di].font.path = Some(p.clone());
        self.docs[di].font.dirty = false;
        self.prefs.recent.retain(|r| r != &p);
        self.prefs.recent.insert(0, p.clone());
        self.prefs_dirty = true;
        let msg = format!("Saved {}", p.display());
        self.toast(ctx, msg.clone());
        Ok(msg)
    }

    pub fn save_all(&mut self, ctx: &egui::Context) {
        let cur = self.active;
        for di in 0..self.docs.len() {
            if self.docs[di].font.dirty || self.docs[di].font.path.is_none() {
                if let Some(ti) = self.tabs.iter().position(|t| t.doc == di) {
                    self.active = ti;
                    self.save(ctx, false);
                }
            }
        }
        self.active = cur.min(self.tabs.len() - 1);
    }

    pub fn close_doc(&mut self, di: usize) {
        if self.docs.len() == 1 {
            let f = Font::new("Unnamed", &glyphnames::glyph_set(1));
            self.docs[0] = Doc::new(f);
            self.docs[0].font.dirty = false;
            self.tabs = vec![Tab::font(0)];
            self.active = 0;
            self.cache.clear();
            return;
        }
        self.docs.remove(di);
        self.tabs.retain(|t| t.doc != di);
        for t in &mut self.tabs {
            if t.doc > di {
                t.doc -= 1;
            }
        }
        if self.tabs.is_empty() {
            self.tabs.push(Tab::font(0));
        }
        self.active = self.active.min(self.tabs.len() - 1);
        self.cache.clear();
    }

    pub fn export(&mut self, ctx: &egui::Context, format: usize) {
        let di = self.doc_idx();
        let font = &self.docs[di].font;
        let base = format!("{}-{}", font.info.family, font.info.style).replace(' ', "");
        let picked = match format {
            0 => rfd::FileDialog::new().set_title("Export TrueType fonts to folder").pick_folder(),
            1 => rfd::FileDialog::new().set_title("Export UFO").set_file_name(format!("{base}.ufo")).save_file(),
            2 => rfd::FileDialog::new().add_filter("Fontabulous Studio", &[io::NATIVE_EXT]).set_file_name(format!("{base}.{}", io::NATIVE_EXT)).save_file(),
            _ => {
                self.dlg.coming_soon = Some("OpenType PS (CFF) and web font (WOFF2) export are coming in a later version. TrueType and UFO export work now.".into());
                return;
            }
        };
        let Some(p) = picked else { return };
        match self.export_to(ctx, format, &p) {
            Ok(msg) => {
                if format != 2 {
                    self.dlg.message = Some(("Export finished".into(), msg));
                }
            }
            Err(e) => self.dlg.message = Some(("Export failed".into(), e)),
        }
    }

    /// Export without a dialog. TrueType (0) writes into the folder `p`; UFO (1) and
    /// native (2) write to the file path `p`. Returns a report.
    pub fn export_to(&mut self, ctx: &egui::Context, format: usize, p: &std::path::Path) -> Result<String, String> {
        let di = self.doc_idx();
        let font = self.docs[di].font.clone();
        match format {
            0 => {
                // TrueType, one file per master.
                let dir = p.to_path_buf();
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                let mut lines = Vec::new();
                let masters: Vec<usize> = if self.dlg.export_masters { (0..font.masters.len()).collect() } else { vec![self.docs[di].master] };
                for m in masters {
                    let style = if font.masters.len() > 1 { font.masters[m].name.clone() } else { font.info.style.clone() };
                    let file = dir.join(format!("{}-{}.ttf", font.info.family.replace(' ', ""), style.replace(' ', "")));
                    match io::export_ttf(&font, m, &file, Some(&style)) {
                        Ok(r) => {
                            lines.push(format!("{} — {} glyphs, {} kerning pairs", file.display(), r.glyphs, r.kern_pairs));
                            lines.extend(r.notes);
                        }
                        Err(e) => lines.push(format!("{}: {e}", file.display())),
                    }
                }
                if self.dlg.export_instances && font.masters.len() >= 2 {
                    for inst in font.instances.clone() {
                        let f2 = self.instance_font(di, &inst.location, &inst.name);
                        let file = dir.join(format!("{}-{}.ttf", font.info.family.replace(' ', ""), inst.name.replace(' ', "")));
                        match io::export_ttf(&f2, 0, &file, Some(&inst.name)) {
                            Ok(r) => lines.push(format!("{} — {} glyphs (instance)", file.display(), r.glyphs)),
                            Err(e) => lines.push(format!("{}: {e}", file.display())),
                        }
                    }
                }
                for l in &lines {
                    self.log(l.clone());
                }
                Ok(lines.join("\n"))
            }
            1 => {
                let p = if p.extension().is_none() { p.with_extension("ufo") } else { p.to_path_buf() };
                let files = io::export_ufo_family(&font, &p)?;
                let s: Vec<String> = files.iter().map(|f| f.display().to_string()).collect();
                self.log(format!("Exported UFO: {}", s.join(", ")));
                Ok(s.join("\n"))
            }
            2 => {
                io::save_native(&font, p)?;
                let msg = format!("Exported {}", p.display());
                self.toast(ctx, msg.clone());
                Ok(msg)
            }
            _ => Err("OpenType PS (CFF) and web font (WOFF2) export are coming in a later version.".into()),
        }
    }

    pub fn export_text(&mut self, ctx: &egui::Context, what: &str) {
        let di = self.doc_idx();
        let ext = match what {
            "classes" => "flc",
            "features" | "kernfea" => "fea",
            _ => "txt",
        };
        if let Some(p) = rfd::FileDialog::new().set_file_name(format!("{}-{what}.{ext}", self.docs[di].font.info.family.replace(' ', ""))).save_file() {
            if let Err(e) = self.export_text_to(ctx, what, &p) {
                self.dlg.message = Some(("Export failed".into(), e));
            }
        }
    }

    /// Write kerning / classes / features / kerning feature code to `p` (no dialog).
    pub fn export_text_to(&mut self, ctx: &egui::Context, what: &str, p: &std::path::Path) -> Result<String, String> {
        let di = self.doc_idx();
        let m = self.docs[di].master;
        let text = match what {
            "kerning" => io::kerning_text(&self.docs[di].font, m),
            "classes" => io::classes_text(&self.docs[di].font),
            "features" => self.docs[di].font.features.clone(),
            "kernfea" => io::kern_feature(&self.docs[di].font, m),
            _ => return Err(format!("Unknown text export '{what}'")),
        };
        std::fs::write(p, text).map_err(|e| e.to_string())?;
        let msg = format!("Exported {what} to {}", p.display());
        self.toast(ctx, msg.clone());
        Ok(msg)
    }

    pub fn import_text(&mut self, ctx: &egui::Context, what: &str) {
        let Some(p) = rfd::FileDialog::new().pick_file() else { return };
        if let Err(e) = self.import_text_from(ctx, what, &p) {
            self.dlg.message = Some(("Import failed".into(), e));
        }
    }

    /// Read kerning / classes / features from the text file `p` (no dialog).
    pub fn import_text_from(&mut self, ctx: &egui::Context, what: &str, p: &std::path::Path) -> Result<String, String> {
        let text = std::fs::read_to_string(p).map_err(|_| "Could not read the file as text.".to_string())?;
        if !matches!(what, "kerning" | "classes" | "features") {
            return Err(format!("Unknown text import '{what}'"));
        }
        let di = self.doc_idx();
        let m = self.docs[di].master;
        self.docs[di].checkpoint_font(&format!("Import {what}"));
        let d = &mut self.docs[di];
        let msg = match what {
            "kerning" => {
                let pairs = io::parse_kerning_text(&text);
                let n = pairs.len();
                for k in pairs {
                    d.font.set_kerning(m, &k.left, &k.right, k.value);
                }
                format!("Imported {n} kerning pairs")
            }
            "classes" => {
                let cls = io::parse_classes_text(&text);
                let n = cls.len();
                for c in cls {
                    d.font.classes.retain(|x| x.name != c.name);
                    d.font.classes.push(c);
                }
                format!("Imported {n} classes")
            }
            _ => {
                d.font.features = text;
                "Imported feature code".to_string()
            }
        };
        d.font.dirty = true;
        self.toast(ctx, msg.clone());
        Ok(msg)
    }

    pub fn open_installed_font(&mut self, p: &std::path::Path) {
        match io::import_sfnt(p, 0) {
            Ok(mut f) => {
                f.rebuild_index();
                for g in &mut f.glyphs {
                    g.touch();
                }
                f.dirty = false;
                let n = f.glyphs.len();
                self.add_doc(f);
                self.log(format!("Opened installed font {} ({n} glyphs)", p.display()));
            }
            Err(e) => self.dlg.message = Some(("Could not open font".into(), e)),
        }
    }

    // ---------------------------------------------------------------------------------- glyph tabs

    pub fn open_glyph_tab(&mut self, gi: usize) {
        let di = self.doc_idx();
        let name = self.docs[di].font.glyphs[gi].name.clone();
        // Reuse an existing glyph tab of this font showing just this glyph.
        if let Some(ti) = self.tabs.iter().position(|t| t.doc == di && t.kind == TabKind::Glyph && t.text.len() == 1 && t.text[0] == name) {
            self.active = ti;
            return;
        }
        self.tabs.push(Tab::glyph(di, vec![name], 0));
        self.active = self.tabs.len() - 1;
        if matches!(self.tool, Tool::Metrics | Tool::Kerning | Tool::Text) {
            self.tool = Tool::Contour;
        }
    }

    pub fn open_text_tab(&mut self, text: &str, tool: Tool) {
        let di = self.doc_idx();
        let names = text_to_names(&self.docs[di].font, text);
        if names.is_empty() {
            return;
        }
        let mut t = Tab::glyph(di, names, 0);
        t.text_input = text.to_string();
        self.tabs.push(t);
        self.active = self.tabs.len() - 1;
        self.tool = tool;
    }

    pub fn close_tab(&mut self, ti: usize) {
        if ti >= self.tabs.len() {
            return;
        }
        if self.tabs[ti].kind == TabKind::Font {
            let di = self.tabs[ti].doc;
            if self.docs[di].font.dirty {
                self.dlg.confirm_close = Some(di);
                return;
            }
            self.close_doc(di);
            return;
        }
        self.tabs.remove(ti);
        if self.active >= ti && self.active > 0 {
            self.active -= 1;
        }
        self.active = self.active.min(self.tabs.len() - 1);
    }

    pub fn step_glyph(&mut self, delta: i64) {
        let di = self.doc_idx();
        let view = self.docs[di].view_cache.clone();
        let n = self.docs[di].font.glyphs.len();
        if n == 0 {
            return;
        }
        if self.in_glyph_tab() {
            let Some(gi) = self.current_glyph() else { return };
            let order: Vec<usize> = if view.is_empty() { (0..n).collect() } else { view };
            let pos = order.iter().position(|&i| i == gi).unwrap_or(0) as i64;
            let np = (pos + delta).rem_euclid(order.len() as i64) as usize;
            let name = self.docs[di].font.glyphs[order[np]].name.clone();
            let t = self.tab_mut();
            let cur = t.cur;
            if cur < t.text.len() {
                t.text[cur] = name;
            }
            t.text_input = names_to_text(&t.text);
            for l in &mut self.docs[di].font.glyphs[order[np]].layers {
                l.clear_selection();
            }
        } else {
            let order: Vec<usize> = if view.is_empty() { (0..n).collect() } else { view };
            if order.is_empty() {
                return;
            }
            let cur = self.docs[di].focus.and_then(|f| order.iter().position(|&i| i == f)).map(|p| p as i64).unwrap_or(-1);
            let np = (cur + delta).clamp(0, order.len() as i64 - 1) as usize;
            let d = &mut self.docs[di];
            d.sel.clear();
            d.sel.insert(order[np]);
            d.focus = Some(order[np]);
            d.anchor = Some(order[np]);
        }
    }

    // ---------------------------------------------------------------------------------- edits

    /// Run `f` on the current glyph's current layer with an undo step.
    pub fn edit_layer(&mut self, label: &str, f: impl FnOnce(&mut Layer)) {
        let Some(gi) = self.current_glyph() else { return };
        let di = self.doc_idx();
        let m = self.docs[di].master;
        self.docs[di].checkpoint_glyph(gi, label);
        let g = &mut self.docs[di].font.glyphs[gi];
        f(g.layer_mut(m));
        g.touch();
    }

    /// Run `f` on every selected glyph (font tab) or the current glyph (glyph tab).
    pub fn edit_glyphs(&mut self, label: &str, mut f: impl FnMut(&mut Glyph, usize)) {
        let di = self.doc_idx();
        let m = self.docs[di].master;
        let targets: Vec<usize> = if self.in_glyph_tab() { self.current_glyph().into_iter().collect() } else { self.docs[di].selected() };
        if targets.is_empty() {
            return;
        }
        if targets.len() == 1 {
            self.docs[di].checkpoint_glyph(targets[0], label);
        } else {
            self.docs[di].checkpoint_font(label);
        }
        for gi in targets {
            let g = &mut self.docs[di].font.glyphs[gi];
            f(g, m);
            g.touch();
        }
        self.docs[di].view_key = u64::MAX;
    }

    /// Fill empty selected glyphs with the bundled template letter as a starting outline.
    pub fn copy_template(&mut self, ctx: &egui::Context) {
        let di = self.doc_idx();
        let upm = self.docs[di].font.info.upm;
        let targets: Vec<usize> = if self.in_glyph_tab() { self.current_glyph().into_iter().collect() } else { self.docs[di].selected() };
        let mut outlines = std::collections::HashMap::new();
        for &gi in &targets {
            let g = &self.docs[di].font.glyphs[gi];
            if let Some(&u) = g.unicodes.first() {
                if let Some(t) = self.cache.template_contours(u, upm) {
                    outlines.insert(gi, t);
                }
            }
        }
        let mut n = 0;
        let names: std::collections::HashMap<String, usize> = targets.iter().map(|&gi| (self.docs[di].font.glyphs[gi].name.clone(), gi)).collect();
        self.edit_glyphs("Copy template outlines", |g, m| {
            let Some(gi) = names.get(&g.name) else { return };
            let Some((cs, w)) = outlines.get(gi) else { return };
            let l = g.layer_mut(m);
            if l.contours.is_empty() && l.components.is_empty() {
                l.contours = cs.clone();
                l.width = w.round();
                n += 1;
            }
        });
        self.toast(ctx, format!("Copied template outlines into {n} empty glyph(s)"));
    }

    pub fn undo(&mut self, ctx: &egui::Context) {
        if let Some(l) = self.doc_mut().undo() {
            self.toast(ctx, format!("Undo: {l}"));
        }
    }
    pub fn redo(&mut self, ctx: &egui::Context) {
        if let Some(l) = self.doc_mut().redo() {
            self.toast(ctx, format!("Redo: {l}"));
        }
    }

    pub fn copy(&mut self, ctx: &egui::Context) {
        let di = self.doc_idx();
        let m = self.docs[di].master;
        if self.in_glyph_tab() {
            let Some(gi) = self.current_glyph() else { return };
            let l = self.docs[di].font.glyphs[gi].layer(m);
            let mut out = Layer { width: l.width, ..Default::default() };
            let any = l.has_selection();
            for c in &l.contours {
                if !any || c.points.iter().any(|p| p.sel) {
                    out.contours.push(c.clone());
                }
            }
            for c in &l.components {
                if !any || c.sel {
                    out.components.push(c.clone());
                }
            }
            for a in &l.anchors {
                if !any || a.sel {
                    out.anchors.push(a.clone());
                }
            }
            let n = out.contours.len() + out.components.len();
            self.clip = Some(Clip::Elements(out));
            self.toast(ctx, format!("Copied {n} element(s)"));
        } else {
            let gs: Vec<Glyph> = self.docs[di].selected().iter().map(|&i| self.docs[di].font.glyphs[i].clone()).collect();
            let n = gs.len();
            self.clip = Some(Clip::Glyphs(gs));
            self.toast(ctx, format!("Copied {n} glyph(s)"));
        }
    }

    pub fn cut(&mut self, ctx: &egui::Context) {
        self.copy(ctx);
        self.delete(ctx);
    }

    pub fn paste(&mut self, ctx: &egui::Context) {
        let Some(clip) = self.clip.take() else { return };
        let di = self.doc_idx();
        match &clip {
            Clip::Elements(layer) => {
                if self.in_glyph_tab() {
                    let l2 = layer.clone();
                    self.edit_layer("Paste", |l| {
                        l.clear_selection();
                        for mut c in l2.contours.clone() {
                            for p in &mut c.points {
                                p.sel = true;
                            }
                            l.contours.push(c);
                        }
                        for mut c in l2.components.clone() {
                            c.sel = true;
                            l.components.push(c);
                        }
                        for a in l2.anchors.clone() {
                            l.anchors.push(a);
                        }
                    });
                } else {
                    let l2 = layer.clone();
                    self.edit_glyphs("Paste outlines", |g, m| {
                        let l = g.layer_mut(m);
                        l.contours.extend(l2.contours.iter().cloned());
                        l.components.extend(l2.components.iter().cloned());
                    });
                }
            }
            Clip::Glyphs(gs) => {
                if self.in_glyph_tab() {
                    if let Some(g0) = gs.first() {
                        let m = self.docs[di].master;
                        let src = g0.layer(m).clone();
                        self.edit_layer("Paste glyph", |l| {
                            l.clear_selection();
                            for mut c in src.contours.clone() {
                                for p in &mut c.points {
                                    p.sel = true;
                                }
                                l.contours.push(c);
                            }
                            l.components.extend(src.components.clone());
                        });
                    }
                } else {
                    // Paste onto selected glyphs in order; extra glyphs are appended.
                    let targets = self.docs[di].selected();
                    self.docs[di].checkpoint_font("Paste glyphs");
                    let masters = self.docs[di].font.masters.len();
                    for (k, g) in gs.iter().enumerate() {
                        if let Some(&ti) = targets.get(k) {
                            let t = &mut self.docs[di].font.glyphs[ti];
                            for m in 0..masters.min(g.layers.len()) {
                                let w = t.layers[m].width;
                                t.layers[m] = g.layers[m].clone();
                                if t.layers[m].is_empty() {
                                    t.layers[m].width = w;
                                }
                            }
                            t.touch();
                        } else {
                            let mut ng = g.clone();
                            ng.name = self.docs[di].font.unique_name(&g.name);
                            ng.unicodes.clear();
                            self.docs[di].font.add_glyph(ng);
                        }
                    }
                    self.docs[di].view_key = u64::MAX;
                    self.toast(ctx, format!("Pasted {} glyph(s)", gs.len()));
                }
            }
        }
        self.clip = Some(clip);
    }

    pub fn delete(&mut self, ctx: &egui::Context) {
        let di = self.doc_idx();
        if self.in_glyph_tab() {
            self.edit_layer("Delete", |l| delete_selection(l));
            self.edit.pen_contour = None;
        } else {
            let sel = self.docs[di].selected();
            if sel.is_empty() {
                return;
            }
            self.docs[di].checkpoint_font("Delete glyphs");
            self.docs[di].font.remove_glyphs(&sel);
            self.docs[di].sel.clear();
            self.docs[di].focus = None;
            self.docs[di].view_key = u64::MAX;
            self.toast(ctx, format!("Deleted {} glyph(s)", sel.len()));
        }
    }

    pub fn select_all(&mut self) {
        if self.in_glyph_tab() {
            if let Some(gi) = self.current_glyph() {
                let di = self.doc_idx();
                let m = self.docs[di].master;
                self.docs[di].font.glyphs[gi].layer_mut(m).select_all();
            }
        } else {
            let d = self.doc_mut();
            let v = d.view_cache.clone();
            d.sel = v.into_iter().collect();
        }
    }

    pub fn invert_selection(&mut self) {
        if self.in_glyph_tab() {
            if let Some(gi) = self.current_glyph() {
                let di = self.doc_idx();
                let m = self.docs[di].master;
                let l = self.docs[di].font.glyphs[gi].layer_mut(m);
                for c in &mut l.contours {
                    for p in &mut c.points {
                        p.sel = !p.sel;
                    }
                }
            }
        } else {
            let d = self.doc_mut();
            let v = d.view_cache.clone();
            let cur = d.sel.clone();
            d.sel = v.into_iter().filter(|i| !cur.contains(i)).collect();
        }
    }

    pub fn duplicate_glyphs(&mut self, ctx: &egui::Context) {
        let di = self.doc_idx();
        let sel = if self.in_glyph_tab() { self.current_glyph().into_iter().collect() } else { self.docs[di].selected() };
        if sel.is_empty() {
            return;
        }
        self.docs[di].checkpoint_font("Duplicate glyphs");
        let mut new_sel = BTreeSet::new();
        for gi in sel {
            let mut g = self.docs[di].font.glyphs[gi].clone();
            g.name = self.docs[di].font.unique_name(&g.name);
            g.unicodes.clear();
            let i = self.docs[di].font.add_glyph(g);
            new_sel.insert(i);
        }
        let n = new_sel.len();
        self.docs[di].sel = new_sel;
        self.docs[di].view_key = u64::MAX;
        self.toast(ctx, format!("Duplicated {n} glyph(s)"));
    }

    pub fn add_glyphs_by_names(&mut self, ctx: &egui::Context, names: &[String]) {
        let di = self.doc_idx();
        let mut added = 0;
        self.docs[di].checkpoint_font("Add glyphs");
        let masters = self.docs[di].font.masters.len();
        for n in names {
            let n = n.trim();
            if n.is_empty() || self.docs[di].font.find(n).is_some() {
                continue;
            }
            let (name, u) = if n.chars().count() == 1 {
                let c = n.chars().next().unwrap();
                (glyphnames::name_for_unicode(c as u32), Some(c as u32))
            } else if let Some(h) = n.strip_prefix("U+").or_else(|| n.strip_prefix("u+")) {
                match u32::from_str_radix(h, 16) {
                    Ok(u) => (glyphnames::name_for_unicode(u), Some(u)),
                    Err(_) => (n.to_string(), None),
                }
            } else {
                (n.to_string(), glyphnames::unicode_for_name(n))
            };
            if self.docs[di].font.find(&name).is_some() {
                continue;
            }
            let w = (self.docs[di].font.info.upm * 0.6).round();
            self.docs[di].font.add_glyph(Glyph::new(&name, u.into_iter().collect(), masters, w));
            added += 1;
        }
        self.docs[di].view_key = u64::MAX;
        self.toast(ctx, format!("Added {added} glyph(s)"));
    }

    /// Build composite glyphs (e.g. Aacute = A + acutecomb) for selected glyphs, using anchors.
    pub fn build_glyphs(&mut self, ctx: &egui::Context) {
        let di = self.doc_idx();
        let targets: Vec<usize> = if self.in_glyph_tab() { self.current_glyph().into_iter().collect() } else { self.docs[di].selected() };
        self.docs[di].checkpoint_font("Build glyphs");
        let mut built = 0;
        let masters = self.docs[di].font.masters.len();
        for gi in targets {
            let name = self.docs[di].font.glyphs[gi].name.clone();
            let Some((base, marks)) = glyphnames::composite_recipe(&name) else { continue };
            let font = &self.docs[di].font;
            let Some(bi) = font.find(&base).or_else(|| font.find(base.trim_start_matches("dotless"))) else { continue };
            let base_name = font.glyphs[bi].name.clone();
            let mut layers = Vec::new();
            for m in 0..masters {
                let bl = font.glyphs[bi].layer(m);
                let mut l = Layer { width: bl.width, ..Default::default() };
                l.components.push(Component::new(&base_name, 0.0, 0.0));
                for (mark, anchor) in &marks {
                    let Some(mi) = font.find(mark) else { continue };
                    let ml = font.glyphs[mi].layer(m);
                    let ba = bl.anchors.iter().find(|a| &a.name == anchor);
                    let ma = ml.anchors.iter().find(|a| a.name == format!("_{anchor}"));
                    let (dx, dy) = match (ba, ma) {
                        (Some(b), Some(a)) => (b.x - a.x, b.y - a.y),
                        _ => {
                            // No anchors: centre the mark over the base.
                            let bb = font.layer_bounds(bi, m).unwrap_or([0.0, 0.0, bl.width, 0.0]);
                            let mb = font.layer_bounds(mi, m).unwrap_or([0.0, 0.0, 0.0, 0.0]);
                            let dx = (bb[0] + bb[2]) / 2.0 - (mb[0] + mb[2]) / 2.0;
                            let dy = if anchor == "top" { (bb[3] - font.masters[m].x_height).max(0.0) } else { 0.0 };
                            (dx, dy)
                        }
                    };
                    l.components.push(Component::new(mark, dx, dy));
                }
                layers.push(l);
            }
            let g = &mut self.docs[di].font.glyphs[gi];
            g.layers = layers;
            g.touch();
            built += 1;
        }
        self.toast(ctx, format!("Built {built} composite glyph(s)"));
    }

    pub fn decompose(&mut self) {
        let di = self.doc_idx();
        let targets: Vec<usize> = if self.in_glyph_tab() { self.current_glyph().into_iter().collect() } else { self.docs[di].selected() };
        if targets.is_empty() {
            return;
        }
        self.docs[di].checkpoint_font("Decompose");
        let m = self.docs[di].master;
        for gi in targets {
            let flat = self.docs[di].font.flat_contours(gi, m);
            let g = &mut self.docs[di].font.glyphs[gi];
            let l = g.layer_mut(m);
            l.contours = flat;
            l.components.clear();
            g.touch();
        }
    }

    pub fn set_flag(&mut self, flag: Option<usize>) {
        let di = self.doc_idx();
        let targets: Vec<usize> = if self.in_glyph_tab() { self.current_glyph().into_iter().collect() } else { self.docs[di].selected() };
        if targets.is_empty() {
            return;
        }
        self.docs[di].checkpoint_font("Flag");
        for gi in targets {
            self.docs[di].font.glyphs[gi].flag = flag;
        }
        self.docs[di].view_key = u64::MAX;
    }

    pub fn generate_unicodes(&mut self, ctx: &egui::Context) {
        let di = self.doc_idx();
        self.docs[di].checkpoint_font("Generate Unicodes");
        let mut n = 0;
        for g in &mut self.docs[di].font.glyphs {
            if g.unicodes.is_empty() {
                if let Some(u) = glyphnames::unicode_for_name(&g.name) {
                    g.unicodes.push(u);
                    n += 1;
                }
            }
        }
        self.toast(ctx, format!("Assigned {n} Unicode value(s)"));
    }

    pub fn sort_glyphs(&mut self, by: SortBy) {
        let di = self.doc_idx();
        self.docs[di].checkpoint_font("Sort glyphs");
        let f = &mut self.docs[di].font;
        let notdef = f.find(".notdef");
        let mut gs = std::mem::take(&mut f.glyphs);
        let nd = notdef.map(|i| gs.remove(i));
        match by {
            SortBy::Name => gs.sort_by(|a, b| a.name.cmp(&b.name)),
            SortBy::Unicode => gs.sort_by_key(|g| g.unicodes.first().copied().unwrap_or(u32::MAX)),
            SortBy::Category => gs.sort_by_key(|g| (glyphnames::category(&g.name, g.unicodes.first().copied()) as u8, g.unicodes.first().copied().unwrap_or(u32::MAX))),
            SortBy::Font => {}
        }
        if let Some(nd) = nd {
            gs.insert(0, nd);
        }
        f.glyphs = gs;
        f.rebuild_index();
        self.docs[di].sel.clear();
        self.docs[di].view_key = u64::MAX;
    }

    /// Apply contour operations to the current glyph (glyph tab) or selected glyphs.
    pub fn contour_op(&mut self, label: &str, op: fn(&mut Layer)) {
        if self.in_glyph_tab() {
            self.edit_layer(label, op);
        } else {
            self.edit_glyphs(label, |g, m| op(g.layer_mut(m)));
        }
    }

    pub fn set_sidebearings(&mut self, lsb: Option<f32>, rsb: Option<f32>, width: Option<f32>) {
        let di = self.doc_idx();
        let targets: Vec<usize> = if self.in_glyph_tab() { self.current_glyph().into_iter().collect() } else { self.docs[di].selected() };
        if targets.is_empty() {
            return;
        }
        if targets.len() == 1 {
            self.docs[di].checkpoint_glyph(targets[0], "Metrics");
        } else {
            self.docs[di].checkpoint_font("Metrics");
        }
        let m = self.docs[di].master;
        for gi in targets {
            apply_metrics(&mut self.docs[di].font, gi, m, lsb, rsb, width);
        }
    }

    /// Interpolate a font at a design-space location (needs two or more compatible masters).
    pub fn instance_font(&self, di: usize, loc: &[f32], name: &str) -> Font {
        let src = &self.docs[di].font;
        let mut f = src.clone();
        f.info.style = name.to_string();
        let (a, b, t) = interp_masters(src, loc);
        f.masters = vec![src.masters[a].clone()];
        f.masters[0].name = name.to_string();
        let ma = &src.masters[a];
        let mb = &src.masters[b];
        let lerp = |x: f32, y: f32| x + (y - x) * t;
        f.masters[0].ascender = lerp(ma.ascender, mb.ascender);
        f.masters[0].descender = lerp(ma.descender, mb.descender);
        f.masters[0].x_height = lerp(ma.x_height, mb.x_height);
        f.masters[0].cap_height = lerp(ma.cap_height, mb.cap_height);
        for g in &mut f.glyphs {
            let la = g.layer(a).clone();
            let lb = g.layer(b).clone();
            let l = if compatible(&la, &lb) { interpolate(&la, &lb, t) } else { la };
            g.layers = vec![l];
            g.touch();
        }
        f.path = None;
        f.dirty = true;
        f
    }
}

/// Pick the two masters bracketing a location on the first axis and the blend factor.
pub fn interp_masters(font: &Font, loc: &[f32]) -> (usize, usize, f32) {
    if font.masters.len() < 2 {
        return (0, 0, 0.0);
    }
    let x = loc.first().copied().unwrap_or(0.0);
    let pos = |m: &Master| m.location.first().copied().unwrap_or(0.0);
    let mut idx: Vec<usize> = (0..font.masters.len()).collect();
    idx.sort_by(|&i, &j| pos(&font.masters[i]).partial_cmp(&pos(&font.masters[j])).unwrap_or(std::cmp::Ordering::Equal));
    for w in idx.windows(2) {
        let (pa, pb) = (pos(&font.masters[w[0]]), pos(&font.masters[w[1]]));
        if x <= pb || w[1] == *idx.last().unwrap() {
            let t = if (pb - pa).abs() < 1e-6 { 0.0 } else { ((x - pa) / (pb - pa)).clamp(0.0, 1.0) };
            return (w[0], w[1], t);
        }
    }
    (idx[0], idx[1], 0.0)
}

pub fn apply_metrics(font: &mut Font, gi: usize, m: usize, lsb: Option<f32>, rsb: Option<f32>, width: Option<f32>) {
    let bounds = font.layer_bounds(gi, m);
    let g = &mut font.glyphs[gi];
    let l = g.layer_mut(m);
    if let Some(w) = width {
        l.width = w.max(0.0);
    }
    if let Some(b) = bounds {
        if let Some(v) = lsb {
            let dx = v - b[0];
            for c in &mut l.contours {
                for p in &mut c.points {
                    p.x += dx;
                }
            }
            for c in &mut l.components {
                c.t[4] += dx;
            }
            for a in &mut l.anchors {
                a.x += dx;
            }
            l.width += dx;
            if let Some(r) = rsb {
                l.width = b[2] + dx + r;
            }
        } else if let Some(r) = rsb {
            l.width = b[2] + r;
        }
    }
    g.touch();
}

/// Delete selected points (keeping curves where possible), components and anchors.
pub fn delete_selection(l: &mut Layer) {
    l.components.retain(|c| !c.sel);
    l.anchors.retain(|a| !a.sel);
    let mut out = Vec::new();
    for mut c in std::mem::take(&mut l.contours) {
        let on_sel: Vec<usize> = c.points.iter().enumerate().filter(|(_, p)| p.sel && p.t.on_curve()).map(|(i, _)| i).collect();
        let on_total = c.points.iter().filter(|p| p.t.on_curve()).count();
        if on_sel.len() == on_total {
            continue; // whole contour selected
        }
        if on_sel.is_empty() {
            // Selected off-curves only: turn their segments into lines.
            let segs = geom::segments(&c);
            for (si, s) in segs.iter().enumerate().rev() {
                if s.offs.iter().any(|&o| c.points[o].sel) {
                    geom::curve_to_line(&mut c, si);
                }
            }
            for p in &mut c.points {
                p.sel = false;
            }
            out.push(c);
            continue;
        }
        for &i in on_sel.iter().rev() {
            remove_node(&mut c, i);
        }
        if c.points.iter().filter(|p| p.t.on_curve()).count() >= 2 {
            out.push(c);
        }
    }
    l.contours = out;
}

/// Remove one on-curve node, merging its neighbouring segments.
pub fn remove_node(c: &mut Contour, i: usize) {
    let n = c.points.len();
    if i >= n || !c.points[i].t.on_curve() {
        return;
    }
    let prev = (i + n - 1) % n;
    let next = (i + 1) % n;
    let prev_off = c.points[prev].t == PtType::Off;
    let next_off = c.points[next].t == PtType::Off;
    let next_on = (1..n).map(|k| (i + k) % n).find(|&k| c.points[k].t.on_curve());
    if prev_off && next_off {
        // curve-curve: drop node, its incoming second control and outgoing first control.
        let mut rm = vec![i, prev, next];
        rm.sort_unstable();
        rm.dedup();
        for k in rm.into_iter().rev() {
            c.points.remove(k);
        }
    } else if prev_off {
        // curve then line: following segment becomes a curve with remaining controls.
        if let Some(no) = next_on {
            c.points[no].t = PtType::Curve;
        }
        c.points.remove(prev.max(i));
        c.points.remove(prev.min(i));
        // keep only one remaining control: fine as quadratic-like cubic approximation
    } else if next_off {
        c.points.remove(next.max(i));
        c.points.remove(next.min(i));
    } else {
        c.points.remove(i);
    }
    if !c.closed && !c.points.is_empty() {
        c.points[0].t = PtType::Move;
        while c.points.first().map(|p| p.t == PtType::Off).unwrap_or(false) {
            c.points.remove(0);
        }
        if let Some(p) = c.points.first_mut() {
            p.t = PtType::Move;
        }
    }
    if c.closed {
        if let Some(first_on) = c.points.iter().position(|p| p.t.on_curve()) {
            c.points.rotate_left(first_on);
        }
    }
}

// Contour operations used by menus and the Actions dialog.
pub fn op_reverse(l: &mut Layer) {
    let any = l.has_selection();
    for c in &mut l.contours {
        if !any || c.points.iter().any(|p| p.sel) {
            c.reverse();
        }
    }
}
pub fn op_direction(l: &mut Layer) {
    // PostScript direction: outer counter-clockwise, inner clockwise.
    let n = l.contours.len();
    for i in 0..n {
        let area = l.contours[i].signed_area();
        let pt = l.contours[i].points.iter().find(|p| p.t.on_curve()).map(|p| [p.x, p.y]);
        let depth = pt
            .map(|q| {
                (0..n)
                    .filter(|&j| j != i)
                    .filter(|&j| crate::geom::winding(std::slice::from_ref(&l.contours[j]), q) != 0)
                    .count()
            })
            .unwrap_or(0);
        let want_ccw = depth % 2 == 0;
        if (area > 0.0) != want_ccw {
            l.contours[i].reverse();
        }
    }
}
pub fn op_extremes(l: &mut Layer) {
    for c in &mut l.contours {
        geom::add_extremes(c);
    }
}
pub fn op_round(l: &mut Layer) {
    for c in &mut l.contours {
        geom::round_contour(c);
    }
    l.width = l.width.round();
    for a in &mut l.anchors {
        a.x = a.x.round();
        a.y = a.y.round();
    }
}
pub fn op_clean(l: &mut Layer) {
    for c in &mut l.contours {
        geom::clean_contour(c);
    }
    l.contours.retain(|c| c.points.iter().filter(|p| p.t.on_curve()).count() >= 2);
}
pub fn op_break(l: &mut Layer) {
    // Open a closed contour at the selected node.
    for c in &mut l.contours {
        if !c.closed {
            continue;
        }
        if let Some(i) = c.points.iter().position(|p| p.sel && p.t.on_curve()) {
            c.points.rotate_left(i);
            let first = c.points[0].clone();
            let mut end = first.clone();
            end.t = c.points[0].t;
            c.points[0].t = PtType::Move;
            c.points.push(end);
            c.closed = false;
            break;
        }
    }
}
pub fn op_join(l: &mut Layer) {
    for c in &mut l.contours {
        if !c.closed && c.points.len() > 2 {
            let first = c.points[0].clone();
            let last = c.points.last().unwrap().clone();
            if (first.x - last.x).abs() < 1.0 && (first.y - last.y).abs() < 1.0 {
                let t = last.t;
                c.points.pop();
                c.points[0].t = t;
            } else {
                c.points[0].t = PtType::Line;
            }
            c.closed = true;
        }
    }
}
pub fn op_set_start(l: &mut Layer) {
    for c in &mut l.contours {
        if let Some(i) = c.points.iter().position(|p| p.sel && p.t.on_curve()) {
            c.set_start(i);
        }
    }
}
pub fn op_to_curve(l: &mut Layer) {
    for c in &mut l.contours {
        let segs = geom::segments(c);
        for si in (0..segs.len()).rev() {
            let s = &segs[si];
            if s.offs.is_empty() && c.points[s.start].sel && c.points[s.end].sel {
                geom::line_to_curve(c, si);
            }
        }
    }
}
pub fn op_to_line(l: &mut Layer) {
    for c in &mut l.contours {
        let segs = geom::segments(c);
        for si in (0..segs.len()).rev() {
            let s = &segs[si];
            if !s.offs.is_empty() && c.points[s.start].sel && c.points[s.end].sel {
                geom::curve_to_line(c, si);
            }
        }
    }
}
pub fn op_toggle_smooth(l: &mut Layer) {
    for c in &mut l.contours {
        for p in &mut c.points {
            if p.sel && p.t.on_curve() {
                p.smooth = !p.smooth;
            }
        }
    }
}
pub fn op_to_cubic(l: &mut Layer) {
    // Turn quadratic segments into cubic curves (exact conversion for single-control quads).
    for c in &mut l.contours {
        let segs = geom::segments(c);
        let mut new_pts: Vec<Point> = Vec::new();
        if segs.is_empty() {
            continue;
        }
        new_pts.push(c.points[segs[0].start].clone());
        for s in &segs {
            let pieces = geom::segment_pieces(c, s);
            let endp = c.points[s.end].clone();
            for (k, piece) in pieces.iter().enumerate() {
                match piece {
                    geom::Piece::Quad(a, q, b) => {
                        let c1 = [a[0] + 2.0 / 3.0 * (q[0] - a[0]), a[1] + 2.0 / 3.0 * (q[1] - a[1])];
                        let c2 = [b[0] + 2.0 / 3.0 * (q[0] - b[0]), b[1] + 2.0 / 3.0 * (q[1] - b[1])];
                        new_pts.push(Point::new(c1[0], c1[1], PtType::Off));
                        new_pts.push(Point::new(c2[0], c2[1], PtType::Off));
                        let mut e = Point::new(b[0], b[1], PtType::Curve);
                        e.smooth = if k + 1 < pieces.len() { true } else { endp.smooth };
                        new_pts.push(e);
                    }
                    geom::Piece::Cubic(_, c1, c2, b) => {
                        new_pts.push(Point::new(c1[0], c1[1], PtType::Off));
                        new_pts.push(Point::new(c2[0], c2[1], PtType::Off));
                        let mut e = Point::new(b[0], b[1], PtType::Curve);
                        e.smooth = endp.smooth;
                        new_pts.push(e);
                    }
                    geom::Piece::Line(_, b) => {
                        let mut e = Point::new(b[0], b[1], PtType::Line);
                        e.smooth = endp.smooth;
                        new_pts.push(e);
                    }
                }
            }
        }
        if c.closed {
            let last = new_pts.pop().unwrap();
            new_pts[0].t = last.t;
            new_pts[0].smooth = last.smooth;
        } else {
            new_pts[0].t = PtType::Move;
        }
        c.points = new_pts;
    }
}
pub fn op_align_h(l: &mut Layer) {
    let sel = l.selected_points();
    if sel.is_empty() {
        return;
    }
    let y = sel.iter().map(|&(c, p)| l.contours[c].points[p].y).sum::<f32>() / sel.len() as f32;
    for (c, p) in sel {
        l.contours[c].points[p].y = y.round();
    }
}
pub fn op_align_v(l: &mut Layer) {
    let sel = l.selected_points();
    if sel.is_empty() {
        return;
    }
    let x = sel.iter().map(|&(c, p)| l.contours[c].points[p].x).sum::<f32>() / sel.len() as f32;
    for (c, p) in sel {
        l.contours[c].points[p].x = x.round();
    }
}
