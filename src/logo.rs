//! The Fontabulous Studio mark: a pen nib (accent diamond) with a dark breather
//! hole and slit. The title bar draws it from these numbers, and the program icon
//! (window, taskbar and the Windows .exe via build.rs) is rasterised from them too,
//! so both always match.

#![allow(dead_code)]

/// Accent colour of the nib (the theme accent).
pub const COLOR: [u8; 3] = [0xe8, 0x57, 0x3f];
/// Colour of the hole and slit.
pub const DARK: [u8; 3] = [0x0b, 0x0d, 0x12];

/// Geometry in mark units (y down, origin at the mark centre; 18 units = full height
/// of the title-bar slot).
pub const NIB: [[f32; 2]; 4] = [[0.0, -8.0], [6.0, 1.0], [0.0, 7.0], [-6.0, 1.0]];
pub const HOLE_C: [f32; 2] = [0.0, 1.5];
pub const HOLE_R: f32 = 1.6;
pub const SLIT_W: f32 = 1.2;
pub const SLIT_END_Y: f32 = 7.0;

fn in_nib(x: f32, y: f32) -> bool {
    // Convex quad: inside when on the same side of every edge.
    let mut sign = 0.0f32;
    for i in 0..4 {
        let a = NIB[i];
        let b = NIB[(i + 1) % 4];
        let c = (b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0]);
        if c != 0.0 {
            if sign == 0.0 {
                sign = c.signum();
            } else if c.signum() != sign {
                return false;
            }
        }
    }
    true
}

fn in_hole(x: f32, y: f32) -> bool {
    let (dx, dy) = (x - HOLE_C[0], y - HOLE_C[1]);
    dx * dx + dy * dy <= HOLE_R * HOLE_R || (x.abs() <= SLIT_W / 2.0 && y >= HOLE_C[1] && y <= SLIT_END_Y)
}

/// RGBA pixels (straight alpha) of the mark in a `size`×`size` square.
pub fn rasterize(size: u32) -> Vec<u8> {
    let n = size as f32;
    // The mark spans 12×15 units; leave a small margin and centre it.
    let k = n / 16.5;
    let (cx, cy) = (n / 2.0, n / 2.0 + 0.5 * k);
    const SS: u32 = 4;
    let mut out = vec![0u8; (size * size * 4) as usize];
    for py in 0..size {
        for px in 0..size {
            let (mut r, mut g, mut b, mut a) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
            for sy in 0..SS {
                for sx in 0..SS {
                    let x = (px as f32 + (sx as f32 + 0.5) / SS as f32 - cx) / k;
                    let y = (py as f32 + (sy as f32 + 0.5) / SS as f32 - cy) / k;
                    if !in_nib(x, y) {
                        continue;
                    }
                    let c = if in_hole(x, y) { DARK } else { COLOR };
                    r += c[0] as f32;
                    g += c[1] as f32;
                    b += c[2] as f32;
                    a += 1.0;
                }
            }
            if a > 0.0 {
                let i = ((py * size + px) * 4) as usize;
                out[i] = (r / a).round() as u8;
                out[i + 1] = (g / a).round() as u8;
                out[i + 2] = (b / a).round() as u8;
                out[i + 3] = (a / (SS * SS) as f32 * 255.0).round() as u8;
            }
        }
    }
    out
}
