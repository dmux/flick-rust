use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

pub const KEYCHRON_WS_PORT: u16 = 40003;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Binding {
    pub key_id: u32,
    pub action_type: String, // "launcher" | "launch_app" | "none"
    pub target_app_id: Option<String>,
    pub target_app_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuickStartOptionData {
    pub name: String,
    pub path: String,
    pub icon: String,
    pub icon_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuickStartOption {
    pub opt_type: String, // "App" | "Url"
    pub opt_data: QuickStartOptionData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub theme: String, // "light" | "dark" | "system"
    pub language: String, // "en" | "pt-BR"
    pub autostart: bool,
    pub ws_port: u16,
    pub bindings: Vec<Binding>,
    pub quick_start_binds: HashMap<String, Vec<QuickStartOption>>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: "system".to_string(),
            language: "en".to_string(),
            autostart: false,
            ws_port: KEYCHRON_WS_PORT,
            bindings: vec![Binding {
                key_id: 1,
                action_type: "launcher".to_string(),
                target_app_id: None,
                target_app_name: None,
            }],
            quick_start_binds: HashMap::new(),
        }
    }
}

// Config manager
static CONFIG: OnceLock<RwLock<AppConfig>> = OnceLock::new();

fn get_config_dir() -> PathBuf {
    if cfg!(test) || std::env::var("CARGO_MANIFEST_DIR").is_ok() {
        let mut path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        path.push("target");
        path.push("flick_test");
        path
    } else {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("flick");
        path
    }
}

fn get_config_path() -> PathBuf {
    let mut path = get_config_dir();
    path.push("config.json");
    path
}

pub fn init() {
    let config_path = get_config_path();
    let mut config = AppConfig::default();

    if config_path.exists() {
        if let Ok(content) = fs::read_to_string(&config_path) {
            // Attempt to parse normal config
            if let Ok(mut parsed) = serde_json::from_str::<AppConfig>(&content) {
                // Migrate legacy wsPort (5005)
                if parsed.ws_port == 5005 {
                    parsed.ws_port = KEYCHRON_WS_PORT;
                }
                config = parsed;
            } else {
                // If it fails to parse AppConfig directly, try parsing with migration of quickStartBinds.
                // We'll read it as a raw Value.
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) {
                    let theme = value.get("theme").and_then(|v| v.as_str()).unwrap_or("system").to_string();
                    let language = value.get("language").and_then(|v| v.as_str()).unwrap_or("en").to_string();
                    let autostart = value.get("autostart").and_then(|v| v.as_bool()).unwrap_or(false);
                    let mut ws_port = value.get("wsPort").and_then(|v| v.as_u64()).unwrap_or(KEYCHRON_WS_PORT as u64) as u16;
                    if ws_port == 5005 {
                        ws_port = KEYCHRON_WS_PORT;
                    }
                    let bindings = value.get("bindings")
                        .and_then(|v| serde_json::from_value::<Vec<Binding>>(v.clone()).ok())
                        .unwrap_or_else(|| AppConfig::default().bindings);

                    // Migration of quickStartBinds (handling map-of-maps layout)
                    let mut quick_start_binds = HashMap::new();
                    if let Some(qs_val) = value.get("quickStartBinds") {
                        if let Some(qs_obj) = qs_val.as_object() {
                            for (key, val) in qs_obj {
                                // If the val is an array, it's already flat
                                if let Ok(opts) = serde_json::from_value::<Vec<QuickStartOption>>(val.clone()) {
                                    quick_start_binds.insert(key.clone(), opts);
                                } else if let Some(device_binds) = val.as_object() {
                                    // It is nested under a device key. Extract it
                                    for (fkey, fopts) in device_binds {
                                        if let Ok(opts) = serde_json::from_value::<Vec<QuickStartOption>>(fopts.clone()) {
                                            quick_start_binds.insert(fkey.clone(), opts);
                                        }
                                    }
                                }
                            }
                        }
                    }

                    config = AppConfig {
                        theme,
                        language,
                        autostart,
                        ws_port,
                        bindings,
                        quick_start_binds,
                    };
                }
            }
        }
    }

    // Save initial / migrated config
    let _ = save_config_internal(&config);

    if let Some(rw) = CONFIG.get() {
        let mut write_guard = rw.write().unwrap();
        *write_guard = config;
    } else {
        let _ = CONFIG.set(RwLock::new(config));
    }
}

fn save_config_internal(config: &AppConfig) -> Result<(), std::io::Error> {
    let dir = get_config_dir();
    fs::create_dir_all(&dir)?;
    let config_path = get_config_path();
    let content = serde_json::to_string_pretty(config)?;
    fs::write(config_path, content)?;
    Ok(())
}

pub fn get_config() -> AppConfig {
    if CONFIG.get().is_none() {
        init();
    }
    let rw = CONFIG.get().unwrap();
    let read_guard = rw.read().unwrap();
    read_guard.clone()
}

pub fn update_config(update: serde_json::Value) -> AppConfig {
    if CONFIG.get().is_none() {
        init();
    }
    let rw = CONFIG.get().unwrap();
    let mut write_guard = rw.write().unwrap();

    // Partial update using serde_json merging
    let mut current_val = serde_json::to_value(&*write_guard).unwrap();
    if let Some(obj) = current_val.as_object_mut() {
        if let Some(update_obj) = update.as_object() {
            for (key, val) in update_obj {
                obj.insert(key.clone(), val.clone());
            }
        }
    }

    if let Ok(mut updated_config) = serde_json::from_value::<AppConfig>(current_val) {
        if updated_config.ws_port == 5005 {
            updated_config.ws_port = KEYCHRON_WS_PORT;
        }
        *write_guard = updated_config;
        let _ = save_config_internal(&*write_guard);
    }

    write_guard.clone()
}

pub struct FsConfigRepository;

impl crate::ports::ConfigRepository for FsConfigRepository {
    fn load(&self) -> AppConfig {
        get_config()
    }
    fn save(&self, config: &AppConfig) -> Result<(), std::io::Error> {
        // Update the global static CONFIG as well to stay in sync if needed,
        // but since we save to disk, we can update the lock:
        if let Some(rw) = CONFIG.get() {
            let mut write_guard = rw.write().unwrap();
            *write_guard = config.clone();
        } else {
            let _ = CONFIG.set(RwLock::new(config.clone()));
        }
        save_config_internal(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_default_config() {
        let default_config = AppConfig::default();
        assert_eq!(default_config.theme, "system");
        assert_eq!(default_config.language, "en");
        assert_eq!(default_config.autostart, false);
        assert_eq!(default_config.ws_port, KEYCHRON_WS_PORT);
        assert_eq!(default_config.bindings.len(), 1);
        assert_eq!(default_config.bindings[0].action_type, "launcher");
    }

    #[test]
    fn test_save_and_load_config() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let path = get_config_path();
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }

        let mut config = AppConfig::default();
        config.theme = "dark".to_string();
        config.language = "pt-BR".to_string();
        config.autostart = true;

        let save_result = save_config_internal(&config);
        assert!(save_result.is_ok());
        assert!(path.exists());

        init();
        let loaded = get_config();
        assert_eq!(loaded.theme, "dark");
        assert_eq!(loaded.language, "pt-BR");
        assert_eq!(loaded.autostart, true);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_update_config() {
        let _guard = TEST_MUTEX.lock().unwrap();
        init();
        let update_val = json!({
            "theme": "light",
            "language": "en",
            "autostart": false
        });

        let updated = update_config(update_val);
        assert_eq!(updated.theme, "light");
        assert_eq!(updated.language, "en");
        assert_eq!(updated.autostart, false);
    }
}

