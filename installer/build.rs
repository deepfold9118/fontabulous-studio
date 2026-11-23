//! Embeds the Fontabulous Studio icon in the installer and checks the app build exists.

#[path = "../src/logo.rs"]
mod logo;

fn png_bytes(size: u32) -> Vec<u8> {
    let rgba = logo::rasterize(size);
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, size, size);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().expect("png header");
        w.write_image_data(&rgba).expect("png data");
    }
    out
}

fn main() {
    println!("cargo:rerun-if-changed=../src/logo.rs");
    println!("cargo:rerun-if-changed=../target/release/fontabulous_studio.exe");
    let app = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/release/fontabulous_studio.exe");
    if !app.exists() {
        panic!("Build the app first: cargo build --release (in the app folder). Missing {}", app.display());
    }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let sizes = [16u32, 20, 24, 32, 40, 48, 64, 128, 256];
    let images: Vec<Vec<u8>> = sizes.iter().map(|&s| png_bytes(s)).collect();
    let mut ico = Vec::new();
    ico.extend_from_slice(&[0, 0, 1, 0]);
    ico.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (s, img) in sizes.iter().zip(&images) {
        let b = if *s >= 256 { 0 } else { *s as u8 };
        ico.extend_from_slice(&[b, b, 0, 0]);
        ico.extend_from_slice(&1u16.to_le_bytes());
        ico.extend_from_slice(&32u16.to_le_bytes());
        ico.extend_from_slice(&(img.len() as u32).to_le_bytes());
        ico.extend_from_slice(&offset.to_le_bytes());
        offset += img.len() as u32;
    }
    for img in &images {
        ico.extend_from_slice(img);
    }
    let ico_path = out_dir.join("setup.ico");
    std::fs::write(&ico_path, ico).unwrap();
    let rc_path = out_dir.join("setup.rc");
    let ico_str = ico_path.display().to_string().replace(char::from(92), "/");
    std::fs::write(&rc_path, format!("1 ICON \"{ico_str}\"\n")).unwrap();
    let _ = embed_resource::compile(&rc_path, embed_resource::NONE);
}
