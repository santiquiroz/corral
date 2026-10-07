mod common;

use axum::{http::StatusCode, routing::{get, post}, Json, Router};
use corral_lib::claude_mem::{read_status, CheckLevel, ClaudeMemStatus};
use serde_json::{json, Value};
use std::{path::PathBuf, sync::atomic::{AtomicU64, Ordering}, time::{Duration, Instant}};

const NOW: u64 = 7_200_000;
static NEXT_PATH: AtomicU64 = AtomicU64::new(0);

struct Settings(PathBuf);

impl Settings {
    fn new() -> Self {
        let id = NEXT_PATH.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("corral-mem-status-{}-{id}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir.join("settings.json"))
    }

    fn write(&self, base_url: &str) {
        let settings = json!({"CLAUDE_MEM_PROVIDER":"openrouter", "CLAUDE_MEM_OPENROUTER_BASE_URL":format!("{base_url}/v1"), "CLAUDE_MEM_OPENROUTER_MODEL":"memory"});
        std::fs::write(&self.0, serde_json::to_string(&settings).unwrap()).unwrap();
    }
}

impl Drop for Settings {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(self.0.parent().unwrap()); }
}

fn worker() -> Router {
    Router::new()
        .route("/api/health", get(|| async { Json(json!({"ai":{"lastInteraction":{"timestamp":NOW,"success":true}}})) }))
        .route("/api/processing-status", get(|| async { Json(json!({"queueDepth":0})) }))
}

fn ollama(installed: bool, loaded_name: &str) -> Router {
    let loaded_name = loaded_name.to_string();
    Router::new()
        .route("/api/tags", get(move || async move {
            let models = if installed { vec![json!({"name":"memory:latest","digest":"abc","size":1024})] } else { vec![] };
            Json(json!({"models":models}))
        }))
        .route("/api/ps", get(move || { let loaded_name = loaded_name.clone(); async move {
            Json(json!({"models":[{"name":loaded_name,"digest":"abc","size":1024,"size_vram":1024}]}))
        } }))
        .route("/api/show", post(|| async { Json(json!({"parameters":"num_ctx                        32768"})) }))
}

fn assert_level(status: &ClaudeMemStatus, id: &str, expected: CheckLevel) {
    assert_eq!(status.checks.iter().find(|check| check.id == id).unwrap().level, expected, "{id}");
}

#[tokio::test]
async fn combina_settings_y_servidores_con_seis_checks_ok() {
    let ollama = common::spawn(ollama(true, "memory:latest")).await;
    let worker = common::spawn(worker()).await;
    let settings = Settings::new();
    settings.write(&ollama);

    let status = read_status(&settings.0, &ollama, &worker, NOW).await.unwrap().unwrap();

    assert_eq!(status.provider, "openrouter");
    assert_eq!(status.base_url, format!("{ollama}/v1"));
    assert_eq!(status.model, "memory");
    assert_eq!(status.queue_depth, Some(0));
    assert_eq!(status.checks.len(), 6);
    assert!(status.checks.iter().all(|check| check.level == CheckLevel::Ok));
}

#[tokio::test]
async fn modelo_borrado_falla_aunque_este_cargado_por_digest() {
    let ollama = common::spawn(ollama(false, "memory:latest")).await;
    let worker = common::spawn(worker()).await;
    let settings = Settings::new();
    settings.write(&ollama);

    let status = read_status(&settings.0, &ollama, &worker, NOW).await.unwrap().unwrap();

    assert_level(&status, "model_installed", CheckLevel::Fail);
}

#[tokio::test]
async fn alias_interno_cargado_se_reconoce_por_digest_instalado() {
    let ollama = common::spawn(ollama(true, "llamacpp:abc")).await;
    let worker = common::spawn(worker()).await;
    let settings = Settings::new();
    settings.write(&ollama);

    let status = read_status(&settings.0, &ollama, &worker, NOW).await.unwrap().unwrap();

    assert_level(&status, "model_loaded", CheckLevel::Ok);
}

#[tokio::test]
async fn worker_inaccesible_falla_sin_ocultar_checks_de_ollama() {
    let ollama = common::spawn(ollama(true, "memory:latest")).await;
    let settings = Settings::new();
    settings.write(&ollama);

    let status = read_status(&settings.0, &ollama, &common::closed_port_url(), NOW).await.unwrap().unwrap();

    assert_level(&status, "worker", CheckLevel::Fail);
    assert_level(&status, "model_installed", CheckLevel::Ok);
    assert_eq!(status.queue_depth, None);
}

#[tokio::test]
async fn settings_ausentes_no_consultan_servidores_y_json_invalido_es_error() {
    let settings = Settings::new();
    let dead = common::closed_port_url();
    assert!(read_status(&settings.0, &dead, &dead, NOW).await.unwrap().is_none());
    std::fs::write(&settings.0, "{invalid").unwrap();
    assert!(read_status(&settings.0, &dead, &dead, NOW).await.is_err());
}

#[tokio::test]
async fn endpoint_ajeno_advierte_y_no_atribuye_modelos_de_este_ollama() {
    let ollama = common::spawn(ollama(true, "memory:latest")).await;
    let worker = common::spawn(worker()).await;
    let settings = Settings::new();
    settings.write("http://127.0.0.1:1");

    let status = read_status(&settings.0, &ollama, &worker, NOW).await.unwrap().unwrap();

    assert_level(&status, "endpoint", CheckLevel::Warn);
    for id in ["model_installed", "model_loaded", "context"] { assert_level(&status, id, CheckLevel::Warn); }
}

#[tokio::test]
async fn proveedor_claude_no_atribuye_modelos_de_ollama_aun_con_misma_url() {
    let ollama = common::spawn(ollama(true, "memory:latest")).await;
    let worker = common::spawn(worker()).await;
    let settings = Settings::new();
    let content = json!({"CLAUDE_MEM_PROVIDER":"claude", "CLAUDE_MEM_OPENROUTER_BASE_URL":format!("{ollama}/v1"), "CLAUDE_MEM_OPENROUTER_MODEL":"memory"});
    std::fs::write(&settings.0, serde_json::to_string(&content).unwrap()).unwrap();

    let status = read_status(&settings.0, &ollama, &worker, NOW).await.unwrap().unwrap();

    assert_level(&status, "endpoint", CheckLevel::Warn);
    for id in ["model_installed", "model_loaded", "context"] { assert_level(&status, id, CheckLevel::Warn); }
}

#[tokio::test]
async fn errores_http_de_ollama_dejan_checks_desconocidos_sin_panico() {
    let ollama = common::spawn(Router::new()
        .route("/api/tags", get(|| async { StatusCode::SERVICE_UNAVAILABLE }))
        .route("/api/ps", get(|| async { Json(json!({"respuesta":"invalida"})) }))
        .route("/api/show", post(|| async { StatusCode::INTERNAL_SERVER_ERROR }))).await;
    let worker = common::spawn(worker()).await;
    let settings = Settings::new();
    settings.write(&ollama);

    let status = read_status(&settings.0, &ollama, &worker, NOW).await.unwrap().unwrap();

    for id in ["model_installed", "model_loaded", "context"] { assert_level(&status, id, CheckLevel::Warn); }
    assert_level(&status, "worker", CheckLevel::Ok);
}

async fn slow() -> Json<Value> {
    tokio::time::sleep(Duration::from_secs(5)).await;
    Json(json!({}))
}

#[tokio::test]
async fn consultas_lentas_se_hacen_en_paralelo_con_tope_de_dos_segundos() {
    let ollama = common::spawn(Router::new().route("/api/tags", get(slow)).route("/api/ps", get(slow)).route("/api/show", post(slow))).await;
    let worker = common::spawn(Router::new().route("/api/health", get(slow)).route("/api/processing-status", get(slow))).await;
    let settings = Settings::new();
    settings.write(&ollama);
    let started = Instant::now();

    let status = read_status(&settings.0, &ollama, &worker, NOW).await.unwrap().unwrap();

    assert!(started.elapsed() < Duration::from_secs(3), "demoró {:?}", started.elapsed());
    assert_level(&status, "worker", CheckLevel::Fail);
    assert_level(&status, "model_installed", CheckLevel::Warn);
}
