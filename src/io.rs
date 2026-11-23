//! Reading and writing fonts: native .fstudio (JSON), TrueType/OpenType import,
//! UFO 3 import/export (plus .designspace for several masters), TrueType export,
//! and the small text formats for kerning, classes and features.

#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use crate::font::*;
use crate::geom;
use crate::glyphnames;

pub const NATIVE_EXT: &str = "fstudio";
pub const OPEN_EXTS: [&str; 6] = ["fstudio", "ttf", "otf", "ttc", "otc", "ufo"];

pub fn open_any(path: &Path) -> Result<Font, String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let mut f = match ext.as_str() {
        "fstudio" | "json" => load_native(path)?,
        "ufo" => import_ufo(path)?,
        "designspace" => import_designspace(path)?,
        "ttf" | "otf" | "ttc" | "otc" => import_sfnt(path, 0)?,
        _ => {
            if path.is_dir() && path.join("metainfo.plist").exists() {
                import_ufo(path)?
            } else {
                return Err(format!("Unsupported file type: .{ext}"));
            }
        }
    };
    f.rebuild_index();
    for g in &mut f.glyphs {
        g.touch();
    }
    if ext == "fstudio" {
        f.path = Some(path.to_path_buf());
    }
    f.dirty = false;
    Ok(f)
}

// ------------------------------------------------------------------------------------------
// Native

pub fn load_native(path: &Path) -> Result<Font, String> {
    let s = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&s).map_err(|e| format!("Not a Fontabulous Studio file: {e}"))
}

pub fn save_native(font: &Font, path: &Path) -> Result<(), String> {
    let s = serde_json::to_string_pretty(font).map_err(|e| e.to_string())?;
    std::fs::write(path, s).map_err(|e| e.to_string())
}

// ------------------------------------------------------------------------------------------
// TrueType / OpenType import

struct Pen {
    contours: Vec<Contour>,
    cur: Vec<Point>,
}

impl Pen {
    fn finish(&mut self) {
        if self.cur.is_empty() {
            return;
        }
        let mut pts = std::mem::take(&mut self.cur);
        // The path returns to its start: drop the duplicate closing point and give its
        // segment type to the start point.
        if pts.len() > 1 {
            let last = pts.last().unwrap().clone();
            if (last.x - pts[0].x).abs() < 0.001 && (last.y - pts[0].y).abs() < 0.001 {
                pts[0].t = last.t;
                pts.pop();
            } else {
                pts[0].t = PtType::Line;
            }
        }
        if pts[0].t == PtType::Move {
            pts[0].t = PtType::Line;
        }
        self.contours.push(Contour { points: pts, closed: true });
    }
}

impl ttf_parser::OutlineBuilder for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.finish();
        self.cur.push(Point::new(x, y, PtType::Move));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.cur.push(Point::new(x, y, PtType::Line));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.cur.push(Point::new(x1, y1, PtType::Off));
        self.cur.push(Point::new(x, y, PtType::QCurve));
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.cur.push(Point::new(x1, y1, PtType::Off));
        self.cur.push(Point::new(x2, y2, PtType::Off));
        self.cur.push(Point::new(x, y, PtType::Curve));
    }
    fn close(&mut self) {
        self.finish();
    }
}

fn mark_smooth(c: &mut Contour) {
    let n = c.points.len();
    if n < 3 {
        return;
    }
    for i in 0..n {
        if !c.points[i].t.on_curve() {
            continue;
        }
        let a = &c.points[(i + n - 1) % n];
        let b = &c.points[(i + 1) % n];
        if a.t == PtType::Off || b.t == PtType::Off {
            let p = &c.points[i];
            let v1 = [p.x - a.x, p.y - a.y];
            let v2 = [b.x - p.x, b.y - p.y];
            let cross = v1[0] * v2[1] - v1[1] * v2[0];
            let l = (v1[0].hypot(v1[1]) * v2[0].hypot(v2[1])).max(1e-6);
            let dot = v1[0] * v2[0] + v1[1] * v2[1];
            if (cross / l).abs() < 0.02 && dot > 0.0 {
                c.points[i].smooth = true;
            }
        }
    }
}

pub fn import_sfnt(path: &Path, index: u32) -> Result<Font, String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    let face = ttf_parser::Face::parse(&data, index).map_err(|e| format!("Could not read font: {e}"))?;
    let mut names: HashMap<u16, String> = HashMap::new();
    for n in face.names() {
        if let Some(s) = n.to_string() {
            names.entry(n.name_id).or_insert(s);
        }
    }
    let get = |id: u16| names.get(&id).cloned().unwrap_or_default();
    let upm = face.units_per_em() as f32;
    let mut info = FontInfo {
        family: if !get(16).is_empty() { get(16) } else { get(1) },
        style: if !get(17).is_empty() { get(17) } else { get(2) },
        full_name: get(4),
        ps_name: get(6),
        upm,
        copyright: get(0),
        trademark: get(7),
        manufacturer: get(8),
        designer: get(9),
        description: get(10),
        manufacturer_url: get(11),
        designer_url: get(12),
        license: get(13),
        license_url: get(14),
        weight_class: face.weight().to_number(),
        width_class: face.width().to_number(),
        monospaced: face.is_monospaced(),
        ..Default::default()
    };
    if info.family.is_empty() {
        info.family = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Imported".into());
    }
    if info.style.is_empty() {
        info.style = "Regular".into();
    }
    info.style_group = info.style.clone();
    let ver = get(5);
    if let Some(v) = ver.split_whitespace().find_map(|w| w.parse::<f32>().ok()) {
        info.version_major = v.trunc() as u16;
        info.version_minor = ((v.fract() * 1000.0).round()) as u16;
    }
    let mut master = Master::new(&info.style, upm);
    master.ascender = face.ascender() as f32;
    master.descender = face.descender() as f32;
    master.line_gap = face.line_gap() as f32;
    master.x_height = face.x_height().map(|v| v as f32).unwrap_or(upm * 0.5);
    master.cap_height = face.capital_height().map(|v| v as f32).unwrap_or(upm * 0.7);
    master.italic_angle = face.italic_angle();
    if let Some(u) = face.underline_metrics() {
        master.underline_position = u.position as f32;
        master.underline_thickness = u.thickness as f32;
    }
    master.weight_class = info.weight_class;
    master.width_class = info.width_class;
    master.zones = vec![
        (0.0, -15.0),
        (master.x_height, 15.0),
        (master.cap_height, 15.0),
        (master.ascender, 15.0),
        (master.descender, -15.0),
    ];

    // Unicode map.
    let mut umap: HashMap<u16, Vec<u32>> = HashMap::new();
    if let Some(cmap) = face.tables().cmap {
        for st in cmap.subtables {
            if !st.is_unicode() {
                continue;
            }
            st.codepoints(|cp| {
                if let Some(g) = st.glyph_index(cp) {
                    let v = umap.entry(g.0).or_default();
                    if !v.contains(&cp) {
                        v.push(cp);
                    }
                }
            });
        }
    }
    let n = face.number_of_glyphs();
    let mut glyphs = Vec::with_capacity(n as usize);
    let mut used: HashMap<String, usize> = HashMap::new();
    for gid in 0..n {
        let id = ttf_parser::GlyphId(gid);
        let mut us = umap.remove(&gid).unwrap_or_default();
        us.sort_unstable();
        let mut name = face.glyph_name(id).map(|s| s.to_string()).unwrap_or_default();
        if name.is_empty() {
            name = if gid == 0 { ".notdef".into() } else if let Some(&u) = us.first() { glyphnames::name_for_unicode(u) } else { format!("glyph{gid:05}") };
        }
        let cnt = used.entry(name.clone()).or_insert(0);
        if *cnt > 0 {
            name = format!("{name}.{cnt}");
        }
        *cnt += 1;
        let mut pen = Pen { contours: Vec::new(), cur: Vec::new() };
        face.outline_glyph(id, &mut pen);
        pen.finish();
        for c in &mut pen.contours {
            mark_smooth(c);
        }
        let width = face.glyph_hor_advance(id).unwrap_or(0) as f32;
        let mut g = Glyph::new(&name, us, 1, width);
        g.layers[0].contours = pen.contours;
        glyphs.push(g);
    }

    let mut font = Font::new(&info.family, &[]);
    font.glyphs.clear();
    font.info = info;
    font.masters = vec![master];
    for g in glyphs {
        font.glyphs.push(g);
    }
    font.rebuild_index();
    read_gpos_kerning(&face, &mut font);
    if font.masters[0].kerning.is_empty() {
        read_kern_table(&face, &mut font);
    }
    font.features = "# Feature code was not decompiled from the binary font.\n# Kerning was imported into the Kerning panel.\n".into();
    font.lib.insert("imported.from".into(), path.display().to_string());
    Ok(font)
}

fn read_kern_table(face: &ttf_parser::Face, font: &mut Font) {
    let Some(kern) = face.tables().kern else { return };
    let n = face.number_of_glyphs();
    if n > 1500 {
        return;
    }
    for st in kern.subtables {
        if !st.horizontal || st.variable {
            continue;
        }
        for l in 0..n {
            for r in 0..n {
                if let Some(v) = st.glyphs_kerning(ttf_parser::GlyphId(l), ttf_parser::GlyphId(r)) {
                    if v != 0 {
                        let ln = font.glyphs[l as usize].name.clone();
                        let rn = font.glyphs[r as usize].name.clone();
                        font.masters[0].kerning.push(KernPair { left: ln, right: rn, value: v as f32 });
                    }
                }
            }
        }
        break;
    }
}

fn read_gpos_kerning(face: &ttf_parser::Face, font: &mut Font) {
    use ttf_parser::gpos::{PairAdjustment, PositioningSubtable};
    let Some(gpos) = face.tables().gpos else { return };
    let n = face.number_of_glyphs();
    let names: Vec<String> = font.glyphs.iter().map(|g| g.name.clone()).collect();
    let mut pairs: Vec<KernPair> = Vec::new();
    let mut seen: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    let mut class_no = 0usize;
    for (li, lookup) in gpos.lookups.into_iter().enumerate() {
        for si in 0..lookup.subtables.len() {
            let Some(PositioningSubtable::Pair(pa)) = lookup.subtables.get::<PositioningSubtable>(si) else { continue };
            match pa {
                PairAdjustment::Format1 { coverage, sets } => {
                    if n > 3000 {
                        continue;
                    }
                    for l in 0..n {
                        let Some(ci) = coverage.get(ttf_parser::GlyphId(l)) else { continue };
                        let Some(set) = sets.get(ci) else { continue };
                        for r in 0..n {
                            if let Some((v1, _)) = set.get(ttf_parser::GlyphId(r)) {
                                if v1.x_advance != 0 {
                                    let key = (names[l as usize].clone(), names[r as usize].clone());
                                    if seen.insert(key.clone()) {
                                        pairs.push(KernPair { left: key.0, right: key.1, value: v1.x_advance as f32 });
                                    }
                                }
                            }
                        }
                    }
                }
                PairAdjustment::Format2 { coverage, classes, matrix } => {
                    let mut left: BTreeMap<u16, Vec<String>> = BTreeMap::new();
                    let mut right: BTreeMap<u16, Vec<String>> = BTreeMap::new();
                    for g in 0..n {
                        let id = ttf_parser::GlyphId(g);
                        if coverage.contains(id) {
                            left.entry(classes.0.get(id)).or_default().push(names[g as usize].clone());
                        }
                        let c2 = classes.1.get(id);
                        if c2 != 0 {
                            right.entry(c2).or_default().push(names[g as usize].clone());
                        }
                    }
                    let base = class_no;
                    class_no += 1;
                    let lname = |c: u16| format!("kern{li}_{base}_L{c}");
                    let rname = |c: u16| format!("kern{li}_{base}_R{c}");
                    for (&c1, lg) in &left {
                        for (&c2, rg) in &right {
                            if let Some((v1, _)) = matrix.get((c1, c2)) {
                                if v1.x_advance == 0 {
                                    continue;
                                }
                                let (ln, rn) = (lname(c1), rname(c2));
                                if !font.classes.iter().any(|c| c.name == ln) {
                                    font.classes.push(Class { name: ln.clone(), glyphs: lg.clone(), kern_left: true, kern_right: false, key: lg[0].clone() });
                                }
                                if !font.classes.iter().any(|c| c.name == rn) {
                                    font.classes.push(Class { name: rn.clone(), glyphs: rg.clone(), kern_left: false, kern_right: true, key: rg[0].clone() });
                                }
                                let key = (format!("@{ln}"), format!("@{rn}"));
                                if seen.insert(key.clone()) {
                                    pairs.push(KernPair { left: key.0, right: key.1, value: v1.x_advance as f32 });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    // A glyph may only sit in one kerning class per side for our model; drop duplicates.
    let mut taken_l: std::collections::HashSet<String> = Default::default();
    let mut taken_r: std::collections::HashSet<String> = Default::default();
    for c in &mut font.classes {
        let taken = if c.kern_left { &mut taken_l } else { &mut taken_r };
        c.glyphs.retain(|g| taken.insert(g.clone()));
    }
    font.classes.retain(|c| !c.glyphs.is_empty());
    let valid: std::collections::HashSet<String> = font.classes.iter().map(|c| format!("@{}", c.name)).collect();
    pairs.retain(|p| (!p.left.starts_with('@') || valid.contains(&p.left)) && (!p.right.starts_with('@') || valid.contains(&p.right)));
    font.masters[0].kerning = pairs;
}

// ------------------------------------------------------------------------------------------
// Property lists (for UFO)

#[derive(Clone, Debug)]
pub enum PV {
    Str(String),
    Num(f64),
    Bool(bool),
    Arr(Vec<PV>),
    Dict(Vec<(String, PV)>),
}

impl PV {
    pub fn get(&self, k: &str) -> Option<&PV> {
        match self {
            PV::Dict(v) => v.iter().find(|(kk, _)| kk == k).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn str(&self) -> Option<String> {
        match self {
            PV::Str(s) => Some(s.clone()),
            PV::Num(n) => Some(n.to_string()),
            _ => None,
        }
    }
    pub fn num(&self) -> Option<f64> {
        match self {
            PV::Num(n) => Some(*n),
            PV::Str(s) => s.parse().ok(),
            _ => None,
        }
    }
}

fn parse_plist_node(n: roxmltree::Node) -> Option<PV> {
    match n.tag_name().name() {
        "string" => Some(PV::Str(n.text().unwrap_or("").to_string())),
        "integer" | "real" => n.text().and_then(|t| t.trim().parse().ok()).map(PV::Num),
        "true" => Some(PV::Bool(true)),
        "false" => Some(PV::Bool(false)),
        "array" => Some(PV::Arr(n.children().filter(|c| c.is_element()).filter_map(parse_plist_node).collect())),
        "dict" => {
            let mut out = Vec::new();
            let mut key: Option<String> = None;
            for c in n.children().filter(|c| c.is_element()) {
                if c.tag_name().name() == "key" {
                    key = Some(c.text().unwrap_or("").to_string());
                } else if let Some(k) = key.take() {
                    if let Some(v) = parse_plist_node(c) {
                        out.push((k, v));
                    }
                }
            }
            Some(PV::Dict(out))
        }
        _ => None,
    }
}

pub fn read_plist(path: &Path) -> Option<PV> {
    let s = std::fs::read_to_string(path).ok()?;
    let doc = roxmltree::Document::parse_with_options(&s, roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() }).ok()?;
    let root = doc.root_element();
    let first = root.children().find(|c| c.is_element())?;
    parse_plist_node(first)
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn write_pv(out: &mut String, v: &PV, indent: usize) {
    let pad = "  ".repeat(indent);
    match v {
        PV::Str(s) => out.push_str(&format!("{pad}<string>{}</string>\n", esc(s))),
        PV::Num(n) => {
            if n.fract() == 0.0 && n.abs() < 1e12 {
                out.push_str(&format!("{pad}<integer>{}</integer>\n", *n as i64))
            } else {
                out.push_str(&format!("{pad}<real>{n}</real>\n"))
            }
        }
        PV::Bool(b) => out.push_str(&format!("{pad}<{}/>\n", if *b { "true" } else { "false" })),
        PV::Arr(a) => {
            out.push_str(&format!("{pad}<array>\n"));
            for x in a {
                write_pv(out, x, indent + 1);
            }
            out.push_str(&format!("{pad}</array>\n"));
        }
        PV::Dict(d) => {
            out.push_str(&format!("{pad}<dict>\n"));
            for (k, x) in d {
                out.push_str(&format!("{pad}  <key>{}</key>\n", esc(k)));
                write_pv(out, x, indent + 1);
            }
            out.push_str(&format!("{pad}</dict>\n"));
        }
    }
}

pub fn plist_string(v: &PV) -> String {
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n");
    write_pv(&mut s, v, 0);
    s.push_str("</plist>\n");
    s
}

// ------------------------------------------------------------------------------------------
// UFO import

pub fn parse_glif(s: &str) -> Option<Glyph> {
    let doc = roxmltree::Document::parse(s).ok()?;
    let root = doc.root_element();
    let name = root.attribute("name")?.to_string();
    let mut g = Glyph::new(&name, vec![], 1, 0.0);
    let l = &mut g.layers[0];
    let num = |n: &roxmltree::Node, a: &str| n.attribute(a).and_then(|v| v.parse::<f32>().ok());
    for n in root.children().filter(|c| c.is_element()) {
        match n.tag_name().name() {
            "advance" => {
                l.width = num(&n, "width").unwrap_or(0.0);
                l.vwidth = num(&n, "height");
            }
            "unicode" => {
                if let Some(h) = n.attribute("hex").and_then(|h| u32::from_str_radix(h, 16).ok()) {
                    g.unicodes.push(h);
                }
            }
            "anchor" => l.anchors.push(Anchor { name: n.attribute("name").unwrap_or("").into(), x: num(&n, "x").unwrap_or(0.0), y: num(&n, "y").unwrap_or(0.0), sel: false }),
            "guideline" => l.guides.push(Guide {
                name: n.attribute("name").unwrap_or("").into(),
                x: num(&n, "x").unwrap_or(0.0),
                y: num(&n, "y").unwrap_or(0.0),
                angle: num(&n, "angle").unwrap_or(if n.attribute("x").is_some() && n.attribute("y").is_none() { 90.0 } else { 0.0 }),
                color: None,
            }),
            "note" => g.note = n.text().unwrap_or("").trim().to_string(),
            "outline" => {
                for c in n.children().filter(|c| c.is_element()) {
                    match c.tag_name().name() {
                        "contour" => {
                            let mut pts = Vec::new();
                            for p in c.children().filter(|p| p.is_element() && p.tag_name().name() == "point") {
                                let t = match p.attribute("type").unwrap_or("offcurve") {
                                    "move" => PtType::Move,
                                    "line" => PtType::Line,
                                    "curve" => PtType::Curve,
                                    "qcurve" => PtType::QCurve,
                                    _ => PtType::Off,
                                };
                                let mut pt = Point::new(num(&p, "x").unwrap_or(0.0), num(&p, "y").unwrap_or(0.0), t);
                                pt.smooth = p.attribute("smooth") == Some("yes");
                                pts.push(pt);
                            }
                            if pts.is_empty() {
                                continue;
                            }
                            let closed = pts[0].t != PtType::Move;
                            // Normalise: start a closed contour on an on-curve point.
                            if closed {
                                if let Some(first_on) = pts.iter().position(|p| p.t.on_curve()) {
                                    pts.rotate_left(first_on);
                                } else {
                                    // All off-curve (TrueType): insert an implied on-curve point.
                                    let a = pts[0].clone();
                                    let b = pts[1 % pts.len()].clone();
                                    pts.insert(1, Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0, PtType::QCurve));
                                    pts.rotate_left(1);
                                }
                            }
                            l.contours.push(Contour { points: pts, closed });
                        }
                        "component" => {
                            let f = |a: &str, d: f32| c.attribute(a).and_then(|v| v.parse::<f32>().ok()).unwrap_or(d);
                            l.components.push(Component {
                                base: c.attribute("base").unwrap_or("").into(),
                                t: [f("xScale", 1.0), f("xyScale", 0.0), f("yxScale", 0.0), f("yScale", 1.0), f("xOffset", 0.0), f("yOffset", 0.0)],
                                sel: false,
                            });
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    Some(g)
}

fn ufo_info(font: &mut Font, master: &mut Master, info: &PV) {
    let s = |k: &str| info.get(k).and_then(|v| v.str()).unwrap_or_default();
    let n = |k: &str| info.get(k).and_then(|v| v.num());
    let fi = &mut font.info;
    if !s("familyName").is_empty() {
        fi.family = s("familyName");
    }
    if !s("styleName").is_empty() {
        fi.style = s("styleName");
    }
    fi.copyright = s("copyright");
    fi.trademark = s("trademark");
    fi.designer = s("openTypeNameDesigner");
    fi.designer_url = s("openTypeNameDesignerURL");
    fi.manufacturer = s("openTypeNameManufacturer");
    fi.manufacturer_url = s("openTypeNameManufacturerURL");
    fi.license = s("openTypeNameLicense");
    fi.license_url = s("openTypeNameLicenseURL");
    fi.description = s("openTypeNameDescription");
    fi.note = s("note");
    fi.vendor_id = s("openTypeOS2VendorID");
    fi.ps_name = s("postscriptFontName");
    fi.full_name = s("postscriptFullName");
    fi.style_group = s("styleMapStyleName");
    if let Some(v) = n("unitsPerEm") {
        fi.upm = v as f32;
    }
    if let Some(v) = n("versionMajor") {
        fi.version_major = v as u16;
    }
    if let Some(v) = n("versionMinor") {
        fi.version_minor = v as u16;
    }
    if let Some(v) = n("openTypeOS2WeightClass") {
        fi.weight_class = v as u16;
        master.weight_class = v as u16;
    }
    if let Some(v) = n("openTypeOS2WidthClass") {
        fi.width_class = v as u16;
        master.width_class = v as u16;
    }
    master.name = fi.style.clone();
    if let Some(v) = n("ascender") {
        master.ascender = v as f32;
    }
    if let Some(v) = n("descender") {
        master.descender = v as f32;
    }
    if let Some(v) = n("xHeight") {
        master.x_height = v as f32;
    }
    if let Some(v) = n("capHeight") {
        master.cap_height = v as f32;
    }
    if let Some(v) = n("italicAngle") {
        master.italic_angle = v as f32;
    }
    if let Some(v) = n("postscriptUnderlinePosition") {
        master.underline_position = v as f32;
    }
    if let Some(v) = n("postscriptUnderlineThickness") {
        master.underline_thickness = v as f32;
    }
    if let Some(v) = n("openTypeHheaLineGap") {
        master.line_gap = v as f32;
    }
    if let Some(PV::Arr(a)) = info.get("postscriptStemSnapH") {
        master.h_stems = a.iter().filter_map(|v| v.num()).map(|v| v as f32).collect();
    }
    if let Some(PV::Arr(a)) = info.get("postscriptStemSnapV") {
        master.v_stems = a.iter().filter_map(|v| v.num()).map(|v| v as f32).collect();
    }
    let mut zones = Vec::new();
    for key in ["postscriptBlueValues", "postscriptOtherBlues"] {
        if let Some(PV::Arr(a)) = info.get(key) {
            let v: Vec<f32> = a.iter().filter_map(|v| v.num()).map(|v| v as f32).collect();
            for pair in v.chunks(2) {
                if pair.len() == 2 {
                    let bottom = key == "postscriptOtherBlues" || pair[0] <= 0.0 && pair[1] <= 0.0 && zones.is_empty();
                    if bottom {
                        zones.push((pair[1], pair[0] - pair[1]));
                    } else {
                        zones.push((pair[0], pair[1] - pair[0]));
                    }
                }
            }
        }
    }
    if !zones.is_empty() {
        master.zones = zones;
    }
}

/// Import one UFO as a single-master font.
pub fn import_ufo(path: &Path) -> Result<Font, String> {
    if !path.join("metainfo.plist").exists() {
        return Err("This folder is not a UFO font (no metainfo.plist).".into());
    }
    let mut font = Font::new("Unnamed", &[]);
    font.glyphs.clear();
    let mut master = font.masters[0].clone();
    if let Some(info) = read_plist(&path.join("fontinfo.plist")) {
        ufo_info(&mut font, &mut master, &info);
    }
    // Glyph order from lib.plist, contents from glyphs/contents.plist.
    let lib = read_plist(&path.join("lib.plist"));
    let order: Vec<String> = match lib.as_ref().and_then(|l| l.get("public.glyphOrder")) {
        Some(PV::Arr(a)) => a.iter().filter_map(|v| v.str()).collect(),
        _ => Vec::new(),
    };
    let contents = read_plist(&path.join("glyphs").join("contents.plist")).ok_or("Missing glyphs/contents.plist")?;
    let mut glyphs: Vec<Glyph> = Vec::new();
    if let PV::Dict(d) = &contents {
        for (_name, file) in d {
            if let Some(f) = file.str() {
                if let Ok(s) = std::fs::read_to_string(path.join("glyphs").join(&f)) {
                    if let Some(g) = parse_glif(&s) {
                        glyphs.push(g);
                    }
                }
            }
        }
    }
    if !order.is_empty() {
        let pos: HashMap<&str, usize> = order.iter().enumerate().map(|(i, n)| (n.as_str(), i)).collect();
        glyphs.sort_by_key(|g| pos.get(g.name.as_str()).copied().unwrap_or(usize::MAX));
    }
    // Groups → classes.
    if let Some(PV::Dict(groups)) = read_plist(&path.join("groups.plist")) {
        for (name, members) in groups {
            if let PV::Arr(a) = members {
                let glyphs: Vec<String> = a.iter().filter_map(|v| v.str()).collect();
                let (short, kl, kr) = if let Some(s) = name.strip_prefix("public.kern1.") {
                    (s.to_string(), true, false)
                } else if let Some(s) = name.strip_prefix("public.kern2.") {
                    (s.to_string(), false, true)
                } else {
                    (name.clone(), false, false)
                };
                let key = glyphs.first().cloned().unwrap_or_default();
                let mut nm = short.clone();
                if kl && font.classes.iter().any(|c| c.name == nm) || kr && font.classes.iter().any(|c| c.name == nm) {
                    nm = format!("{short}_{}", if kl { "L" } else { "R" });
                }
                font.classes.push(Class { name: nm, glyphs, kern_left: kl, kern_right: kr, key });
            }
        }
    }
    let group_name = |font: &Font, g: &str| -> String {
        if let Some(s) = g.strip_prefix("public.kern1.") {
            font.classes.iter().find(|c| c.kern_left && (c.name == s || c.name == format!("{s}_L"))).map(|c| format!("@{}", c.name)).unwrap_or_else(|| format!("@{s}"))
        } else if let Some(s) = g.strip_prefix("public.kern2.") {
            font.classes.iter().find(|c| c.kern_right && (c.name == s || c.name == format!("{s}_R"))).map(|c| format!("@{}", c.name)).unwrap_or_else(|| format!("@{s}"))
        } else {
            g.to_string()
        }
    };
    if let Some(PV::Dict(k)) = read_plist(&path.join("kerning.plist")) {
        for (l, rights) in k {
            if let PV::Dict(rs) = rights {
                for (r, v) in rs {
                    if let Some(val) = v.num() {
                        let ln = group_name(&font, &l);
                        let rn = group_name(&font, &r);
                        master.kerning.push(KernPair { left: ln, right: rn, value: val as f32 });
                    }
                }
            }
        }
    }
    if let Ok(fea) = std::fs::read_to_string(path.join("features.fea")) {
        font.features = fea;
    }
    font.masters = vec![master];
    for g in glyphs {
        font.glyphs.push(g);
    }
    if font.glyphs.is_empty() {
        font.glyphs.push(Glyph::new(".notdef", vec![], 1, 500.0));
    }
    font.rebuild_index();
    Ok(font)
}

/// Import a .designspace: each source UFO becomes a master.
pub fn import_designspace(path: &Path) -> Result<Font, String> {
    let s = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let doc = roxmltree::Document::parse(&s).map_err(|e| e.to_string())?;
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut axes = Vec::new();
    for a in doc.descendants().filter(|n| n.has_tag_name("axis")) {
        let f = |k: &str| a.attribute(k).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
        axes.push(Axis { name: a.attribute("name").unwrap_or("").into(), tag: a.attribute("tag").unwrap_or("").into(), min: f("minimum"), default: f("default"), max: f("maximum") });
    }
    let mut font: Option<Font> = None;
    for src in doc.descendants().filter(|n| n.has_tag_name("source")) {
        let Some(file) = src.attribute("filename") else { continue };
        let ufo = import_ufo(&dir.join(file))?;
        let mut loc = vec![0.0; axes.len()];
        for d in src.descendants().filter(|n| n.has_tag_name("dimension")) {
            if let Some(i) = axes.iter().position(|a| Some(a.name.as_str()) == d.attribute("name")) {
                loc[i] = d.attribute("xvalue").and_then(|v| v.parse().ok()).unwrap_or(0.0);
            }
        }
        match font.as_mut() {
            None => {
                let mut f = ufo;
                f.masters[0].location = loc;
                if let Some(n) = src.attribute("stylename") {
                    f.masters[0].name = n.to_string();
                }
                font = Some(f);
            }
            Some(f) => {
                let mut m = ufo.masters[0].clone();
                m.location = loc;
                if let Some(n) = src.attribute("stylename") {
                    m.name = n.to_string();
                }
                f.masters.push(m);
                let mi = f.masters.len() - 1;
                for g in &mut f.glyphs {
                    let l = ufo.glyph(&g.name).map(|ug| ug.layers[0].clone()).unwrap_or_else(|| g.layers[0].clone());
                    while g.layers.len() < mi {
                        let c = g.layers[0].clone();
                        g.layers.push(c);
                    }
                    g.layers.push(l);
                }
            }
        }
    }
    let mut f = font.ok_or("The designspace file lists no sources.")?;
    f.axes = axes;
    for inst in doc.descendants().filter(|n| n.has_tag_name("instance")) {
        let mut loc = vec![0.0; f.axes.len()];
        for d in inst.descendants().filter(|n| n.has_tag_name("dimension")) {
            if let Some(i) = f.axes.iter().position(|a| Some(a.name.as_str()) == d.attribute("name")) {
                loc[i] = d.attribute("xvalue").and_then(|v| v.parse().ok()).unwrap_or(0.0);
            }
        }
        f.instances.push(Instance { name: inst.attribute("stylename").unwrap_or("Instance").into(), location: loc, export: true });
    }
    Ok(f)
}

// ------------------------------------------------------------------------------------------
// UFO export

pub fn glif_file_name(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            out.push(c);
            out.push('_');
        } else if "\"*+/:<>?[\\]|".contains(c) || (c as u32) < 32 || (i == 0 && c == '.') {
            out.push('_');
        } else {
            out.push(c);
        }
    }
    let reserved = ["CON", "PRN", "AUX", "NUL", "COM1", "LPT1", "CLOCK$", "A:-Z:"];
    if reserved.contains(&out.to_ascii_uppercase().as_str()) {
        out.insert(0, '_');
    }
    format!("{out}.glif")
}

pub fn glif_string(g: &Glyph, l: &Layer) -> String {
    let mut s = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<glyph name=\"{}\" format=\"2\">\n", esc(&g.name));
    s.push_str(&format!("  <advance width=\"{}\"/>\n", fmt_num(l.width)));
    for u in &g.unicodes {
        s.push_str(&format!("  <unicode hex=\"{u:04X}\"/>\n"));
    }
    for a in &l.anchors {
        s.push_str(&format!("  <anchor x=\"{}\" y=\"{}\" name=\"{}\"/>\n", fmt_num(a.x), fmt_num(a.y), esc(&a.name)));
    }
    for gd in &l.guides {
        s.push_str(&format!("  <guideline x=\"{}\" y=\"{}\" angle=\"{}\" name=\"{}\"/>\n", fmt_num(gd.x), fmt_num(gd.y), fmt_num(gd.angle.rem_euclid(360.0)), esc(&gd.name)));
    }
    if !l.contours.is_empty() || !l.components.is_empty() {
        s.push_str("  <outline>\n");
        for c in &l.components {
            let t = c.t;
            let mut attrs = format!("base=\"{}\"", esc(&c.base));
            let names = ["xScale", "xyScale", "yxScale", "yScale", "xOffset", "yOffset"];
            let defaults = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
            for k in 0..6 {
                if (t[k] - defaults[k]).abs() > 1e-6 {
                    attrs.push_str(&format!(" {}=\"{}\"", names[k], fmt_num(t[k])));
                }
            }
            s.push_str(&format!("    <component {attrs}/>\n"));
        }
        for c in &l.contours {
            s.push_str("    <contour>\n");
            for (i, p) in c.points.iter().enumerate() {
                let t = match p.t {
                    PtType::Move => Some("move"),
                    PtType::Line => Some("line"),
                    PtType::Curve => Some("curve"),
                    PtType::QCurve => Some("qcurve"),
                    PtType::Off => None,
                };
                let t = if i == 0 && !c.closed { Some("move") } else { t };
                let mut a = format!("x=\"{}\" y=\"{}\"", fmt_num(p.x), fmt_num(p.y));
                if let Some(t) = t {
                    a.push_str(&format!(" type=\"{t}\""));
                    if p.smooth {
                        a.push_str(" smooth=\"yes\"");
                    }
                }
                s.push_str(&format!("      <point {a}/>\n"));
            }
            s.push_str("    </contour>\n");
        }
        s.push_str("  </outline>\n");
    }
    if !g.note.is_empty() {
        s.push_str(&format!("  <note>{}</note>\n", esc(&g.note)));
    }
    s.push_str("</glyph>\n");
    s
}

fn fmt_num(v: f32) -> String {
    if (v - v.round()).abs() < 0.0005 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn s(v: &str) -> PV {
    PV::Str(v.to_string())
}
fn nm(v: f32) -> PV {
    PV::Num(v as f64)
}

pub fn export_ufo(font: &Font, m: usize, dir: &Path) -> Result<(), String> {
    if dir.exists() {
        std::fs::remove_dir_all(dir).map_err(|e| format!("Could not replace {}: {e}", dir.display()))?;
    }
    let gdir = dir.join("glyphs");
    std::fs::create_dir_all(&gdir).map_err(|e| e.to_string())?;
    let w = |p: PathBuf, c: String| std::fs::write(p, c).map_err(|e| e.to_string());
    w(dir.join("metainfo.plist"), plist_string(&PV::Dict(vec![("creator".into(), s("com.fontabulous.studio")), ("formatVersion".into(), PV::Num(3.0))])))?;
    let fi = &font.info;
    let ms = &font.masters[m];
    let mut blues = Vec::new();
    let mut others = Vec::new();
    for (pos, width) in &ms.zones {
        if *width < 0.0 && *pos <= 0.0 && *pos != 0.0 {
            others.push(nm(pos + width));
            others.push(nm(*pos));
        } else if *width < 0.0 {
            blues.push(nm(pos + width));
            blues.push(nm(*pos));
        } else {
            blues.push(nm(*pos));
            blues.push(nm(pos + width));
        }
    }
    let style = if font.masters.len() > 1 { ms.name.clone() } else { fi.style.clone() };
    let info = PV::Dict(vec![
        ("familyName".into(), s(&fi.family)),
        ("styleName".into(), s(&style)),
        ("styleMapFamilyName".into(), s(&fi.family)),
        ("styleMapStyleName".into(), s(&style_map(&style))),
        ("versionMajor".into(), PV::Num(fi.version_major as f64)),
        ("versionMinor".into(), PV::Num(fi.version_minor as f64)),
        ("copyright".into(), s(&fi.copyright)),
        ("trademark".into(), s(&fi.trademark)),
        ("unitsPerEm".into(), nm(fi.upm)),
        ("descender".into(), nm(ms.descender)),
        ("xHeight".into(), nm(ms.x_height)),
        ("capHeight".into(), nm(ms.cap_height)),
        ("ascender".into(), nm(ms.ascender)),
        ("italicAngle".into(), nm(ms.italic_angle)),
        ("note".into(), s(&fi.note)),
        ("openTypeHheaLineGap".into(), nm(ms.line_gap)),
        ("openTypeNameDesigner".into(), s(&fi.designer)),
        ("openTypeNameDesignerURL".into(), s(&fi.designer_url)),
        ("openTypeNameManufacturer".into(), s(&fi.manufacturer)),
        ("openTypeNameManufacturerURL".into(), s(&fi.manufacturer_url)),
        ("openTypeNameLicense".into(), s(&fi.license)),
        ("openTypeNameLicenseURL".into(), s(&fi.license_url)),
        ("openTypeNameDescription".into(), s(&fi.description)),
        ("openTypeOS2VendorID".into(), s(&fi.vendor_id)),
        ("openTypeOS2WeightClass".into(), PV::Num(if ms.weight_class > 0 { ms.weight_class } else { fi.weight_class } as f64)),
        ("openTypeOS2WidthClass".into(), PV::Num(if ms.width_class > 0 { ms.width_class } else { fi.width_class }.clamp(1, 9) as f64)),
        ("postscriptFontName".into(), s(&format!("{}-{}", fi.family, style).replace(' ', ""))),
        ("postscriptUnderlinePosition".into(), nm(ms.underline_position)),
        ("postscriptUnderlineThickness".into(), nm(ms.underline_thickness)),
        ("postscriptBlueValues".into(), PV::Arr(blues)),
        ("postscriptOtherBlues".into(), PV::Arr(others)),
        ("postscriptStemSnapH".into(), PV::Arr(ms.h_stems.iter().map(|v| nm(*v)).collect())),
        ("postscriptStemSnapV".into(), PV::Arr(ms.v_stems.iter().map(|v| nm(*v)).collect())),
    ]);
    w(dir.join("fontinfo.plist"), plist_string(&info))?;
    let mut contents = Vec::new();
    let mut used = std::collections::HashSet::new();
    for g in &font.glyphs {
        let mut file = glif_file_name(&g.name);
        while !used.insert(file.to_ascii_lowercase()) {
            file = format!("_{file}");
        }
        w(gdir.join(&file), glif_string(g, g.layer(m)))?;
        contents.push((g.name.clone(), s(&file)));
    }
    w(gdir.join("contents.plist"), plist_string(&PV::Dict(contents)))?;
    w(dir.join("layercontents.plist"), plist_string(&PV::Arr(vec![PV::Arr(vec![s("public.default"), s("glyphs")])])))?;
    let order: Vec<PV> = font.glyphs.iter().map(|g| s(&g.name)).collect();
    w(dir.join("lib.plist"), plist_string(&PV::Dict(vec![("public.glyphOrder".into(), PV::Arr(order))])))?;
    // Groups and kerning.
    let gname = |c: &Class| if c.kern_left { format!("public.kern1.{}", c.name) } else if c.kern_right { format!("public.kern2.{}", c.name) } else { c.name.clone() };
    let groups: Vec<(String, PV)> = font.classes.iter().map(|c| (gname(c), PV::Arr(c.glyphs.iter().map(|g| s(g)).collect()))).collect();
    w(dir.join("groups.plist"), plist_string(&PV::Dict(groups)))?;
    let resolve = |n: &str, left: bool| -> String {
        if let Some(cn) = n.strip_prefix('@') {
            if let Some(c) = font.classes.iter().find(|c| c.name == cn && if left { c.kern_left } else { c.kern_right }) {
                return gname(c);
            }
            return format!("{}{cn}", if left { "public.kern1." } else { "public.kern2." });
        }
        n.to_string()
    };
    let mut kmap: BTreeMap<String, Vec<(String, PV)>> = BTreeMap::new();
    for k in &ms.kerning {
        kmap.entry(resolve(&k.left, true)).or_default().push((resolve(&k.right, false), nm(k.value)));
    }
    w(dir.join("kerning.plist"), plist_string(&PV::Dict(kmap.into_iter().map(|(k, v)| (k, PV::Dict(v))).collect())))?;
    w(dir.join("features.fea"), font.features.clone())?;
    Ok(())
}

fn style_map(style: &str) -> String {
    let l = style.to_ascii_lowercase();
    match (l.contains("bold"), l.contains("italic") || l.contains("oblique")) {
        (true, true) => "bold italic".into(),
        (true, false) => "bold".into(),
        (false, true) => "italic".into(),
        _ => "regular".into(),
    }
}

/// Export all masters as UFOs next to a .designspace file (or one UFO for one master).
pub fn export_ufo_family(font: &Font, target: &Path) -> Result<Vec<PathBuf>, String> {
    if font.masters.len() == 1 {
        export_ufo(font, 0, target)?;
        return Ok(vec![target.to_path_buf()]);
    }
    let dir = target.parent().unwrap_or(Path::new(".")).to_path_buf();
    let stem = target.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| font.info.family.clone());
    let mut out = Vec::new();
    let mut ds = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<designspace format=\"4.1\">\n  <axes>\n");
    for a in &font.axes {
        ds.push_str(&format!("    <axis tag=\"{}\" name=\"{}\" minimum=\"{}\" default=\"{}\" maximum=\"{}\"/>\n", esc(&a.tag), esc(&a.name), a.min, a.default, a.max));
    }
    ds.push_str("  </axes>\n  <sources>\n");
    for (i, m) in font.masters.iter().enumerate() {
        let file = format!("{stem}-{}.ufo", m.name.replace(' ', ""));
        export_ufo(font, i, &dir.join(&file))?;
        out.push(dir.join(&file));
        ds.push_str(&format!("    <source filename=\"{}\" familyname=\"{}\" stylename=\"{}\">\n      <location>\n", esc(&file), esc(&font.info.family), esc(&m.name)));
        for (ai, a) in font.axes.iter().enumerate() {
            ds.push_str(&format!("        <dimension name=\"{}\" xvalue=\"{}\"/>\n", esc(&a.name), m.location.get(ai).copied().unwrap_or(a.default)));
        }
        ds.push_str("      </location>\n    </source>\n");
    }
    ds.push_str("  </sources>\n  <instances>\n");
    for inst in &font.instances {
        ds.push_str(&format!("    <instance familyname=\"{}\" stylename=\"{}\">\n      <location>\n", esc(&font.info.family), esc(&inst.name)));
        for (ai, a) in font.axes.iter().enumerate() {
            ds.push_str(&format!("        <dimension name=\"{}\" xvalue=\"{}\"/>\n", esc(&a.name), inst.location.get(ai).copied().unwrap_or(a.default)));
        }
        ds.push_str("      </location>\n    </instance>\n");
    }
    ds.push_str("  </instances>\n</designspace>\n");
    let dsp = dir.join(format!("{stem}.designspace"));
    std::fs::write(&dsp, ds).map_err(|e| e.to_string())?;
    out.push(dsp);
    Ok(out)
}

// ------------------------------------------------------------------------------------------
// TrueType export

struct W(Vec<u8>);
impl W {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_be_bytes());
    }
    fn i16(&mut self, v: i16) {
        self.0.extend_from_slice(&v.to_be_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_be_bytes());
    }
    fn i64(&mut self, v: i64) {
        self.0.extend_from_slice(&v.to_be_bytes());
    }
    fn fixed(&mut self, v: f32) {
        self.0.extend_from_slice(&((v * 65536.0).round() as i32).to_be_bytes());
    }
    fn pad4(&mut self) {
        while self.0.len() % 4 != 0 {
            self.0.push(0);
        }
    }
}

fn checksum(data: &[u8]) -> u32 {
    let mut sum: u32 = 0;
    for chunk in data.chunks(4) {
        let mut b = [0u8; 4];
        b[..chunk.len()].copy_from_slice(chunk);
        sum = sum.wrapping_add(u32::from_be_bytes(b));
    }
    sum
}

fn i16c(v: f32) -> i16 {
    v.round().clamp(-32768.0, 32767.0) as i16
}

pub struct ExportReport {
    pub glyphs: usize,
    pub kern_pairs: usize,
    pub notes: Vec<String>,
}

/// Write a TrueType (.ttf) font for one master (or an interpolated layer set).
pub fn export_ttf(font: &Font, m: usize, path: &Path, style_override: Option<&str>) -> Result<ExportReport, String> {
    let mut notes = Vec::new();
    // Glyph order: .notdef first.
    let mut order: Vec<usize> = (0..font.glyphs.len()).collect();
    if let Some(nd) = font.find(".notdef") {
        order.retain(|&i| i != nd);
        order.insert(0, nd);
    } else {
        notes.push("The font has no .notdef glyph; the first glyph is used in its place.".into());
    }
    let gid_of: HashMap<&str, u16> = order.iter().enumerate().map(|(gid, &gi)| (font.glyphs[gi].name.as_str(), gid as u16)).collect();
    let ms = &font.masters[m.min(font.masters.len() - 1)];
    let upm = font.info.upm;

    // glyf + loca + hmtx
    let mut glyf = W(Vec::new());
    let mut loca: Vec<u32> = Vec::new();
    let mut hmtx = W(Vec::new());
    let (mut max_pts, mut max_ctrs) = (0u16, 0u16);
    let mut gb = [i16::MAX, i16::MAX, i16::MIN, i16::MIN];
    let (mut adv_max, mut min_lsb, mut min_rsb, mut max_ext) = (0u16, i16::MAX, i16::MAX, i16::MIN);
    let mut open_skipped = 0;
    let mut total_adv = 0f64;
    let mut adv_count = 0usize;
    for &gi in &order {
        loca.push(glyf.0.len() as u32);
        let g = &font.glyphs[gi];
        let adv = g.layer(m).width.round().clamp(0.0, 65535.0) as u16;
        if adv > 0 {
            total_adv += adv as f64;
            adv_count += 1;
        }
        let mut contours: Vec<Contour> = Vec::new();
        for c in font.flat_contours(gi, m) {
            if !c.closed {
                open_skipped += 1;
                continue;
            }
            let mut q = geom::to_quadratic(&c, 1.0);
            q.reverse(); // TrueType winds outer contours clockwise.
            geom::round_contour(&mut q);
            if q.points.len() >= 2 {
                contours.push(q);
            }
        }
        adv_max = adv_max.max(adv);
        if contours.is_empty() {
            hmtx.u16(adv);
            hmtx.i16(0);
            continue;
        }
        let mut b = [i16::MAX, i16::MAX, i16::MIN, i16::MIN];
        let mut npts = 0u16;
        for c in &contours {
            for p in &c.points {
                b[0] = b[0].min(i16c(p.x));
                b[1] = b[1].min(i16c(p.y));
                b[2] = b[2].max(i16c(p.x));
                b[3] = b[3].max(i16c(p.y));
                npts += 1;
            }
        }
        max_pts = max_pts.max(npts);
        max_ctrs = max_ctrs.max(contours.len() as u16);
        gb = [gb[0].min(b[0]), gb[1].min(b[1]), gb[2].max(b[2]), gb[3].max(b[3])];
        min_lsb = min_lsb.min(b[0]);
        min_rsb = min_rsb.min((adv as i32 - b[2] as i32).clamp(-32768, 32767) as i16);
        max_ext = max_ext.max(b[2]);
        hmtx.u16(adv);
        hmtx.i16(b[0]);
        glyf.i16(contours.len() as i16);
        for v in b {
            glyf.i16(v);
        }
        let mut end = 0i32;
        for c in &contours {
            end += c.points.len() as i32;
            glyf.u16((end - 1) as u16);
        }
        glyf.u16(0); // instructions
        for c in &contours {
            for p in &c.points {
                glyf.u8(if p.t.on_curve() { 1 } else { 0 });
            }
        }
        let (mut px, mut py) = (0i32, 0i32);
        let mut ys = Vec::new();
        for c in &contours {
            for p in &c.points {
                let (x, y) = (p.x.round() as i32, p.y.round() as i32);
                glyf.i16((x - px).clamp(-32768, 32767) as i16);
                ys.push((y - py).clamp(-32768, 32767) as i16);
                px = x;
                py = y;
            }
        }
        for y in ys {
            glyf.i16(y);
        }
        glyf.pad4();
    }
    loca.push(glyf.0.len() as u32);
    if open_skipped > 0 {
        notes.push(format!("{open_skipped} open contour(s) were left out (TrueType needs closed contours)."));
    }
    if gb[0] == i16::MAX {
        gb = [0, 0, 0, 0];
        min_lsb = 0;
        min_rsb = 0;
        max_ext = 0;
    }
    let n = order.len() as u16;
    let mut loca_t = W(Vec::new());
    for o in &loca {
        loca_t.u32(*o);
    }

    let style = style_override.map(|s| s.to_string()).unwrap_or_else(|| if font.masters.len() > 1 { ms.name.clone() } else { font.info.style.clone() });
    let sl = style.to_ascii_lowercase();
    let bold = sl.contains("bold") && !sl.contains("semi") && !sl.contains("extra") && !sl.contains("ultra");
    let italic = sl.contains("italic") || sl.contains("oblique") || ms.italic_angle != 0.0;
    let weight = if ms.weight_class > 0 { ms.weight_class } else { font.info.weight_class.max(100) };

    // head
    let secs_1904 = {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
        now + 2_082_844_800
    };
    let mut head = W(Vec::new());
    head.u32(0x0001_0000);
    head.fixed(font.info.version_major as f32 + font.info.version_minor as f32 / 1000.0);
    head.u32(0); // checkSumAdjustment
    head.u32(0x5F0F_3CF5);
    head.u16(0x000B);
    head.u16(upm.round() as u16);
    head.i64(secs_1904);
    head.i64(secs_1904);
    for v in gb {
        head.i16(v);
    }
    head.u16((bold as u16) | ((italic as u16) << 1));
    head.u16(8);
    head.i16(2);
    head.i16(1); // long loca
    head.i16(0);

    // hhea
    let mut hhea = W(Vec::new());
    hhea.u32(0x0001_0000);
    hhea.i16(i16c(ms.ascender));
    hhea.i16(i16c(ms.descender));
    hhea.i16(i16c(ms.line_gap));
    hhea.u16(adv_max);
    hhea.i16(min_lsb);
    hhea.i16(min_rsb);
    hhea.i16(max_ext);
    let (rise, run) = if ms.italic_angle != 0.0 { (1000i16, i16c(-(ms.italic_angle.to_radians().tan()) * 1000.0)) } else { (1, 0) };
    hhea.i16(rise);
    hhea.i16(run);
    hhea.i16(0);
    for _ in 0..4 {
        hhea.i16(0);
    }
    hhea.i16(0);
    hhea.u16(n);

    // maxp
    let mut maxp = W(Vec::new());
    maxp.u32(0x0001_0000);
    maxp.u16(n);
    maxp.u16(max_pts);
    maxp.u16(max_ctrs);
    maxp.u16(0);
    maxp.u16(0);
    maxp.u16(2);
    for _ in 0..8 {
        maxp.u16(0);
    }

    // cmap
    let mut map: Vec<(u32, u16)> = Vec::new();
    for (gid, &gi) in order.iter().enumerate() {
        for &u in &font.glyphs[gi].unicodes {
            map.push((u, gid as u16));
        }
    }
    map.sort_unstable();
    map.dedup_by_key(|e| e.0);
    let runs = |items: &[(u32, u16)]| -> Vec<(u32, u32, u16)> {
        let mut out: Vec<(u32, u32, u16)> = Vec::new();
        for &(cp, g) in items {
            if let Some(last) = out.last_mut() {
                if cp == last.1 + 1 && g as u32 == last.2 as u32 + (cp - last.0) {
                    last.1 = cp;
                    continue;
                }
            }
            out.push((cp, cp, g));
        }
        out
    };
    let bmp: Vec<(u32, u16)> = map.iter().copied().filter(|(c, _)| *c < 0xFFFF).collect();
    let mut segs = runs(&bmp);
    segs.push((0xFFFF, 0xFFFF, 0));
    let mut f4 = W(Vec::new());
    let sc = segs.len() as u16;
    let mut sr = 2u16;
    let mut es = 0u16;
    while sr * 2 <= sc * 2 && sr <= sc {
        sr *= 2;
        es += 1;
    }
    sr = sr.min(sc * 2);
    // searchRange = 2 * 2^floor(log2(segCount))
    let mut p2 = 1u16;
    let mut e2 = 0u16;
    while p2 * 2 <= sc {
        p2 *= 2;
        e2 += 1;
    }
    let _ = (sr, es);
    let search_range = p2 * 2;
    f4.u16(4);
    f4.u16(0); // length placeholder
    f4.u16(0);
    f4.u16(sc * 2);
    f4.u16(search_range);
    f4.u16(e2);
    f4.u16(sc * 2 - search_range);
    for s in &segs {
        f4.u16(s.1 as u16);
    }
    f4.u16(0);
    for s in &segs {
        f4.u16(s.0 as u16);
    }
    for s in &segs {
        let delta = if s.0 == 0xFFFF { 1u16 } else { (s.2 as u32).wrapping_sub(s.0) as u16 };
        f4.u16(delta);
    }
    for _ in &segs {
        f4.u16(0);
    }
    let len4 = f4.0.len() as u16;
    f4.0[2..4].copy_from_slice(&len4.to_be_bytes());
    let groups = runs(&map);
    let mut f12 = W(Vec::new());
    f12.u16(12);
    f12.u16(0);
    f12.u32(16 + 12 * groups.len() as u32);
    f12.u32(0);
    f12.u32(groups.len() as u32);
    for g in &groups {
        f12.u32(g.0);
        f12.u32(g.1);
        f12.u32(g.2 as u32);
    }
    let mut cmap = W(Vec::new());
    cmap.u16(0);
    cmap.u16(2);
    cmap.u16(3);
    cmap.u16(1);
    cmap.u32(4 + 8 * 2);
    cmap.u16(3);
    cmap.u16(10);
    cmap.u32(4 + 8 * 2 + f4.0.len() as u32);
    cmap.0.extend_from_slice(&f4.0);
    cmap.0.extend_from_slice(&f12.0);

    // OS/2
    let first = map.first().map(|m| m.0.min(0xFFFF) as u16).unwrap_or(0x20);
    let last = map.last().map(|m| m.0.min(0xFFFF) as u16).unwrap_or(0x7E);
    let mut ur1 = 0u32;
    for (cp, _) in &map {
        let bit = match cp {
            0x0000..=0x007F => 0,
            0x0080..=0x00FF => 1,
            0x0100..=0x017F => 2,
            0x0180..=0x024F => 3,
            0x02B0..=0x02FF => 5,
            0x0300..=0x036F => 6,
            0x0370..=0x03FF => 7,
            0x0400..=0x04FF => 9,
            0x2000..=0x206F => 31,
            _ => continue,
        };
        ur1 |= 1 << bit;
    }
    let xh = ms.x_height;
    let mut os2 = W(Vec::new());
    os2.u16(4);
    os2.i16(if adv_count > 0 { (total_adv / adv_count as f64).round() as i16 } else { 0 });
    os2.u16(weight);
    os2.u16(if ms.width_class > 0 { ms.width_class } else { font.info.width_class }.clamp(1, 9));
    os2.u16(font.info.fs_type);
    let sub = |f: f32| i16c(upm * f);
    for v in [sub(0.65), sub(0.6), 0, sub(0.075), sub(0.65), sub(0.6), 0, sub(0.35)] {
        os2.i16(v);
    }
    os2.i16(i16c(ms.underline_thickness.max(upm * 0.05)));
    os2.i16(i16c(xh * 0.5 + ms.underline_thickness * 0.5));
    os2.i16(0);
    for _ in 0..10 {
        os2.u8(0);
    }
    os2.u32(ur1);
    os2.u32(0);
    os2.u32(0);
    os2.u32(0);
    let mut vend = font.info.vendor_id.clone().into_bytes();
    vend.resize(4, b' ');
    for b in vend.iter().take(4) {
        os2.u8(*b);
    }
    let mut sel = 0u16;
    if italic {
        sel |= 0x01;
    }
    if bold {
        sel |= 0x20;
    }
    if !italic && !bold {
        sel |= 0x40;
    }
    sel |= 0x80;
    os2.u16(sel);
    os2.u16(first);
    os2.u16(last);
    os2.i16(i16c(ms.ascender));
    os2.i16(i16c(ms.descender));
    os2.i16(i16c(ms.line_gap));
    os2.u16(gb[3].max(i16c(ms.ascender)).max(0) as u16);
    os2.u16((-(gb[1].min(i16c(ms.descender)) as i32)).max(0) as u16);
    os2.u32(1);
    os2.u32(0);
    os2.i16(i16c(xh));
    os2.i16(i16c(ms.cap_height));
    os2.u16(0);
    os2.u16(0x20);
    os2.u16(2);

    // name
    let fi = &font.info;
    let ribbi = matches!(sl.as_str(), "regular" | "bold" | "italic" | "bold italic");
    let ver = format!("Version {}.{:03}", fi.version_major, fi.version_minor);
    let ps = format!("{}-{}", fi.family, style).chars().filter(|c| c.is_ascii_graphic() && !"[](){}<>/%".contains(*c)).collect::<String>();
    let full = format!("{} {}", fi.family, style);
    let mut recs: Vec<(u16, String)> = vec![
        (0, fi.copyright.clone()),
        (1, if ribbi { fi.family.clone() } else { full.clone() }),
        (2, if ribbi { style.clone() } else { "Regular".into() }),
        (3, format!("{ver};{};{ps}", fi.vendor_id.trim())),
        (4, full.clone()),
        (5, ver.clone()),
        (6, ps.clone()),
        (7, fi.trademark.clone()),
        (8, fi.manufacturer.clone()),
        (9, fi.designer.clone()),
        (10, fi.description.clone()),
        (11, fi.manufacturer_url.clone()),
        (12, fi.designer_url.clone()),
        (13, fi.license.clone()),
        (14, fi.license_url.clone()),
    ];
    if !ribbi {
        recs.push((16, fi.family.clone()));
        recs.push((17, style.clone()));
    }
    recs.retain(|(_, s)| !s.is_empty());
    let mut strings = W(Vec::new());
    let mut name = W(Vec::new());
    name.u16(0);
    name.u16(recs.len() as u16);
    name.u16(6 + 12 * recs.len() as u16);
    for (id, s) in &recs {
        let off = strings.0.len() as u16;
        for u in s.encode_utf16() {
            strings.u16(u);
        }
        name.u16(3);
        name.u16(1);
        name.u16(0x409);
        name.u16(*id);
        name.u16(strings.0.len() as u16 - off);
        name.u16(off);
    }
    name.0.extend_from_slice(&strings.0);

    // post v2
    let mut post = W(Vec::new());
    post.u32(0x0002_0000);
    post.fixed(ms.italic_angle);
    post.i16(i16c(ms.underline_position));
    post.i16(i16c(ms.underline_thickness));
    post.u32(fi.monospaced as u32);
    for _ in 0..4 {
        post.u32(0);
    }
    post.u16(n);
    for i in 0..n {
        post.u16(258 + i);
    }
    for &gi in &order {
        let nm: String = font.glyphs[gi].name.chars().filter(|c| c.is_ascii()).take(63).collect();
        post.u8(nm.len() as u8);
        post.0.extend_from_slice(nm.as_bytes());
    }

    // kern (format 0) from expanded pairs
    let mut flat: BTreeMap<(u16, u16), i16> = BTreeMap::new();
    let members = |n: &str, left: bool| -> Vec<u16> {
        if let Some(cn) = n.strip_prefix('@') {
            font.classes.iter().find(|c| c.name == cn && (if left { c.kern_left } else { c.kern_right } || (!c.kern_left && !c.kern_right))).map(|c| c.glyphs.iter().filter_map(|g| gid_of.get(g.as_str()).copied()).collect()).unwrap_or_default()
        } else {
            gid_of.get(n).copied().into_iter().collect()
        }
    };
    // Class pairs first, glyph pairs override.
    let mut sorted: Vec<&KernPair> = ms.kerning.iter().collect();
    sorted.sort_by_key(|k| -((k.left.starts_with('@') as i32) + (k.right.starts_with('@') as i32)));
    for k in sorted {
        for l in members(&k.left, true) {
            for r in members(&k.right, false) {
                flat.insert((l, r), i16c(k.value));
            }
        }
    }
    let mut kern_pairs = flat.len();
    if kern_pairs > 10920 {
        notes.push(format!("Only the first 10920 of {kern_pairs} kerning pairs fit the kern table."));
        kern_pairs = 10920;
    }
    let kern = if kern_pairs > 0 {
        let mut k = W(Vec::new());
        k.u16(0);
        k.u16(1);
        k.u16(0);
        k.u16((14 + 6 * kern_pairs) as u16);
        k.u16(0x0001);
        k.u16(kern_pairs as u16);
        let mut p2 = 1u16;
        let mut e2 = 0u16;
        while (p2 as usize) * 2 <= kern_pairs {
            p2 *= 2;
            e2 += 1;
        }
        k.u16(p2 * 6);
        k.u16(e2);
        k.u16((kern_pairs as u16 - p2) * 6);
        for ((l, r), v) in flat.iter().take(kern_pairs) {
            k.u16(*l);
            k.u16(*r);
            k.i16(*v);
        }
        Some(k.0)
    } else {
        None
    };

    // Assemble.
    let mut tables: Vec<(&[u8; 4], Vec<u8>)> = vec![
        (b"OS/2", os2.0),
        (b"cmap", cmap.0),
        (b"glyf", glyf.0),
        (b"head", head.0),
        (b"hhea", hhea.0),
        (b"hmtx", hmtx.0),
        (b"loca", loca_t.0),
        (b"maxp", maxp.0),
        (b"name", name.0),
        (b"post", post.0),
    ];
    if let Some(k) = kern {
        tables.push((b"kern", k));
    }
    tables.sort_by(|a, b| a.0.cmp(b.0));
    let nt = tables.len() as u16;
    let mut p2 = 1u16;
    let mut e2 = 0u16;
    while p2 * 2 <= nt {
        p2 *= 2;
        e2 += 1;
    }
    let mut out = W(Vec::new());
    out.u32(0x0001_0000);
    out.u16(nt);
    out.u16(p2 * 16);
    out.u16(e2);
    out.u16(nt * 16 - p2 * 16);
    let mut offset = 12 + 16 * nt as u32;
    let mut head_off = 0usize;
    for (tag, data) in &tables {
        out.0.extend_from_slice(*tag);
        out.u32(checksum(data));
        out.u32(offset);
        out.u32(data.len() as u32);
        if *tag == b"head" {
            head_off = offset as usize;
        }
        offset += ((data.len() + 3) / 4 * 4) as u32;
    }
    for (_, data) in &tables {
        out.0.extend_from_slice(data);
        out.pad4();
    }
    let adj = 0xB1B0_AFBAu32.wrapping_sub(checksum(&out.0));
    out.0[head_off + 8..head_off + 12].copy_from_slice(&adj.to_be_bytes());
    std::fs::write(path, &out.0).map_err(|e| e.to_string())?;
    Ok(ExportReport { glyphs: order.len(), kern_pairs, notes })
}

// ------------------------------------------------------------------------------------------
// Small text formats

pub fn kerning_text(font: &Font, m: usize) -> String {
    let mut s = String::from("# Fontabulous Studio kerning: left right value\n");
    for k in &font.masters[m].kerning {
        s.push_str(&format!("{} {} {}\n", k.left, k.right, fmt_num(k.value)));
    }
    s
}

pub fn parse_kerning_text(text: &str) -> Vec<KernPair> {
    text.lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            Some(KernPair { left: it.next()?.to_string(), right: it.next()?.to_string(), value: it.next()?.parse().ok()? })
        })
        .collect()
}

pub fn classes_text(font: &Font) -> String {
    let mut s = String::new();
    for c in &font.classes {
        let pre = if c.kern_left && c.kern_right { "_" } else if c.kern_left { "L" } else if c.kern_right { "R" } else { "" };
        s.push_str(&format!("@{}{} = [{}];\n", if pre.is_empty() { "" } else { pre }, c.name, c.glyphs.join(" ")));
    }
    s
}

pub fn parse_classes_text(text: &str) -> Vec<Class> {
    let mut out = Vec::new();
    for stmt in text.split(';') {
        let stmt = stmt.trim();
        let Some((lhs, rhs)) = stmt.split_once('=') else { continue };
        let mut name = lhs.trim().trim_start_matches('@').to_string();
        let (mut kl, mut kr) = (false, false);
        if let Some(r) = name.strip_prefix('_') {
            name = r.to_string();
            kl = true;
            kr = true;
        }
        let glyphs: Vec<String> = rhs.trim().trim_start_matches('[').trim_end_matches(']').split_whitespace().map(|s| s.to_string()).collect();
        if name.is_empty() {
            continue;
        }
        let key = glyphs.first().cloned().unwrap_or_default();
        out.push(Class { name, glyphs, kern_left: kl, kern_right: kr, key });
    }
    out
}

/// Build the kerning part of a .fea file from the font's pairs and classes.
pub fn kern_feature(font: &Font, m: usize) -> String {
    let mut s = String::new();
    for c in &font.classes {
        s.push_str(&format!("@{} = [{}];\n", c.name, c.glyphs.join(" ")));
    }
    s.push_str("\nfeature kern {\n");
    for k in &font.masters[m].kerning {
        let both_glyphs = !k.left.starts_with('@') && !k.right.starts_with('@');
        s.push_str(&format!("  {}pos {} {} {};\n", if both_glyphs { "" } else { "" }, k.left, k.right, fmt_num(k.value)));
    }
    s.push_str("} kern;\n");
    s
}

// ------------------------------------------------------------------------------------------
// Installed fonts

pub fn font_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    #[cfg(windows)]
    {
        let win = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
        v.push(PathBuf::from(win).join("Fonts"));
        if let Some(l) = dirs::data_local_dir() {
            v.push(l.join("Microsoft").join("Windows").join("Fonts"));
        }
    }
    #[cfg(target_os = "linux")]
    {
        v.push(PathBuf::from("/usr/share/fonts"));
        v.push(PathBuf::from("/usr/local/share/fonts"));
        if let Some(h) = dirs::home_dir() {
            v.push(h.join(".local/share/fonts"));
            v.push(h.join(".fonts"));
        }
    }
    #[cfg(target_os = "macos")]
    {
        v.push(PathBuf::from("/System/Library/Fonts"));
        v.push(PathBuf::from("/Library/Fonts"));
        if let Some(h) = dirs::home_dir() {
            v.push(h.join("Library/Fonts"));
        }
    }
    v
}

pub fn installed_fonts() -> Vec<PathBuf> {
    let mut out = Vec::new();
    fn walk(d: &Path, out: &mut Vec<PathBuf>, depth: usize) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() && depth < 4 {
                walk(&p, out, depth + 1);
            } else if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                if ["ttf", "otf", "ttc", "otc"].contains(&ext.to_ascii_lowercase().as_str()) {
                    out.push(p);
                }
            }
        }
    }
    for d in font_dirs() {
        walk(&d, &mut out, 0);
    }
    out.sort_by_key(|p| p.file_name().map(|s| s.to_string_lossy().to_ascii_lowercase()));
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small font with real outlines (from the bundled template) and one kerning pair.
    fn sample() -> Font {
        let names: Vec<String> = ["A", "V", "a", "o", "space"].iter().map(|s| s.to_string()).collect();
        let mut f = Font::new("Roundtrip", &names);
        let mut cache = crate::render::Cache::default();
        let upm = f.info.upm;
        for g in &mut f.glyphs {
            if let Some(&u) = g.unicodes.first() {
                if let Some((cs, w)) = cache.template_contours(u, upm) {
                    let l = g.layer_mut(0);
                    l.contours = cs;
                    l.width = w.round();
                }
            }
        }
        f.set_kerning(0, "A", "V", -80.0);
        f
    }

    #[test]
    fn ttf_ufo_native_roundtrip() {
        let f = sample();
        let dir = std::env::temp_dir().join(format!("fstudio_rt_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let ttf = dir.join("rt.ttf");
        let rep = export_ttf(&f, 0, &ttf, None).unwrap();
        assert_eq!(rep.kern_pairs, 1);
        let back = import_sfnt(&ttf, 0).unwrap();
        let a = back.find("A").expect("A survives TTF");
        assert!(!back.glyphs[a].layers[0].contours.is_empty());
        assert_eq!(back.glyphs[a].layers[0].width, f.glyphs[f.find("A").unwrap()].layers[0].width);
        assert_eq!(back.kerning(0, "A", "V").map(|k| k.0), Some(-80.0));

        let ufo = dir.join("rt.ufo");
        export_ufo(&f, 0, &ufo).unwrap();
        let u = import_ufo(&ufo).unwrap();
        let o = u.find("o").unwrap();
        assert_eq!(u.glyphs[o].layers[0].contours.len(), 2);
        assert_eq!(u.kerning(0, "A", "V").map(|k| k.0), Some(-80.0));

        let nat = dir.join("rt.fstudio");
        save_native(&f, &nat).unwrap();
        let n = load_native(&nat).unwrap();
        assert_eq!(n.glyphs.len(), f.glyphs.len());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
