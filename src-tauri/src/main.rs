// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    // 1. Check if we are running a trigger CLI command
    if let Some(idx) = args.iter().position(|a| a == "--trigger") {
        if idx + 1 < args.len() {
            let key = &args[idx + 1];
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let relayed = rt.block_on(tauri_app_lib::trigger_ipc::relay_trigger(key));
            if relayed {
                println!("[trigger] relayed {} to running instance — exiting", key);
                std::process::exit(0);
            } else {
                println!("[trigger] no running instance for {} — cold starting", key);
            }
        }
    } else {
        // 2. Not a trigger command. Check if another instance is already running.
        // If so, relay a "FOCUS" command to focus its window and exit this process.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let relayed = rt.block_on(tauri_app_lib::trigger_ipc::relay_trigger("FOCUS"));
        if relayed {
            println!("[main] another instance is already running. Relayed FOCUS command — exiting");
            std::process::exit(0);
        }
    }

    tauri_app_lib::run();
}
