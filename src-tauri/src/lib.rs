pub mod config;
pub mod apps;
pub mod autostart;
pub mod gnome_shortcuts;
pub mod trigger_ipc;
pub mod hid;
pub mod bridge_ws;

use tauri::{Manager, Emitter};
use tauri::menu::{Menu, MenuItem, CheckMenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use std::sync::Mutex;
use serde_json::Value;

pub struct AppState {
    pub current_view: Mutex<String>,
    pub hid_manager: crate::hid::HidManager,
}

// Helper to map keyId (1-12) to F13-F24 key names
fn hid_key_id_to_f_key(key_id: u8) -> Option<String> {
    if key_id < 1 || key_id > 12 {
        return None;
    }
    Some(format!("F{}", 12 + key_id))
}

fn fire_quick_start(key: &str, app_handle: &tauri::AppHandle) {
    let key_upper = key.to_uppercase();
    if key_upper == "FOCUS" {
        if let Some(main_window) = app_handle.get_webview_window("main") {
            let _ = main_window.show();
            let _ = main_window.set_focus();
        }
        return;
    }
    static LAST_FIRED: Mutex<Option<(String, std::time::Instant)>> = Mutex::new(None);
    {
        let mut last_fired = LAST_FIRED.lock().unwrap();
        if let Some((ref last_key, last_time)) = *last_fired {
            if last_key == &key_upper && last_time.elapsed() < std::time::Duration::from_millis(400) {
                println!("[trigger] {} debounced (duplicate within 400ms)", key_upper);
                return;
            }
        }
        *last_fired = Some((key_upper.clone(), std::time::Instant::now()));
    }

    let config = crate::config::get_config();
    if let Some(options) = config.quick_start_binds.get(&key_upper) {
        println!("[trigger] {} -> launching {} action(s)", key_upper, options.len());
        for opt in options {
            let opt = opt.clone();
            tauri::async_runtime::spawn(async move {
                if opt.opt_type == "Url" {
                    crate::apps::launch_url(&opt.opt_data.path);
                } else {
                    crate::apps::launch_app(&opt.opt_data.path);
                }
            });
        }
    } else {
        println!("[trigger] {} pressed but no Quick Start bind configured", key_upper);
    }
}

// Helper to update / rebuild the tray menu
fn update_tray_menu(app_handle: &tauri::AppHandle, autostart: bool) {
    if let Some(tray) = app_handle.tray_by_id("main_tray") {
        let config = crate::config::get_config();
        let lang = &config.language;
        let is_en = lang == "en";
        let settings_label = if is_en { "Open Settings" } else { "Abrir Configurações" };
        let launcher_label = if is_en { "Open Launcher (Search)" } else { "Abrir Launcher (Buscar)" };
        let autostart_label = if is_en { "Launch at Startup" } else { "Iniciar com o Sistema" };
        let quit_label = if is_en { "Quit" } else { "Sair" };

        if let Ok(menu) = Menu::with_items(app_handle, &[
            &MenuItem::with_id(app_handle, "title", "Flick", false, None::<&str>).unwrap(),
            &PredefinedMenuItem::separator(app_handle).unwrap(),
            &MenuItem::with_id(app_handle, "settings", settings_label, true, None::<&str>).unwrap(),
            &MenuItem::with_id(app_handle, "launcher", launcher_label, true, None::<&str>).unwrap(),
            &CheckMenuItem::with_id(app_handle, "autostart", autostart_label, true, autostart, None::<&str>).unwrap(),
            &PredefinedMenuItem::separator(app_handle).unwrap(),
            &MenuItem::with_id(app_handle, "quit", quit_label, true, None::<&str>).unwrap(),
        ]) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

// Tauri commands implementation

#[tauri::command]
fn get_config() -> crate::config::AppConfig {
    crate::config::get_config()
}

#[tauri::command]
fn scan_hid() -> Value {
    let keyboards = crate::hid::list_keychron_devices();
    let connected = !keyboards.is_empty();
    serde_json::json!({
        "connected": connected,
        "keyboards": keyboards
    })
}

#[tauri::command]
fn update_config(config: Value, app_handle: tauri::AppHandle) -> crate::config::AppConfig {
    let updated = crate::config::update_config(config.clone());

    // Apply side effects
    if config.get("autostart").is_some() {
        crate::autostart::set_autostart_enabled(updated.autostart);
    }
    if config.get("quickStartBinds").is_some() {
        let keys: Vec<String> = updated.quick_start_binds.keys().cloned().collect();
        let _ = crate::gnome_shortcuts::sync_gnome_shortcuts(&keys);
        crate::bridge_ws::broadcast_key_binds();
    }
    
    update_tray_menu(&app_handle, updated.autostart);
    updated
}

#[tauri::command]
fn list_apps() -> Vec<crate::apps::AppInfo> {
    crate::apps::list_installed_apps_with_icons()
}

#[tauri::command]
fn get_app_version() -> String {
    "1.1.0".to_string()
}

#[tauri::command]
fn open_external(url: String) {
    crate::apps::launch_url(&url);
}

#[tauri::command]
fn launch_app(exec_cmd: String, window: tauri::Window) {
    crate::apps::launch_app(&exec_cmd);
    let _ = window.hide();
}

#[tauri::command]
fn resize_window(width: u32, height: u32, mode: String, window: tauri::Window, state: tauri::State<'_, AppState>) {
    let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize::new(width as f64, height as f64)));
    let _ = window.center();
    *state.current_view.lock().unwrap() = mode.clone();
    if mode == "launcher" {
        let _ = window.set_always_on_top(true);
        let _ = window.set_skip_taskbar(true);
    } else {
        let _ = window.set_always_on_top(false);
        let _ = window.set_skip_taskbar(false);
    }
}

#[tauri::command]
fn hide_window(window: tauri::Window) {
    let _ = window.hide();
}

#[tauri::command]
fn minimize_window(window: tauri::Window) {
    let _ = window.minimize();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 1. Initialize configuration
    crate::config::init();

    let app_config = crate::config::get_config();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            current_view: Mutex::new("launcher".to_string()),
            hid_manager: crate::hid::HidManager::new(),
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            scan_hid,
            update_config,
            list_apps,
            get_app_version,
            open_external,
            launch_app,
            resize_window,
            hide_window,
            minimize_window
        ])
        .setup(move |app| {
            let app_handle = app.handle().clone();

            // Synchronize GNOME custom shortcuts on startup
            let keys: Vec<String> = app_config.quick_start_binds.keys().cloned().collect();
            let _ = crate::gnome_shortcuts::sync_gnome_shortcuts(&keys);

            // 2. Setup System Tray
            let lang = &app_config.language;
            let is_en = lang == "en";
            let settings_label = if is_en { "Open Settings" } else { "Abrir Configurações" };
            let launcher_label = if is_en { "Open Launcher (Search)" } else { "Abrir Launcher (Buscar)" };
            let autostart_label = if is_en { "Launch at Startup" } else { "Iniciar com o Sistema" };
            let quit_label = if is_en { "Quit" } else { "Sair" };

            let menu = Menu::with_items(app, &[
                &MenuItem::with_id(app, "title", "Flick", false, None::<&str>)?,
                &PredefinedMenuItem::separator(app)?,
                &MenuItem::with_id(app, "settings", settings_label, true, None::<&str>)?,
                &MenuItem::with_id(app, "launcher", launcher_label, true, None::<&str>)?,
                &CheckMenuItem::with_id(app, "autostart", autostart_label, true, app_config.autostart, None::<&str>)?,
                &PredefinedMenuItem::separator(app)?,
                &MenuItem::with_id(app, "quit", quit_label, true, None::<&str>)?,
            ])?;

            let _tray = TrayIconBuilder::with_id("main_tray")
                .icon(tauri::image::Image::from_bytes(include_bytes!("../icons/32x32.png")).unwrap())
                .menu(&menu)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: tauri::tray::MouseButton::Left, .. } = event {
                        let app_handle = tray.app_handle();
                        if let Some(main_window) = app_handle.get_webview_window("main") {
                            if main_window.is_visible().unwrap_or(false) {
                                let _ = main_window.hide();
                            } else {
                                let _ = main_window.show();
                                let _ = main_window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            // 3. Setup Menu Event Handlers
            app.on_menu_event(move |handle, event| {
                let id = event.id.as_ref();
                if let Some(main_window) = handle.get_webview_window("main") {
                    match id {
                        "settings" => {
                            let _ = main_window.show();
                            let _ = main_window.set_focus();
                            let _ = main_window.emit("navigate", "settings");
                            let state = handle.state::<AppState>();
                            *state.current_view.lock().unwrap() = "settings".to_string();
                        }
                        "launcher" => {
                            let _ = main_window.show();
                            let _ = main_window.set_focus();
                            let _ = main_window.emit("navigate", "launcher");
                            let state = handle.state::<AppState>();
                            *state.current_view.lock().unwrap() = "launcher".to_string();
                        }
                        "autostart" => {
                            let config = crate::config::get_config();
                            let new_val = !config.autostart;
                            crate::autostart::set_autostart_enabled(new_val);
                            let _ = crate::config::update_config(serde_json::json!({ "autostart": new_val }));
                            update_tray_menu(handle, new_val);
                        }
                        "quit" => {
                            handle.exit(0);
                        }
                        _ => {}
                    }
                }
            });

            // 4. Start Trigger IPC Server
            let handle_clone_1 = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                crate::trigger_ipc::start_trigger_server(move |key| {
                    fire_quick_start(&key, &handle_clone_1);
                }).await;
            });

            let port = app_config.ws_port;
            tauri::async_runtime::spawn(async move {
                crate::bridge_ws::start_server(port, move || {
                    let config = crate::config::get_config();
                    let keys: Vec<String> = config.quick_start_binds.keys().cloned().collect();
                    let _ = crate::gnome_shortcuts::sync_gnome_shortcuts(&keys);
                    crate::bridge_ws::broadcast_key_binds();
                }).await;
            });

            // 6. Setup HID Manager listener
            let state = app_handle.state::<AppState>();
            let (hid_tx, mut hid_rx) = tokio::sync::mpsc::unbounded_channel();
            state.hid_manager.set_sender(hid_tx);
            state.hid_manager.start();

            let handle_clone_3 = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                while let Some(event) = hid_rx.recv().await {
                    match event {
                        crate::hid::HidEvent::QuickstartKey { f_key } => {
                            fire_quick_start(&f_key, &handle_clone_3);
                        }
                        crate::hid::HidEvent::AssistKey { key_id, .. } => {
                            if let Some(f_key) = hid_key_id_to_f_key(key_id) {
                                let config = crate::config::get_config();
                                if config.quick_start_binds.contains_key(&f_key) {
                                    fire_quick_start(&f_key, &handle_clone_3);
                                    continue;
                                }
                            }
                            // Default: open launcher search
                            if let Some(main_window) = handle_clone_3.get_webview_window("main") {
                                let _ = main_window.show();
                                let _ = main_window.set_focus();
                                let _ = main_window.emit("navigate", "launcher");
                                let state = handle_clone_3.state::<AppState>();
                                *state.current_view.lock().unwrap() = "launcher".to_string();
                            }
                        }
                    }
                }
            });

            // 7. Window Event Handlers (Blur/Focus)
            if let Some(main_window) = app.get_webview_window("main") {
                let handle_clone_4 = app_handle.clone();
                main_window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        let state = handle_clone_4.state::<AppState>();
                        let view = state.current_view.lock().unwrap().clone();
                        if view == "launcher" {
                            let _ = handle_clone_4.get_webview_window("main").unwrap().hide();
                        }
                    }
                });

                // Show window initially unless `--hidden` or `--trigger` was specified
                let args: Vec<String> = std::env::args().collect();
                let has_trigger = args.contains(&"--trigger".to_string());
                if !args.contains(&"--hidden".to_string()) && !has_trigger {
                    let _ = main_window.show();
                    let _ = main_window.set_focus();
                }

                if has_trigger {
                    if let Some(idx) = args.iter().position(|a| a == "--trigger") {
                        if idx + 1 < args.len() {
                            let key = args[idx + 1].clone();
                            let handle_clone_cold = app_handle.clone();
                            tauri::async_runtime::spawn(async move {
                                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                                fire_quick_start(&key, &handle_clone_cold);
                            });
                        }
                    }
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Prevent app from quitting when settings window is closed
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            crate::gnome_shortcuts::clear_gnome_shortcuts_sync();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hid_key_id_to_f_key() {
        assert_eq!(hid_key_id_to_f_key(1), Some("F13".to_string()));
        assert_eq!(hid_key_id_to_f_key(5), Some("F17".to_string()));
        assert_eq!(hid_key_id_to_f_key(12), Some("F24".to_string()));

        // Out of bounds cases
        assert_eq!(hid_key_id_to_f_key(0), None);
        assert_eq!(hid_key_id_to_f_key(13), None);
        assert_eq!(hid_key_id_to_f_key(255), None);
    }
}

