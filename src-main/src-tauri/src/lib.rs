use rdev::{listen, Event};
use serde::Serialize;
pub mod utils;
use open;
use std::sync::mpsc::{channel};
use std::{
    fs,
    path::Path,
    sync::{Arc, Mutex},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::Emitter;
use tauri::State;
use utils::macro_record;

// ---- Shared State Structure ---- //
struct AppState {
    app_config_path: String,
    selected_macro: String,
}

// ---- COMMANDS ---- //
#[tauri::command]
fn open_file_browser(path: String) {
    if let Err(err) = open::that(path) {
        eprintln!("Failed to open path: {}", err);
    }
}

#[tauri::command]
fn get_app_config_path(state: State<'_, Arc<Mutex<AppState>>>) -> String {
    let s = state.lock().unwrap();
    s.app_config_path.clone()
}

#[tauri::command]
fn select_macro(name: String, state: State<'_, Arc<Mutex<AppState>>>) -> String {
    let mut s = state.lock().unwrap();
    s.selected_macro = name.clone();
    name
}

#[derive(Serialize)]
struct FileInfo {
    file_name: String,
    file_creation_date: u64, // timestamp in seconds
    macro_duration: u64,     // macro duration in sec
    file_content: String,
}
#[tauri::command]
fn list_files_from_config(path: String) -> Vec<FileInfo> {
    let mut files_info = Vec::new();

    if let Ok(entries) = fs::read_dir(Path::new(&path)) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_file() {
                    if let Some(name) = entry.file_name().to_str() {
                        let metadata = entry.metadata().ok();
                        let creation_timestamp = metadata
                            .and_then(|m| m.created().ok())
                            .unwrap_or(SystemTime::now());
                        let timestamp = creation_timestamp
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();

                        // Load macro and estimate duration
                        let macro_events = macro_record::load_macro(&path, name);
                        let duration = macro_record::estimate_macro_time(&macro_events);
                        // Read the raw file content
                        let file_content =
                            fs::read_to_string(Path::new(&path).join(name)).unwrap_or_default();

                        files_info.push(FileInfo {
                            file_name: name.to_string(),
                            file_creation_date: timestamp,
                            macro_duration: duration,
                            file_content,
                        });
                    }
                }
            }
        }
    }

    files_info
}

#[tauri::command]
fn delete_file(path: String) -> Result<(), String> {
    let path_obj = Path::new(&path);
    if !path_obj.exists() {
        return Err(format!("File does not exist: {}", path));
    }
    fs::remove_file(path_obj).map_err(|e| format!("Failed to delete file {}: {}", path, e))
}
#[tauri::command]
fn rename_file(old_path: String, new_path: String) -> Result<(), String> {
    let old_path_obj = Path::new(&old_path);
    if !old_path_obj.exists() {
        return Err(format!("File does not exist: {}", old_path));
    }

    let new_path_obj = Path::new(&new_path);
    if new_path_obj.exists() {
        return Err(format!("Target file already exists: {}", new_path));
    }

    fs::rename(old_path_obj, new_path_obj)
        .map_err(|e| format!("Failed to rename file {} -> {}: {}", old_path, new_path, e))
}

#[tauri::command]
fn refresh_app(app_handle: tauri::AppHandle) {
    // will change a state (useState hook) on the frontend
    app_handle.emit("refresh-app", ()).unwrap();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Get or Create a path to save macro, path is different on each OS
    let app_path = macro_record::get_config_path();
    let app_path_str = app_path.to_string_lossy().to_string();

    // Initialize shared state
    let tauri_state = Arc::new(Mutex::new(AppState {
        app_config_path: app_path_str.clone(),
        selected_macro: "macro.json".into(),
    }));
    // Init macro_record state
    let mut state = macro_record::AppState::new();

    // Init refresh app callback
    // -------------------------------------------------
    // create channel
    // -------------------------------------------------
    let (tx, rx) = channel::<()>();

    // closure given to some_logic
    let refresh_callback = {
        let tx = tx.clone();
        move || {
            let _ = tx.send(()); // never panics
        }
    };

    // --- Spawn rdev listener in a separate thread --- //
    {
        let callback_state = tauri_state.clone();
        let app_path_str_clone = app_path_str.clone();
        thread::spawn(move || {
            let rdev_callback = move |event: Event| {
                let s = callback_state.lock().unwrap();

                let selected_macro = &s.selected_macro;

                state.record_macro_handler(&event);
                state.handle_key_combinaison(
                    &event,
                    &app_path_str_clone,
                    &selected_macro,
                    refresh_callback.clone(),
                );
                state.execute_macro_handler_async();
            };

            if let Err(error) = listen(rdev_callback) {
                println!("Error: {:?}", error);
            }
        });
    }

    tauri::Builder::default()
        .manage(tauri_state)
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_app_config_path,
            select_macro,
            list_files_from_config,
            delete_file,
            rename_file,
            refresh_app,
            open_file_browser
        ])
        .setup(move |app| {
            let handle = app.handle().clone();

            // spawn listener thread
            tauri::async_runtime::spawn(async move {
                for _ in rx {
                    let _ = handle.emit("refresh-app", ());
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
