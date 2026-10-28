mod files;
mod jobs;
mod recipes;
mod selection;

use std::sync::Arc;

use serde::Serialize;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, State, WindowEvent};

const MENU: &str = "menu";

#[derive(Clone, Serialize)]
struct MenuOpen {
    files: Vec<files::FileInfo>,
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

    let _ = app.emit_to(MENU, "menu:open", MenuOpen { files });
    let _ = win.show();
    let _ = win.set_focus();
}

/// Starts the action on every selected file. Probing the inputs runs
/// ffmpeg, so this stays off the main thread.
#[tauri::command]
async fn run_action(
    app: AppHandle,
    jobs: State<'_, Arc<jobs::Jobs>>,
    action: String,
    paths: Vec<String>,
) -> Result<(), String> {
    let jobs = jobs.inner().clone();
    tauri::async_runtime::spawn_blocking(move || jobs.start(&app, &action, paths))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
fn cancel_job(jobs: State<'_, Arc<jobs::Jobs>>, id: u64) {
    jobs.cancel(id);
}

#[cfg(desktop)]
fn setup_desktop(app: &mut tauri::App) -> tauri::Result<()> {
    use tauri_plugin_global_shortcut::{
        Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState,
    };

    // Ctrl+Alt+C on Windows/Linux, Cmd+Option+C on macOS.
    #[cfg(target_os = "macos")]
    let mods = Modifiers::SUPER | Modifiers::ALT;
    #[cfg(not(target_os = "macos"))]
    let mods = Modifiers::CONTROL | Modifiers::ALT;
    let hotkey = Shortcut::new(Some(mods), Code::KeyC);

    app.handle().plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(move |app, shortcut, event| {
                if shortcut == &hotkey && event.state() == ShortcutState::Pressed {
                    // Reading the selection spawns a process; keep it off the event loop.
                    let app = app.clone();
                    std::thread::spawn(move || open_menu(&app));
                }
            })
            .build(),
    )?;
    if let Err(e) = app.global_shortcut().register(hotkey) {
        // Another program may own the combination; the tray still works.
        eprintln!("hotkey registration failed: {e}");
    }

    let quit = MenuItem::with_id(app, "quit", "Quit Mutabor", true, None::<&str>)?;
    let tray_menu = Menu::with_items(app, &[&quit])?;
    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().expect("bundle icon"))
        .tooltip("Mutabor")
        .menu(&tray_menu)
        .on_menu_event(|app, event| {
            if event.id.as_ref() == "quit" {
                app.exit(0);
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
        // Must be registered first: a second launch just exits.
        builder = builder.plugin(tauri_plugin_single_instance::init(|_, _, _| {}));
    }
    builder
        .plugin(tauri_plugin_opener::init())
        .manage(Arc::new(jobs::Jobs::default()))
        .invoke_handler(tauri::generate_handler![run_action, cancel_job])
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
