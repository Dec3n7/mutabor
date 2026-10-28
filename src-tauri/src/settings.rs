//! User settings, stored as JSON in the OS config folder
//! (%APPDATA%\io.github.dec3n7.mutabor\settings.json on Windows).

use crate::recipes::{Options, Quality};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Global shortcut in the global-hotkey syntax, e.g. "Control+Alt+KeyC".
    pub hotkey: String,
    /// "auto" follows the system language; otherwise "ru" or "en".
    pub language: String,
    /// Quality for conversions when the submenu was not opened.
    pub quality: Quality,
    /// Reuse the options last chosen in an action's submenu.
    pub remember: bool,
    /// Last submenu choice per action id.
    pub last: HashMap<String, Options>,
}

pub const DEFAULT_HOTKEY: &str = if cfg!(target_os = "macos") { "Super+Alt+KeyC" } else { "Control+Alt+KeyC" };

impl Default for Settings {
    fn default() -> Self {
        Settings {
            hotkey: DEFAULT_HOTKEY.into(),
            language: "auto".into(),
            quality: Quality::Normal,
            remember: true,
            last: HashMap::new(),
        }
    }
}

impl Settings {
    /// Options for a run: the explicit choice, else the remembered one, else
    /// the default quality (actions with their own default keep it).
    pub fn options_for(&self, action: &str, chosen: Option<Options>) -> Options {
        if let Some(o) = chosen {
            return o;
        }
        if self.remember {
            if let Some(o) = self.last.get(action) {
                return *o;
            }
        }
        let own = crate::recipes::default_quality(action);
        Options {
            quality: Some(if own == Quality::Normal { self.quality } else { own }),
            ..Default::default()
        }
    }
}

/// Loaded on first use: webviews may call in before the app's `setup` runs.
#[derive(Default)]
pub struct Store {
    loaded: Mutex<Option<(PathBuf, Settings)>>,
}

impl Store {
    fn with<R>(&self, app: &AppHandle, f: impl FnOnce(&Path, &mut Settings) -> R) -> R {
        let mut slot = self.loaded.lock().unwrap();
        let (path, settings) = slot.get_or_insert_with(|| {
            let path = app.path().app_config_dir().unwrap_or_default().join("settings.json");
            // A missing or broken file falls back to defaults rather than failing.
            let settings = std::fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();
            (path, settings)
        });
        f(path, settings)
    }

    pub fn get(&self, app: &AppHandle) -> Settings {
        self.with(app, |_, s| s.clone())
    }

    pub fn save(&self, app: &AppHandle, next: Settings) -> Result<(), String> {
        self.with(app, |path, current| {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            let json = serde_json::to_string_pretty(&next).map_err(|e| e.to_string())?;
            std::fs::write(path, json).map_err(|e| e.to_string())?;
            *current = next;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_or_partial_files_load() {
        let s: Settings = serde_json::from_str(r#"{"language":"en"}"#).unwrap();
        assert_eq!(s.language, "en");
        assert_eq!(s.hotkey, DEFAULT_HOTKEY);
        assert!(s.remember);
    }

    #[test]
    fn options_fallbacks() {
        let mut s = Settings { quality: Quality::High, ..Default::default() };
        assert_eq!(s.options_for("to_mp4", None).quality, Some(Quality::High));
        // Compression keeps aiming small whatever the general default is.
        assert_eq!(s.options_for("compress", None).quality, Some(Quality::Small));
        let chosen = Options { quality: Some(Quality::Small), height: Some(720), ..Default::default() };
        s.last.insert("to_mp4".into(), chosen);
        assert_eq!(s.options_for("to_mp4", None), chosen);
        s.remember = false;
        assert_eq!(s.options_for("to_mp4", None).height, None);
    }

    #[test]
    fn default_hotkey_parses() {
        use std::str::FromStr;
        tauri_plugin_global_shortcut::Shortcut::from_str(DEFAULT_HOTKEY).unwrap();
        tauri_plugin_global_shortcut::Shortcut::from_str("Control+Shift+Digit1").unwrap();
    }
}
