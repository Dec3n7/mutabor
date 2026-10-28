mod engines;
mod files;
mod jobs;
mod pdf;
mod recipes;
mod selection;
mod settings;

use std::sync::Arc;

use serde::Serialize;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, State, WindowEvent};

const MENU: &str = "menu";
const SETTINGS: &str = "settings";

#[derive(Clone, Serialize)]
struct MenuOpen {
    files: Vec<files::FileInfo>,
    settings: settings::Settings,
    engines: Vec<engines::EngineInfo>,
}

/// Collects the selection, places the popup next to the cursor and shows it.
fn open_menu(app: &AppHandle) {
    let files = files::describe(selection::selected_paths());
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
    let _ = app.emit_to(MENU, "menu:open", MenuOpen { files, settings, engines });
    let _ = win.show();
    let _ = win.set_focus();
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
    let jobs = jobs.inner().clone();
    tauri::async_runtime::spawn_blocking(move || jobs.start(&app, &action, paths, &opts))
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

/// Saves the settings, re-registering the hotkey first when it changed so a
/// combination taken by another program is rejected and nothing is saved.
#[tauri::command]
fn save_settings(app: AppHandle, store: State<'_, settings::Store>, next: settings::Settings) -> Result<(), String> {
    let old = store.get(&app);
    #[cfg(desktop)]
    if next.hotkey != old.hotkey {
        swap_hotkey(&app, &old.hotkey, &next.hotkey)?;
    }
    store.save(&app, next.clone())?;
    let _ = app.emit("settings", next);
    Ok(())
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

#[cfg(desktop)]
fn swap_hotkey(app: &AppHandle, old: &str, new: &str) -> Result<(), String> {
    use std::str::FromStr;
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
    let new_key = Shortcut::from_str(new).map_err(|_| "hotkey_invalid".to_string())?;
    let gs = app.global_shortcut();
    if let Ok(old_key) = Shortcut::from_str(old) {
        let _ = gs.unregister(old_key);
    }
    gs.register(new_key).map_err(|_| {
        // Put the previous combination back so the app stays reachable.
        if let Ok(old_key) = Shortcut::from_str(old) {
            let _ = gs.register(old_key);
        }
        "hotkey_taken".to_string()
    })
}

#[tauri::command]
fn cancel_job(jobs: State<'_, Arc<jobs::Jobs>>, id: u64) {
    jobs.cancel(id);
}

#[cfg(desktop)]
fn setup_desktop(app: &mut tauri::App) -> tauri::Result<()> {
    use std::str::FromStr;
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

    // Only the menu hotkey is ever registered, so any press opens the menu.
    app.handle().plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(move |app, _shortcut, event| {
                if event.state() == ShortcutState::Pressed {
                    // Reading the selection spawns a process; keep it off the event loop.
                    let app = app.clone();
                    std::thread::spawn(move || open_menu(&app));
                }
            })
            .build(),
    )?;
    let hotkey = app.state::<settings::Store>().get(app.handle()).hotkey;
    let shortcut = Shortcut::from_str(&hotkey)
        .or_else(|_| Shortcut::from_str(settings::DEFAULT_HOTKEY))
        .expect("default hotkey parses");
    if let Err(e) = app.global_shortcut().register(shortcut) {
        // Another program may own the combination; the tray still works.
        eprintln!("hotkey registration failed: {e}");
    }

    let open = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Mutabor", true, None::<&str>)?;
    let tray_menu = Menu::with_items(app, &[&open, &quit])?;
    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().expect("bundle icon"))
        .tooltip("Mutabor")
        .menu(&tray_menu)
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "settings" => show_settings(app),
                "quit" => app.exit(0),
                _ => {}
            }
        })
        .build(app)?;
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
        .invoke_handler(tauri::generate_handler![
            run_action,
            cancel_job,
            get_settings,
            save_settings,
            list_engines,
            install_engine,
            remove_engine,
            open_settings
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
