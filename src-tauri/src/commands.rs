use crate::config::{self, Config, GpuProfile};
use crate::control::{self, PauseReport, ResumeReport};
use crate::ollama::{InstalledModel, OllamaClient};
use crate::snapshot::{PausedBy, Snapshot};
use crate::state::AppState;
use serde::Serialize;
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};

#[derive(Clone, Serialize)]
struct PullProgressEvent {
    name: String,
    status: String,
    completed: Option<u64>,
    total: Option<u64>,
}

#[derive(Clone, Serialize)]
struct PullDoneEvent {
    name: String,
    error: Option<String>,
}

pub fn push_notice(app: &AppHandle, state: &AppState, message: String) {
    state.notices.lock().unwrap().push(message.clone());
    let _ = app.emit("notice", message);
}

#[tauri::command]
pub fn take_notices(state: State<'_, AppState>) -> Vec<String> {
    std::mem::take(&mut *state.notices.lock().unwrap())
}

pub async fn do_pause(app: &AppHandle, state: &AppState) -> Result<PauseReport, String> {
    let _lifecycle = state.lifecycle.lock().await;
    *state.paused_by.lock().unwrap() = Some(PausedBy::User);
    let client = state.client.read().await.clone();
    let install_dir = state.config.read().await.ollama_install_dir.clone();
    let result = control::pause(&client, state.procs.as_ref(), &install_dir).await.map_err(|e| e.to_string());
    if let Err(message) = &result {
        push_notice(app, state, message.clone());
    }
    state.wake.notify_one();
    result
}

pub async fn do_resume(app: &AppHandle, state: &AppState) -> Result<ResumeReport, String> {
    let _lifecycle = state.lifecycle.lock().await;
    let result = resume_action(state).await;
    match &result {
        Err(message) => push_notice(app, state, message.clone()),
        Ok(report) => {
            for hook in report.hooks.iter().filter(|h| !h.ok) {
                push_notice(app, state, format!("El aviso {} falló: {}", hook.name, hook.detail));
            }
        }
    }
    result
}

async fn resume_action(state: &AppState) -> Result<ResumeReport, String> {
    *state.paused_by.lock().unwrap() = None;
    let client = state.client.read().await.clone();
    let config = state.config.read().await.clone();
    let timeout = Duration::from_secs(config.resume_timeout_secs);
    let env = crate::ollama_gpus::gpu_profile_env(&config.gpu_profile, config.igpu_enabled, std::env::consts::OS);
    let result = control::resume(&client, state.launcher.as_ref(), &config.ollama_install_dir, &config.hooks, &state.http, &env, timeout)
        .await
        .map_err(|e| e.to_string());
    state.wake.notify_one();
    result
}

async fn client(state: &AppState) -> OllamaClient {
    state.client.read().await.clone()
}

#[tauri::command]
pub async fn get_snapshot(state: State<'_, AppState>) -> Result<Option<Snapshot>, String> {
    Ok(state.latest.lock().unwrap().clone())
}

#[tauri::command]
pub async fn list_models(state: State<'_, AppState>) -> Result<Vec<InstalledModel>, String> {
    client(&state).await.installed().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pause_ollama(app: AppHandle, state: State<'_, AppState>) -> Result<PauseReport, String> {
    do_pause(&app, &state).await
}

#[tauri::command]
pub async fn resume_ollama(app: AppHandle, state: State<'_, AppState>) -> Result<ResumeReport, String> {
    do_resume(&app, &state).await
}

#[tauri::command]
pub async fn unload_model(state: State<'_, AppState>, name: String) -> Result<(), String> {
    let result = client(&state).await.unload(&name).await.map_err(|e| e.to_string());
    state.wake.notify_one();
    result
}

pub async fn load_action(state: &AppState, name: &str) -> Result<(), String> {
    let keep_alive = state.config.read().await.load_keep_alive.clone();
    let result = client(state).await.load(name, &keep_alive).await.map_err(|e| e.to_string());
    state.wake.notify_one();
    result
}

#[tauri::command]
pub async fn load_model(app: AppHandle, state: State<'_, AppState>, name: String) -> Result<(), String> {
    let result = load_action(&state, &name).await;
    if let Err(message) = &result {
        push_notice(&app, &state, message.clone());
    }
    result
}

#[tauri::command]
pub async fn delete_model(state: State<'_, AppState>, name: String) -> Result<(), String> {
    client(&state).await.delete(&name).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn copy_model(state: State<'_, AppState>, source: String, destination: String) -> Result<(), String> {
    client(&state).await.copy(&source, &destination).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pull_model(app: AppHandle, state: State<'_, AppState>, name: String) -> Result<(), String> {
    let ollama = client(&state).await;
    tauri::async_runtime::spawn(async move {
        let progress_app = app.clone();
        let progress_name = name.clone();
        let result = ollama
            .pull(&name, move |p| {
                let event = PullProgressEvent { name: progress_name.clone(), status: p.status, completed: p.completed, total: p.total };
                let _ = progress_app.emit("pull-progress", event);
            })
            .await;
        let _ = app.emit("pull-done", PullDoneEvent { name, error: result.err().map(|e| e.to_string()) });
    });
    Ok(())
}

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<Config, String> {
    Ok(state.config.read().await.clone())
}

#[tauri::command]
pub async fn save_config(state: State<'_, AppState>, config: Config) -> Result<(), String> {
    let _profile_action = state.gpu_profile_apply.lock().await;
    let config = config::validate(config)?;
    let mut current = state.config.write().await;
    config::validate_gpu_config_change(&current, &config)?;
    config::save(&state.config_path, &config)?;
    *state.client.write().await = OllamaClient::new(&config.ollama_url);
    *current = config;
    state.wake.notify_one();
    Ok(())
}

#[tauri::command]
pub async fn list_ollama_gpus() -> Result<Vec<crate::ollama_gpus::OllamaGpu>, String> {
    tauri::async_runtime::spawn_blocking(crate::ollama_gpus::list_ollama_gpus).await.map_err(|e| e.to_string())
}

async fn save_gpu_selection(state: &AppState, profile: GpuProfile, igpu_enabled: bool) -> Result<(), String> {
    let mut current = state.config.write().await;
    let next = config::validate(Config { gpu_profile: profile, igpu_enabled, ..current.clone() })?;
    config::save(&state.config_path, &next)?;
    *current = next;
    Ok(())
}

pub async fn apply_gpu_profile_with<P, PF, R, F>(
    state: &AppState,
    profile: GpuProfile,
    igpu_enabled: bool,
    gpus: &[crate::ollama_gpus::OllamaGpu],
    persist: P,
    restart: R,
) -> Result<(), String>
where
    P: FnOnce(&[(String, Option<String>)]) -> PF,
    PF: std::future::Future<Output = Result<(), String>>,
    R: FnOnce() -> F,
    F: std::future::Future<Output = Result<(), String>>,
{
    let _profile_action = state.gpu_profile_apply.lock().await;
    crate::ollama_gpus::validate_gpu_profile(&profile, gpus)?;
    let env = crate::ollama_gpus::gpu_profile_env(&profile, igpu_enabled, std::env::consts::OS);
    save_gpu_selection(state, profile, igpu_enabled).await?;
    persist(&env).await?;
    restart().await
}

#[tauri::command]
pub async fn apply_gpu_profile(app: AppHandle, state: State<'_, AppState>, profile: GpuProfile, igpu_enabled: bool) -> Result<(), String> {
    let result = apply_gpu_profile_action(&app, &state, profile, igpu_enabled).await;
    if let Err(message) = &result {
        push_notice(&app, &state, message.clone());
    }
    result
}

async fn persist_gpu_env(env: Vec<(String, Option<String>)>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || crate::user_env::persist(&env)).await.map_err(|e| e.to_string())?
}

async fn apply_gpu_profile_action(app: &AppHandle, state: &AppState, profile: GpuProfile, igpu_enabled: bool) -> Result<(), String> {
    let gpus = list_ollama_gpus().await?;
    apply_gpu_profile_with(state, profile, igpu_enabled, &gpus, |env| persist_gpu_env(env.to_vec()), || async {
        do_pause(app, state).await?;
        do_resume(app, state).await.map(|_| ())
    }).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::Launcher;
    use std::path::{Path, PathBuf};

    struct FailedLauncher;
    impl Launcher for FailedLauncher {
        fn launch(&self, _: &Path, _: &[(String, Option<String>)]) -> Result<(), String> { Err("falló el lanzamiento".into()) }
    }

    #[tokio::test]
    async fn failed_resume_clears_the_user_pause_marker() {
        let config = Config { ollama_url: "http://127.0.0.1:1".into(), ..Config::default() };
        let mut state = AppState::new(PathBuf::new(), config);
        state.launcher = Box::new(FailedLauncher);
        *state.paused_by.lock().unwrap() = Some(PausedBy::User);
        let _lifecycle = state.lifecycle.lock().await;
        assert_eq!(resume_action(&state).await.unwrap_err(), "falló el lanzamiento");
        assert_eq!(*state.paused_by.lock().unwrap(), None);
    }
}
