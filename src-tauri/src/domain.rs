use std::sync::{Arc, Mutex};
use std::time::{Instant, Duration};
use crate::config::AppConfig;
use crate::apps::AppInfo;
use crate::ports::{
    ConfigRepository, GnomeShortcutsService, AppLauncher, AutostartManager,
    TriggerIpcService, WsServer, HidService, WindowService
};

pub struct FlickCore {
    config_repo: Arc<dyn ConfigRepository>,
    gnome_shortcuts: Arc<dyn GnomeShortcutsService>,
    app_launcher: Arc<dyn AppLauncher>,
    autostart_manager: Arc<dyn AutostartManager>,
    trigger_ipc: Arc<dyn TriggerIpcService>,
    ws_server: Arc<dyn WsServer>,
    hid_service: Arc<dyn HidService>,
    window_service: Arc<dyn WindowService>,
    last_fired: Mutex<Option<(String, Instant)>>,
    current_view: Mutex<String>,
}

impl FlickCore {
    pub fn new(
        config_repo: Arc<dyn ConfigRepository>,
        gnome_shortcuts: Arc<dyn GnomeShortcutsService>,
        app_launcher: Arc<dyn AppLauncher>,
        autostart_manager: Arc<dyn AutostartManager>,
        trigger_ipc: Arc<dyn TriggerIpcService>,
        ws_server: Arc<dyn WsServer>,
        hid_service: Arc<dyn HidService>,
        window_service: Arc<dyn WindowService>,
    ) -> Self {
        Self {
            config_repo,
            gnome_shortcuts,
            app_launcher,
            autostart_manager,
            trigger_ipc,
            ws_server,
            hid_service,
            window_service,
            last_fired: Mutex::new(None),
            current_view: Mutex::new("launcher".to_string()),
        }
    }

    pub fn init_app(self: &Arc<Self>) {
        let config = self.config_repo.load();

        // 1. Sync gnome shortcuts
        let keys: Vec<String> = config.quick_start_binds.keys().cloned().collect();
        let _ = self.gnome_shortcuts.sync_shortcuts(&keys);

        // 2. Start trigger IPC server
        let self_clone = self.clone();
        self.trigger_ipc.start_server(
            crate::trigger_ipc::FLICK_TRIGGER_PORT,
            Box::new(move |key| {
                self_clone.handle_key_trigger(&key);
            }),
        );

        // 3. Start WS server
        let self_clone_ws1 = self.clone();
        let self_clone_ws2 = self.clone();
        let self_clone_ws3 = self.clone();
        let self_clone_ws4 = self.clone();
        self.ws_server.start(
            config.ws_port,
            // on_get_binds
            Box::new(move || {
                self_clone_ws1.get_config().quick_start_binds
            }),
            // on_add_bind
            Box::new(move |key, bind| {
                let mut current_binds = self_clone_ws2.get_config().quick_start_binds;
                current_binds.insert(key, vec![bind]);
                let _ = self_clone_ws2.update_config(serde_json::json!({ "quickStartBinds": current_binds }));
            }),
            // on_clear_bind
            Box::new(move |key_opt| {
                let mut current_binds = self_clone_ws3.get_config().quick_start_binds;
                if let Some(key) = key_opt {
                    current_binds.remove(&key);
                } else {
                    current_binds.clear();
                }
                let _ = self_clone_ws3.update_config(serde_json::json!({ "quickStartBinds": current_binds }));
            }),
            // on_get_apps
            Box::new(move || {
                self_clone_ws4.list_apps()
            }),
        );
    }

    pub fn get_config(&self) -> AppConfig {
        self.config_repo.load()
    }

    pub fn update_config(&self, update: serde_json::Value) -> AppConfig {
        let current = self.config_repo.load();

        // Partial update using serde_json merging
        let mut current_val = serde_json::to_value(&current).unwrap();
        if let Some(obj) = current_val.as_object_mut() {
            if let Some(update_obj) = update.as_object() {
                for (key, val) in update_obj {
                    obj.insert(key.clone(), val.clone());
                }
            }
        }

        let mut updated = serde_json::from_value::<AppConfig>(current_val).unwrap_or(current);
        if updated.ws_port == 5005 {
            updated.ws_port = crate::config::KEYCHRON_WS_PORT;
        }

        let _ = self.config_repo.save(&updated);

        // Apply side effects
        if update.get("autostart").is_some() {
            self.autostart_manager.set_enabled(updated.autostart);
        }
        if update.get("quickStartBinds").is_some() {
            let keys: Vec<String> = updated.quick_start_binds.keys().cloned().collect();
            let _ = self.gnome_shortcuts.sync_shortcuts(&keys);
            self.ws_server.broadcast_binds(&updated.quick_start_binds);
        }

        self.window_service.update_tray_menu(updated.autostart);

        updated
    }

    pub fn handle_key_trigger(&self, key: &str) {
        let key_upper = key.to_uppercase();
        if key_upper == "FOCUS" {
            self.window_service.show_and_focus();
            return;
        }

        // Debounce logic
        {
            let mut last_fired = self.last_fired.lock().unwrap();
            if let Some((ref last_key, last_time)) = *last_fired {
                if last_key == &key_upper && last_time.elapsed() < Duration::from_millis(400) {
                    println!("[trigger] {} debounced (duplicate within 400ms)", key_upper);
                    return;
                }
            }
            *last_fired = Some((key_upper.clone(), Instant::now()));
        }

        let config = self.config_repo.load();
        if let Some(options) = config.quick_start_binds.get(&key_upper) {
            println!("[trigger] {} -> launching {} action(s)", key_upper, options.len());
            for opt in options {
                let opt = opt.clone();
                let app_launcher = self.app_launcher.clone();
                tokio::spawn(async move {
                    if opt.opt_type == "Url" {
                        app_launcher.launch_url(&opt.opt_data.path);
                    } else {
                        app_launcher.launch_app(&opt.opt_data.path);
                    }
                });
            }
        } else {
            println!("[trigger] {} pressed but no Quick Start bind configured", key_upper);
        }
    }

    pub fn scan_hid(&self) -> serde_json::Value {
        let keyboards = self.hid_service.list_devices();
        let connected = !keyboards.is_empty();
        serde_json::json!({
            "connected": connected,
            "keyboards": keyboards
        })
    }

    pub fn list_apps(&self) -> Vec<AppInfo> {
        self.app_launcher.list_apps()
    }

    pub fn open_external(&self, url: &str) {
        self.app_launcher.launch_url(url);
    }

    pub fn launch_app(&self, exec_cmd: &str) {
        self.app_launcher.launch_app(exec_cmd);
    }

    pub fn resize_window(&self, width: u32, height: u32, mode: &str) {
        *self.current_view.lock().unwrap() = mode.to_string();
        self.window_service.resize(width, height, mode);
    }

    pub fn hide_window(&self) {
        self.window_service.hide();
    }

    pub fn minimize_window(&self) {
        self.window_service.minimize();
    }

    pub fn get_current_view(&self) -> String {
        self.current_view.lock().unwrap().clone()
    }

    pub fn set_current_view(&self, view: &str) {
        *self.current_view.lock().unwrap() = view.to_string();
    }

    pub fn handle_second_instance(&self, args: &[String]) -> bool {
        if let Some(idx) = args.iter().position(|a| a == "--trigger") {
            if idx + 1 < args.len() {
                let key = &args[idx + 1];
                let relayed = self.trigger_ipc.relay_trigger(key, crate::trigger_ipc::FLICK_TRIGGER_PORT);
                if relayed {
                    return true;
                }
            }
        } else {
            let relayed = self.trigger_ipc.relay_trigger("FOCUS", crate::trigger_ipc::FLICK_TRIGGER_PORT);
            if relayed {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{QuickStartOption, QuickStartOptionData};
    use crate::hid::KeychronDevice;
    use std::collections::HashMap;
    use std::io;

    struct MockConfigRepository {
        config: Mutex<AppConfig>,
    }
    impl ConfigRepository for MockConfigRepository {
        fn load(&self) -> AppConfig {
            self.config.lock().unwrap().clone()
        }
        fn save(&self, config: &AppConfig) -> Result<(), io::Error> {
            *self.config.lock().unwrap() = config.clone();
            Ok(())
        }
    }

    struct MockGnomeShortcutsService {
        synced_keys: Mutex<Vec<String>>,
    }
    impl GnomeShortcutsService for MockGnomeShortcutsService {
        fn is_gnome(&self) -> bool {
            true
        }
        fn sync_shortcuts(&self, keys: &[String]) -> Result<(), io::Error> {
            *self.synced_keys.lock().unwrap() = keys.to_vec();
            Ok(())
        }
        fn clear_shortcuts(&self) -> Result<(), io::Error> {
            self.synced_keys.lock().unwrap().clear();
            Ok(())
        }
    }

    struct MockAppLauncher {
        launched_apps: Mutex<Vec<String>>,
        launched_urls: Mutex<Vec<String>>,
        mock_apps: Vec<AppInfo>,
    }
    impl AppLauncher for MockAppLauncher {
        fn launch_app(&self, exec_cmd: &str) {
            self.launched_apps.lock().unwrap().push(exec_cmd.to_string());
        }
        fn launch_url(&self, url: &str) {
            self.launched_urls.lock().unwrap().push(url.to_string());
        }
        fn list_apps(&self) -> Vec<AppInfo> {
            self.mock_apps.clone()
        }
    }

    struct MockAutostartManager {
        enabled: Mutex<bool>,
    }
    impl AutostartManager for MockAutostartManager {
        fn set_enabled(&self, enabled: bool) {
            *self.enabled.lock().unwrap() = enabled;
        }
        fn is_enabled(&self) -> bool {
            *self.enabled.lock().unwrap()
        }
    }

    struct MockTriggerIpcService {
        trigger_callback: Mutex<Option<Box<dyn Fn(String) + Send + Sync + 'static>>>,
        relays: Mutex<Vec<(String, u16)>>,
        relay_result: bool,
    }
    impl TriggerIpcService for MockTriggerIpcService {
        fn start_server(&self, _port: u16, on_trigger: Box<dyn Fn(String) + Send + Sync + 'static>) {
            *self.trigger_callback.lock().unwrap() = Some(on_trigger);
        }
        fn relay_trigger(&self, key: &str, port: u16) -> bool {
            self.relays.lock().unwrap().push((key.to_string(), port));
            self.relay_result
        }
    }

    struct MockWsServer {
        get_binds_cb: Mutex<Option<Box<dyn Fn() -> HashMap<String, Vec<QuickStartOption>> + Send + Sync + 'static>>>,
        add_bind_cb: Mutex<Option<Box<dyn Fn(String, QuickStartOption) + Send + Sync + 'static>>>,
        clear_bind_cb: Mutex<Option<Box<dyn Fn(Option<String>) + Send + Sync + 'static>>>,
        get_apps_cb: Mutex<Option<Box<dyn Fn() -> Vec<AppInfo> + Send + Sync + 'static>>>,
        broadcasted_binds: Mutex<Vec<HashMap<String, Vec<QuickStartOption>>>>,
    }
    impl WsServer for MockWsServer {
        fn start(
            &self,
            _port: u16,
            on_get_binds: Box<dyn Fn() -> HashMap<String, Vec<QuickStartOption>> + Send + Sync + 'static>,
            on_add_bind: Box<dyn Fn(String, QuickStartOption) + Send + Sync + 'static>,
            on_clear_bind: Box<dyn Fn(Option<String>) + Send + Sync + 'static>,
            on_get_apps: Box<dyn Fn() -> Vec<AppInfo> + Send + Sync + 'static>,
        ) {
            *self.get_binds_cb.lock().unwrap() = Some(on_get_binds);
            *self.add_bind_cb.lock().unwrap() = Some(on_add_bind);
            *self.clear_bind_cb.lock().unwrap() = Some(on_clear_bind);
            *self.get_apps_cb.lock().unwrap() = Some(on_get_apps);
        }
        fn broadcast_binds(&self, binds: &HashMap<String, Vec<QuickStartOption>>) {
            self.broadcasted_binds.lock().unwrap().push(binds.clone());
        }
    }

    struct MockHidService {
        keyboards: Vec<KeychronDevice>,
    }
    impl HidService for MockHidService {
        fn list_devices(&self) -> Vec<KeychronDevice> {
            self.keyboards.clone()
        }
    }

    struct MockWindowService {
        shows: Mutex<usize>,
        hides: Mutex<usize>,
        minimizes: Mutex<usize>,
        resizes: Mutex<Vec<(u32, u32, String)>>,
        tray_updates: Mutex<Vec<bool>>,
    }
    impl WindowService for MockWindowService {
        fn show_and_focus(&self) {
            *self.shows.lock().unwrap() += 1;
        }
        fn hide(&self) {
            *self.hides.lock().unwrap() += 1;
        }
        fn minimize(&self) {
            *self.minimizes.lock().unwrap() += 1;
        }
        fn resize(&self, width: u32, height: u32, mode: &str) {
            self.resizes.lock().unwrap().push((width, height, mode.to_string()));
        }
        fn emit_navigate(&self, _view: &str) {}
        fn update_tray_menu(&self, autostart: bool) {
            self.tray_updates.lock().unwrap().push(autostart);
        }
    }

    fn setup_core() -> (Arc<FlickCore>, Arc<MockConfigRepository>, Arc<MockGnomeShortcutsService>, Arc<MockAppLauncher>, Arc<MockAutostartManager>, Arc<MockTriggerIpcService>, Arc<MockWsServer>, Arc<MockHidService>, Arc<MockWindowService>) {
        let repo = Arc::new(MockConfigRepository { config: Mutex::new(AppConfig::default()) });
        let gnome = Arc::new(MockGnomeShortcutsService { synced_keys: Mutex::new(Vec::new()) });
        let launcher = Arc::new(MockAppLauncher { launched_apps: Mutex::new(Vec::new()), launched_urls: Mutex::new(Vec::new()), mock_apps: vec![AppInfo { id: "a.desktop".to_string(), name: "A".to_string(), exec: "a".to_string(), icon: "icon".to_string() }] });
        let auto = Arc::new(MockAutostartManager { enabled: Mutex::new(false) });
        let trigger = Arc::new(MockTriggerIpcService { trigger_callback: Mutex::new(None), relays: Mutex::new(Vec::new()), relay_result: true });
        let ws = Arc::new(MockWsServer { get_binds_cb: Mutex::new(None), add_bind_cb: Mutex::new(None), clear_bind_cb: Mutex::new(None), get_apps_cb: Mutex::new(None), broadcasted_binds: Mutex::new(Vec::new()) });
        let hid = Arc::new(MockHidService { keyboards: vec![KeychronDevice { vp_id: 12345, vendor_id: 0x3434, product_id: 0x01, name: "Keychron".to_string() }] });
        let win = Arc::new(MockWindowService { shows: Mutex::new(0), hides: Mutex::new(0), minimizes: Mutex::new(0), resizes: Mutex::new(Vec::new()), tray_updates: Mutex::new(Vec::new()) });

        let core = Arc::new(FlickCore::new(
            repo.clone(),
            gnome.clone(),
            launcher.clone(),
            auto.clone(),
            trigger.clone(),
            ws.clone(),
            hid.clone(),
            win.clone(),
        ));

        (core, repo, gnome, launcher, auto, trigger, ws, hid, win)
    }

    #[tokio::test]
    async fn test_init_app() {
        let (core, repo, gnome, _launcher, _auto, trigger, ws, _hid, _win) = setup_core();

        // Put a bind in repo
        {
            let mut conf = repo.config.lock().unwrap();
            conf.quick_start_binds.insert("F13".to_string(), vec![]);
        }

        core.init_app();

        // 1. GNOME shortcuts should be synced with ["F13"]
        assert_eq!(*gnome.synced_keys.lock().unwrap(), vec!["F13".to_string()]);

        // 2. Trigger server callback should be set
        assert!(trigger.trigger_callback.lock().unwrap().is_some());

        // 3. WS server callbacks should be set
        assert!(ws.get_binds_cb.lock().unwrap().is_some());
        assert!(ws.add_bind_cb.lock().unwrap().is_some());
        assert!(ws.clear_bind_cb.lock().unwrap().is_some());
        assert!(ws.get_apps_cb.lock().unwrap().is_some());
    }

    #[tokio::test]
    async fn test_ws_callbacks() {
        let (core, repo, _gnome, _launcher, _auto, _trigger, ws, _hid, _win) = setup_core();
        core.init_app();

        // Trigger on_get_binds callback
        let get_binds = ws.get_binds_cb.lock().unwrap().as_ref().unwrap().as_ref()();
        assert!(get_binds.is_empty());

        // Trigger on_add_bind callback
        let bind = QuickStartOption {
            opt_type: "App".to_string(),
            opt_data: QuickStartOptionData {
                name: "A".to_string(),
                path: "a".to_string(),
                icon: "icon".to_string(),
                icon_path: "".to_string(),
            },
        };
        ws.add_bind_cb.lock().unwrap().as_ref().unwrap().as_ref()("F13".to_string(), bind.clone());
        let after_add = repo.load();
        assert_eq!(after_add.quick_start_binds.get("F13").unwrap()[0], bind);

        // Trigger on_get_apps
        let apps = ws.get_apps_cb.lock().unwrap().as_ref().unwrap().as_ref()();
        assert_eq!(apps[0].id, "a.desktop");

        // Trigger on_clear_bind with specific key
        ws.clear_bind_cb.lock().unwrap().as_ref().unwrap().as_ref()(Some("F13".to_string()));
        let after_clear_key = repo.load();
        assert!(after_clear_key.quick_start_binds.is_empty());

        // Setup a bind and trigger on_clear_bind with None (all)
        ws.add_bind_cb.lock().unwrap().as_ref().unwrap().as_ref()("F14".to_string(), bind.clone());
        ws.clear_bind_cb.lock().unwrap().as_ref().unwrap().as_ref()(None);
        let after_clear_all = repo.load();
        assert!(after_clear_all.quick_start_binds.is_empty());
    }

    #[tokio::test]
    async fn test_update_config() {
        let (core, repo, gnome, _launcher, auto, _trigger, ws, _hid, win) = setup_core();

        // 1. Simple update of theme
        let updated = core.update_config(serde_json::json!({
            "theme": "dark"
        }));
        assert_eq!(updated.theme, "dark");
        assert_eq!(repo.load().theme, "dark");
        assert_eq!(win.tray_updates.lock().unwrap().len(), 1);

        // 2. Update autostart
        let updated = core.update_config(serde_json::json!({
            "autostart": true
        }));
        assert_eq!(updated.autostart, true);
        assert_eq!(*auto.enabled.lock().unwrap(), true);

        // 3. Update quickStartBinds
        let bind = QuickStartOption {
            opt_type: "Url".to_string(),
            opt_data: QuickStartOptionData {
                name: "Web".to_string(),
                path: "https://localhost".to_string(),
                icon: "url".to_string(),
                icon_path: "".to_string(),
            },
        };
        let binds_map = HashMap::from([("F15".to_string(), vec![bind])]);
        let _ = core.update_config(serde_json::json!({
            "quickStartBinds": binds_map
        }));

        assert_eq!(*gnome.synced_keys.lock().unwrap(), vec!["F15".to_string()]);
        assert_eq!(ws.broadcasted_binds.lock().unwrap().len(), 1);
        assert_eq!(ws.broadcasted_binds.lock().unwrap()[0].get("F15").unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_handle_key_trigger() {
        let (core, repo, _gnome, launcher, _auto, _trigger, _ws, _hid, win) = setup_core();

        // 1. FOCUS key
        core.handle_key_trigger("FOCUS");
        assert_eq!(*win.shows.lock().unwrap(), 1);

        // 2. Non-existent bind
        core.handle_key_trigger("F13");
        assert_eq!(launcher.launched_apps.lock().unwrap().len(), 0);

        // 3. Setup binds
        let app_bind = QuickStartOption {
            opt_type: "App".to_string(),
            opt_data: QuickStartOptionData {
                name: "AppA".to_string(),
                path: "appa".to_string(),
                icon: "".to_string(),
                icon_path: "".to_string(),
            },
        };
        let url_bind = QuickStartOption {
            opt_type: "Url".to_string(),
            opt_data: QuickStartOptionData {
                name: "UrlA".to_string(),
                path: "urla".to_string(),
                icon: "".to_string(),
                icon_path: "".to_string(),
            },
        };
        {
            let mut conf = repo.config.lock().unwrap();
            conf.quick_start_binds.insert("F14".to_string(), vec![app_bind, url_bind]);
        }

        // Trigger F14
        core.handle_key_trigger("F14");
        // Give tokio tasks a moment to run
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert_eq!(launcher.launched_apps.lock().unwrap()[0], "appa");
        assert_eq!(launcher.launched_urls.lock().unwrap()[0], "urla");

        // 4. Debounce: Trigger F14 again immediately -> should be ignored
        core.handle_key_trigger("F14");
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(launcher.launched_apps.lock().unwrap().len(), 1); // no new launch

        // 5. Sleep 450ms and trigger again -> should trigger
        tokio::time::sleep(Duration::from_millis(450)).await;
        core.handle_key_trigger("F14");
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(launcher.launched_apps.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_scan_hid() {
        let (core, _, _, _, _, _, _, _, _) = setup_core();
        let scan_res = core.scan_hid();
        assert_eq!(scan_res.get("connected").unwrap().as_bool().unwrap(), true);
        assert_eq!(scan_res.get("keyboards").unwrap().as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_other_methods() {
        let (core, _, _, launcher, _, _, _, _, win) = setup_core();

        // list_apps
        let apps = core.list_apps();
        assert_eq!(apps.len(), 1);

        // open_external
        core.open_external("http://google.com");
        assert_eq!(launcher.launched_urls.lock().unwrap()[0], "http://google.com");

        // launch_app
        core.launch_app("myapp");
        assert_eq!(launcher.launched_apps.lock().unwrap()[0], "myapp");

        // resize_window
        core.resize_window(800, 600, "launcher");
        assert_eq!(core.get_current_view(), "launcher");
        assert_eq!(win.resizes.lock().unwrap()[0], (800, 600, "launcher".to_string()));

        // set_current_view
        core.set_current_view("settings");
        assert_eq!(core.get_current_view(), "settings");

        // hide_window
        core.hide_window();
        assert_eq!(*win.hides.lock().unwrap(), 1);

        // minimize_window
        core.minimize_window();
        assert_eq!(*win.minimizes.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn test_handle_second_instance() {
        // Test case where relay succeeds
        {
            let (core, _, _, _, _, trigger, _, _, _) = setup_core();
            // trigger has relay_result: true by default
            let relayed = core.handle_second_instance(&["--trigger".to_string(), "F16".to_string()]);
            assert!(relayed);
            assert_eq!(trigger.relays.lock().unwrap()[0], ("F16".to_string(), crate::trigger_ipc::FLICK_TRIGGER_PORT));
        }

        // Test case where relay fails
        {
            let repo = Arc::new(MockConfigRepository { config: Mutex::new(AppConfig::default()) });
            let gnome = Arc::new(MockGnomeShortcutsService { synced_keys: Mutex::new(Vec::new()) });
            let launcher = Arc::new(MockAppLauncher { launched_apps: Mutex::new(Vec::new()), launched_urls: Mutex::new(Vec::new()), mock_apps: vec![] });
            let auto = Arc::new(MockAutostartManager { enabled: Mutex::new(false) });
            let trigger = Arc::new(MockTriggerIpcService { trigger_callback: Mutex::new(None), relays: Mutex::new(Vec::new()), relay_result: false });
            let ws = Arc::new(MockWsServer { get_binds_cb: Mutex::new(None), add_bind_cb: Mutex::new(None), clear_bind_cb: Mutex::new(None), get_apps_cb: Mutex::new(None), broadcasted_binds: Mutex::new(Vec::new()) });
            let hid = Arc::new(MockHidService { keyboards: vec![] });
            let win = Arc::new(MockWindowService { shows: Mutex::new(0), hides: Mutex::new(0), minimizes: Mutex::new(0), resizes: Mutex::new(Vec::new()), tray_updates: Mutex::new(Vec::new()) });

            let core = FlickCore::new(repo, gnome, launcher, auto, trigger, ws, hid, win);
            let relayed = core.handle_second_instance(&["--trigger".to_string(), "F16".to_string()]);
            assert!(!relayed);
        }

        // Test case with FOCUS (no --trigger arg)
        {
            let (core, _, _, _, _, trigger, _, _, _) = setup_core();
            let relayed = core.handle_second_instance(&[]);
            assert!(relayed);
            assert_eq!(trigger.relays.lock().unwrap()[0], ("FOCUS".to_string(), crate::trigger_ipc::FLICK_TRIGGER_PORT));
        }
    }
}
