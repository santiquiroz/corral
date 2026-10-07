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

fn should_keep_running(exit_code: Option<i32>) -> bool {
    exit_code.is_none()
}

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
        .build(tauri::generate_context!())
        .expect("error al iniciar Corral")
        .run(|_app, event| {
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                if should_keep_running(code) {
                    api.prevent_exit();
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_the_last_window_keeps_the_tray_alive_but_quit_exits() {
        assert!(should_keep_running(None));
        assert!(!should_keep_running(Some(0)));
    }
}
