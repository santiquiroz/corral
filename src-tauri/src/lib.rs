pub mod gpu;
pub mod config;
pub mod ollama;
pub mod procs;
pub mod snapshot;

pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error al iniciar Corral");
}
