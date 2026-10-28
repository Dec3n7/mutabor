use serde::Serialize;
use std::path::Path;

/// Broad file category; decides which actions the menu offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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

/// Builds `FileInfo` for regular files; folders and missing paths are skipped.
pub fn describe(paths: impl IntoIterator<Item = String>) -> Vec<FileInfo> {
    paths
        .into_iter()
        .filter_map(|path| {
            let meta = std::fs::metadata(&path).ok().filter(|m| m.is_file())?;
            let p = Path::new(&path);
            let name = p.file_name()?.to_string_lossy().into_owned();
            let ext = p
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            Some(FileInfo {
                kind: kind_of(&ext),
                path,
                name,
                ext,
                size: meta.len(),
            })
        })
        .collect()
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
}
