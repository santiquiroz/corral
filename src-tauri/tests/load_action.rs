mod common;

use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use corral_lib::{commands::load_action, config::Config, state::AppState};
use serde_json::{json, Value};
use std::{path::PathBuf, sync::{Arc, Mutex}, time::Duration};

type Seen = Arc<Mutex<Vec<Value>>>;

async fn record(State(seen): State<Seen>, Json(body): Json<Value>) -> StatusCode {
    seen.lock().unwrap().push(body);
    StatusCode::OK
}

#[tokio::test]
async fn cargar_usa_duracion_configurada_y_despierta_el_poller() {
    let seen: Seen = Arc::default();
    let router = Router::new().route("/api/generate", post(record)).with_state(seen.clone());
    let config = Config { ollama_url: common::spawn(router).await, load_keep_alive: "1h".into(), ..Config::default() };
    let state = AppState::new(PathBuf::new(), config);

    load_action(&state, "qwen3.5-mem:latest").await.unwrap();

    assert_eq!(*seen.lock().unwrap(), vec![json!({"model": "qwen3.5-mem:latest", "keep_alive": "1h"})]);
    assert!(tokio::time::timeout(Duration::from_millis(100), state.wake.notified()).await.is_ok());
}

#[tokio::test]
async fn cargar_propaga_error_y_despierta_el_poller() {
    let router = Router::new().route("/api/generate", post(|| async { (StatusCode::NOT_FOUND, "model not found") }));
    let config = Config { ollama_url: common::spawn(router).await, ..Config::default() };
    let state = AppState::new(PathBuf::new(), config);

    let error = load_action(&state, "missing").await.unwrap_err();

    assert!(error.contains("404"));
    assert!(error.contains("model not found"));
    assert!(tokio::time::timeout(Duration::from_millis(100), state.wake.notified()).await.is_ok());
}
