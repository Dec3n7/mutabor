//! Optional engines (MuPDF, Pandoc). They are listed in tools/manifest.json
//! and downloaded the first time the user needs one, from the official
//! source, and only kept when the SHA-256 matches the manifest.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Manager};

const MANIFEST: &str = include_str!("../../tools/manifest.json");
const TARGET: &str = env!("MUTABOR_TARGET");

#[derive(Deserialize)]
struct Tool {
    title: String,
    version: String,
    license: String,
    purpose: String,
    #[serde(default)]
    optional: bool,
    builds: BTreeMap<String, Build>,
}

#[derive(Deserialize)]
struct Build {
    url: String,
    sha256: String,
    #[serde(default)]
    size: u64,
    /// Path inside the archive -> program name.
    files: BTreeMap<String, String>,
}

fn tools() -> &'static BTreeMap<String, Tool> {
    static TOOLS: OnceLock<BTreeMap<String, Tool>> = OnceLock::new();
    TOOLS.get_or_init(|| {
        let raw: BTreeMap<String, serde_json::Value> = serde_json::from_str(MANIFEST).expect("manifest.json");
        raw.into_iter()
            .filter(|(k, _)| !k.starts_with('$') && k != "planned")
            .filter_map(|(k, v)| Some((k, serde_json::from_value::<Tool>(v).ok()?)))
            .filter(|(_, t)| t.optional)
            .collect()
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct EngineInfo {
    pub name: String,
    pub title: String,
    pub version: String,
    pub license: String,
    pub purpose: String,
    /// Download size in bytes.
    pub size: u64,
    /// A build exists for this OS and CPU.
    pub available: bool,
    pub installed: bool,
}

fn exe_name(program: &str) -> String {
    if cfg!(windows) { format!("{program}.exe") } else { program.to_string() }
}

fn engines_root(app: &AppHandle) -> PathBuf {
    app.path().app_local_data_dir().unwrap_or_default().join("engines")
}

fn engine_dir(app: &AppHandle, name: &str, tool: &Tool) -> PathBuf {
    engines_root(app).join(name).join(&tool.version)
}

pub fn list(app: &AppHandle) -> Vec<EngineInfo> {
    tools()
        .iter()
        .map(|(name, t)| {
            let build = t.builds.get(TARGET);
            let dir = engine_dir(app, name, t);
            EngineInfo {
                name: name.clone(),
                title: t.title.clone(),
                version: t.version.clone(),
                license: t.license.clone(),
                purpose: t.purpose.clone(),
                size: build.map_or(0, |b| b.size),
                available: build.is_some(),
                installed: build.is_some_and(|b| b.files.values().all(|p| dir.join(exe_name(p)).is_file())),
            }
        })
        .collect()
}

/// The installed program, e.g. ("mupdf", "mutool").
pub fn program(app: &AppHandle, engine: &str, program: &str) -> Option<PathBuf> {
    let tool = tools().get(engine)?;
    let path = engine_dir(app, engine, tool).join(exe_name(program));
    path.is_file().then_some(path)
}

/// Downloads, verifies and unpacks an engine. `progress` gets 0–100.
pub fn install(app: &AppHandle, name: &str, cancel: &AtomicBool, mut progress: impl FnMut(u8)) -> Result<(), String> {
    let tool = tools().get(name).ok_or("unknown engine")?;
    let build = tool.builds.get(TARGET).ok_or("engine_unavailable")?;
    let dir = engine_dir(app, name, tool);
    let root = dir.parent().expect("versioned dir").to_path_buf();
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;

    let archive = root.join(build.url.rsplit('/').next().unwrap_or("download"));
    let unpacked = root.join("unpacking");
    let result = (|| {
        download(&build.url, &archive, &build.sha256, build.size, cancel, &mut progress)?;
        let _ = std::fs::remove_dir_all(&unpacked);
        std::fs::create_dir_all(&unpacked).map_err(|e| e.to_string())?;
        extract(&archive, &unpacked)?;
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        for (rel, program) in &build.files {
            let src = find(&unpacked, rel).ok_or_else(|| format!("{rel} not found in the archive"))?;
            let dst = dir.join(exe_name(program));
            std::fs::copy(&src, &dst).map_err(|e| e.to_string())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&dst, std::fs::Permissions::from_mode(0o755));
            }
        }
        Ok(())
    })();
    let _ = std::fs::remove_file(&archive);
    let _ = std::fs::remove_dir_all(&unpacked);
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&dir);
        return result;
    }
    // Older versions of the same engine are no longer used.
    if let Ok(entries) = std::fs::read_dir(&root) {
        for e in entries.flatten().filter(|e| e.path() != dir) {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
    Ok(())
}

pub fn remove(app: &AppHandle, name: &str) -> Result<(), String> {
    tools().get(name).ok_or("unknown engine")?;
    let dir = engines_root(app).join(name);
    if dir.exists() {
        std::fs::remove_dir_all(dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn download(
    url: &str,
    to: &Path,
    sha256: &str,
    size_hint: u64,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(u8),
) -> Result<(), String> {
    let response = ureq::get(url).call().map_err(|e| format!("download: {e}"))?;
    let total = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok()?.parse::<u64>().ok())
        .unwrap_or(size_hint);
    let mut reader = response.into_body().into_reader();
    let mut file = std::fs::File::create(to).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    let (mut done, mut last) = (0u64, 0u8);
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("cancelled".into());
        }
        let n = reader.read(&mut buf).map_err(|e| format!("download: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        if total > 0 {
            let p = (done * 100 / total).min(99) as u8;
            if p > last {
                last = p;
                progress(p);
            }
        }
    }
    file.flush().map_err(|e| e.to_string())?;
    let actual: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if actual != sha256 {
        return Err(format!("SHA-256 mismatch: expected {sha256}, got {actual}"));
    }
    Ok(())
}

/// bsdtar reads both zip and tar.gz; it ships with Windows 10+ and macOS,
/// and Linux archives are tar.gz, which GNU tar reads.
fn extract(archive: &Path, into: &Path) -> Result<(), String> {
    let tar = if cfg!(windows) {
        PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into()))
            .join("System32")
            .join("tar.exe")
    } else {
        PathBuf::from("tar")
    };
    let mut cmd = Command::new(tar);
    cmd.arg("-xf").arg(archive).arg("-C").arg(into);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().map_err(|e| format!("tar: {e}"))?;
    if !out.status.success() {
        return Err(format!("tar: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(())
}

/// Finds `rel` (e.g. "bin/pandoc") under `dir`, whatever top folder the archive has.
fn find(dir: &Path, rel: &str) -> Option<PathBuf> {
    if dir.join(rel).is_file() {
        return Some(dir.join(rel));
    }
    std::fs::read_dir(dir).ok()?.flatten().filter(|e| e.path().is_dir()).find_map(|e| find(&e.path(), rel))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unpacks the archive `npm run tools -- --optional` left in .cache and
    /// finds the program in it, as `install` does after downloading.
    #[test]
    #[ignore]
    fn unpacks_cached_archive() {
        let tool = &tools()["mupdf"];
        let build = &tool.builds[TARGET];
        let archive = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.cache/tools"))
            .join(build.url.rsplit('/').next().unwrap());
        assert!(archive.is_file(), "run `npm run tools -- --optional` first");
        let into = std::env::temp_dir().join("mutabor-unpack");
        let _ = std::fs::remove_dir_all(&into);
        std::fs::create_dir_all(&into).unwrap();
        extract(&archive, &into).unwrap();
        for rel in build.files.keys() {
            assert!(find(&into, rel).is_some(), "{rel} not found");
        }
        let _ = std::fs::remove_dir_all(&into);
    }

    /// Serves the archive `npm run tools -- --optional` left in .cache from a
    /// local HTTP server and downloads it, so no network is needed.
    #[test]
    #[ignore]
    fn downloads_and_checks_hash() {
        use std::net::TcpListener;
        let build = &tools()["pandoc"].builds[TARGET];
        let cached = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.cache/tools"))
            .join(build.url.rsplit('/').next().unwrap());
        let body = std::fs::read(&cached).expect("run `npm run tools -- --optional` first");
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/archive.zip", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for mut conn in listener.incoming().flatten() {
                let mut req = [0u8; 4096];
                let _ = conn.read(&mut req);
                let head = format!("HTTP/1.1 200 OK
Content-Length: {}
Connection: close

", body.len());
                let _ = conn.write_all(head.as_bytes());
                let _ = conn.write_all(&body);
            }
        });

        let to = std::env::temp_dir().join("mutabor-download.zip");
        let mut seen = Vec::new();
        download(&url, &to, &build.sha256, 0, &AtomicBool::new(false), &mut |p| seen.push(p)).unwrap();
        assert!(seen.windows(2).all(|w| w[0] < w[1]) && seen.last() > Some(&90), "progress {seen:?}");
        // A wrong hash is rejected.
        let err = download(&url, &to, &"0".repeat(64), 0, &AtomicBool::new(false), &mut |_| {}).unwrap_err();
        assert!(err.contains("SHA-256 mismatch"), "{err}");
        // Cancelling stops at once.
        let err = download(&url, &to, &build.sha256, 0, &AtomicBool::new(true), &mut |_| {}).unwrap_err();
        assert_eq!(err, "cancelled");
        let _ = std::fs::remove_file(&to);
    }

    #[test]
    fn manifest_lists_optional_engines() {
        let t = tools();
        assert!(t.contains_key("mupdf") && t.contains_key("pandoc"));
        assert!(!t.contains_key("ffmpeg"), "ffmpeg is bundled, not downloaded");
        for (name, tool) in t {
            for (target, b) in &tool.builds {
                assert_eq!(b.sha256.len(), 64, "{name} {target}");
                assert!(b.url.starts_with("https://"), "{name} {target}");
                assert!(b.size > 0, "{name} {target}: size shown before download");
            }
        }
    }
}
