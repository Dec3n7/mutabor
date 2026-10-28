//! Files on the clipboard: results go out as a file list, so Ctrl+V pastes
//! them into a chat; files or a picture copied by the user come in when
//! nothing is selected.

use std::path::{Path, PathBuf};

/// What the user copied, if it is something Mutabor can work on.
#[derive(Debug, PartialEq)]
pub enum Copied {
    Files(Vec<String>),
    /// A picture (a screenshot) saved as this PNG.
    Image(PathBuf),
}

/// Folder for pictures taken from the clipboard until an action uses them.
pub fn image_dir() -> PathBuf {
    std::env::temp_dir().join("mutabor-clipboard")
}

pub fn is_clipboard_image(p: &Path) -> bool {
    p.parent() == Some(image_dir().as_path())
}

pub fn copy_files(paths: &[PathBuf]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    imp::copy_files(paths)
}

pub fn read() -> Option<Copied> {
    imp::read()
}

#[cfg(target_os = "windows")]
mod imp {
    use super::*;
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    /// PowerShell single-quoted string.
    fn quote(p: &Path) -> String {
        format!("'{}'", p.to_string_lossy().replace('\'', "''"))
    }

    fn powershell(script: &str) -> Option<std::process::Output> {
        Command::new("powershell")
            // The clipboard needs a single-threaded apartment.
            .args(["-NoProfile", "-NonInteractive", "-STA", "-Command", script])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()
    }

    pub fn copy_files(paths: &[PathBuf]) -> Result<(), String> {
        let list: Vec<String> = paths.iter().map(|p| quote(p)).collect();
        let out = powershell(&format!("Set-Clipboard -LiteralPath {}", list.join(","))).ok_or("powershell")?;
        if out.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
        }
    }

    pub fn read() -> Option<Copied> {
        let dir = image_dir();
        let _ = std::fs::create_dir_all(&dir);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let png = dir.join(format!("clipboard-{stamp}.png"));
        let script = format!(
            "[Console]::OutputEncoding=[Text.Encoding]::UTF8; \
             Add-Type -AssemblyName System.Windows.Forms,System.Drawing; \
             $f=[Windows.Forms.Clipboard]::GetFileDropList(); \
             if($f.Count -gt 0){{ 'files'; $f }} \
             elseif([Windows.Forms.Clipboard]::ContainsImage()){{ \
               [Windows.Forms.Clipboard]::GetImage().Save({}, [Drawing.Imaging.ImageFormat]::Png); 'image' }}",
            quote(&png)
        );
        let out = powershell(&script)?;
        let lines = crate::selection::lines(&out.stdout);
        match lines.first().map(String::as_str) {
            Some("files") => Some(Copied::Files(lines[1..].to_vec())),
            Some("image") if png.is_file() => Some(Copied::Image(png)),
            _ => None,
        }
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use std::process::Command;

    pub fn copy_files(paths: &[PathBuf]) -> Result<(), String> {
        let list: Vec<String> = paths
            .iter()
            .map(|p| format!("POSIX file \"{}\"", p.to_string_lossy().replace('"', "\\\"")))
            .collect();
        let out = Command::new("osascript")
            .args(["-e", &format!("set the clipboard to {{{}}}", list.join(", "))])
            .output()
            .map_err(|e| e.to_string())?;
        if out.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&out.stderr).into()) }
    }

    // TODO: read copied Finder files and pictures («class furl», «class PNGf»).
    pub fn read() -> Option<Copied> {
        None
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod imp {
    use super::*;
    use std::io::Write;
    use std::process::{Command, Stdio};

    /// Wayland's wl-copy first, then X11's xclip.
    pub fn copy_files(paths: &[PathBuf]) -> Result<(), String> {
        let uris: String = paths.iter().map(|p| format!("file://{}\n", p.to_string_lossy())).collect();
        for (cmd, args) in [
            ("wl-copy", vec!["--type", "text/uri-list"]),
            ("xclip", vec!["-selection", "clipboard", "-t", "text/uri-list"]),
        ] {
            if let Ok(mut child) = Command::new(cmd).args(&args).stdin(Stdio::piped()).spawn() {
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(uris.as_bytes());
                }
                if child.wait().is_ok_and(|s| s.success()) {
                    return Ok(());
                }
            }
        }
        Err("no wl-copy or xclip".into())
    }

    // TODO: read text/uri-list and image/png from wl-paste or xclip.
    pub fn read() -> Option<Copied> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Uses the real clipboard (overwrites it): `cargo test clipboard -- --ignored`.
    #[test]
    #[ignore]
    fn files_round_trip() {
        let dir = std::env::temp_dir().join("mutabor-test-clip");
        let _ = std::fs::create_dir_all(&dir);
        let a = dir.join("a b's.txt");
        std::fs::write(&a, b"x").unwrap();
        copy_files(std::slice::from_ref(&a)).unwrap();
        assert_eq!(read(), Some(Copied::Files(vec![a.to_string_lossy().into_owned()])));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
