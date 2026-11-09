// No console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use osctool_core::{Config, Engine, Snapshot};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{Emitter, Manager, RunEvent, State};
use tokio::sync::broadcast::error::RecvError;

struct AppState {
    engine: Arc<Engine>,
    config_path: PathBuf,
}

#[tauri::command]
fn get_config(state: State<'_, AppState>) -> Config {
    state.engine.config()
}

#[tauri::command]
fn set_config(state: State<'_, AppState>, config: Config) -> Result<Config, String> {
    let applied = state.engine.set_config(config);
    applied
        .save(&state.config_path)
        .map_err(|e| format!("{}: {e}", state.config_path.display()))?;
    Ok(applied)
}

#[tauri::command]
fn get_snapshot(state: State<'_, AppState>) -> Snapshot {
    state.engine.snapshot()
}

#[tauri::command]
fn send_message(state: State<'_, AppState>, text: String) {
    state.engine.send_message(&text);
}

#[tauri::command]
fn set_typing(state: State<'_, AppState>, typing: bool) {
    state.engine.set_typing(typing);
}

fn main() {
    let app = tauri::Builder::default()
        // A second instance would fight over the chatbox and the watch port.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            let config_path = app.path().app_config_dir()?.join("config.json");
            let config = Config::load(&config_path);
            let engine = Arc::new(tauri::async_runtime::block_on(Engine::start(config))?);

            let mut snapshots = engine.subscribe();
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    match snapshots.recv().await {
                        Ok(snapshot) => {
                            let _ = handle.emit("snapshot", snapshot);
                        }
                        Err(RecvError::Lagged(_)) => continue,
                        Err(RecvError::Closed) => break,
                    }
                }
            });

            app.manage(AppState { engine, config_path });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            set_config,
            get_snapshot,
            send_message,
            set_typing
        ])
        .build(tauri::generate_context!())
        .expect("failed to start OSC Tool VRC");

    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            // Leave VRChat clean: empty chatbox, heart rate flags off.
            if let Some(state) = handle.try_state::<AppState>() {
                tauri::async_runtime::block_on(state.engine.shutdown());
            }
        }
    });
}
