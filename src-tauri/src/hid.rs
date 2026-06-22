use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::collections::{HashMap, HashSet};
use serde::Serialize;
use hidapi::HidApi;

pub const KEYCHRON_VENDOR_ID: u16 = 0x3434;
const HID_USAGE_F13: u8 = 0x68;
const HID_USAGE_F24: u8 = 0x73;

#[derive(Debug, Clone, Serialize)]
pub struct KeychronDevice {
    #[serde(rename = "vpId")]
    pub vp_id: u32,
    pub vendor_id: u16,
    pub product_id: u16,
    pub name: String,
}

fn usage_to_fkey(usage: u8) -> Option<String> {
    if usage < HID_USAGE_F13 || usage > HID_USAGE_F24 {
        return None;
    }
    Some(format!("F{}", 13 + (usage - HID_USAGE_F13)))
}

#[derive(Debug, Clone)]
pub enum HidEvent {
    QuickstartKey { f_key: String },
    AssistKey { key_id: u8, data: Vec<u8> },
}

pub struct HidManager {
    running: Arc<AtomicBool>,
    threads: Mutex<Vec<thread::JoinHandle<()>>>,
    sender: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedSender<HidEvent>>>>,
}

impl HidManager {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            threads: Mutex::new(Vec::new()),
            sender: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_sender(&self, sender: tokio::sync::mpsc::UnboundedSender<HidEvent>) {
        *self.sender.lock().unwrap() = Some(sender);
    }

    pub fn start(&self) {
        // Under non-Linux platforms, Electron relies on globalShortcut instead of raw HID read-loops,
        // but we can compile it on Linux.
        #[cfg(not(target_os = "linux"))]
        {
            println!("HID read-loop not active on this platform.");
            return;
        }

        #[cfg(target_os = "linux")]
        {
            if self.running.load(Ordering::SeqCst) {
                return;
            }
            self.running.store(true, Ordering::SeqCst);

            let running = self.running.clone();
            let sender_cb = self.sender.clone();

            let handle = thread::spawn(move || {
                let api = match HidApi::new() {
                    Ok(a) => a,
                    Err(e) => {
                        eprintln!("[hid] failed to initialize HidApi: {}", e);
                        return;
                    }
                };

                let mut opened_paths = HashSet::new();
                let mut local_threads = Vec::new();

                // Periodic check for new devices / keepalive
                while running.load(Ordering::SeqCst) {
                    let devices = api.device_list();
                    for d in devices {
                            if d.vendor_id() != KEYCHRON_VENDOR_ID {
                                continue;
                            }
                            let path = d.path().to_owned();
                            if opened_paths.contains(&path) {
                                continue;
                            }

                            let usage_page = d.usage_page();
                            let usage = d.usage();

                            // 1. Keyboard NKRO interface
                            let is_keyboard = usage_page == 0x01 && usage == 0x06;
                            // 2. Keychron assist vendor interface
                            let is_assist = (usage_page == 0xff00 || usage_page == 65280) && (usage == 0x0d || usage == 13);

                            if is_keyboard || is_assist {
                                opened_paths.insert(path.clone());
                                let running_inner = running.clone();
                                let sender_inner = sender_cb.clone();
                                let path_str = path.to_string_lossy().to_string();

                                let t = thread::spawn(move || {
                                    let api_inner = match HidApi::new() {
                                        Ok(a) => a,
                                        Err(_) => return,
                                    };
                                    if let Ok(dev) = api_inner.open_path(&path) {
                                        println!("[hid] opened Keychron interface: {}", path_str);
                                        let mut buf = [0u8; 64];
                                        let mut pressed_keys = HashSet::new();

                                        while running_inner.load(Ordering::SeqCst) {
                                            match dev.read_timeout(&mut buf, 200) {
                                                Ok(0) => {} // timeout
                                                Ok(n) => {
                                                    let report = &buf[..n];
                                                    if is_keyboard {
                                                        let mut now_pressed = HashSet::new();
                                                        for &val in report {
                                                            if let Some(fkey) = usage_to_fkey(val) {
                                                                now_pressed.insert(fkey);
                                                            }
                                                        }
                                                        for fkey in &now_pressed {
                                                            if !pressed_keys.contains(fkey) {
                                                                println!("[hid] keyboard F-key pressed: {}", fkey);
                                                                let sender_lock = sender_inner.lock().unwrap();
                                                                if let Some(ref s) = *sender_lock {
                                                                    let _ = s.send(HidEvent::QuickstartKey { f_key: fkey.clone() });
                                                                }
                                                            }
                                                        }
                                                        pressed_keys = now_pressed;
                                                    } else if is_assist {
                                                        println!("[hid] received assist report: {:02x?}", report);
                                                        let key_id = if report.is_empty() { 1 } else { report[0] };
                                                        let key_id = if key_id == 0 { 1 } else { key_id };
                                                        let sender_lock = sender_inner.lock().unwrap();
                                                        if let Some(ref s) = *sender_lock {
                                                            let _ = s.send(HidEvent::AssistKey {
                                                                key_id,
                                                                data: report.to_vec(),
                                                            });
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    eprintln!("[hid] read error on {}: {}", path_str, e);
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                });
                                local_threads.push(t);
                        }
                    }
                    thread::sleep(std::time::Duration::from_secs(2));
                }

                for t in local_threads {
                    let _ = t.join();
                }
            });

            self.threads.lock().unwrap().push(handle);
        }
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        let mut threads = self.threads.lock().unwrap();
        for t in threads.drain(..) {
            let _ = t.join();
        }
        println!("[hid] stopped HID watcher");
    }
}

pub fn list_keychron_devices() -> Vec<KeychronDevice> {
    let api = match HidApi::new() {
        Ok(a) => a,
        Err(_) => return Vec::new(),
    };
    let mut by_vp: HashMap<u32, KeychronDevice> = HashMap::new();
    let devices = api.device_list();
    for d in devices {
            if d.vendor_id() != KEYCHRON_VENDOR_ID {
                continue;
            }
            let vp_id = ((d.vendor_id() as u32) << 16) | (d.product_id() as u32);
            let name = d.product_string().unwrap_or_default().trim().to_string();

            let existing = by_vp.get(&vp_id);
            if existing.is_none() || (!name.is_empty() && existing.unwrap().name.is_empty()) {
                let display_name = if !name.is_empty() {
                    name
                } else {
                    format!("Keychron (0x{:04x})", d.product_id())
                };
                by_vp.insert(vp_id, KeychronDevice {
                    vp_id,
                    vendor_id: d.vendor_id(),
                    product_id: d.product_id(),
                    name: display_name,
                });
        }
    }
    by_vp.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usage_to_fkey() {
        // F13 boundary
        assert_eq!(usage_to_fkey(0x68), Some("F13".to_string()));
        // F17
        assert_eq!(usage_to_fkey(0x6c), Some("F17".to_string()));
        // F24 boundary
        assert_eq!(usage_to_fkey(0x73), Some("F24".to_string()));

        // Out of bounds
        assert_eq!(usage_to_fkey(0x67), None);
        assert_eq!(usage_to_fkey(0x74), None);
        assert_eq!(usage_to_fkey(0x00), None);
    }

    #[test]
    fn test_list_keychron_devices() {
        let devices = list_keychron_devices();
        let _ = devices.len();
    }

    #[test]
    fn test_hid_manager_creation() {
        let manager = HidManager::new();
        // Just verify starting/stopping doesn't panic
        manager.start();
        manager.stop();
    }
}

