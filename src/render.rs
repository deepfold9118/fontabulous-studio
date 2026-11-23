//! Glyph painting: tessellated fills cached per glyph revision, outline strokes,
//! and faint "template" letters (from the bundled Inter font) for empty glyphs.

#![allow(dead_code)]

use egui::{Color32, Mesh, Painter, Pos2, Shape, Stroke};
use std::collections::HashMap;
use std::sync::Arc;

use crate::font::{Contour, Font, Point, PtType};
use crate::geom::{self, P2};

pub type Tess = Arc<(Vec<P2>, Vec<u32>)>;

/// Maps font units to screen points: screen = origin + (x * s, -y * s).
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub origin: Pos2,
    pub s: f32,
}

impl View {
    pub fn to_screen(&self, p: P2) -> Pos2 {
        Pos2::new(self.origin.x + p[0] * self.s, self.origin.y - p[1] * self.s)
    }
    pub fn to_font(&self, p: Pos2) -> P2 {
        [(p.x - self.origin.x) / self.s, (self.origin.y - p.y) / self.s]
    }
    pub fn shifted(&self, dx_units: f32) -> View {
        View { origin: Pos2::new(self.origin.x + dx_units * self.s, self.origin.y), s: self.s }
    }
}

#[derive(Default)]
pub struct Cache {
    fills: HashMap<(usize, usize, usize), (u64, Tess)>,
    template: Option<TemplateFont>,
    template_fills: HashMap<u32, Option<(Tess, f32)>>,
}

impl Cache {
    pub fn clear_doc(&mut self, doc: usize) {
        self.fills.retain(|k, _| k.0 != doc);
    }

    pub fn clear(&mut self) {
        self.fills.clear();
    }

    /// Fill geometry of a glyph layer (components decomposed), cached by revision.
    pub fn fill(&mut self, doc: usize, font: &Font, gi: usize, m: usize) -> Tess {
        let rev = font.deep_rev(gi, 0);
        let key = (doc, gi, m);
        if let Some((r, t)) = self.fills.get(&key) {
            if *r == rev {
                return t.clone();
            }
        }
        let contours = font.flat_contours(gi, m);
        let t: Tess = Arc::new(geom::tessellate(&contours, 1.0));
        self.fills.insert(key, (rev, t.clone()));
        t
    }

    /// Template outline for a code point (scaled to `upm`), with its advance width.
    pub fn template(&mut self, u: u32, upm: f32) -> Option<(Tess, f32)> {
        if self.template.is_none() {
            self.template = TemplateFont::load();
        }
        let tf = self.template.as_ref()?;
        let entry = self.template_fills.entry(u).or_insert_with(|| tf.glyph(u).map(|(c, w)| (Arc::new(geom::tessellate(&c, 1.0)) as Tess, w)));
        entry.as_ref().map(|(t, w)| {
            let k = upm / tf.upm;
            if (k - 1.0).abs() < 1e-3 {
                (t.clone(), *w)
            } else {
                (Arc::new((t.0.iter().map(|p| [p[0] * k, p[1] * k]).collect(), t.1.clone())), w * k)
            }
        })
    }

    pub fn template_contours(&mut self, u: u32, upm: f32) -> Option<(Vec<Contour>, f32)> {
        if self.template.is_none() {
            self.template = TemplateFont::load();
        }
        let tf = self.template.as_ref()?;
        let (mut cs, w) = tf.glyph(u)?;
        let k = upm / tf.upm;
        for c in &mut cs {
            for p in &mut c.points {
                p.x *= k;
                p.y *= k;
            }
        }
        Some((cs, w * k))
    }
}

pub fn paint_fill(painter: &Painter, t: &Tess, view: View, color: Color32) {
    if t.1.is_empty() {
        return;
    }
    let mut mesh = Mesh::default();
    mesh.vertices.reserve(t.0.len());
    for p in &t.0 {
        mesh.colored_vertex(view.to_screen(*p), color);
    }
    mesh.indices = t.1.clone();
    painter.add(Shape::mesh(mesh));
}

/// Screen-space polyline of a contour for stroking.
pub fn contour_screen_points(c: &Contour, view: View) -> Vec<Pos2> {
    let steps = ((view.s * 40.0).clamp(4.0, 24.0)) as usize;
    geom::flatten_contour(c, steps).into_iter().map(|p| view.to_screen(p)).collect()
}

pub fn stroke_contour(painter: &Painter, c: &Contour, view: View, stroke: Stroke) {
    let pts = contour_screen_points(c, view);
    if pts.len() < 2 {
        return;
    }
    if c.closed {
        painter.add(Shape::closed_line(pts, stroke));
    } else {
        painter.add(Shape::line(pts, stroke));
    }
}

/// The bundled Inter font, used for faint template letters in empty glyph cells.
pub struct TemplateFont {
    data: &'static [u8],
    pub upm: f32,
}

struct Pen {
    cs: Vec<Contour>,
    cur: Vec<Point>,
}

impl Pen {
    fn finish(&mut self) {
        if self.cur.len() > 1 {
            let mut pts = std::mem::take(&mut self.cur);
            let last = pts.last().unwrap().clone();
            if (last.x - pts[0].x).abs() < 0.01 && (last.y - pts[0].y).abs() < 0.01 {
                pts[0].t = last.t;
                pts.pop();
            } else {
                pts[0].t = PtType::Line;
            }
            self.cs.push(Contour { points: pts, closed: true });
        }
        self.cur.clear();
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

impl TemplateFont {
    fn load() -> Option<Self> {
        let data: &'static [u8] = include_bytes!("../assets/Inter-Regular.ttf");
        let face = ttf_parser::Face::parse(data, 0).ok()?;
        Some(TemplateFont { data, upm: face.units_per_em() as f32 })
    }

    pub fn glyph(&self, u: u32) -> Option<(Vec<Contour>, f32)> {
        let face = ttf_parser::Face::parse(self.data, 0).ok()?;
        let id = face.glyph_index(char::from_u32(u)?)?;
        let mut pen = Pen { cs: Vec::new(), cur: Vec::new() };
        face.outline_glyph(id, &mut pen);
        pen.finish();
        Some((pen.cs, face.glyph_hor_advance(id).unwrap_or(0) as f32))
    }
}
