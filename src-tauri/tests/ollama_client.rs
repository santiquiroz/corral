mod common;

use axum::{body::Body, extract::State, http::StatusCode, routing::{delete, get, post}, Json, Router};
use corral_lib::ollama::{OllamaClient, OllamaError};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

type Seen = Arc<Mutex<Vec<Value>>>;

async fn record(State(seen): State<Seen>, Json(body): Json<Value>) -> StatusCode {
    seen.lock().unwrap().push(body);
    StatusCode::OK
}

#[tokio::test]
async fn reads_version_loaded_and_installed() {
    let router = Router::new()
        .route("/api/version", get(|| async { Json(json!({"version": "0.35.1"})) }))
        .route("/api/ps", get(|| async { Json(json!({"models": [{
            "name": "qwen3.5-mem:latest", "model": "qwen3.5-mem:latest", "size": 6_584_805_620u64,
            "digest": "41d7", "expires_at": "2026-10-06T10:27:48-05:00", "size_vram": 6_584_805_620u64, "context_length": 32768
        }]})) }))
        .route("/api/tags", get(|| async { Json(json!({"models": [{
            "name": "hf.co/openbmb/MiniCPM5-2B-GGUF:Q4_K_M", "digest": "e680", "size": 1_600_000_000u64,
            "modified_at": "2026-10-05T10:00:00-05:00",
            "details": {"family": "minicpm", "parameter_size": "2.5B", "quantization_level": "Q4_K_M", "context_length": 131072}
        }]})) }));
    let client = OllamaClient::new(&common::spawn(router).await);

    assert_eq!(client.version().await.unwrap(), "0.35.1");
    let loaded = client.loaded().await.unwrap();
    assert_eq!(loaded[0].name, "qwen3.5-mem:latest");
    assert_eq!(loaded[0].vram_mb, 6279);
    assert_eq!(loaded[0].context_length, 32768);
    let installed = client.installed().await.unwrap();
    assert_eq!(installed[0].name, "hf.co/openbmb/MiniCPM5-2B-GGUF:Q4_K_M");
    assert_eq!(installed[0].quantization, "Q4_K_M");
    assert_eq!(installed[0].context_length, Some(131072));
}

#[tokio::test]
async fn sends_exact_namespaced_names_in_actions() {
    let seen: Seen = Arc::default();
    let router = Router::new()
        .route("/api/generate", post(record))
        .route("/api/delete", delete(record))
        .route("/api/copy", post(record))
        .with_state(seen.clone());
    let client = OllamaClient::new(&common::spawn(router).await);
    let name = "hf.co/openbmb/MiniCPM5-2B-GGUF:Q4_K_M";

    client.unload(name).await.unwrap();
    client.delete(name).await.unwrap();
    client.copy(name, "minicpm5-32k").await.unwrap();

    let bodies = seen.lock().unwrap().clone();
    assert_eq!(bodies[0], json!({"model": name, "keep_alive": 0}));
    assert_eq!(bodies[1], json!({"model": name}));
    assert_eq!(bodies[2], json!({"source": name, "destination": "minicpm5-32k"}));
}

#[tokio::test]
async fn http_errors_and_unreachable_are_distinct() {
    let router = Router::new().route("/api/delete", delete(|| async { (StatusCode::NOT_FOUND, "model not found") }));
    let client = OllamaClient::new(&common::spawn(router).await);
    assert_eq!(client.delete("nope").await, Err(OllamaError::Http { status: 404, body: "model not found".into() }));

    let down = OllamaClient::new(&common::closed_port_url());
    assert!(matches!(down.version().await, Err(OllamaError::Unreachable(_))));
}

#[tokio::test]
async fn blob_path_comes_from_modelfile() {
    let router = Router::new().route("/api/show", post(|| async {
        Json(json!({"modelfile": "FROM C:\\m\\blobs\\sha256-dec52a\nPARAMETER num_ctx 32768"}))
    }));
    let client = OllamaClient::new(&common::spawn(router).await);
    assert_eq!(client.blob_path("qwen3.5-mem:latest").await.unwrap().as_deref(), Some("C:\\m\\blobs\\sha256-dec52a"));
}

#[tokio::test]
async fn pull_streams_progress_and_surfaces_errors() {
    let router = Router::new()
        .route("/api/pull", post(|Json(body): Json<Value>| async move {
            if body["model"] == "missing" {
                return Body::from("{\"status\":\"pulling manifest\"}\n{\"error\":\"file does not exist\"}\n");
            }
            Body::from("{\"status\":\"pulling manifest\"}\n{\"status\":\"downloading\",\"completed\":50,\"total\":100}\n{\"status\":\"success\"}\n")
        }));
    let client = OllamaClient::new(&common::spawn(router).await);

    let mut statuses = Vec::new();
    client.pull("qwen3.5:4b", |p| statuses.push((p.status, p.completed))).await.unwrap();
    assert_eq!(statuses, vec![("pulling manifest".into(), None), ("downloading".into(), Some(50)), ("success".into(), None)]);

    let result = client.pull("missing", |_| {}).await;
    assert!(matches!(result, Err(OllamaError::Http { ref body, .. }) if body.contains("file does not exist")));
}
