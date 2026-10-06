pub mod collector;
pub mod commands;
pub mod config;
pub mod control;
pub mod gpu;
pub mod hooks;
pub mod ollama;
pub mod poller;
pub mod procs;
pub mod snapshot;
pub mod state;
pub mod tray;

use tauri::WindowEvent;

pub fn run() {
    let config_path = config::config_path();
    let loaded = config::load_or_create(&config_path).unwrap_or_else(|reason| {
        eprintln!("corral: usando config por defecto: {reason}");
        config::Config::default()
    });
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| tray::show_panel(app)))
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--hidden"])))
        .manage(state::AppState::new(config_path, loaded))
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::list_models,
            commands::pause_ollama,
            commands::resume_ollama,
            commands::unload_model,
            commands::delete_model,
            commands::copy_model,
            commands::pull_model,
            commands::get_config,
            commands::save_config,
        ])
        .setup(|app| {
            tray::build(app.handle())?;
            if !std::env::args().any(|arg| arg == "--hidden") {
                tray::show_panel(app.handle());
            }
            poller::spawn(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error al iniciar Corral");
}
