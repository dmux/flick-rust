use std::sync::{Arc, Mutex, RwLock};
use std::collections::HashMap;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::accept_async;
use futures_util::{StreamExt, SinkExt};
use serde::{Serialize, Deserialize};
use serde_json::Value;

use crate::config::QuickStartOption;
use crate::apps::AppInfo;

const ASSIST_VERSION: &str = "1.1.1";

#[derive(Serialize, Deserialize, Debug)]
struct FeatureInfo {
    name: String,
    version: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct VersionInfo {
    version: String,
    feature: Vec<FeatureInfo>,
}

#[derive(Deserialize, Debug)]
struct EvtMessage {
    evt_type: Option<String>,
    evt_data: Option<EvtData>,
}

#[derive(Deserialize, Serialize, Debug)]
struct EvtData {
    cmd: Option<String>,
    data: Option<Value>,
}

#[derive(Serialize, Debug)]
struct OutMessage {
    evt_type: String,
    evt_data: OutData,
}

#[derive(Serialize, Debug)]
struct OutData {
    cmd: String,
    data: Value,
}

#[derive(Deserialize, Debug)]
struct AddBindPayload {
    key: String,
    bind: QuickStartOption,
}

type ClientSender = tokio::sync::mpsc::UnboundedSender<Message>;
type Clients = Arc<Mutex<Vec<ClientSender>>>;

static CLIENTS: std::sync::OnceLock<Clients> = std::sync::OnceLock::new();

fn get_clients() -> &'static Clients {
    CLIENTS.get_or_init(|| Arc::new(Mutex::new(Vec::new())))
}

pub fn broadcast_key_binds_adapter(binds: &HashMap<String, Vec<QuickStartOption>>) {
    let clients_guard = get_clients().lock().unwrap();
    if clients_guard.is_empty() {
        return;
    }
    let payload = OutMessage {
        evt_type: "QuickStart".to_string(),
        evt_data: OutData {
            cmd: "GetKeyBinds".to_string(),
            data: serde_json::to_value(vec![binds]).unwrap_or(Value::Null),
        },
    };
    if let Ok(msg_str) = serde_json::to_string(&payload) {
        let msg = Message::Text(msg_str.into());
        for tx in clients_guard.iter() {
            let _ = tx.send(msg.clone());
        }
    }
}

struct WsCallbacks {
    on_get_binds: Box<dyn Fn() -> HashMap<String, Vec<QuickStartOption>> + Send + Sync + 'static>,
    on_add_bind: Box<dyn Fn(String, QuickStartOption) + Send + Sync + 'static>,
    on_clear_bind: Box<dyn Fn(Option<String>) + Send + Sync + 'static>,
    on_get_apps: Box<dyn Fn() -> Vec<AppInfo> + Send + Sync + 'static>,
}

type SharedCallbacks = Arc<RwLock<Option<Arc<WsCallbacks>>>>;

pub struct TungsteniteWsServer {
    callbacks: SharedCallbacks,
}

impl TungsteniteWsServer {
    pub fn new() -> Self {
        Self {
            callbacks: Arc::new(RwLock::new(None)),
        }
    }
}

impl crate::ports::WsServer for TungsteniteWsServer {
    fn start(
        &self,
        port: u16,
        on_get_binds: Box<dyn Fn() -> HashMap<String, Vec<QuickStartOption>> + Send + Sync + 'static>,
        on_add_bind: Box<dyn Fn(String, QuickStartOption) + Send + Sync + 'static>,
        on_clear_bind: Box<dyn Fn(Option<String>) + Send + Sync + 'static>,
        on_get_apps: Box<dyn Fn() -> Vec<AppInfo> + Send + Sync + 'static>,
    ) {
        let callbacks_arc = Arc::new(WsCallbacks {
            on_get_binds,
            on_add_bind,
            on_clear_bind,
            on_get_apps,
        });
        *self.callbacks.write().unwrap() = Some(callbacks_arc);

        let callbacks = self.callbacks.clone();
        // Use Tauri's managed async runtime instead of `Handle::current()`. This is
        // called during app setup, outside of any Tokio runtime context, where
        // `Handle::current()` would panic ("there is no reactor running").
        tauri::async_runtime::spawn(async move {
            start_server_internal(port, callbacks).await;
        });
    }

    fn broadcast_binds(&self, binds: &HashMap<String, Vec<QuickStartOption>>) {
        broadcast_key_binds_adapter(binds);
    }
}

fn get_callbacks(shared: &SharedCallbacks) -> Option<Arc<WsCallbacks>> {
    shared.read().unwrap().clone()
}

async fn start_server_internal(port: u16, callbacks: SharedCallbacks) {
    let addr = format!("127.0.0.1:{}", port);
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[WS] Failed to bind to WS port {}: {}", port, e);
            return;
        }
    };
    println!("[WS] Keychron-compatible WebSocket server listening on port {}", port);

    while let Ok((stream, _)) = listener.accept().await {
        let callbacks_clone = callbacks.clone();
        tokio::spawn(async move {
            let ws_stream = match accept_async(stream).await {
                Ok(ws) => ws,
                Err(e) => {
                    eprintln!("[WS] Error during WebSocket handshake: {}", e);
                    return;
                }
            };
            println!("[WS] Client connected");

            let (mut ws_write, mut ws_read) = ws_stream.split();
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

            get_clients().lock().unwrap().push(tx);

            // Write worker
            let write_task = tokio::spawn(async move {
                while let Some(msg) = rx.recv().await {
                    if ws_write.send(msg).await.is_err() {
                        break;
                    }
                }
            });

            // Read worker
            while let Some(Ok(msg)) = ws_read.next().await {
                if let Message::Text(text) = msg {
                    println!("[WS ↓ launcher] {}", text);
                    if let Ok(parsed) = serde_json::from_str::<EvtMessage>(&text) {
                        handle_message(parsed, &callbacks_clone).await;
                    }
                }
            }

            println!("[WS] Client disconnected");
            write_task.abort();
        });
    }
}

async fn handle_message(msg: EvtMessage, callbacks: &SharedCallbacks) {
    let cb = match get_callbacks(callbacks) {
        Some(c) => c,
        None => return,
    };

    let evt_type = msg.evt_type.unwrap_or_default();
    let cmd = msg.evt_data.as_ref().and_then(|d| d.cmd.clone()).unwrap_or_default();
    let data = msg.evt_data.as_ref().and_then(|d| d.data.clone()).unwrap_or(Value::Null);

    match evt_type.as_str() {
        "Common" => {
            if cmd == "GetVersion" {
                let info = VersionInfo {
                    version: ASSIST_VERSION.to_string(),
                    feature: vec![FeatureInfo {
                        name: "quickStart".to_string(),
                        version: "1".to_string(),
                    }],
                };
                send_response("Common", &cmd, serde_json::to_value(info).unwrap());
            } else {
                send_response("Common", &cmd, Value::String("".to_string()));
            }
        }
        "QuickStart" => {
            handle_quick_start(&cmd, data, &cb).await;
        }
        _ => {
            send_response(&evt_type, &cmd, Value::String("".to_string()));
        }
    }
}

async fn handle_quick_start(cmd: &str, data: Value, cb: &Arc<WsCallbacks>) {
    if let Some(s) = data.as_str() {
        if s == "Success" || s == "KeyDuplicate" || s == "Fail" {
            println!("[WS] ignoring QuickStart {} status acknowledgment: {}", cmd, s);
            return;
        }
    }

    match cmd {
        "Get" => {
            // run on_get_apps under spawn_blocking
            let cb_clone = cb.clone();
            let apps = tokio::task::spawn_blocking(move || {
                cb_clone.on_get_apps.as_ref()()
            }).await.unwrap_or_default();

            println!("[WS] streaming app basket: {} apps", apps.len());

            send_response("QuickStart", "Get", serde_json::json!({
                "action": "FetchStart",
                "count": apps.len(),
                "data": Value::Null
            }));

            for (i, app) in apps.iter().enumerate() {
                send_response("QuickStart", "Get", serde_json::json!({
                    "action": "Fetch",
                    "count": i,
                    "data": {
                        "name": app.name,
                        "path": app.exec,
                        "icon": app.icon,
                        "icon_path": ""
                    }
                }));
            }

            send_response("QuickStart", "Get", serde_json::json!({
                "action": "FetchEnd",
                "count": apps.len(),
                "data": Value::Null
            }));
        }
        "GetKeyBinds" => {
            let binds = cb.on_get_binds.as_ref()();
            send_response("QuickStart", "GetKeyBinds", serde_json::to_value(vec![binds]).unwrap());
        }
        "AddKeyBind" => {
            let payload: Result<AddBindPayload, _> = if data.is_string() {
                serde_json::from_str(data.as_str().unwrap())
            } else {
                serde_json::from_value(data)
            };

            if let Ok(p) = payload {
                let binds = cb.on_get_binds.as_ref()();
                let existing = binds.get(&p.key).and_then(|v| v.first());
                let duplicate = existing.map_or(false, |e| {
                    e.opt_type == p.bind.opt_type && e.opt_data.path == p.bind.opt_data.path
                });

                if duplicate {
                    println!("[WS] bind for {} is already up to date", p.key);
                } else {
                    cb.on_add_bind.as_ref()(p.key.clone(), p.bind.clone());
                    println!("[WS] saved bind {} -> {}", p.key, p.bind.opt_type);
                }
            }

            send_response("QuickStart", "AddKeyBind", Value::String("Success".to_string()));
        }
        "ClearKeyBind" => {
            let payload_str = if data.is_string() {
                data.as_str().unwrap().to_string()
            } else {
                data.to_string()
            };

            if payload_str.to_uppercase().starts_with('F') && payload_str.len() > 1 && payload_str[1..].parse::<u32>().is_ok() {
                cb.on_clear_bind.as_ref()(Some(payload_str.clone()));
                println!("[WS] cleared bind for key: {}", payload_str);
            } else {
                cb.on_clear_bind.as_ref()(None);
                println!("[WS] cleared all binds");
            }
            send_response("QuickStart", "ClearKeyBind", Value::String("Success".to_string()));
        }
        "RemoveKeyBind" | "DelKeyBind" | "DeleteKeyBind" => {
            let payload: Result<AddBindPayload, _> = if data.is_string() {
                serde_json::from_str(data.as_str().unwrap())
            } else {
                serde_json::from_value(data)
            };

            if let Ok(p) = payload {
                cb.on_clear_bind.as_ref()(Some(p.key.clone()));
                println!("[WS] cleared bind for key: {}", p.key);
            }
            send_response("QuickStart", cmd, Value::String("Success".to_string()));
        }
        _ => {
            send_response("QuickStart", cmd, Value::String("".to_string()));
        }
    }
}

fn send_response(evt_type: &str, cmd: &str, data: Value) {
    let payload = OutMessage {
        evt_type: evt_type.to_string(),
        evt_data: OutData {
            cmd: cmd.to_string(),
            data,
        },
    };
    if let Ok(msg_str) = serde_json::to_string(&payload) {
        let clients_guard = get_clients().lock().unwrap();
        let msg = Message::Text(msg_str.into());
        for tx in clients_guard.iter() {
            let _ = tx.send(msg.clone());
        }
    }
}
