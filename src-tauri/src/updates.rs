//! Asks GitHub once a day whether a newer Mutabor has been released. Only
//! the public "latest release" page of this repository is read; nothing
//! about the user or their files is sent. Can be turned off in Settings.

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

const LATEST: &str = "https://api.github.com/repos/Dec3n7/mutabor/releases/latest";
const FIRST_CHECK: Duration = Duration::from_secs(30);
const EVERY: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Update {
    pub version: String,
    /// The release page, where the user picks the file for their system.
    pub url: String,
}

/// The newest release found so far, if it is newer than this build.
#[derive(Default)]
pub struct Found(pub Mutex<Option<Update>>);

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

/// "v0.10.2" -> [0, 10, 2]; anything after a "-" (pre-release) is ignored.
fn parse(v: &str) -> Vec<u64> {
    v.trim_start_matches('v').split('-').next().unwrap_or("").split('.').filter_map(|p| p.parse().ok()).collect()
}

fn newer(candidate: &str, current: &str) -> bool {
    let (a, b) = (parse(candidate), parse(current));
    !a.is_empty() && a > b
}

/// Asks GitHub for the latest release; Ok(None) when this build is current.
pub fn check() -> Result<Option<Update>, String> {
    let response = ureq::get(LATEST)
        .header("User-Agent", concat!("Mutabor/", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/vnd.github+json")
        .config()
        .timeout_global(Some(Duration::from_secs(20)))
        .build()
        .call()
        .map_err(|e| e.to_string())?;
    let body = response.into_body().read_to_string().map_err(|e| e.to_string())?;
    let release: Release = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    if release.draft || release.prerelease || !newer(&release.tag_name, env!("CARGO_PKG_VERSION")) {
        return Ok(None);
    }
    Ok(Some(Update { version: release.tag_name.trim_start_matches('v').to_string(), url: release.html_url }))
}

/// Remembers and announces a found update to every window. The toast pops
/// up once per version; after that the tray and Settings keep showing it.
pub fn announce(app: &AppHandle, update: Option<Update>) {
    *app.state::<Found>().0.lock().unwrap() = update.clone();
    let _ = app.emit("update", update.clone());
    let Some(update) = update else { return };
    let store = app.state::<crate::settings::Store>();
    let mut settings = store.get(app);
    if settings.update_seen != update.version {
        settings.update_seen = update.version.clone();
        if store.save(app, settings.clone()).is_ok() {
            let _ = app.emit("settings", settings);
        }
        let _ = app.emit_to(crate::jobs::TOAST, "update:new", update);
        crate::jobs::show_toast(app);
    }
}

/// Background loop: a first check shortly after start, then once a day,
/// skipped while the setting is off.
pub fn watch(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(FIRST_CHECK);
        loop {
            if app.state::<crate::settings::Store>().get(&app).check_updates {
                match check() {
                    Ok(update) => announce(&app, update),
                    Err(e) => eprintln!("update check: {e}"),
                }
            }
            std::thread::sleep(EVERY);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert!(newer("v0.2.0", "0.1.9"));
        assert!(newer("v0.10.0", "0.9.0"));
        assert!(newer("1.0.0", "0.99.99"));
        assert!(!newer("v0.1.0", "0.1.0"));
        assert!(!newer("v0.1.0", "0.2.0"));
        assert!(!newer("nightly", "0.1.0"));
        assert!(!newer("v0.2.0-beta.1", "0.2.0"));
    }
}

#[cfg(test)]
mod live {
    /// Asks the real GitHub API: `cargo test live -- --ignored`.
    #[test]
    #[ignore]
    fn asks_github() {
        // The published 0.1.0 is this build's version, so nothing newer yet.
        assert_eq!(super::check(), Ok(None));
    }
}
