mod clipboard;
mod engines;
mod files;
mod jobs;
mod pdf;
mod recipes;
mod selection;
mod settings;
mod svg;
mod matting;
mod net;
mod updates;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, State, WindowEvent};

const MENU: &str = "menu";
const SETTINGS: &str = "settings";

#[derive(Clone, Serialize)]
struct MenuOpen {
    files: Vec<files::FileInfo>,
    /// "selection", "clipboard_files", "clipboard_image" or "none".
    source: &'static str,
    settings: settings::Settings,
    engines: Vec<engines::EngineInfo>,
}

/// The files to work on: the file manager's selection, else whatever was
/// copied to the clipboard (files, or a picture such as a screenshot).
fn gather() -> (Vec<files::FileInfo>, &'static str) {
    let selected = files::describe(selection::selected_paths());
    if !selected.is_empty() {
        return (selected, "selection");
    }
    match clipboard::read() {
        Some(clipboard::Copied::Files(paths)) => (files::describe(paths), "clipboard_files"),
        Some(clipboard::Copied::Image(png)) => {
            (files::describe([png.to_string_lossy().into_owned()]), "clipboard_image")
        }
        None => (Vec::new(), "none"),
    }
}

/// Collects the files, places the popup next to the cursor and shows it.
fn open_menu(app: &AppHandle) {
    let (files, source) = gather();
    let Some(win) = app.get_webview_window(MENU) else {
        return;
    };

    if let Ok(cursor) = win.cursor_position() {
        let (cx, cy) = (cursor.x as i32, cursor.y as i32);
        let size = win.outer_size().unwrap_or_default();
        let (w, h) = (size.width as i32, size.height as i32);
        let (mut x, mut y) = (cx + 8, cy + 8);
        // Keep the popup on the monitor the cursor is on.
        if let Ok(Some(m)) = win.monitor_from_point(cursor.x, cursor.y) {
            let (mx, my) = (m.position().x, m.position().y);
            let (mw, mh) = (m.size().width as i32, m.size().height as i32);
            if x + w > mx + mw {
                x = cx - w - 8;
            }
            if y + h > my + mh {
                y = (my + mh - h).max(my);
            }
            x = x.max(mx);
        }
        let _ = win.set_position(PhysicalPosition::new(x, y));
    }

    let settings = app.state::<settings::Store>().get(app);
    let engines = engines::list(app);
    let _ = app.emit_to(MENU, "menu:open", MenuOpen { files, source, settings, engines });
    let _ = win.show();
    let _ = win.set_focus();
}

/// An action hotkey: runs the action on the selection without the menu,
/// with the remembered or default options. Problems go to the toast.
fn run_hotkey(app: &AppHandle, action: &str) {
    let (files, _) = gather();
    if files.is_empty() {
        jobs::notice(app, "nothing_selected");
        return;
    }
    let settings = app.state::<settings::Store>().get(app);
    let opts = settings.options_for(action, None);
    let paths = files.into_iter().map(|f| f.path).collect();
    let jobs = app.state::<Arc<jobs::Jobs>>().inner().clone();
    if let Err(e) = jobs.start(app, action, paths, &opts, &settings) {
        jobs::notice(app, &e);
    }
}

/// Starts the action on every selected file. Probing the inputs runs
/// ffmpeg, so this stays off the main thread. `options` comes from the
/// action's submenu; without it the remembered or default options apply.
#[tauri::command]
async fn run_action(
    app: AppHandle,
    jobs: State<'_, Arc<jobs::Jobs>>,
    store: State<'_, settings::Store>,
    action: String,
    paths: Vec<String>,
    options: Option<recipes::Options>,
) -> Result<(), String> {
    let current = store.get(&app);
    let opts = current.options_for(&action, options);
    let keep = opts.reusable();
    if options.is_some() && current.remember && current.last.get(&action) != Some(&keep) {
        let mut next = current;
        next.last.insert(action.clone(), keep);
        // Failing to remember a choice must not block the conversion.
        match store.save(&app, next.clone()) {
            Ok(()) => {
                let _ = app.emit("settings", next);
            }
            Err(e) => eprintln!("settings: {e}"),
        }
    }
    let settings = store.get(&app);
    let jobs = jobs.inner().clone();
    tauri::async_runtime::spawn_blocking(move || jobs.start(&app, &action, paths, &opts, &settings))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
fn list_engines(app: AppHandle) -> Vec<engines::EngineInfo> {
    engines::list(&app)
}

/// Downloads an engine with progress in the toast; resolves when it is ready.
#[tauri::command]
async fn install_engine(app: AppHandle, jobs: State<'_, Arc<jobs::Jobs>>, engine: String) -> Result<(), String> {
    let info = engines::list(&app).into_iter().find(|e| e.name == engine).ok_or("unknown engine")?;
    let jobs = jobs.inner().clone();
    let title = format!("{} {}", info.title, info.version);
    let result = tauri::async_runtime::spawn_blocking({
        let app = app.clone();
        move || jobs.install_engine(&app, &engine, &title)
    })
    .await
    .map_err(|e| e.to_string())?;
    let _ = app.emit("engines", engines::list(&app));
    result
}

#[tauri::command]
fn remove_engine(app: AppHandle, engine: String) -> Result<(), String> {
    engines::remove(&app, &engine)?;
    let _ = app.emit("engines", engines::list(&app));
    Ok(())
}

#[tauri::command]
fn get_settings(app: AppHandle, store: State<'_, settings::Store>) -> settings::Settings {
    store.get(&app)
}

/// Saves the settings, re-registering the hotkeys first when they changed so
/// a combination taken by another program is rejected and nothing is saved.
#[tauri::command]
fn save_settings(app: AppHandle, store: State<'_, settings::Store>, next: settings::Settings) -> Result<(), String> {
    let old = store.get(&app);
    #[cfg(desktop)]
    if next.hotkey != old.hotkey || next.hotkeys != old.hotkeys {
        if let Err(e) = register_hotkeys(&app, &next) {
            // Put the previous combinations back so the app stays reachable.
            let _ = register_hotkeys(&app, &old);
            return Err(e);
        }
    }
    store.save(&app, next.clone())?;
    let _ = app.emit("settings", next);
    Ok(())
}

const CROP: &str = "crop";

/// Opens the crop window for these pictures; it shows them one by one.
#[tauri::command]
fn open_crop(app: AppHandle, paths: Vec<String>) {
    if let Some(win) = app.get_webview_window(CROP) {
        let _ = app.emit_to(CROP, "crop:open", paths);
        let _ = win.center();
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

/// Preview of a picture for the crop window. Runs ffmpeg, so off the main thread.
#[tauri::command]
async fn crop_preview(path: String) -> Result<jobs::Preview, String> {
    tauri::async_runtime::spawn_blocking(move || jobs::preview(std::path::Path::new(&path)))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
fn open_settings(app: AppHandle) {
    show_settings(&app);
}

fn show_settings(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(SETTINGS) {
        let _ = win.center();
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

/// Shortcut id -> what it does: "" opens the menu, anything else is an
/// action id run directly.
#[derive(Default)]
struct Hotkeys(Mutex<HashMap<u32, String>>);

/// Registers the menu hotkey and the action hotkeys from `s`, replacing
/// whatever was registered before.
#[cfg(desktop)]
fn register_hotkeys(app: &AppHandle, s: &settings::Settings) -> Result<(), String> {
    use std::str::FromStr;
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let mut map = HashMap::new();
    let wanted = std::iter::once((&s.hotkey, String::new())).chain(s.hotkeys.iter().map(|(a, k)| (k, a.clone())));
    for (spec, target) in wanted {
        let key = Shortcut::from_str(spec).map_err(|_| "hotkey_invalid".to_string())?;
        if map.contains_key(&key.id()) {
            return Err("hotkey_duplicate".into());
        }
        gs.register(key).map_err(|_| "hotkey_taken".to_string())?;
        map.insert(key.id(), target);
    }
    *app.state::<Hotkeys>().0.lock().unwrap() = map;
    Ok(())
}

#[tauri::command]
fn cancel_job(jobs: State<'_, Arc<jobs::Jobs>>, id: u64) {
    jobs.cancel(id);
}

/// Tray menu entries, relabelled when the interface language changes.
struct TrayItems {
    menu: Menu<tauri::Wry>,
    settings: MenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
    /// "Version X is available"; in the menu only while there is one.
    update: MenuItem<tauri::Wry>,
    update_shown: Mutex<bool>,
}

/// Shows or hides the tray's update entry; `text` comes translated from the menu window.
#[tauri::command]
fn set_tray_update(app: AppHandle, text: Option<String>) {
    let Some(items) = app.try_state::<TrayItems>() else { return };
    let mut shown = items.update_shown.lock().unwrap();
    match text {
        Some(text) => {
            let _ = items.update.set_text(text);
            if !*shown && items.menu.insert(&items.update, 0).is_ok() {
                *shown = true;
            }
        }
        None if *shown => {
            let _ = items.menu.remove(&items.update);
            *shown = false;
        }
        None => {}
    }
}

#[tauri::command]
fn get_update(found: State<'_, updates::Found>) -> Option<updates::Update> {
    found.0.lock().unwrap().clone()
}

/// "Check now" in Settings; network, so off the main thread.
#[tauri::command]
async fn check_updates_now(app: AppHandle) -> Result<Option<updates::Update>, String> {
    let found = tauri::async_runtime::spawn_blocking(updates::check).await.map_err(|e| e.to_string())??;
    updates::announce(&app, found.clone());
    Ok(found)
}

fn open_update(app: &AppHandle) {
    use tauri_plugin_opener::OpenerExt;
    if let Some(update) = app.state::<updates::Found>().0.lock().unwrap().clone() {
        let _ = app.opener().open_url(update.url, None::<&str>);
    }
}

#[tauri::command]
fn set_tray_language(app: AppHandle, settings: String, quit: String) {
    if let Some(items) = app.try_state::<TrayItems>() {
        let _ = items.settings.set_text(settings);
        let _ = items.quit.set_text(quit);
    }
}

#[tauri::command]
fn cancel_batch(jobs: State<'_, Arc<jobs::Jobs>>, batch: u64) {
    jobs.cancel_batch(batch);
}

/// Deletes what a batch produced; returns how many results were removed.
#[tauri::command]
fn undo_batch(jobs: State<'_, Arc<jobs::Jobs>>, batch: u64) -> usize {
    jobs.undo(batch)
}

#[tauri::command]
fn copy_batch(jobs: State<'_, Arc<jobs::Jobs>>, batch: u64) -> Result<(), String> {
    jobs.copy(batch)
}

#[cfg(desktop)]
fn setup_desktop(app: &mut tauri::App) -> tauri::Result<()> {
    use tauri_plugin_global_shortcut::ShortcutState;

    app.handle().plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(move |app, shortcut, event| {
                if event.state() == ShortcutState::Pressed {
                    let target = app.state::<Hotkeys>().0.lock().unwrap().get(&shortcut.id()).cloned();
                    // Reading the selection spawns a process; keep it off the event loop.
                    let app = app.clone();
                    std::thread::spawn(move || match target.as_deref() {
                        Some(action) if !action.is_empty() => run_hotkey(&app, action),
                        _ => open_menu(&app),
                    });
                }
            })
            .build(),
    )?;
    let saved = app.state::<settings::Store>().get(app.handle());
    if let Err(e) = register_hotkeys(app.handle(), &saved) {
        // Another program may own a combination: keep at least the menu.
        eprintln!("hotkey registration failed: {e}");
        let menu_only = settings::Settings { hotkeys: Default::default(), ..saved };
        if register_hotkeys(app.handle(), &menu_only).is_err() {
            let fallback = settings::Settings { hotkey: settings::DEFAULT_HOTKEY.into(), ..menu_only };
            let _ = register_hotkeys(app.handle(), &fallback);
        }
    }

    let open = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Mutabor", true, None::<&str>)?;
    let update = MenuItem::with_id(app, "update", "Update available", true, None::<&str>)?;
    let tray_menu = Menu::with_items(app, &[&open, &quit])?;
    app.manage(TrayItems {
        menu: tray_menu.clone(),
        settings: open.clone(),
        quit: quit.clone(),
        update,
        update_shown: Mutex::new(false),
    });
    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().expect("bundle icon"))
        .tooltip("Mutabor")
        .menu(&tray_menu)
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "settings" => show_settings(app),
                "update" => open_update(app),
                "quit" => app.exit(0),
                _ => {}
            }
        })
        .build(app)?;
    updates::watch(app.handle().clone());
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();
    #[cfg(desktop)]
    {
        // Must be registered first: a second launch exits and opens the settings here.
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _, _| show_settings(app)));
    }
    builder
        .plugin(tauri_plugin_opener::init())
        .manage(Arc::new(jobs::Jobs::default()))
        .manage(settings::Store::default())
        .manage(Hotkeys::default())
        .manage(updates::Found::default())
        .invoke_handler(tauri::generate_handler![
            run_action,
            cancel_job,
            cancel_batch,
            undo_batch,
            copy_batch,
            get_settings,
            save_settings,
            list_engines,
            install_engine,
            remove_engine,
            open_settings,
            set_tray_language,
            open_crop,
            crop_preview,
            set_tray_update,
            get_update,
            check_updates_now
        ])
        .setup(|app| {
            #[cfg(desktop)]
            setup_desktop(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                // The popup behaves like a context menu: clicking away closes it.
                WindowEvent::Focused(false) if window.label() == MENU => {
                    let _ = window.hide();
                }
                // Alt+F4 must not destroy the app's windows.
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    let _ = window.hide();
                }
                _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Mutabor");
}
