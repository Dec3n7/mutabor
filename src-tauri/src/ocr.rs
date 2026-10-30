//! Text recognition: Tesseract, compiled into the app by tesseract-rs, reads
//! the language files of the "ocr" engine (tessdata_fast, downloaded once).

use std::path::{Path, PathBuf};
use tesseract_rs::TesseractAPI;

/// The file whose presence means the languages are downloaded.
pub const MARKER_FILE: &str = "eng.traineddata";

/// Two-letter code -> Tesseract's name for that language's file.
pub fn language(code: &str) -> Option<&'static str> {
    Some(match code {
        "en" => "eng",
        "ru" => "rus",
        "uk" => "ukr",
        "es" => "spa",
        "pt" => "por",
        "de" => "deu",
        "fr" => "fra",
        "it" => "ita",
        "pl" => "pol",
        "tr" => "tur",
        "zh" => "chi_sim",
        "ja" => "jpn",
        "ko" => "kor",
        _ => return None,
    })
}

/// "ru" -> "rus+eng": English is read too, it turns up in most texts.
/// "auto" picks the interface language, `ui` ("ru", or "auto" for the system's).
pub fn languages(code: &str, ui: &str) -> String {
    let code = if code == "auto" { ui } else { code };
    let code = if code == "auto" { sys_locale::get_locale().unwrap_or_default() } else { code.to_string() };
    match language(code.get(..2).unwrap_or("")) {
        Some("eng") | None => "eng".into(),
        Some(lang) => format!("{lang}+eng"),
    }
}

/// Recognises the text of an 8-bit grey picture.
pub fn text(tessdata: &Path, languages: &str, gray: &[u8], width: u32, height: u32) -> Result<String, String> {
    let api = TesseractAPI::new();
    api.init(ascii_path(tessdata), languages).map_err(|e| format!("tesseract: {e}"))?;
    api.set_image(gray, width as i32, height as i32, 1, width as i32).map_err(|e| format!("tesseract: {e}"))?;
    // The pictures come without a resolution; this silences the warning.
    let _ = api.set_source_resolution(300);
    api.get_utf8_text().map_err(|e| format!("tesseract: {e}"))
}

/// Tesseract opens files with the ANSI code page on Windows, so a user name
/// in Cyrillic would hide the languages: use the short 8.3 path there.
#[cfg(windows)]
fn ascii_path(path: &Path) -> PathBuf {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    if path.to_str().is_some_and(|s| s.is_ascii()) {
        return path.to_path_buf();
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetShortPathNameW(long: *const u16, short: *mut u16, len: u32) -> u32;
    }
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut buf = vec![0u16; 1024];
    let n = unsafe { GetShortPathNameW(wide.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) } as usize;
    if n == 0 || n >= buf.len() {
        return path.to_path_buf();
    }
    PathBuf::from(std::ffi::OsString::from_wide(&buf[..n]))
}

#[cfg(not(windows))]
fn ascii_path(path: &Path) -> PathBuf {
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_languages() {
        assert_eq!(languages("ru", "en"), "rus+eng");
        assert_eq!(languages("en", "ru"), "eng");
        assert_eq!(languages("auto", "zh"), "chi_sim+eng");
        assert_eq!(languages("xx", "ru"), "eng");
    }

    /// Needs the language files in ~/Downloads/tessdata and Windows' Arial:
    /// `cargo test ocr -- --ignored`. Text drawn by ffmpeg must be read back.
    #[test]
    #[ignore]
    fn reads_text() {
        let tessdata = PathBuf::from(std::env::var("USERPROFILE").unwrap_or_default()).join("Downloads").join("tessdata");
        let font = Path::new(r"C:\Windows\Fonts\arial.ttf");
        if !tessdata.join(MARKER_FILE).exists() || !font.exists() {
            eprintln!("skipped: no {} or no Arial", tessdata.display());
            return;
        }
        let ffmpeg = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/binaries"))
            .expect("run `npm run tools` first")
            .flatten()
            .map(|e| e.path())
            .find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("mutabor-ffmpeg")))
            .expect("ffmpeg sidecar");
        let (w, h) = (900u32, 200u32);
        let draw = format!(
            "color=white:s={w}x{h},drawtext=fontfile='C\\:/Windows/Fonts/arial.ttf':text='Hello Mutabor 2026':fontsize=64:x=40:y=60,format=gray"
        );
        let out = std::process::Command::new(ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", &draw])
            .args(["-frames:v", "1", "-f", "rawvideo", "-"])
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let text = text(&tessdata, "eng", &out.stdout, w, h).unwrap();
        assert_eq!(text.trim(), "Hello Mutabor 2026");
    }
}
