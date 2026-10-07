mod common;

use axum::{extract::State, http::StatusCode, response::{IntoResponse, Response}, routing::{get, post}, Json, Router};
use corral_lib::collector::{assemble, gather, BlobCache};
use corral_lib::gpu::UnsupportedGpuProbe;
use corral_lib::ollama::OllamaClient;
use corral_lib::procs::{ProcInfo, ProcessSource};
use serde_json::{json, Value};
use std::{path::{Path, PathBuf}, sync::{Arc, Mutex}};

#[derive(Default)]
struct FakeState {
    digest: String,
    tags_calls: usize,
    show_calls: Vec<String>,
    missing_show: bool,
    wrong_alias_digest: bool,
}

type SharedState = Arc<Mutex<FakeState>>;

async fn loaded(State(state): State<SharedState>) -> Json<Value> {
    let state = state.lock().unwrap();
    Json(json!({"models": [{"name": "qwen3.5-mem:latest", "digest": state.digest, "size": 1048576}]}))
}

async fn installed(State(state): State<SharedState>) -> Json<Value> {
    let mut state = state.lock().unwrap();
    state.tags_calls += 1;
    let digest = if state.wrong_alias_digest { "other" } else { &state.digest };
    Json(json!({"models": [{"name": format!("llamacpp:{digest}"), "digest": digest, "size": 1048576}]}))
}

async fn show(State(state): State<SharedState>, Json(body): Json<Value>) -> Response {
    let mut state = state.lock().unwrap();
    let name = body["model"].as_str().unwrap().to_string();
    state.show_calls.push(name.clone());
    if name.starts_with("llamacpp:") {
        return Json(json!({"modelfile": "FROM C:\\m\\blobs\\sha256-733ca5"})).into_response();
    }
    if state.missing_show {
        return Json(json!({"modelfile": "FROM base:latest"})).into_response();
    }
    (StatusCode::NOT_FOUND, "manifest missing").into_response()
}

async fn fake_client(state: SharedState) -> OllamaClient {
    let router = Router::new()
        .route("/api/version", get(|| async { Json(json!({"version": "0.40"})) }))
        .route("/api/ps", get(loaded))
        .route("/api/tags", get(installed))
        .route("/api/show", post(show))
        .with_state(state);
    OllamaClient::new(&common::spawn(router).await)
}

struct RunnerSource;

impl ProcessSource for RunnerSource {
    fn kill(&self, _: &ProcInfo) -> bool { false }

    fn list(&self) -> Vec<ProcInfo> {
        vec![ProcInfo {
            pid: 59564, start_time: 1, name: "llama-server.exe".into(),
            exe: Some(PathBuf::from(r"C:\Ollama\lib\llama-server.exe")),
            cmd: vec!["--model".into(), r"C:\m\blobs\sha256-733ca5".into()],
            cpu_pct: 0.0, ram_mb: 1,
        }]
    }
}

async fn runner_name(client: &OllamaClient, cache: &mut BlobCache) -> Option<String> {
    let inputs = gather(client, &UnsupportedGpuProbe, &RunnerSource, cache, Path::new(r"C:\Ollama"), None, 64, 1).await;
    assemble(&inputs).runners.as_ok().unwrap()[0].model.clone()
}

fn state() -> SharedState {
    Arc::new(Mutex::new(FakeState { digest: "1d171a".into(), ..Default::default() }))
}

#[tokio::test]
async fn runner_uses_loaded_name_when_missing_manifest_resolves_through_same_digest_alias() {
    let state = state();
    let client = fake_client(state.clone()).await;
    assert_eq!(runner_name(&client, &mut BlobCache::default()).await.as_deref(), Some("qwen3.5-mem:latest"));
    assert_eq!(state.lock().unwrap().show_calls, vec!["qwen3.5-mem:latest", "llamacpp:1d171a"]);
}

#[tokio::test]
async fn runner_resolves_alias_when_loaded_show_has_no_blob() {
    let state = state();
    state.lock().unwrap().missing_show = true;
    let client = fake_client(state).await;
    assert_eq!(runner_name(&client, &mut BlobCache::default()).await.as_deref(), Some("qwen3.5-mem:latest"));
}

#[tokio::test]
async fn runner_does_not_use_an_alias_with_a_different_digest() {
    let state = state();
    state.lock().unwrap().wrong_alias_digest = true;
    let client = fake_client(state.clone()).await;
    assert_eq!(runner_name(&client, &mut BlobCache::default()).await, None);
    assert_eq!(state.lock().unwrap().show_calls, vec!["qwen3.5-mem:latest"]);
}

#[tokio::test]
async fn installed_aliases_are_cached_and_refreshed_for_a_new_loaded_digest() {
    let state = state();
    let client = fake_client(state.clone()).await;
    let mut cache = BlobCache::default();
    for _ in 0..2 {
        assert_eq!(runner_name(&client, &mut cache).await.as_deref(), Some("qwen3.5-mem:latest"));
    }
    assert_eq!(state.lock().unwrap().tags_calls, 1);
    assert_eq!(state.lock().unwrap().show_calls.len(), 2);
    state.lock().unwrap().digest = "new-digest".into();
    assert_eq!(runner_name(&client, &mut cache).await.as_deref(), Some("qwen3.5-mem:latest"));
    assert_eq!(state.lock().unwrap().tags_calls, 2);
    assert_eq!(state.lock().unwrap().show_calls.last().unwrap(), "llamacpp:new-digest");
}
