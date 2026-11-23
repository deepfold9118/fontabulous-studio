//! Outline geometry: segments, flattening, bounds, hit testing, splitting,
//! extremes, cubic-to-quadratic conversion and fill tessellation.

#![allow(dead_code)]

use crate::font::{Contour, Point, PtType};

pub type P2 = [f32; 2];

#[derive(Clone, Debug)]
pub struct Segment {
    pub start: usize,
    pub offs: Vec<usize>,
    pub end: usize,
    pub kind: PtType,
}

/// The segments of a contour (each ends at an on-curve point).
pub fn segments(c: &Contour) -> Vec<Segment> {
    let n = c.points.len();
    let on: Vec<usize> = (0..n).filter(|&i| c.points[i].t.on_curve()).collect();
    let mut out = Vec::new();
    if on.is_empty() || n < 2 {
        return out;
    }
    let count = if c.closed { on.len() } else { on.len() - 1 };
    for k in 0..count {
        let (start, end) = if c.closed { (on[(k + on.len() - 1) % on.len()], on[k]) } else { (on[k], on[k + 1]) };
        let mut offs = Vec::new();
        let mut i = (start + 1) % n;
        while i != end {
            offs.push(i);
            i = (i + 1) % n;
        }
        if c.closed && on.len() == 1 && offs.is_empty() && start == end {
            continue;
        }
        out.push(Segment { start, offs, end, kind: c.points[end].t });
    }
    out
}

fn p(c: &Contour, i: usize) -> P2 {
    [c.points[i].x, c.points[i].y]
}

pub fn lerp(a: P2, b: P2, t: f32) -> P2 {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

pub fn cubic_at(p0: P2, p1: P2, p2: P2, p3: P2, t: f32) -> P2 {
    let u = 1.0 - t;
    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    [a * p0[0] + b * p1[0] + c * p2[0] + d * p3[0], a * p0[1] + b * p1[1] + c * p2[1] + d * p3[1]]
}

pub fn quad_at(p0: P2, p1: P2, p2: P2, t: f32) -> P2 {
    let u = 1.0 - t;
    [u * u * p0[0] + 2.0 * u * t * p1[0] + t * t * p2[0], u * u * p0[1] + 2.0 * u * t * p1[1] + t * t * p2[1]]
}

/// Primitive curve pieces of a segment.
pub enum Piece {
    Line(P2, P2),
    Quad(P2, P2, P2),
    Cubic(P2, P2, P2, P2),
}

pub fn segment_pieces(c: &Contour, s: &Segment) -> Vec<Piece> {
    let a = p(c, s.start);
    let b = p(c, s.end);
    match s.offs.len() {
        0 => vec![Piece::Line(a, b)],
        1 => vec![Piece::Quad(a, p(c, s.offs[0]), b)],
        2 if s.kind != PtType::QCurve => vec![Piece::Cubic(a, p(c, s.offs[0]), p(c, s.offs[1]), b)],
        _ => {
            // TrueType-style run of off-curves with implied on-curve midpoints.
            let mut out = Vec::new();
            let mut cur = a;
            for k in 0..s.offs.len() {
                let ctrl = p(c, s.offs[k]);
                let next = if k + 1 < s.offs.len() { lerp(ctrl, p(c, s.offs[k + 1]), 0.5) } else { b };
                out.push(Piece::Quad(cur, ctrl, next));
                cur = next;
            }
            out
        }
    }
}

/// Polyline approximation of a contour (closed contours do not repeat the start).
pub fn flatten_contour(c: &Contour, steps: usize) -> Vec<P2> {
    let mut out = Vec::new();
    let segs = segments(c);
    if segs.is_empty() {
        return c.points.iter().map(|q| [q.x, q.y]).collect();
    }
    out.push(p(c, segs[0].start));
    for s in &segs {
        for piece in segment_pieces(c, s) {
            match piece {
                Piece::Line(_, b) => out.push(b),
                Piece::Quad(a, b, d) => {
                    for i in 1..=steps {
                        out.push(quad_at(a, b, d, i as f32 / steps as f32));
                    }
                }
                Piece::Cubic(a, b, cc, d) => {
                    for i in 1..=steps {
                        out.push(cubic_at(a, b, cc, d, i as f32 / steps as f32));
                    }
                }
            }
        }
    }
    if c.closed && out.len() > 1 {
        out.pop();
    }
    out
}

fn cubic_extrema(p0: f32, p1: f32, p2: f32, p3: f32) -> Vec<f32> {
    // derivative: a t^2 + b t + c
    let a = -p0 + 3.0 * p1 - 3.0 * p2 + p3;
    let b = 2.0 * (p0 - 2.0 * p1 + p2);
    let c = p1 - p0;
    let mut ts = Vec::new();
    if a.abs() < 1e-6 {
        if b.abs() > 1e-6 {
            ts.push(-c / b);
        }
    } else {
        let d = b * b - 4.0 * a * c;
        if d >= 0.0 {
            let s = d.sqrt();
            ts.push((-b + s) / (2.0 * a));
            ts.push((-b - s) / (2.0 * a));
        }
    }
    ts.into_iter().filter(|t| *t > 0.0 && *t < 1.0).collect()
}

pub fn contour_bounds(c: &Contour) -> Option<[f32; 4]> {
    if c.points.is_empty() {
        return None;
    }
    let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    let mut add = |q: P2| {
        b[0] = b[0].min(q[0]);
        b[1] = b[1].min(q[1]);
        b[2] = b[2].max(q[0]);
        b[3] = b[3].max(q[1]);
    };
    let segs = segments(c);
    if segs.is_empty() {
        for q in &c.points {
            add([q.x, q.y]);
        }
        return Some(b);
    }
    for s in &segs {
        for piece in segment_pieces(c, s) {
            match piece {
                Piece::Line(a, d) => {
                    add(a);
                    add(d);
                }
                Piece::Quad(a, ctrl, d) => {
                    add(a);
                    add(d);
                    for axis in 0..2 {
                        let den = a[axis] - 2.0 * ctrl[axis] + d[axis];
                        if den.abs() > 1e-6 {
                            let t = (a[axis] - ctrl[axis]) / den;
                            if t > 0.0 && t < 1.0 {
                                add(quad_at(a, ctrl, d, t));
                            }
                        }
                    }
                }
                Piece::Cubic(a, c1, c2, d) => {
                    add(a);
                    add(d);
                    for axis in 0..2 {
                        for t in cubic_extrema(a[axis], c1[axis], c2[axis], d[axis]) {
                            add(cubic_at(a, c1, c2, d, t));
                        }
                    }
                }
            }
        }
    }
    Some(b)
}

pub fn union_bounds(a: Option<[f32; 4]>, b: Option<[f32; 4]>) -> Option<[f32; 4]> {
    match (a, b) {
        (None, x) | (x, None) => x,
        (Some(a), Some(b)) => Some([a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])]),
    }
}

pub fn rect_contour(x: f32, y: f32, w: f32, h: f32, hole: bool) -> Contour {
    let mut pts = vec![
        Point::new(x, y, PtType::Line),
        Point::new(x + w, y, PtType::Line),
        Point::new(x + w, y + h, PtType::Line),
        Point::new(x, y + h, PtType::Line),
    ];
    if hole {
        pts.reverse();
    }
    Contour { points: pts, closed: true }
}

pub fn ellipse_contour(cx: f32, cy: f32, rx: f32, ry: f32) -> Contour {
    let k = 0.552_284_8;
    let (kx, ky) = (rx * k, ry * k);
    let on = |x: f32, y: f32| {
        let mut q = Point::new(x, y, PtType::Curve);
        q.smooth = true;
        q
    };
    let off = |x: f32, y: f32| Point::new(x, y, PtType::Off);
    // Counter-clockwise starting at the bottom.
    let pts = vec![
        on(cx, cy - ry),
        off(cx + kx, cy - ry),
        off(cx + rx, cy - ky),
        on(cx + rx, cy),
        off(cx + rx, cy + ky),
        off(cx + kx, cy + ry),
        on(cx, cy + ry),
        off(cx - kx, cy + ry),
        off(cx - rx, cy + ky),
        on(cx - rx, cy),
        off(cx - rx, cy - ky),
        off(cx - kx, cy - ry),
    ];
    // The segment ending at the first point uses the last two off-curves: rotate so the
    // list starts after them, matching the closed-contour convention.
    let mut c = Contour { points: pts, closed: true };
    c.points.rotate_left(1);
    c.points.rotate_right(1);
    c
}

pub fn dist(a: P2, b: P2) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

fn dist_to_line(q: P2, a: P2, b: P2) -> (f32, f32) {
    let d = [b[0] - a[0], b[1] - a[1]];
    let len2 = d[0] * d[0] + d[1] * d[1];
    let t = if len2 < 1e-9 { 0.0 } else { (((q[0] - a[0]) * d[0] + (q[1] - a[1]) * d[1]) / len2).clamp(0.0, 1.0) };
    let pr = [a[0] + d[0] * t, a[1] + d[1] * t];
    (dist(q, pr), t)
}

/// Nearest segment to `q`: (segment index, t along the whole segment, distance).
pub fn nearest_segment(c: &Contour, q: P2) -> Option<(usize, f32, f32)> {
    let mut best: Option<(usize, f32, f32)> = None;
    for (si, s) in segments(c).iter().enumerate() {
        let pieces = segment_pieces(c, s);
        let np = pieces.len() as f32;
        for (pi, piece) in pieces.iter().enumerate() {
            let steps = 24;
            let f = |t: f32| match piece {
                Piece::Line(a, b) => lerp(*a, *b, t),
                Piece::Quad(a, b, d) => quad_at(*a, *b, *d, t),
                Piece::Cubic(a, b, cc, d) => cubic_at(*a, *b, *cc, *d, t),
            };
            let mut prev = f(0.0);
            for i in 1..=steps {
                let t1 = i as f32 / steps as f32;
                let cur = f(t1);
                let (d, lt) = dist_to_line(q, prev, cur);
                let t = (pi as f32 + (t1 - 1.0 / steps as f32) + lt / steps as f32) / np;
                if best.map(|b| d < b.2).unwrap_or(true) {
                    best = Some((si, t, d));
                }
                prev = cur;
            }
        }
    }
    best
}

/// Point-in-outline test (non-zero winding) over flattened contours.
pub fn winding(contours: &[Contour], q: P2) -> i32 {
    let mut w = 0;
    for c in contours {
        if !c.closed {
            continue;
        }
        let poly = flatten_contour(c, 12);
        let n = poly.len();
        for i in 0..n {
            let a = poly[i];
            let b = poly[(i + 1) % n];
            if a[1] <= q[1] {
                if b[1] > q[1] && (b[0] - a[0]) * (q[1] - a[1]) - (q[0] - a[0]) * (b[1] - a[1]) > 0.0 {
                    w += 1;
                }
            } else if b[1] <= q[1] && (b[0] - a[0]) * (q[1] - a[1]) - (q[0] - a[0]) * (b[1] - a[1]) < 0.0 {
                w -= 1;
            }
        }
    }
    w
}

pub fn split_cubic(p0: P2, p1: P2, p2: P2, p3: P2, t: f32) -> ([P2; 4], [P2; 4]) {
    let a = lerp(p0, p1, t);
    let b = lerp(p1, p2, t);
    let c = lerp(p2, p3, t);
    let d = lerp(a, b, t);
    let e = lerp(b, c, t);
    let f = lerp(d, e, t);
    ([p0, a, d, f], [f, e, c, p3])
}

/// Insert a new on-curve node into segment `si` at parameter t. Returns the new point index.
pub fn insert_node(c: &mut Contour, si: usize, t: f32) -> Option<usize> {
    let segs = segments(c);
    let s = segs.get(si)?.clone();
    let a = p(c, s.start);
    let b = p(c, s.end);
    match (s.offs.len(), s.kind) {
        (0, _) => {
            let m = lerp(a, b, t);
            let at = s.start + 1;
            let mut np = Point::new(m[0], m[1], PtType::Line);
            np.sel = true;
            if s.end == 0 && c.closed {
                c.points.push(np);
                Some(c.points.len() - 1)
            } else {
                c.points.insert(at, np);
                Some(at)
            }
        }
        (2, k) if k != PtType::QCurve => {
            let (l, r) = split_cubic(a, p(c, s.offs[0]), p(c, s.offs[1]), b, t);
            let (i1, i2) = (s.offs[0], s.offs[1]);
            c.points[i1].x = l[1][0];
            c.points[i1].y = l[1][1];
            c.points[i2].x = l[2][0];
            c.points[i2].y = l[2][1];
            let mut mid = Point::new(l[3][0], l[3][1], PtType::Curve);
            mid.smooth = true;
            mid.sel = true;
            let new_pts = vec![mid, Point::new(r[1][0], r[1][1], PtType::Off), Point::new(r[2][0], r[2][1], PtType::Off)];
            let at = i2 + 1;
            if at > c.points.len() {
                return None;
            }
            for (k, q) in new_pts.into_iter().enumerate() {
                c.points.insert(at + k, q);
            }
            Some(at)
        }
        _ => None,
    }
}

/// Convert a line segment into a cubic curve (controls at thirds).
pub fn line_to_curve(c: &mut Contour, si: usize) {
    let segs = segments(c);
    let Some(s) = segs.get(si).cloned() else { return };
    if !s.offs.is_empty() {
        return;
    }
    let a = p(c, s.start);
    let b = p(c, s.end);
    let c1 = lerp(a, b, 1.0 / 3.0);
    let c2 = lerp(a, b, 2.0 / 3.0);
    c.points[s.end].t = PtType::Curve;
    let at = if s.end == 0 { c.points.len() } else { s.end };
    c.points.insert(at, Point::new(c2[0], c2[1], PtType::Off));
    c.points.insert(at, Point::new(c1[0], c1[1], PtType::Off));
}

/// Convert a curve segment into a straight line (drops its controls).
pub fn curve_to_line(c: &mut Contour, si: usize) {
    let segs = segments(c);
    let Some(s) = segs.get(si).cloned() else { return };
    if s.offs.is_empty() {
        return;
    }
    c.points[s.end].t = PtType::Line;
    c.points[s.end].smooth = false;
    let mut offs = s.offs.clone();
    offs.sort_unstable();
    for i in offs.into_iter().rev() {
        c.points.remove(i);
    }
}

/// Add nodes at the horizontal and vertical extremes of cubic segments.
pub fn add_extremes(c: &mut Contour) -> usize {
    let mut added = 0;
    loop {
        let segs = segments(c);
        let mut did = false;
        for (si, s) in segs.iter().enumerate() {
            if s.offs.len() != 2 || s.kind == PtType::QCurve {
                continue;
            }
            let (a, c1, c2, d) = (p(c, s.start), p(c, s.offs[0]), p(c, s.offs[1]), p(c, s.end));
            let mut ts: Vec<f32> = Vec::new();
            for axis in 0..2 {
                ts.extend(cubic_extrema(a[axis], c1[axis], c2[axis], d[axis]));
            }
            if let Some(&t) = ts.iter().find(|t| **t > 0.03 && **t < 0.97) {
                insert_node(c, si, t);
                added += 1;
                did = true;
                break;
            }
        }
        if !did || added > 200 {
            break;
        }
    }
    for q in &mut c.points {
        q.sel = false;
    }
    added
}

/// Approximate a cubic with quadratic pieces: returns (control, end) pairs.
pub fn cubic_to_quads(p0: P2, p1: P2, p2: P2, p3: P2, tol: f32) -> Vec<(P2, P2)> {
    for n in 1..=16usize {
        let mut out = Vec::with_capacity(n);
        let mut ok = true;
        for i in 0..n {
            let t0 = i as f32 / n as f32;
            let t1 = (i + 1) as f32 / n as f32;
            // Sub-cubic between t0 and t1.
            let (_, right) = split_cubic(p0, p1, p2, p3, t0);
            let local = if t0 < 1.0 { (t1 - t0) / (1.0 - t0) } else { 1.0 };
            let (sub, _) = split_cubic(right[0], right[1], right[2], right[3], local.clamp(0.0, 1.0));
            let ctrl = [
                (3.0 * (sub[1][0] + sub[2][0]) - (sub[0][0] + sub[3][0])) / 4.0,
                (3.0 * (sub[1][1] + sub[2][1]) - (sub[0][1] + sub[3][1])) / 4.0,
            ];
            for k in 1..4 {
                let t = k as f32 / 4.0;
                if dist(quad_at(sub[0], ctrl, sub[3], t), cubic_at(sub[0], sub[1], sub[2], sub[3], t)) > tol {
                    ok = false;
                }
            }
            out.push((ctrl, sub[3]));
        }
        if ok || n == 16 {
            return out;
        }
    }
    unreachable!()
}

/// Rebuild a contour with only quadratic curves (for TrueType output).
pub fn to_quadratic(c: &Contour, tol: f32) -> Contour {
    let segs = segments(c);
    if segs.is_empty() {
        return c.clone();
    }
    let mut pts: Vec<Point> = Vec::new();
    let first = p(c, segs[0].start);
    let mut body: Vec<Point> = Vec::new();
    for s in &segs {
        for piece in segment_pieces(c, s) {
            match piece {
                Piece::Line(_, b) => body.push(Point::new(b[0], b[1], PtType::Line)),
                Piece::Quad(_, ctrl, b) => {
                    body.push(Point::new(ctrl[0], ctrl[1], PtType::Off));
                    body.push(Point::new(b[0], b[1], PtType::QCurve));
                }
                Piece::Cubic(a, c1, c2, b) => {
                    for (ctrl, e) in cubic_to_quads(a, c1, c2, b, tol) {
                        body.push(Point::new(ctrl[0], ctrl[1], PtType::Off));
                        body.push(Point::new(e[0], e[1], PtType::QCurve));
                    }
                }
            }
        }
    }
    if c.closed {
        // body ends with the start point again; move it to the front.
        if let Some(last) = body.pop() {
            pts.push(last);
        }
        pts.extend(body);
        // Keep closed-contour convention: the first point is on-curve; its incoming
        // segment's control (if any) stays at the end of the list.
        let firstp = pts.remove(0);
        pts.insert(0, firstp);
    } else {
        pts.push(Point::new(first[0], first[1], PtType::Move));
        pts.extend(body);
    }
    Contour { points: pts, closed: c.closed }
}

/// Simplify a freehand polyline into a smooth cubic contour (Catmull-Rom fit).
pub fn fit_freehand(points: &[P2], closed: bool) -> Contour {
    // Thin out the input.
    let mut pts: Vec<P2> = Vec::new();
    for q in points {
        if pts.last().map(|l| dist(*l, *q) > 12.0).unwrap_or(true) {
            pts.push(*q);
        }
    }
    if pts.len() < 2 {
        return Contour { points: pts.iter().map(|q| Point::new(q[0], q[1], PtType::Move)).collect(), closed: false };
    }
    let n = pts.len();
    let get = |i: isize| -> P2 {
        if closed {
            pts[((i % n as isize + n as isize) % n as isize) as usize]
        } else {
            pts[i.clamp(0, n as isize - 1) as usize]
        }
    };
    let mut out = vec![Point::new(pts[0][0], pts[0][1], if closed { PtType::Curve } else { PtType::Move })];
    let count = if closed { n } else { n - 1 };
    for i in 0..count as isize {
        let (p0, p1, p2, p3) = (get(i - 1), get(i), get(i + 1), get(i + 2));
        let c1 = [p1[0] + (p2[0] - p0[0]) / 6.0, p1[1] + (p2[1] - p0[1]) / 6.0];
        let c2 = [p2[0] - (p3[0] - p1[0]) / 6.0, p2[1] - (p3[1] - p1[1]) / 6.0];
        out.push(Point::new(c1[0], c1[1], PtType::Off));
        out.push(Point::new(c2[0], c2[1], PtType::Off));
        let mut e = Point::new(p2[0], p2[1], PtType::Curve);
        e.smooth = true;
        out.push(e);
    }
    if closed {
        // last on-curve duplicates the first; drop it and keep its controls.
        out.pop();
        out[0].smooth = true;
    }
    Contour { points: out, closed }
}

/// Fill tessellation in font units: (vertices, triangle indices).
pub fn tessellate(contours: &[Contour], tolerance: f32) -> (Vec<P2>, Vec<u32>) {
    use lyon_tessellation::math::point;
    use lyon_tessellation::path::Path;
    use lyon_tessellation::{BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers};
    let mut b = Path::builder();
    let mut any = false;
    for c in contours {
        if !c.closed {
            continue;
        }
        let segs = segments(c);
        if segs.is_empty() {
            continue;
        }
        let s0 = p(c, segs[0].start);
        b.begin(point(s0[0], s0[1]));
        for s in &segs {
            for piece in segment_pieces(c, s) {
                match piece {
                    Piece::Line(_, e) => {
                        b.line_to(point(e[0], e[1]));
                    }
                    Piece::Quad(_, c1, e) => {
                        b.quadratic_bezier_to(point(c1[0], c1[1]), point(e[0], e[1]));
                    }
                    Piece::Cubic(_, c1, c2, e) => {
                        b.cubic_bezier_to(point(c1[0], c1[1]), point(c2[0], c2[1]), point(e[0], e[1]));
                    }
                }
            }
        }
        b.end(true);
        any = true;
    }
    if !any {
        return (Vec::new(), Vec::new());
    }
    let path = b.build();
    let mut geo: VertexBuffers<P2, u32> = VertexBuffers::new();
    let mut tess = FillTessellator::new();
    let _ = tess.tessellate_path(
        &path,
        &FillOptions::non_zero().with_tolerance(tolerance.max(0.01)),
        &mut BuffersBuilder::new(&mut geo, |v: FillVertex| v.position().to_array()),
    );
    (geo.vertices, geo.indices)
}

/// Round every coordinate to integers.
pub fn round_contour(c: &mut Contour) {
    for q in &mut c.points {
        q.x = q.x.round();
        q.y = q.y.round();
    }
}

/// Remove consecutive duplicate on-curve points (and degenerate segments).
pub fn clean_contour(c: &mut Contour) -> usize {
    let mut removed = 0;
    let mut i = 0;
    while c.points.len() > 2 && i < c.points.len() {
        let n = c.points.len();
        let j = (i + 1) % n;
        let (a, b) = (&c.points[i], &c.points[j]);
        if a.t.on_curve() && b.t.on_curve() && (a.x - b.x).abs() < 0.01 && (a.y - b.y).abs() < 0.01 && (c.closed || j != 0) {
            c.points.remove(j);
            removed += 1;
            continue;
        }
        i += 1;
    }
    removed
}
