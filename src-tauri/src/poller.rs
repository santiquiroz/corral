use crate::collector::{assemble, gather, CollectInputs};
use crate::snapshot::{PausedBy, Snapshot};
use crate::state::AppState;
use crate::tray;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

pub fn reconcile_paused(paused_by: Option<PausedBy>, version_ok: bool) -> Option<PausedBy> {
    if version_ok { None } else { paused_by }
}

pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let state = app.state::<AppState>();
            let snapshot = tick(&state).await;
            *state.latest.lock().unwrap() = Some(snapshot.clone());
            let _ = app.emit("snapshot", &snapshot);
            tray::update(&app, &snapshot);
            let wait = Duration::from_secs(cadence_secs(&app, &state).await);
            tokio::select! {
                _ = tokio::time::sleep(wait) => {}
                _ = state.wake.notified() => {}
            }
        }
    });
}

async fn tick(state: &AppState) -> Snapshot {
    let _lifecycle = state.lifecycle.lock().await;
    let client = state.client.read().await.clone();
    let config = state.config.read().await.clone();
    let paused_by = *state.paused_by.lock().unwrap();
    let mut cache = state.blob_cache.lock().await;
    let inputs = gather(&client, state.gpu.as_ref(), state.procs.as_ref(), &mut cache, &config.ollama_install_dir, paused_by, config.spill_floor_mb, now_ms()).await;
    let reconciled = reconcile_paused(paused_by, inputs.version.is_ok());
    *state.paused_by.lock().unwrap() = reconciled;
    assemble(&CollectInputs { paused_by: reconciled, ..inputs })
}

async fn cadence_secs(app: &AppHandle, state: &AppState) -> u64 {
    let config = state.config.read().await;
    if panel_visible(app) { config.poll_panel_secs } else { config.poll_tray_secs }
}

fn panel_visible(app: &AppHandle) -> bool {
    app.get_webview_window("main").and_then(|w| w.is_visible().ok()).unwrap_or(false)
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn tick_waits_for_the_lifecycle_action_to_finish() {
        let state = AppState::new(std::path::PathBuf::new(), crate::config::Config::default());
        let guard = state.lifecycle.lock().await;
        assert!(tokio::time::timeout(Duration::from_millis(50), tick(&state)).await.is_err());
        drop(guard);
        assert!(state.lifecycle.try_lock().is_ok());
    }

    #[test]
    fn pause_mark_clears_once_ollama_answers_again() {
        assert_eq!(reconcile_paused(Some(PausedBy::User), true), None);
        assert_eq!(reconcile_paused(Some(PausedBy::User), false), Some(PausedBy::User));
        assert_eq!(reconcile_paused(None, false), None);
    }
}
