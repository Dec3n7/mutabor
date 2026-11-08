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
    /// Files the same on every system (a model), saved next to the program.
    #[serde(default)]
    extra: Vec<Extra>,
}

#[derive(Deserialize)]
struct Extra {
    url: String,
    sha256: String,
    size: u64,
    /// File name inside the engine's folder.
    name: String,
}

#[derive(Deserialize)]
struct Build {
    url: String,
    sha256: String,
    #[serde(default)]
    size: u64,
    /// Path inside the archive -> program name.
    files: BTreeMap<String, String>,
    /// Keep the whole unpacked archive (the program needs the models or
    /// libraries next to it); `files` then only says where the programs are.
    #[serde(default)]
    keep: bool,
}

impl Tool {
    /// Only extra files, no program to unpack: a model run by the app itself.
    fn model_only(&self) -> bool {
        self.builds.is_empty()
    }
}

impl Build {
    /// Where `program` lives inside the engine's folder, once installed.
    fn locate(&self, dir: &Path, program: &str) -> Option<PathBuf> {
        if !self.keep {
            let path = dir.join(exe_name(program));
            return path.is_file().then_some(path);
        }
        let rel = self.files.iter().find(|(_, p)| p.as_str() == program)?.0;
        find(dir, rel)
    }
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
            let extras = t.extra.iter().all(|e| dir.join(&e.name).is_file());
            EngineInfo {
                name: name.clone(),
                title: t.title.clone(),
                version: t.version.clone(),
                license: t.license.clone(),
                purpose: t.purpose.clone(),
                size: build.map_or(0, |b| b.size) + t.extra.iter().map(|e| e.size).sum::<u64>(),
                available: build.is_some() || t.model_only(),
                installed: extras
                    && (t.model_only() || build.is_some_and(|b| b.files.values().all(|p| b.locate(&dir, p).is_some()))),
            }
        })
        .collect()
}

/// The installed program, e.g. ("mupdf", "mutool").
pub fn program(app: &AppHandle, engine: &str, program: &str) -> Option<PathBuf> {
    let tool = tools().get(engine)?;
    let dir = engine_dir(app, engine, tool);
    if tool.model_only() {
        let path = dir.join(program);
        return path.is_file().then_some(path);
    }
    tool.builds.get(TARGET)?.locate(&dir, program)
}

/// The installed engine's folder, where its extra files are.
pub fn dir(app: &AppHandle, engine: &str) -> Option<PathBuf> {
    let tool = tools().get(engine)?;
    Some(engine_dir(app, engine, tool))
}

/// Downloads, verifies and unpacks an engine. `progress` gets 0–100.
pub fn install(app: &AppHandle, name: &str, cancel: &AtomicBool, mut progress: impl FnMut(u8)) -> Result<(), String> {
    let tool = tools().get(name).ok_or("unknown engine")?;
    let dir = engine_dir(app, name, tool);
    if tool.model_only() {
        return install_extras(tool, &dir, cancel, &mut progress);
    }
    let build = tool.builds.get(TARGET).ok_or("engine_unavailable")?;
    let root = dir.parent().expect("versioned dir").to_path_buf();
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;

    let archive = root.join(build.url.rsplit('/').next().unwrap_or("download"));
    let unpacked = root.join("unpacking");
    // One progress bar over the archive and the extra files, by size.
    let total = (build.size + tool.extra.iter().map(|e| e.size).sum::<u64>()).max(1);
    let share = |done: u64, size: u64| move |p: u8| ((done + size * p as u64 / 100) * 100 / total).min(99) as u8;
    let result = (|| {
        let part = share(0, build.size);
        download(&build.url, &archive, &build.sha256, build.size, cancel, &mut |p| progress(part(p)))?;
        let _ = std::fs::remove_dir_all(&unpacked);
        std::fs::create_dir_all(&unpacked).map_err(|e| e.to_string())?;
        extract(&archive, &unpacked)?;
        // Extra files go into the unpacked tree, which becomes the engine folder.
        let mut done = build.size;
        for extra in &tool.extra {
            let part = share(done, extra.size);
            let target = if build.keep { unpacked.join(&extra.name) } else { dir.join(&extra.name) };
            if !build.keep {
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            }
            // Partial downloads stay in the root folder, where a retry finds them.
            let partial = root.join(format!("{}.part", extra.name));
            download(&extra.url, &partial, &extra.sha256, extra.size, cancel, &mut |p| progress(part(p)))?;
            std::fs::rename(&partial, &target).map_err(|e| e.to_string())?;
            done += extra.size;
        }
        if build.keep {
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::rename(&unpacked, &dir).map_err(|e| e.to_string())?;
            for (rel, _) in &build.files {
                let path = find(&dir, rel).ok_or_else(|| format!("{rel} not found in the archive"))?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755));
                }
                let _ = path;
            }
            return Ok(());
        }
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

/// A model-only engine: just its extra files, into a fresh folder.
fn install_extras(tool: &Tool, dir: &Path, cancel: &AtomicBool, progress: &mut impl FnMut(u8)) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let total = tool.extra.iter().map(|e| e.size).sum::<u64>().max(1);
    let mut done = 0;
    for extra in &tool.extra {
        let part = |p: u8| ((done + extra.size * p as u64 / 100) * 100 / total).min(99) as u8;
        // Downloads go to `.part` files, so an interrupted one can resume and
        // is never mistaken for an installed model.
        let partial = dir.join(format!("{}.part", extra.name));
        download(&extra.url, &partial, &extra.sha256, extra.size, cancel, &mut |p| progress(part(p)))?;
        std::fs::rename(&partial, dir.join(&extra.name)).map_err(|e| e.to_string())?;
        done += extra.size;
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

/// Downloads `url` to `to` and checks its SHA-256. A partial file left by an
/// earlier, interrupted attempt is continued with an HTTP Range request, so a
/// slow or dropped connection does not start the whole download again. A
/// cancelled or failed attempt keeps the partial file; a file that does not
/// match its hash is deleted.
fn download(
    url: &str,
    to: &Path,
    sha256: &str,
    size_hint: u64,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(u8),
) -> Result<(), String> {
    let mut hasher = Sha256::new();
    let mut done = 0u64;
    let have = std::fs::metadata(to).map(|m| m.len()).unwrap_or(0);
    let response = if have > 0 {
        crate::net::agent()
            .get(url)
            .header("Range", format!("bytes={have}-"))
            .config()
            .http_status_as_error(false)
            .build()
            .call()
            .map_err(|e| format!("download: {e}"))?
    } else {
        crate::net::agent().get(url).call().map_err(|e| format!("download: {e}"))?
    };
    let status = response.status().as_u16();
    let mut file = if have > 0 && status == 206 {
        // Resume: the bytes already on disk count toward the hash and the progress.
        hash_file(to, &mut hasher)?;
        done = have;
        std::fs::OpenOptions::new().append(true).open(to).map_err(|e| e.to_string())?
    } else if have > 0 && status == 416 {
        // The partial file already has every byte: only the hash is left to check.
        drop(response);
        hash_file(to, &mut hasher)?;
        return finish(to, &mut hasher, sha256);
    } else if (200..300).contains(&status) {
        std::fs::File::create(to).map_err(|e| e.to_string())?
    } else {
        return Err(format!("download: HTTP {status}"));
    };
    let content = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok()?.parse::<u64>().ok());
    let total = match content {
        Some(c) if done > 0 => done + c,
        Some(c) => c,
        None => size_hint,
    };
    let mut reader = response.into_body().into_reader();
    let mut buf = vec![0u8; 256 * 1024];
    let mut last = (done * 100 / total.max(1)).min(99) as u8;
    loop {
        if cancel.load(Ordering::Relaxed) {
            file.flush().ok();
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
    drop(file);
    finish(to, &mut hasher, sha256)
}

/// Feeds the bytes of a file already on disk into the hash.
fn hash_file(path: &Path, hasher: &mut Sha256) -> Result<(), String> {
    let mut old = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut chunk = vec![0u8; 256 * 1024];
    loop {
        let n = old.read(&mut chunk).map_err(|e| e.to_string())?;
        if n == 0 {
            return Ok(());
        }
        hasher.update(&chunk[..n]);
    }
}

/// Checks the hash of a finished download; a file that does not match is
/// deleted so the next attempt starts clean.
fn finish(to: &Path, hasher: &mut Sha256, sha256: &str) -> Result<(), String> {
    let actual: String = hasher.clone().finalize().iter().map(|b| format!("{b:02x}")).collect();
    if actual != sha256 {
        let _ = std::fs::remove_file(to);
        return Err(format!("SHA-256 mismatch: expected {sha256}, got {actual}"));
    }
    Ok(())
}

/// bsdtar reads zip, tar.gz, tar.xz and 7-Zip (7-Zip's own installer, too);
/// it ships with Windows 10+ and macOS. On Linux, tarballs go to GNU tar and
/// zips to unzip. LibreOffice comes as an MSI (Windows), a disk image (macOS)
/// or Debian packages in a tarball (Linux); none of them is installed, they
/// are only unpacked into the engine folder.
fn extract(archive: &Path, into: &Path) -> Result<(), String> {
    match archive.extension().and_then(|e| e.to_str()) {
        Some("msi") => return unpack_msi(archive, into),
        Some("dmg") => return unpack_dmg(archive, into),
        _ => {}
    }
    untar(archive, into)?;
    unpack_debs(into)
}

fn untar(archive: &Path, into: &Path) -> Result<(), String> {
    // GNU tar on Linux does not read zip.
    if cfg!(target_os = "linux") && archive.extension().is_some_and(|e| e == "zip") {
        let out = Command::new("unzip").arg("-q").arg("-o").arg(archive).arg("-d").arg(into).output();
        let out = out.map_err(|e| format!("unzip: {e}"))?;
        if !out.status.success() {
            return Err(format!("unzip: {}", String::from_utf8_lossy(&out.stderr).trim()));
        }
        return Ok(());
    }
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

/// An administrative install: msiexec only copies the files out, with no
/// registry entries, shortcuts or admin rights. It puts the bundled fonts
/// where Windows keeps fonts and the C++ runtime where System32 would be, so
/// both are moved to where LibreOffice looks for them.
#[cfg(windows)]
fn unpack_msi(archive: &Path, into: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let out = Command::new("msiexec")
        .arg("/a")
        .arg(archive)
        .arg("/qn")
        // msiexec wants the quotes around the value only, not the whole argument.
        .raw_arg(format!("TARGETDIR=\"{}\"", into.display()))
        .creation_flags(0x0800_0000)
        .output()
        .map_err(|e| format!("msiexec: {e}"))?;
    if !out.status.success() {
        return Err(format!("msiexec: exit code {}", out.status.code().unwrap_or(-1)));
    }
    let fonts = into.join("Fonts");
    if fonts.is_dir() {
        let to = into.join("share").join("fonts").join("truetype");
        std::fs::create_dir_all(into.join("share").join("fonts")).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_dir_all(&to);
        std::fs::rename(&fonts, &to).map_err(|e| e.to_string())?;
    }
    if let Ok(dlls) = std::fs::read_dir(into.join("System64")) {
        for dll in dlls.flatten() {
            let to = into.join("program").join(dll.file_name());
            if !to.exists() {
                std::fs::copy(dll.path(), to).map_err(|e| e.to_string())?;
            }
        }
    }
    for leftover in ["System", "System64"] {
        let _ = std::fs::remove_dir_all(into.join(leftover));
    }
    if let Some(name) = archive.file_name() {
        let _ = std::fs::remove_file(into.join(name));
    }
    Ok(())
}

#[cfg(not(windows))]
fn unpack_msi(_: &Path, _: &Path) -> Result<(), String> {
    Err("an MSI can only be unpacked on Windows".into())
}

/// Mounts the disk image read-only and out of sight, copies the app out.
fn unpack_dmg(archive: &Path, into: &Path) -> Result<(), String> {
    let mount = into.join(".mount");
    std::fs::create_dir_all(&mount).map_err(|e| e.to_string())?;
    let run = |cmd: &mut Command, what: &str| -> Result<(), String> {
        let out = cmd.output().map_err(|e| format!("{what}: {e}"))?;
        if !out.status.success() {
            return Err(format!("{what}: {}", String::from_utf8_lossy(&out.stderr).trim()));
        }
        Ok(())
    };
    run(
        Command::new("hdiutil").args(["attach", "-nobrowse", "-readonly", "-noautoopen", "-mountpoint"]).arg(&mount).arg(archive),
        "hdiutil",
    )?;
    let copied = (|| {
        let app = std::fs::read_dir(&mount)
            .map_err(|e| e.to_string())?
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|e| e == "app"))
            .ok_or("no app in the disk image")?;
        run(Command::new("ditto").arg(&app).arg(into.join(app.file_name().unwrap_or_default())), "ditto")
    })();
    let _ = Command::new("hdiutil").args(["detach", "-force"]).arg(&mount).output();
    let _ = std::fs::remove_dir(&mount);
    copied
}

/// Debian packages found in the unpacked tarball are opened in place: each
/// is an `ar` archive whose `data.tar.*` member holds the files.
fn unpack_debs(into: &Path) -> Result<(), String> {
    fn debs(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                debs(&p, out);
            } else if p.extension().is_some_and(|x| x == "deb") {
                out.push(p);
            }
        }
    }
    let mut found = Vec::new();
    debs(into, &mut found);
    for deb in found {
        let bytes = std::fs::read(&deb).map_err(|e| e.to_string())?;
        let (name, data) = deb_data(&bytes).ok_or_else(|| format!("{} is not a Debian package", deb.display()))?;
        let member = into.join(name);
        std::fs::write(&member, data).map_err(|e| e.to_string())?;
        drop(bytes);
        let result = untar(&member, into);
        let _ = std::fs::remove_file(&member);
        result?;
        let _ = std::fs::remove_file(&deb);
    }
    Ok(())
}

/// The `data.tar.*` member of a Debian package (an `ar` archive): its name and bytes.
fn deb_data(deb: &[u8]) -> Option<(&str, &[u8])> {
    let mut rest = deb.strip_prefix(b"!<arch>\n")?;
    while rest.len() >= 60 {
        let (head, body) = rest.split_at(60);
        let name = std::str::from_utf8(&head[..16]).ok()?.trim_end().trim_end_matches('/');
        let size: usize = std::str::from_utf8(&head[48..58]).ok()?.trim().parse().ok()?;
        let data = body.get(..size)?;
        if name.starts_with("data.tar") {
            return Some((name, data));
        }
        // Members start on even offsets.
        rest = body.get(size + size % 2..).unwrap_or_default();
    }
    None
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

    /// An `ar` archive as `dpkg-deb` writes it, with an odd-sized member
    /// before the data, so the padding byte is skipped.
    #[test]
    fn finds_the_data_of_a_debian_package() {
        let member = |name: &str, body: &[u8]| {
            let mut m = format!("{name:<16}{:<12}{:<6}{:<6}{:<8}{:<10}`\n", 0, 0, 0, 100644, body.len()).into_bytes();
            m.extend_from_slice(body);
            if body.len() % 2 == 1 {
                m.push(b'\n');
            }
            m
        };
        let mut deb = b"!<arch>\n".to_vec();
        deb.extend(member("debian-binary", b"2.0\n"));
        deb.extend(member("control.tar.xz", b"abc"));
        deb.extend(member("data.tar.xz", b"files"));
        assert_eq!(deb_data(&deb), Some(("data.tar.xz", &b"files"[..])));
        assert_eq!(deb_data(b"not an archive"), None);
    }

    /// The whole LibreOffice path on this system: download (or the file named
    /// by MUTABOR_LIBREOFFICE), unpack as `install` does, convert a text and a
    /// table. Run by the "Engines" workflow on GitHub for every system.
    #[test]
    #[ignore]
    fn downloads_unpacks_and_converts_with_libreoffice() {
        let tool = &tools()["libreoffice"];
        let build = &tool.builds[TARGET];
        let archive = match std::env::var("MUTABOR_LIBREOFFICE") {
            Ok(path) => PathBuf::from(path),
            Err(_) => {
                let to = std::env::temp_dir().join(build.url.rsplit('/').next().unwrap());
                download(&build.url, &to, &build.sha256, build.size, &AtomicBool::new(false), &mut |_| {}).unwrap();
                to
            }
        };
        let mut hasher = Sha256::new();
        hash_file(&archive, &mut hasher).unwrap();
        let actual: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(actual, build.sha256);

        let root = std::env::temp_dir().join("mutabor-libreoffice");
        let _ = std::fs::remove_dir_all(&root);
        let into = root.join("engine");
        std::fs::create_dir_all(&into).unwrap();
        extract(&archive, &into).unwrap();
        let soffice = build.locate(&into, "soffice").expect("soffice in the unpacked files");
        if cfg!(windows) {
            assert!(into.join("share/fonts/truetype").is_dir(), "bundled fonts moved");
            assert!(into.join("program/vcruntime140.dll").is_file(), "C++ runtime next to the program");
        }

        let input = root.join("in");
        std::fs::create_dir_all(&input).unwrap();
        // "Привет" in RTF's Unicode escapes, and a UTF-8 table.
        let rtf = r"{\rtf1\ansi{\fonttbl{\f0 Liberation Serif;}}\f0 ၕ?ႈ?ႀ?ၴ?ၷ?႐?, Mutabor.\par}";
        std::fs::write(input.join("letter.rtf"), rtf).unwrap();
        std::fs::write(input.join("table.csv"), "Имя,Число\nАня,1\nБорис,2\n").unwrap();
        let out = root.join("out");
        let profile = crate::jobs::file_url(&root.join("profile"));
        for (file, filter, made) in [
            ("letter.rtf", "pdf", "letter.pdf"),
            ("letter.rtf", "docx:MS Word 2007 XML", "letter.docx"),
            ("table.csv", "xlsx:Calc MS Excel 2007 XML", "table.xlsx"),
        ] {
            let status = Command::new(&soffice)
                .args(["--headless", "--invisible", "--norestore", "--nolockcheck", "--nodefault", "--nologo"])
                .arg(format!("-env:UserInstallation={profile}"))
                .args(["--convert-to", filter, "--outdir"])
                .arg(&out)
                .arg(input.join(file))
                .status()
                .unwrap();
            assert!(status.success(), "{file} -> {filter}: {status}");
            let bytes = std::fs::read(out.join(made)).unwrap_or_else(|e| panic!("{made}: {e}"));
            let magic: &[u8] = if made.ends_with(".pdf") { b"%PDF" } else { b"PK" };
            assert!(bytes.starts_with(magic), "{made} is not what it should be");
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Serves the archive `npm run tools -- --optional` left in .cache from a
    /// local HTTP server and downloads it, so no network is needed.
    #[test]
    #[ignore]
    fn resumes_a_partial_download() {
        use std::net::TcpListener;
        let build = &tools()["pandoc"].builds[TARGET];
        let cached = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.cache/tools"))
            .join(build.url.rsplit('/').next().unwrap());
        let body = std::fs::read(&cached).expect("run `npm run tools -- --optional` first");
        // A server that honours Range requests, as GitHub and mupdf.com do.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/archive.zip", listener.local_addr().unwrap());
        let served = body.clone();
        std::thread::spawn(move || {
            for mut conn in listener.incoming().flatten() {
                let mut req = [0u8; 4096];
                let n = conn.read(&mut req).unwrap_or(0);
                let text = String::from_utf8_lossy(&req[..n]).to_ascii_lowercase();
                let from = text
                    .lines()
                    .find_map(|l| l.strip_prefix("range: bytes="))
                    .and_then(|r| r.trim_end_matches('-').trim().parse::<usize>().ok())
                    .unwrap_or(0);
                let rest = &served[from.min(served.len())..];
                let status = if from > 0 { "206 Partial Content" } else { "200 OK" };
                let head = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", rest.len());
                let _ = conn.write_all(head.as_bytes());
                let _ = conn.write_all(rest);
            }
        });

        let to = std::env::temp_dir().join("mutabor-resume.zip");
        let half = body.len() / 2;
        std::fs::write(&to, &body[..half]).unwrap();
        // A cancelled attempt keeps what it has.
        let err = download(&url, &to, &build.sha256, 0, &AtomicBool::new(true), &mut |_| {}).unwrap_err();
        assert_eq!(err, "cancelled");
        assert_eq!(std::fs::metadata(&to).unwrap().len(), half as u64);
        // The next attempt continues where the first one stopped.
        download(&url, &to, &build.sha256, 0, &AtomicBool::new(false), &mut |_| {}).unwrap();
        assert_eq!(std::fs::read(&to).unwrap(), body);
        let _ = std::fs::remove_file(&to);
    }

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
