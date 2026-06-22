use std::collections::HashMap;
use std::io;
use crate::config::{AppConfig, QuickStartOption};
use crate::apps::AppInfo;
use crate::hid::KeychronDevice;

pub trait ConfigRepository: Send + Sync {
    fn load(&self) -> AppConfig;
    fn save(&self, config: &AppConfig) -> Result<(), io::Error>;
}

pub trait GnomeShortcutsService: Send + Sync {
    fn is_gnome(&self) -> bool;
    fn sync_shortcuts(&self, keys: &[String]) -> Result<(), io::Error>;
    fn clear_shortcuts(&self) -> Result<(), io::Error>;
}

pub trait AppLauncher: Send + Sync {
    fn launch_app(&self, exec_cmd: &str);
    fn launch_url(&self, url: &str);
    fn list_apps(&self) -> Vec<AppInfo>;
}

pub trait AutostartManager: Send + Sync {
    fn set_enabled(&self, enabled: bool);
    fn is_enabled(&self) -> bool;
}

pub trait TriggerIpcService: Send + Sync {
    fn start_server(&self, port: u16, on_trigger: Box<dyn Fn(String) + Send + Sync + 'static>);
    fn relay_trigger(&self, key: &str, port: u16) -> bool;
}

pub trait WsServer: Send + Sync {
    fn start(
        &self,
        port: u16,
        on_get_binds: Box<dyn Fn() -> HashMap<String, Vec<QuickStartOption>> + Send + Sync + 'static>,
        on_add_bind: Box<dyn Fn(String, QuickStartOption) + Send + Sync + 'static>,
        on_clear_bind: Box<dyn Fn(Option<String>) + Send + Sync + 'static>,
        on_get_apps: Box<dyn Fn() -> Vec<AppInfo> + Send + Sync + 'static>,
    );
    fn broadcast_binds(&self, binds: &HashMap<String, Vec<QuickStartOption>>);
}

pub trait HidService: Send + Sync {
    fn list_devices(&self) -> Vec<KeychronDevice>;
}

pub trait WindowService: Send + Sync {
    fn show_and_focus(&self);
    fn hide(&self);
    fn minimize(&self);
    fn resize(&self, width: u32, height: u32, mode: &str);
    fn emit_navigate(&self, view: &str);
    fn update_tray_menu(&self, autostart: bool);
}

