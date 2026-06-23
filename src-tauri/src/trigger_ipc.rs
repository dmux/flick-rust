use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::time::Duration;
use std::sync::Arc;

pub const FLICK_TRIGGER_PORT: u16 = 40104;

pub async fn start_trigger_server<F>(on_trigger: F)
where
    F: Fn(String) + Send + Sync + 'static,
{
    start_trigger_server_on_port(on_trigger, FLICK_TRIGGER_PORT).await;
}

pub async fn start_trigger_server_on_port<F>(on_trigger: F, port: u16)
where
    F: Fn(String) + Send + Sync + 'static,
{
    let listener = match TcpListener::bind(("127.0.0.1", port)).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[trigger-ipc] Failed to bind to 127.0.0.1:{}: {}", port, e);
            return;
        }
    };
    println!("[trigger-ipc] listening on 127.0.0.1:{}", port);

    let on_trigger = Arc::new(on_trigger);

    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((mut socket, _)) => {
                    let on_trigger = on_trigger.clone();
                    tokio::spawn(async move {
                        let (reader, mut writer) = socket.split();
                        let mut reader = BufReader::new(reader);
                        let mut line = String::new();
                        while let Ok(n) = reader.read_line(&mut line).await {
                            if n == 0 {
                                break;
                            }
                            let trimmed = line.trim();
                            if trimmed.starts_with("TRIGGER ") {
                                let key = trimmed["TRIGGER ".len()..].trim().to_string();
                                if !key.is_empty() {
                                    on_trigger(key);
                                    let _ = writer.write_all(b"OK\n").await;
                                }
                            }
                            line.clear();
                        }
                    });
                }
                Err(e) => {
                    eprintln!("[trigger-ipc] accept error: {}", e);
                }
            }
        }
    });
}

pub async fn relay_trigger(key: &str) -> bool {
    relay_trigger_on_port(key, FLICK_TRIGGER_PORT).await
}

pub async fn relay_trigger_on_port(key: &str, port: u16) -> bool {
    let addr = format!("127.0.0.1:{}", port);
    let stream = tokio::time::timeout(
        Duration::from_millis(600),
        TcpStream::connect(&addr)
    ).await;

    let mut stream = match stream {
        Ok(Ok(s)) => s,
        _ => return false, // Connection refused or timed out
    };

    let msg = format!("TRIGGER {}\n", key);
    if stream.write_all(msg.as_bytes()).await.is_err() {
        return false;
    }

    let (reader, _) = stream.split();
    let mut reader = BufReader::new(reader);
    let mut response = String::new();

    let read_result = tokio::time::timeout(
        Duration::from_millis(600),
        reader.read_line(&mut response)
    ).await;

    match read_result {
        Ok(Ok(_)) => response.contains("OK"),
        _ => false,
    }
}

pub struct TcpTriggerIpcService;

impl crate::ports::TriggerIpcService for TcpTriggerIpcService {
    fn start_server(&self, port: u16, on_trigger: Box<dyn Fn(String) + Send + Sync + 'static>) {
        // Use Tauri's managed async runtime instead of `Handle::current()`. This is
        // called from Tauri's `.setup()` closure, which does NOT run inside a Tokio
        // runtime context, so `Handle::current()` would panic ("there is no reactor
        // running"). `tauri::async_runtime::spawn` works from any context.
        tauri::async_runtime::spawn(async move {
            start_trigger_server_on_port(move |key| {
                on_trigger(key);
            }, port).await;
        });
    }

    fn relay_trigger(&self, key: &str, port: u16) -> bool {
        let key_str = key.to_string();
        let handle = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(relay_trigger_on_port(&key_str, port))
        });
        handle.join().unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[tokio::test]
    async fn test_trigger_ipc_server_and_relay() {
        let test_port: u16 = 45999;
        let received_key = Arc::new(Mutex::new(String::new()));
        let received_key_clone = received_key.clone();

        // Start the test server on a custom test port
        start_trigger_server_on_port(move |key| {
            let mut lock = received_key_clone.lock().unwrap();
            *lock = key;
        }, test_port).await;

        // Give server a tiny moment to start listening
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Relay a trigger key
        let success = relay_trigger_on_port("F22", test_port).await;
        assert!(success);

        // Give the callback time to run
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Assert the key was received
        let final_key = received_key.lock().unwrap().clone();
        assert_eq!(final_key, "F22");

        // Attempting to relay on a closed port should return false
        let bad_success = relay_trigger_on_port("F22", 45998).await;
        assert!(!bad_success);
    }
}
