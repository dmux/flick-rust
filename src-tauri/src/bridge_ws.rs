use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::accept_async;
use futures_util::{StreamExt, SinkExt};
use serde::{Serialize, Deserialize};
use serde_json::Value;

use crate::config::{get_config, update_config, QuickStartOption};
use crate::apps::list_installed_apps_with_icons;

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

pub fn broadcast_key_binds() {
    let clients_guard = get_clients().lock().unwrap();
    if clients_guard.is_empty() {
        return;
    }
    let config = get_config();
    let payload = OutMessage {
        evt_type: "QuickStart".to_string(),
        evt_data: OutData {
            cmd: "GetKeyBinds".to_string(),
            data: serde_json::to_value(vec![config.quick_start_binds]).unwrap_or(Value::Null),
        },
    };
    if let Ok(msg_str) = serde_json::to_string(&payload) {
        let msg = Message::Text(msg_str.into());
        for tx in clients_guard.iter() {
            let _ = tx.send(msg.clone());
        }
    }
}

pub async fn start_server<F>(port: u16, on_key_binds_changed: F)
where
    F: Fn() + Send + Sync + Clone + 'static,
{
    let addr = format!("127.0.0.1:{}", port);
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[WS] Failed to bind to WS port {}: {}", port, e);
            return;
        }
    };
    println!("[WS] Keychron-compatible WebSocket server listening on port {}", port);

    let on_key_binds_changed = Arc::new(on_key_binds_changed);

    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let on_key_binds_changed = on_key_binds_changed.clone();
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
                            handle_message(parsed, &on_key_binds_changed).await;
                        }
                    }
                }

                println!("[WS] Client disconnected");
                write_task.abort();
                // Clean up sender
                // Note: ideally we filter out dead senders, but since it's a simple app we can clear on next broadcast or keep clean.
            });
        }
    });
}

async fn handle_message<F>(msg: EvtMessage, on_key_binds_changed: &Arc<F>)
where
    F: Fn() + Send + Sync + 'static,
{
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
            handle_quick_start(&cmd, data, on_key_binds_changed).await;
        }
        _ => {
            send_response(&evt_type, &cmd, Value::String("".to_string()));
        }
    }
}

async fn handle_quick_start<F>(cmd: &str, data: Value, on_key_binds_changed: &Arc<F>)
where
    F: Fn() + Send + Sync + 'static,
{
    if let Some(s) = data.as_str() {
        if s == "Success" || s == "KeyDuplicate" || s == "Fail" {
            println!("[WS] ignoring QuickStart {} status acknowledgment: {}", cmd, s);
            return;
        }
    }

    match cmd {
        "Get" => {
            let apps = tokio::task::spawn_blocking(list_installed_apps_with_icons).await.unwrap_or_default();
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
            let config = get_config();
            send_response("QuickStart", "GetKeyBinds", serde_json::to_value(vec![config.quick_start_binds]).unwrap());
        }
        "AddKeyBind" => {
            // Try to parse binding from payload
            let payload: Result<AddBindPayload, _> = if data.is_string() {
                serde_json::from_str(data.as_str().unwrap())
            } else {
                serde_json::from_value(data)
            };

            if let Ok(p) = payload {
                let config = get_config();
                let existing = config.quick_start_binds.get(&p.key).and_then(|v| v.first());
                let duplicate = existing.map_or(false, |e| {
                    e.opt_type == p.bind.opt_type && e.opt_data.path == p.bind.opt_data.path
                });

                if duplicate {
                    println!("[WS] bind for {} is already up to date", p.key);
                } else {
                    let mut binds = config.quick_start_binds.clone();
                    binds.insert(p.key.clone(), vec![p.bind.clone()]);
                    update_config(serde_json::json!({ "quickStartBinds": binds }));
                    println!("[WS] saved bind {} -> {}", p.key, p.bind.opt_type);
                    on_key_binds_changed();
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
                let config = get_config();
                let mut binds = config.quick_start_binds.clone();
                binds.remove(&payload_str);
                update_config(serde_json::json!({ "quickStartBinds": binds }));
                println!("[WS] cleared bind for key: {}", payload_str);
            } else {
                update_config(serde_json::json!({ "quickStartBinds": {} }));
                println!("[WS] cleared all binds");
            }
            on_key_binds_changed();
            send_response("QuickStart", "ClearKeyBind", Value::String("Success".to_string()));
        }
        "RemoveKeyBind" | "DelKeyBind" | "DeleteKeyBind" => {
            let payload: Result<AddBindPayload, _> = if data.is_string() {
                serde_json::from_str(data.as_str().unwrap())
            } else {
                serde_json::from_value(data)
            };

            if let Ok(p) = payload {
                let config = get_config();
                let mut binds = config.quick_start_binds.clone();
                binds.remove(&p.key);
                update_config(serde_json::json!({ "quickStartBinds": binds }));
                println!("[WS] cleared bind for key: {}", p.key);
                on_key_binds_changed();
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
