pub mod config;
pub mod apps;
pub mod autostart;
pub mod gnome_shortcuts;
pub mod trigger_ipc;
pub mod hid;
pub mod bridge_ws;
pub mod ports;
pub mod domain;

use tauri::{Manager, Emitter};
use tauri::menu::{Menu, MenuItem, CheckMenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use std::sync::Arc;
use serde_json::Value;

pub struct AppState {
    pub core: Arc<crate::domain::FlickCore>,
    pub hid_manager: crate::hid::HidManager,
}

// Helper to map keyId (1-12) to F13-F24 key names
fn hid_key_id_to_f_key(key_id: u8) -> Option<String> {
    if key_id < 1 || key_id > 12 {
        return None;
    }
    Some(format!("F{}", 12 + key_id))
}

pub struct TauriWindowService {
    app_handle: tauri::AppHandle,
}

impl TauriWindowService {
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        Self { app_handle }
    }
}

impl crate::ports::WindowService for TauriWindowService {
    fn show_and_focus(&self) {
        if let Some(main_window) = self.app_handle.get_webview_window("main") {
            let _ = main_window.show();
            let _ = main_window.set_focus();
        }
    }
    fn hide(&self) {
        if let Some(main_window) = self.app_handle.get_webview_window("main") {
            let _ = main_window.hide();
        }
    }
    fn minimize(&self) {
        if let Some(main_window) = self.app_handle.get_webview_window("main") {
            let _ = main_window.minimize();
        }
    }
    fn resize(&self, width: u32, height: u32, mode: &str) {
        if let Some(main_window) = self.app_handle.get_webview_window("main") {
            let _ = main_window.set_size(tauri::Size::Logical(tauri::LogicalSize::new(width as f64, height as f64)));
            let _ = main_window.center();
            if mode == "launcher" {
                let _ = main_window.set_always_on_top(true);
                let _ = main_window.set_skip_taskbar(true);
            } else {
                let _ = main_window.set_always_on_top(false);
                let _ = main_window.set_skip_taskbar(false);
            }
        }
    }
    fn emit_navigate(&self, view: &str) {
        if let Some(main_window) = self.app_handle.get_webview_window("main") {
            let _ = main_window.emit("navigate", view);
        }
    }
    fn update_tray_menu(&self, autostart: bool) {
        update_tray_menu(&self.app_handle, autostart);
    }
}

// Helper to update / rebuild the tray menu
fn update_tray_menu(app_handle: &tauri::AppHandle, autostart: bool) {
    if let Some(tray) = app_handle.tray_by_id("main_tray") {
        let state = app_handle.state::<AppState>();
        let config = state.core.get_config();
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
fn get_config(state: tauri::State<'_, AppState>) -> crate::config::AppConfig {
    state.core.get_config()
}

#[tauri::command]
fn scan_hid(state: tauri::State<'_, AppState>) -> Value {
    state.core.scan_hid()
}

#[tauri::command]
fn update_config(config: Value, state: tauri::State<'_, AppState>) -> crate::config::AppConfig {
    state.core.update_config(config)
}

#[tauri::command]
fn list_apps(state: tauri::State<'_, AppState>) -> Vec<crate::apps::AppInfo> {
    state.core.list_apps()
}

#[tauri::command]
fn get_app_version() -> String {
    "1.1.0".to_string()
}

#[tauri::command]
fn open_external(url: String, state: tauri::State<'_, AppState>) {
    state.core.open_external(&url);
}

#[tauri::command]
fn launch_app(exec_cmd: String, window: tauri::Window, state: tauri::State<'_, AppState>) {
    state.core.launch_app(&exec_cmd);
    let _ = window.hide();
}

#[tauri::command]
fn resize_window(width: u32, height: u32, mode: String, state: tauri::State<'_, AppState>) {
    state.core.resize_window(width, height, &mode);
}

#[tauri::command]
fn hide_window(state: tauri::State<'_, AppState>) {
    state.core.hide_window();
}

#[tauri::command]
fn minimize_window(state: tauri::State<'_, AppState>) {
    state.core.minimize_window();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 1. Initialize configuration storage
    crate::config::init();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
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

            // Instantiate concrete adapters
            let config_repo = Arc::new(crate::config::FsConfigRepository);
            let gnome_shortcuts = Arc::new(crate::gnome_shortcuts::GSettingsGnomeShortcutsService);
            let app_launcher = Arc::new(crate::apps::ProcessAppLauncher);
            let autostart_manager = Arc::new(crate::autostart::AutoLaunchManager);
            let trigger_ipc = Arc::new(crate::trigger_ipc::TcpTriggerIpcService);
            let ws_server = Arc::new(crate::bridge_ws::TungsteniteWsServer::new());
            let hid_service = Arc::new(crate::hid::HidApiService);
            let window_service = Arc::new(TauriWindowService::new(app_handle.clone()));

            let core = Arc::new(crate::domain::FlickCore::new(
                config_repo,
                gnome_shortcuts,
                app_launcher,
                autostart_manager,
                trigger_ipc,
                ws_server,
                hid_service,
                window_service,
            ));

            // Run core initialization
            core.init_app();

            app.manage(AppState {
                core: core.clone(),
                hid_manager: crate::hid::HidManager::new(),
            });

            // 2. Setup System Tray
            let app_config = core.get_config();
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
            let core_clone = core.clone();
            app.on_menu_event(move |handle, event| {
                let id = event.id.as_ref();
                if let Some(main_window) = handle.get_webview_window("main") {
                    match id {
                        "settings" => {
                            let _ = main_window.show();
                            let _ = main_window.set_focus();
                            let _ = main_window.emit("navigate", "settings");
                            core_clone.set_current_view("settings");
                        }
                        "launcher" => {
                            let _ = main_window.show();
                            let _ = main_window.set_focus();
                            let _ = main_window.emit("navigate", "launcher");
                            core_clone.set_current_view("launcher");
                        }
                        "autostart" => {
                            let config = core_clone.get_config();
                            let new_val = !config.autostart;
                            let _ = core_clone.update_config(serde_json::json!({ "autostart": new_val }));
                        }
                        "quit" => {
                            handle.exit(0);
                        }
                        _ => {}
                    }
                }
            });

            // 4. Setup HID Manager listener
            let state = app_handle.state::<AppState>();
            let (hid_tx, mut hid_rx) = tokio::sync::mpsc::unbounded_channel();
            state.hid_manager.set_sender(hid_tx);
            state.hid_manager.start();

            let core_clone_hid = core.clone();
            let handle_clone_3 = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                while let Some(event) = hid_rx.recv().await {
                    match event {
                        crate::hid::HidEvent::QuickstartKey { f_key } => {
                            core_clone_hid.handle_key_trigger(&f_key);
                        }
                        crate::hid::HidEvent::AssistKey { key_id, .. } => {
                            if let Some(f_key) = hid_key_id_to_f_key(key_id) {
                                let config = core_clone_hid.get_config();
                                if config.quick_start_binds.contains_key(&f_key) {
                                    core_clone_hid.handle_key_trigger(&f_key);
                                    continue;
                                }
                            }
                            // Default: open launcher search
                            if let Some(main_window) = handle_clone_3.get_webview_window("main") {
                                let _ = main_window.show();
                                let _ = main_window.set_focus();
                                let _ = main_window.emit("navigate", "launcher");
                                core_clone_hid.set_current_view("launcher");
                            }
                        }
                    }
                }
            });

            // 5. Window Event Handlers (Blur/Focus)
            if let Some(main_window) = app.get_webview_window("main") {
                let core_clone_blur = core.clone();
                let handle_clone_4 = app_handle.clone();
                main_window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        let view = core_clone_blur.get_current_view();
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
                            let core_clone_cold = core.clone();
                            tauri::async_runtime::spawn(async move {
                                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                                core_clone_cold.handle_key_trigger(&key);
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
