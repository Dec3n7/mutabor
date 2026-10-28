use serde::Serialize;
use std::path::Path;

/// Broad file category; decides which actions the menu offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Video,
    Audio,
    Image,
    Pdf,
    Markup,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileInfo {
    pub path: String,
    pub name: String,
    pub ext: String,
    pub kind: Kind,
    pub size: u64,
}

pub fn kind_of(ext: &str) -> Kind {
    match ext {
        "mp4" | "mov" | "avi" | "mkv" | "webm" | "flv" | "wmv" | "3gp" | "m4v" => Kind::Video,
        "mp3" | "wav" | "ogg" | "oga" | "flac" | "m4a" | "aac" | "opus" | "wma" => Kind::Audio,
        "jpg" | "jpeg" | "png" | "webp" | "avif" | "gif" | "bmp" | "tif" | "tiff" | "ico"
        | "heic" | "heif" | "svg" => Kind::Image,
        "pdf" => Kind::Pdf,
        "md" | "markdown" | "html" | "htm" | "epub" | "fb2" | "txt" | "docx" => Kind::Markup,
        _ => Kind::Unknown,
    }
}

/// Builds `FileInfo` for the selection. A selected folder stands for the
/// files directly inside it (not subfolders), sorted by name; missing paths
/// are skipped.
pub fn describe(paths: impl IntoIterator<Item = String>) -> Vec<FileInfo> {
    let mut out = Vec::new();
    for path in paths {
        let p = Path::new(&path);
        if p.is_dir() {
            let mut inside: Vec<_> = std::fs::read_dir(p)
                .map(|d| d.flatten().map(|e| e.path()).filter(|e| e.is_file()).collect())
                .unwrap_or_default();
            inside.sort();
            out.extend(inside.iter().filter_map(|f| info(f)));
        } else {
            out.extend(info(p));
        }
    }
    out
}

fn info(p: &Path) -> Option<FileInfo> {
    let meta = std::fs::metadata(p).ok().filter(|m| m.is_file())?;
    let name = p.file_name()?.to_string_lossy().into_owned();
    let ext = p
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    Some(FileInfo {
        kind: kind_of(&ext),
        path: p.to_string_lossy().into_owned(),
        name,
        ext,
        size: meta.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds() {
        assert_eq!(kind_of("mp4"), Kind::Video);
        assert_eq!(kind_of("heic"), Kind::Image);
        assert_eq!(kind_of("ogg"), Kind::Audio);
        assert_eq!(kind_of("pdf"), Kind::Pdf);
        assert_eq!(kind_of("xyz"), Kind::Unknown);
    }

    #[test]
    fn folders_expand_to_their_files() {
        let dir = std::env::temp_dir().join("mutabor-test-describe");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        for f in ["b.png", "a.mp4", "sub/c.mp3"] {
            std::fs::write(dir.join(f), b"x").unwrap();
        }
        let names: Vec<String> = describe([dir.to_string_lossy().into_owned()]).into_iter().map(|f| f.name).collect();
        assert_eq!(names, ["a.mp4", "b.png"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
