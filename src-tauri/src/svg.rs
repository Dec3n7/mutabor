//! SVG input: drawn into a PNG with resvg (pure Rust, part of the app), after
//! which the usual picture recipes take over.

use resvg::{tiny_skia, usvg};
use std::path::Path;
use std::sync::{Arc, OnceLock};

/// Small icons are drawn at least this large: a vector loses nothing by it.
const MIN_SIDE: f32 = 1024.0;
/// Guards against a huge declared size eating all memory.
const MAX_SIDE: f32 = 8192.0;

/// System fonts, for SVGs with text. Loading takes a moment, so once.
fn fonts() -> Arc<usvg::fontdb::Database> {
    static DB: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = usvg::fontdb::Database::new();
        db.load_system_fonts();
        Arc::new(db)
    })
    .clone()
}

/// Pixel size for drawing: `side` is the wanted longest side, if any.
fn target_size(w: f32, h: f32, side: Option<u32>) -> (u32, u32) {
    let longest = w.max(h).max(1.0);
    let want = side.map_or(longest.max(MIN_SIDE), |s| s as f32).clamp(1.0, MAX_SIDE);
    let scale = want / longest;
    (((w * scale).round() as u32).max(1), ((h * scale).round() as u32).max(1))
}

pub fn render(svg: &Path, png: &Path, side: Option<u32>) -> Result<(), String> {
    let data = std::fs::read(svg).map_err(|e| e.to_string())?;
    let opt = usvg::Options {
        // Pictures linked by relative paths sit next to the SVG.
        resources_dir: svg.parent().map(Path::to_path_buf),
        fontdb: fonts(),
        ..Default::default()
    };
    let tree = usvg::Tree::from_data(&data, &opt).map_err(|e| format!("svg: {e}"))?;
    let size = tree.size();
    let (w, h) = target_size(size.width(), size.height(), side);
    let mut pixmap = tiny_skia::Pixmap::new(w, h).ok_or("svg: empty picture")?;
    let transform = tiny_skia::Transform::from_scale(w as f32 / size.width(), h as f32 / size.height());
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    pixmap.save_png(png).map_err(|e| format!("svg: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        // A 24 px icon is drawn at 1024, keeping its proportions.
        assert_eq!(target_size(24.0, 12.0, None), (1024, 512));
        assert_eq!(target_size(3000.0, 1500.0, None), (3000, 1500));
        assert_eq!(target_size(24.0, 12.0, Some(512)), (512, 256));
        assert_eq!(target_size(100_000.0, 10.0, None).0, 8192);
    }

    #[test]
    fn draws_a_picture() {
        let dir = std::env::temp_dir().join("mutabor-test-svg");
        let _ = std::fs::create_dir_all(&dir);
        let svg = dir.join("dot.svg");
        std::fs::write(
            &svg,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><circle cx="5" cy="5" r="5" fill="red"/><text x="10" y="9" font-size="8">A</text></svg>"#,
        )
        .unwrap();
        let png = dir.join("dot.png");
        render(&svg, &png, None).unwrap();
        let bytes = std::fs::read(&png).unwrap();
        // PNG signature, then IHDR width and height.
        assert_eq!(&bytes[1..4], b"PNG");
        assert_eq!(u32::from_be_bytes(bytes[16..20].try_into().unwrap()), 1024);
        assert_eq!(u32::from_be_bytes(bytes[20..24].try_into().unwrap()), 512);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
