//! The font model: a font has masters (each with its own metrics and kerning),
//! glyphs with one layer per master, classes and OpenType feature code.
//! Contours follow the UFO point model: on-curve points (line / curve / qcurve)
//! with off-curve control points in between, closed or open.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static REV: AtomicU64 = AtomicU64::new(1);

/// A fresh revision stamp; bumped whenever a glyph changes so caches refresh.
pub fn next_rev() -> u64 {
    REV.fetch_add(1, Ordering::Relaxed)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PtType {
    /// Start of an open contour.
    Move,
    Line,
    /// Cubic on-curve point (preceded by two off-curves).
    Curve,
    /// Quadratic on-curve point (preceded by one or more off-curves).
    QCurve,
    Off,
}

impl PtType {
    pub fn on_curve(self) -> bool {
        self != PtType::Off
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
    pub t: PtType,
    #[serde(default)]
    pub smooth: bool,
    #[serde(skip)]
    pub sel: bool,
}

impl Point {
    pub fn new(x: f32, y: f32, t: PtType) -> Self {
        Point { x, y, t, smooth: false, sel: false }
    }
    pub fn pos(&self) -> [f32; 2] {
        [self.x, self.y]
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Contour {
    pub points: Vec<Point>,
    #[serde(default = "yes")]
    pub closed: bool,
}

fn yes() -> bool {
    true
}

impl Contour {
    pub fn on_indices(&self) -> Vec<usize> {
        self.points.iter().enumerate().filter(|(_, p)| p.t.on_curve()).map(|(i, _)| i).collect()
    }

    /// Signed area of the polygon through all points (positive = counter-clockwise).
    pub fn signed_area(&self) -> f32 {
        let pts = crate::geom::flatten_contour(self, 8);
        let mut a = 0.0;
        for i in 0..pts.len() {
            let p = pts[i];
            let q = pts[(i + 1) % pts.len()];
            a += p[0] * q[1] - q[0] * p[1];
        }
        a * 0.5
    }

    /// Reverse the drawing direction, keeping the same start node.
    pub fn reverse(&mut self) {
        if self.points.len() < 2 {
            return;
        }
        if !self.closed {
            // Open: [Move, ..., last]; reversed the last point becomes Move and segment types shift.
            let n = self.points.len();
            let types: Vec<PtType> = self.points.iter().map(|p| p.t).collect();
            self.points.reverse();
            // Segment types belong to the segment end point; after reversal, the type of the
            // point at reversed index i comes from the original end of that segment.
            let mut new_types = vec![PtType::Off; n];
            let on: Vec<usize> = (0..n).filter(|&i| types[i] != PtType::Off).collect();
            for w in on.windows(2) {
                // original segment from w[0] to w[1] with type types[w[1]] now ends at n-1-w[0]
                new_types[n - 1 - w[0]] = types[w[1]];
            }
            new_types[0] = PtType::Move;
            for (p, t) in self.points.iter_mut().zip(new_types) {
                p.t = t;
            }
            return;
        }
        let n = self.points.len();
        let types: Vec<PtType> = self.points.iter().map(|p| p.t).collect();
        let on: Vec<usize> = (0..n).filter(|&i| types[i] != PtType::Off).collect();
        if on.is_empty() {
            self.points.reverse();
            return;
        }
        let mut new_types = vec![PtType::Off; n];
        // Segment ending at on[k] starts at on[k-1]; after reversal it ends at the old start.
        for k in 0..on.len() {
            let end = on[k];
            let start = on[(k + on.len() - 1) % on.len()];
            new_types[start] = types[end];
        }
        let mut pts = self.points.clone();
        for (p, t) in pts.iter_mut().zip(new_types) {
            p.t = t;
        }
        // Reverse order, keep the first on-curve point first.
        let first = on[0];
        let mut out = Vec::with_capacity(n);
        for k in 0..n {
            out.push(pts[(first + n - k) % n].clone());
        }
        self.points = out;
    }

    /// Make the on-curve point at `idx` the contour's first point.
    pub fn set_start(&mut self, idx: usize) {
        if !self.closed || idx >= self.points.len() || !self.points[idx].t.on_curve() {
            return;
        }
        self.points.rotate_left(idx);
        // A closed contour stores off-curves before their end point; the segment ending at the
        // new first point has its controls at the end of the list now, which is fine.
    }

    pub fn bounds(&self) -> Option<[f32; 4]> {
        crate::geom::contour_bounds(self)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Component {
    pub base: String,
    /// Affine transform: xx, xy, yx, yy, dx, dy.
    pub t: [f32; 6],
    #[serde(skip)]
    pub sel: bool,
}

impl Component {
    pub fn new(base: &str, dx: f32, dy: f32) -> Self {
        Component { base: base.to_string(), t: [1.0, 0.0, 0.0, 1.0, dx, dy], sel: false }
    }
    pub fn apply(&self, p: [f32; 2]) -> [f32; 2] {
        let t = self.t;
        [t[0] * p[0] + t[2] * p[1] + t[4], t[1] * p[0] + t[3] * p[1] + t[5]]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Anchor {
    pub name: String,
    pub x: f32,
    pub y: f32,
    #[serde(skip)]
    pub sel: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Guide {
    pub name: String,
    pub x: f32,
    pub y: f32,
    /// Degrees: 0 = horizontal line, 90 = vertical line.
    pub angle: f32,
    #[serde(default)]
    pub color: Option<[u8; 3]>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Layer {
    pub width: f32,
    #[serde(default)]
    pub contours: Vec<Contour>,
    #[serde(default)]
    pub components: Vec<Component>,
    #[serde(default)]
    pub anchors: Vec<Anchor>,
    #[serde(default)]
    pub guides: Vec<Guide>,
    /// Optional vertical advance.
    #[serde(default)]
    pub vwidth: Option<f32>,
}

impl Layer {
    pub fn is_empty(&self) -> bool {
        self.contours.is_empty() && self.components.is_empty()
    }
    pub fn node_count(&self) -> usize {
        self.contours.iter().map(|c| c.points.len()).sum()
    }
    pub fn clear_selection(&mut self) {
        for c in &mut self.contours {
            for p in &mut c.points {
                p.sel = false;
            }
        }
        for c in &mut self.components {
            c.sel = false;
        }
        for a in &mut self.anchors {
            a.sel = false;
        }
    }
    pub fn select_all(&mut self) {
        for c in &mut self.contours {
            for p in &mut c.points {
                p.sel = true;
            }
        }
        for c in &mut self.components {
            c.sel = true;
        }
        for a in &mut self.anchors {
            a.sel = true;
        }
    }
    pub fn has_selection(&self) -> bool {
        self.contours.iter().any(|c| c.points.iter().any(|p| p.sel))
            || self.components.iter().any(|c| c.sel)
            || self.anchors.iter().any(|a| a.sel)
    }
    pub fn selected_points(&self) -> Vec<(usize, usize)> {
        let mut v = Vec::new();
        for (ci, c) in self.contours.iter().enumerate() {
            for (pi, p) in c.points.iter().enumerate() {
                if p.sel {
                    v.push((ci, pi));
                }
            }
        }
        v
    }
    /// Move every selected item (points, components, anchors) by dx, dy.
    pub fn move_selection(&mut self, dx: f32, dy: f32) {
        for c in &mut self.contours {
            let n = c.points.len();
            // Off-curves next to a selected on-curve move with it.
            let mut mv = vec![false; n];
            for i in 0..n {
                if c.points[i].sel {
                    mv[i] = true;
                    if c.points[i].t.on_curve() {
                        let prev = (i + n - 1) % n;
                        let next = (i + 1) % n;
                        if (c.closed || i > 0) && c.points[prev].t == PtType::Off {
                            mv[prev] = true;
                        }
                        if (c.closed || i + 1 < n) && c.points[next].t == PtType::Off {
                            mv[next] = true;
                        }
                    }
                }
            }
            for i in 0..n {
                if mv[i] {
                    c.points[i].x += dx;
                    c.points[i].y += dy;
                }
            }
        }
        for c in &mut self.components {
            if c.sel {
                c.t[4] += dx;
                c.t[5] += dy;
            }
        }
        for a in &mut self.anchors {
            if a.sel {
                a.x += dx;
                a.y += dy;
            }
        }
    }
    /// Apply an affine transform to the selection (or everything when nothing is selected).
    pub fn transform(&mut self, m: [f32; 6], only_sel: bool) {
        let f = |x: f32, y: f32| [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]];
        let all = !only_sel || !self.has_selection();
        for c in &mut self.contours {
            let whole = all || c.points.iter().any(|p| p.sel);
            if !whole {
                continue;
            }
            let any_sel = c.points.iter().any(|p| p.sel);
            let n = c.points.len();
            let mut mv = vec![all; n];
            if !all && any_sel {
                for i in 0..n {
                    if c.points[i].sel {
                        mv[i] = true;
                        if c.points[i].t.on_curve() {
                            let prev = (i + n - 1) % n;
                            let next = (i + 1) % n;
                            if c.points[prev].t == PtType::Off {
                                mv[prev] = true;
                            }
                            if c.points[next].t == PtType::Off {
                                mv[next] = true;
                            }
                        }
                    }
                }
            }
            for i in 0..n {
                if mv[i] {
                    let q = f(c.points[i].x, c.points[i].y);
                    c.points[i].x = q[0];
                    c.points[i].y = q[1];
                }
            }
        }
        for c in &mut self.components {
            if all || c.sel {
                let t = c.t;
                // new = m * t
                c.t = [
                    m[0] * t[0] + m[2] * t[1],
                    m[1] * t[0] + m[3] * t[1],
                    m[0] * t[2] + m[2] * t[3],
                    m[1] * t[2] + m[3] * t[3],
                    m[0] * t[4] + m[2] * t[5] + m[4],
                    m[1] * t[4] + m[3] * t[5] + m[5],
                ];
            }
        }
        for a in &mut self.anchors {
            if all || a.sel {
                let q = f(a.x, a.y);
                a.x = q[0];
                a.y = q[1];
            }
        }
        // A mirror flips contour direction; restore it.
        let det = m[0] * m[3] - m[1] * m[2];
        if det < 0.0 {
            for c in &mut self.contours {
                if all || c.points.iter().any(|p| p.sel) {
                    c.reverse();
                }
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Glyph {
    pub name: String,
    #[serde(default)]
    pub unicodes: Vec<u32>,
    /// One layer per master, in master order.
    pub layers: Vec<Layer>,
    /// Colour flag (index into FLAG_COLORS).
    #[serde(default)]
    pub flag: Option<usize>,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Metrics expressions like "=H" or "=o+10" for LSB / RSB / width.
    #[serde(default)]
    pub lsb_expr: String,
    #[serde(default)]
    pub rsb_expr: String,
    #[serde(default)]
    pub width_expr: String,
    /// Kerning class membership is in Font::classes; these are explicit overrides.
    #[serde(skip)]
    pub rev: u64,
}

impl Glyph {
    pub fn new(name: &str, unicodes: Vec<u32>, masters: usize, width: f32) -> Self {
        Glyph {
            name: name.to_string(),
            unicodes,
            layers: (0..masters).map(|_| Layer { width, ..Default::default() }).collect(),
            flag: None,
            note: String::new(),
            tags: Vec::new(),
            lsb_expr: String::new(),
            rsb_expr: String::new(),
            width_expr: String::new(),
            rev: next_rev(),
        }
    }
    pub fn touch(&mut self) {
        self.rev = next_rev();
    }
    pub fn layer(&self, m: usize) -> &Layer {
        &self.layers[m.min(self.layers.len() - 1)]
    }
    pub fn layer_mut(&mut self, m: usize) -> &mut Layer {
        let n = self.layers.len();
        &mut self.layers[m.min(n - 1)]
    }
    pub fn unicode_str(&self) -> String {
        self.unicodes.first().map(|u| format!("{u:04X}")).unwrap_or_default()
    }
    pub fn char(&self) -> Option<char> {
        self.unicodes.first().and_then(|&u| char::from_u32(u))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Axis {
    pub name: String,
    pub tag: String,
    pub min: f32,
    pub default: f32,
    pub max: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KernPair {
    /// Glyph name or "@class" name.
    pub left: String,
    pub right: String,
    pub value: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Master {
    pub name: String,
    pub ascender: f32,
    pub descender: f32,
    pub x_height: f32,
    pub cap_height: f32,
    #[serde(default)]
    pub line_gap: f32,
    #[serde(default)]
    pub italic_angle: f32,
    #[serde(default = "def_ul_pos")]
    pub underline_position: f32,
    #[serde(default = "def_ul_th")]
    pub underline_thickness: f32,
    #[serde(default)]
    pub kerning: Vec<KernPair>,
    /// Location on each axis (same order as Font::axes).
    #[serde(default)]
    pub location: Vec<f32>,
    #[serde(default)]
    pub h_stems: Vec<f32>,
    #[serde(default)]
    pub v_stems: Vec<f32>,
    /// Alignment zones: (position, width). Negative width = bottom zone.
    #[serde(default)]
    pub zones: Vec<(f32, f32)>,
    #[serde(default)]
    pub guides: Vec<Guide>,
    #[serde(default)]
    pub weight_class: u16,
    #[serde(default)]
    pub width_class: u16,
}

fn def_ul_pos() -> f32 {
    -100.0
}
fn def_ul_th() -> f32 {
    50.0
}

impl Master {
    pub fn new(name: &str, upm: f32) -> Self {
        let s = upm / 1000.0;
        Master {
            name: name.to_string(),
            ascender: 750.0 * s,
            descender: -250.0 * s,
            x_height: 500.0 * s,
            cap_height: 700.0 * s,
            line_gap: 0.0,
            italic_angle: 0.0,
            underline_position: -100.0 * s,
            underline_thickness: 50.0 * s,
            kerning: Vec::new(),
            location: Vec::new(),
            h_stems: Vec::new(),
            v_stems: Vec::new(),
            zones: vec![(0.0, -15.0 * s), (500.0 * s, 15.0 * s), (700.0 * s, 15.0 * s), (750.0 * s, 15.0 * s), (-250.0 * s, -15.0 * s)],
            guides: Vec::new(),
            weight_class: 400,
            width_class: 5,
        }
    }
    pub fn kern_index(&self, l: &str, r: &str) -> Option<usize> {
        self.kerning.iter().position(|k| k.left == l && k.right == r)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Class {
    pub name: String,
    pub glyphs: Vec<String>,
    /// Kerning class sides.
    #[serde(default)]
    pub kern_left: bool,
    #[serde(default)]
    pub kern_right: bool,
    /// For kerning classes: the key glyph.
    #[serde(default)]
    pub key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Instance {
    pub name: String,
    pub location: Vec<f32>,
    #[serde(default)]
    pub export: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FontInfo {
    pub family: String,
    pub style: String,
    #[serde(default)]
    pub full_name: String,
    #[serde(default)]
    pub ps_name: String,
    #[serde(default)]
    pub style_group: String,
    pub upm: f32,
    #[serde(default = "one")]
    pub version_major: u16,
    #[serde(default)]
    pub version_minor: u16,
    #[serde(default)]
    pub designer: String,
    #[serde(default)]
    pub designer_url: String,
    #[serde(default)]
    pub manufacturer: String,
    #[serde(default)]
    pub manufacturer_url: String,
    #[serde(default)]
    pub vendor_id: String,
    #[serde(default)]
    pub copyright: String,
    #[serde(default)]
    pub trademark: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub license_url: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub weight_class: u16,
    #[serde(default)]
    pub width_class: u16,
    #[serde(default)]
    pub monospaced: bool,
    #[serde(default)]
    pub fs_type: u16,
    #[serde(default)]
    pub codepages: Vec<String>,
    #[serde(default)]
    pub created: String,
}

fn one() -> u16 {
    1
}

impl Default for FontInfo {
    fn default() -> Self {
        FontInfo {
            family: "Unnamed".into(),
            style: "Regular".into(),
            full_name: String::new(),
            ps_name: String::new(),
            style_group: "Regular".into(),
            upm: 1000.0,
            version_major: 1,
            version_minor: 0,
            designer: String::new(),
            designer_url: String::new(),
            manufacturer: String::new(),
            manufacturer_url: String::new(),
            vendor_id: "NONE".into(),
            copyright: String::new(),
            trademark: String::new(),
            license: String::new(),
            license_url: String::new(),
            description: String::new(),
            note: String::new(),
            weight_class: 400,
            width_class: 5,
            monospaced: false,
            fs_type: 0,
            codepages: vec!["1252 Latin 1".into()],
            created: String::new(),
        }
    }
}

impl FontInfo {
    pub fn full(&self) -> String {
        if self.full_name.is_empty() {
            format!("{} {}", self.family, self.style)
        } else {
            self.full_name.clone()
        }
    }
    pub fn postscript(&self) -> String {
        if self.ps_name.is_empty() {
            format!("{}-{}", self.family, self.style).replace(' ', "")
        } else {
            self.ps_name.clone()
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Font {
    pub info: FontInfo,
    pub masters: Vec<Master>,
    #[serde(default)]
    pub axes: Vec<Axis>,
    #[serde(default)]
    pub instances: Vec<Instance>,
    pub glyphs: Vec<Glyph>,
    #[serde(default)]
    pub classes: Vec<Class>,
    #[serde(default)]
    pub features: String,
    /// Arbitrary key/value data kept with the font (UFO lib equivalent).
    #[serde(default)]
    pub lib: BTreeMap<String, String>,
    #[serde(skip)]
    pub path: Option<PathBuf>,
    #[serde(skip)]
    pub dirty: bool,
    #[serde(skip)]
    index: HashMap<String, usize>,
}

pub const FLAG_COLORS: [(u8, u8, u8, &str); 8] = [
    (0xe5, 0x48, 0x4d, "Red"),
    (0xf0, 0xa2, 0x2e, "Orange"),
    (0xe8, 0xd0, 0x3a, "Yellow"),
    (0x3f, 0xb9, 0x6a, "Green"),
    (0x2f, 0xb5, 0xb0, "Teal"),
    (0x3b, 0x82, 0xf6, "Blue"),
    (0x9b, 0x59, 0xd0, "Purple"),
    (0x91, 0x96, 0x9f, "Grey"),
];

impl Font {
    pub fn new(family: &str, glyph_set: &[String]) -> Self {
        let info = FontInfo { family: family.to_string(), ..Default::default() };
        let masters = vec![Master::new("Regular", info.upm)];
        let mut f = Font {
            info,
            masters,
            axes: Vec::new(),
            instances: Vec::new(),
            glyphs: Vec::new(),
            classes: Vec::new(),
            features: String::new(),
            lib: BTreeMap::new(),
            path: None,
            dirty: false,
            index: HashMap::new(),
        };
        f.add_glyph(Glyph::new(".notdef", vec![], 1, 500.0));
        for name in glyph_set {
            let u = crate::glyphnames::unicode_for_name(name);
            let w = if *name == "space" { 250.0 } else { 600.0 };
            f.add_glyph(Glyph::new(name, u.into_iter().collect(), 1, w));
        }
        // A simple box for .notdef so the font always has something to show.
        {
            let g = &mut f.glyphs[0];
            let l = &mut g.layers[0];
            l.contours.push(crate::geom::rect_contour(50.0, -200.0, 400.0, 900.0, false));
            l.contours.push(crate::geom::rect_contour(100.0, -150.0, 300.0, 800.0, true));
        }
        f
    }

    pub fn rebuild_index(&mut self) {
        self.index = self.glyphs.iter().enumerate().map(|(i, g)| (g.name.clone(), i)).collect();
    }

    pub fn find(&self, name: &str) -> Option<usize> {
        if self.index.len() == self.glyphs.len() {
            if let Some(&i) = self.index.get(name) {
                if i < self.glyphs.len() && self.glyphs[i].name == name {
                    return Some(i);
                }
            }
        }
        self.glyphs.iter().position(|g| g.name == name)
    }

    pub fn glyph(&self, name: &str) -> Option<&Glyph> {
        self.find(name).map(|i| &self.glyphs[i])
    }

    pub fn find_unicode(&self, u: u32) -> Option<usize> {
        self.glyphs.iter().position(|g| g.unicodes.contains(&u))
    }

    pub fn add_glyph(&mut self, mut g: Glyph) -> usize {
        while g.layers.len() < self.masters.len() {
            let w = g.layers.first().map(|l| l.width).unwrap_or(600.0);
            g.layers.push(Layer { width: w, ..Default::default() });
        }
        g.touch();
        self.glyphs.push(g);
        let i = self.glyphs.len() - 1;
        self.index.insert(self.glyphs[i].name.clone(), i);
        self.dirty = true;
        i
    }

    pub fn unique_name(&self, base: &str) -> String {
        if self.find(base).is_none() {
            return base.to_string();
        }
        for n in 1.. {
            let c = format!("{base}.{n:03}");
            if self.find(&c).is_none() {
                return c;
            }
        }
        unreachable!()
    }

    pub fn rename_glyph(&mut self, idx: usize, new: &str) -> bool {
        if new.is_empty() || self.find(new).is_some() {
            return false;
        }
        let old = self.glyphs[idx].name.clone();
        self.glyphs[idx].name = new.to_string();
        // Update references: components, kerning, classes.
        for g in &mut self.glyphs {
            let mut changed = false;
            for l in &mut g.layers {
                for c in &mut l.components {
                    if c.base == old {
                        c.base = new.to_string();
                        changed = true;
                    }
                }
            }
            if changed {
                g.touch();
            }
        }
        for m in &mut self.masters {
            for k in &mut m.kerning {
                if k.left == old {
                    k.left = new.to_string();
                }
                if k.right == old {
                    k.right = new.to_string();
                }
            }
        }
        for c in &mut self.classes {
            for g in &mut c.glyphs {
                if *g == old {
                    *g = new.to_string();
                }
            }
            if c.key == old {
                c.key = new.to_string();
            }
        }
        self.rebuild_index();
        self.dirty = true;
        true
    }

    pub fn remove_glyphs(&mut self, idxs: &[usize]) {
        let mut v: Vec<usize> = idxs.to_vec();
        v.sort_unstable();
        v.dedup();
        for &i in v.iter().rev() {
            if i < self.glyphs.len() {
                self.glyphs.remove(i);
            }
        }
        self.rebuild_index();
        self.dirty = true;
    }

    /// Advance width of a glyph in a master.
    pub fn width(&self, gi: usize, m: usize) -> f32 {
        self.glyphs[gi].layer(m).width
    }

    /// Bounding box of a layer including components (recursively).
    pub fn layer_bounds(&self, gi: usize, m: usize) -> Option<[f32; 4]> {
        let paths = self.flat_contours(gi, m);
        let mut b: Option<[f32; 4]> = None;
        for c in &paths {
            if let Some(cb) = c.bounds() {
                b = Some(match b {
                    None => cb,
                    Some(o) => [o[0].min(cb[0]), o[1].min(cb[1]), o[2].max(cb[2]), o[3].max(cb[3])],
                });
            }
        }
        b
    }

    pub fn lsb(&self, gi: usize, m: usize) -> f32 {
        self.layer_bounds(gi, m).map(|b| b[0]).unwrap_or(0.0)
    }

    pub fn rsb(&self, gi: usize, m: usize) -> f32 {
        let w = self.width(gi, m);
        self.layer_bounds(gi, m).map(|b| w - b[2]).unwrap_or(w)
    }

    /// Contours of a layer with components decomposed into outlines.
    pub fn flat_contours(&self, gi: usize, m: usize) -> Vec<Contour> {
        let mut out = Vec::new();
        self.collect_contours(gi, m, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], 0, &mut out);
        out
    }

    fn collect_contours(&self, gi: usize, m: usize, t: [f32; 6], depth: usize, out: &mut Vec<Contour>) {
        if depth > 8 || gi >= self.glyphs.len() {
            return;
        }
        let l = self.glyphs[gi].layer(m);
        let identity = t == [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
        for c in &l.contours {
            if identity {
                out.push(c.clone());
            } else {
                let mut c2 = c.clone();
                for p in &mut c2.points {
                    let (x, y) = (p.x, p.y);
                    p.x = t[0] * x + t[2] * y + t[4];
                    p.y = t[1] * x + t[3] * y + t[5];
                }
                if t[0] * t[3] - t[1] * t[2] < 0.0 {
                    c2.reverse();
                }
                out.push(c2);
            }
        }
        for comp in &l.components {
            if let Some(bi) = self.find(&comp.base) {
                let ct = comp.t;
                let nt = [
                    t[0] * ct[0] + t[2] * ct[1],
                    t[1] * ct[0] + t[3] * ct[1],
                    t[0] * ct[2] + t[2] * ct[3],
                    t[1] * ct[2] + t[3] * ct[3],
                    t[0] * ct[4] + t[2] * ct[5] + t[4],
                    t[1] * ct[4] + t[3] * ct[5] + t[5],
                ];
                self.collect_contours(bi, m, nt, depth + 1, out);
            }
        }
    }

    /// Revision stamp covering a glyph and everything it references.
    pub fn deep_rev(&self, gi: usize, depth: usize) -> u64 {
        let g = &self.glyphs[gi];
        let mut r = g.rev;
        if depth < 6 {
            for l in &g.layers {
                for c in &l.components {
                    if let Some(bi) = self.find(&c.base) {
                        r = r.max(self.deep_rev(bi, depth + 1)).wrapping_add(bi as u64);
                    }
                }
            }
        }
        r
    }

    /// Classes a glyph belongs to for kerning on the given side.
    pub fn kern_class_of(&self, glyph: &str, left_side: bool) -> Option<&Class> {
        self.classes.iter().find(|c| (if left_side { c.kern_left } else { c.kern_right }) && c.glyphs.iter().any(|g| g == glyph))
    }

    /// Kerning value between two glyphs, honouring class kerning; glyph-glyph pairs win.
    pub fn kerning(&self, m: usize, left: &str, right: &str) -> Option<(f32, String, String)> {
        let master = &self.masters[m.min(self.masters.len() - 1)];
        let lc = self.kern_class_of(left, true).map(|c| format!("@{}", c.name));
        let rc = self.kern_class_of(right, false).map(|c| format!("@{}", c.name));
        let candidates = [
            (left.to_string(), right.to_string()),
            (left.to_string(), rc.clone().unwrap_or_default()),
            (lc.clone().unwrap_or_default(), right.to_string()),
            (lc.unwrap_or_default(), rc.unwrap_or_default()),
        ];
        for (l, r) in candidates {
            if l.is_empty() || r.is_empty() {
                continue;
            }
            if let Some(k) = master.kerning.iter().find(|k| k.left == l && k.right == r) {
                return Some((k.value, l, r));
            }
        }
        None
    }

    pub fn set_kerning(&mut self, m: usize, left: &str, right: &str, value: f32) {
        let master = &mut self.masters[m];
        if let Some(i) = master.kern_index(left, right) {
            if value == 0.0 {
                master.kerning.remove(i);
            } else {
                master.kerning[i].value = value;
            }
        } else if value != 0.0 {
            master.kerning.push(KernPair { left: left.into(), right: right.into(), value });
        }
        self.dirty = true;
    }

    pub fn add_master(&mut self, name: &str, copy_from: usize) {
        let mut m = self.masters[copy_from].clone();
        m.name = name.to_string();
        self.masters.push(m);
        for g in &mut self.glyphs {
            let l = g.layer(copy_from).clone();
            g.layers.push(l);
            g.touch();
        }
        self.dirty = true;
    }

    pub fn remove_master(&mut self, idx: usize) {
        if self.masters.len() <= 1 {
            return;
        }
        self.masters.remove(idx);
        for g in &mut self.glyphs {
            if idx < g.layers.len() {
                g.layers.remove(idx);
            }
            g.touch();
        }
        self.dirty = true;
    }

    pub fn title(&self) -> String {
        self.info.full()
    }
}

/// Make two layers compatible check: same contour count and point structure.
pub fn compatible(a: &Layer, b: &Layer) -> bool {
    a.contours.len() == b.contours.len()
        && a.components.len() == b.components.len()
        && a.contours.iter().zip(&b.contours).all(|(x, y)| x.points.len() == y.points.len() && x.points.iter().zip(&y.points).all(|(p, q)| p.t.on_curve() == q.t.on_curve()))
}

/// Linear interpolation between two compatible layers.
pub fn interpolate(a: &Layer, b: &Layer, t: f32) -> Layer {
    let mut out = a.clone();
    out.width = a.width + (b.width - a.width) * t;
    for (c, cb) in out.contours.iter_mut().zip(&b.contours) {
        for (p, q) in c.points.iter_mut().zip(&cb.points) {
            p.x += (q.x - p.x) * t;
            p.y += (q.y - p.y) * t;
        }
    }
    for (c, cb) in out.components.iter_mut().zip(&b.components) {
        for k in 0..6 {
            c.t[k] += (cb.t[k] - c.t[k]) * t;
        }
    }
    for (an, ab) in out.anchors.iter_mut().zip(&b.anchors) {
        an.x += (ab.x - an.x) * t;
        an.y += (ab.y - an.y) * t;
    }
    out
}
