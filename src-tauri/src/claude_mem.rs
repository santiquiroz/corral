use serde::{Deserialize, Serialize};
use crate::ollama::{InstalledModel, OllamaClient};
use crate::snapshot::LoadedModel;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const WORKER_BASE: &str = "http://127.0.0.1:37777";
const STATUS_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckLevel {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Check {
    pub id: String,
    pub level: CheckLevel,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClaudeMemStatus {
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub queue_depth: Option<u64>,
    pub checks: Vec<Check>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LastInteraction {
    #[serde(rename = "timestamp")]
    pub timestamp_ms: u64,
    pub success: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ClaudeMemInputs {
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub ollama_url: String,
    pub worker_reachable: bool,
    pub model_installed: Option<bool>,
    pub model_loaded: Option<bool>,
    pub last_interaction: Option<LastInteraction>,
    pub queue_depth: Option<u64>,
    pub num_ctx: Option<u64>,
    pub now_ms: u64,
}

fn check(id: &str, level: CheckLevel, message: impl Into<String>) -> Check {
    Check { id: id.into(), level, message: message.into() }
}

fn worker_check(inputs: &ClaudeMemInputs) -> Check {
    if inputs.worker_reachable {
        return check("worker", CheckLevel::Ok, "El worker de claude-mem responde");
    }
    check("worker", CheckLevel::Fail, "El worker de claude-mem no responde")
}

fn endpoint_address(url: &str) -> Option<(String, u16)> {
    let url = reqwest::Url::parse(url).ok()?;
    Some((url.host_str()?.to_string(), url.port_or_known_default()?))
}

fn endpoint_check(inputs: &ClaudeMemInputs) -> Check {
    if uses_current_ollama(&inputs.provider, &inputs.base_url, &inputs.ollama_url) {
        return check("endpoint", CheckLevel::Ok, "claude-mem usa este Ollama");
    }
    check("endpoint", CheckLevel::Warn, format!("claude-mem no usa este Ollama: {}", inputs.base_url))
}

fn uses_current_ollama(provider: &str, base_url: &str, ollama_url: &str) -> bool {
    if provider != "openrouter" {
        return false;
    }
    let Some(address) = endpoint_address(base_url) else { return false };
    Some(address) == endpoint_address(ollama_url)
}

fn installed_check(inputs: &ClaudeMemInputs) -> Check {
    match inputs.model_installed {
        Some(true) => check("model_installed", CheckLevel::Ok, format!("El modelo {} está instalado", inputs.model)),
        Some(false) => check("model_installed", CheckLevel::Fail, format!("El modelo {} no está instalado: claude-mem fallará cuando se descargue de memoria", inputs.model)),
        None => check("model_installed", CheckLevel::Warn, format!("No se pudo comprobar si el modelo {} está instalado", inputs.model)),
    }
}

fn loaded_check(inputs: &ClaudeMemInputs) -> Check {
    match inputs.model_loaded {
        Some(true) => check("model_loaded", CheckLevel::Ok, format!("El modelo {} está cargado", inputs.model)),
        Some(false) => check("model_loaded", CheckLevel::Warn, format!("{} no cargado: se cargará con la próxima observación", inputs.model)),
        None => check("model_loaded", CheckLevel::Warn, format!("No se pudo comprobar si el modelo {} está cargado", inputs.model)),
    }
}

fn interaction_check(inputs: &ClaudeMemInputs) -> Check {
    let Some(last) = &inputs.last_interaction else {
        return check("last_interaction", CheckLevel::Warn, "claude-mem aún no informa una interacción");
    };
    if !last.success {
        return check("last_interaction", CheckLevel::Fail, "La última interacción de claude-mem falló");
    }
    if has_stale_pending_work(inputs, last) {
        return check("last_interaction", CheckLevel::Warn, "La última interacción de claude-mem tiene más de 1 h y hay observaciones en cola");
    }
    check("last_interaction", CheckLevel::Ok, "La última interacción de claude-mem fue exitosa")
}

fn has_stale_pending_work(inputs: &ClaudeMemInputs, last: &LastInteraction) -> bool {
    if inputs.queue_depth.unwrap_or(0) == 0 {
        return false;
    }
    inputs.now_ms.saturating_sub(last.timestamp_ms) > 3_600_000
}

fn context_check(inputs: &ClaudeMemInputs) -> Check {
    match inputs.num_ctx {
        Some(value) if value >= 16384 => check("context", CheckLevel::Ok, format!("Contexto del modelo: {value} tokens")),
        Some(_) => check("context", CheckLevel::Warn, "claude-mem recicla su conversación cerca de 16k tokens; con menos contexto Ollama recorta el prompt"),
        None => check("context", CheckLevel::Warn, format!("No se pudo leer num_ctx del modelo {}", inputs.model)),
    }
}

pub fn evaluate_claude_mem(inputs: &ClaudeMemInputs) -> Vec<Check> {
    vec![worker_check(inputs), endpoint_check(inputs), installed_check(inputs), loaded_check(inputs), interaction_check(inputs), context_check(inputs)]
}

#[derive(Deserialize)]
struct Settings {
    #[serde(rename = "CLAUDE_MEM_PROVIDER", default)]
    provider: String,
    #[serde(rename = "CLAUDE_MEM_OPENROUTER_BASE_URL", default)]
    base_url: String,
    #[serde(rename = "CLAUDE_MEM_OPENROUTER_MODEL", default)]
    model: String,
}

#[derive(Deserialize)]
struct Health {
    ai: Option<AiHealth>,
}

#[derive(Deserialize)]
struct AiHealth {
    #[serde(rename = "lastInteraction")]
    last_interaction: Option<LastInteraction>,
}

#[derive(Deserialize)]
struct ProcessingStatus {
    #[serde(rename = "queueDepth")]
    queue_depth: Option<u64>,
}

pub fn settings_path() -> Result<PathBuf, String> {
    let variable = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    let home = std::env::var_os(variable).ok_or_else(|| format!("No se pudo localizar {variable}"))?;
    Ok(PathBuf::from(home).join(".claude-mem").join("settings.json"))
}

fn load_settings(path: &Path) -> Result<Option<Settings>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("No se pudo leer {}: {error}", path.display())),
    };
    serde_json::from_str(&text).map(Some).map_err(|error| format!("Configuración de claude-mem inválida en {}: {error}", path.display()))
}

async fn bounded<T, E>(future: impl std::future::Future<Output = Result<T, E>>) -> Option<T> {
    tokio::time::timeout(STATUS_TIMEOUT, future).await.ok()?.ok()
}

async fn worker_json<T: serde::de::DeserializeOwned>(http: &reqwest::Client, base: &str, path: &str) -> Option<T> {
    let response = http.get(format!("{}{path}", base.trim_end_matches('/'))).send().await.ok()?;
    response.error_for_status().ok()?.json().await.ok()
}

fn model_name(name: &str) -> String {
    if name.rsplit('/').next().unwrap_or(name).contains(':') {
        return name.to_string();
    }
    format!("{name}:latest")
}

fn matching_installed<'a>(models: &'a [InstalledModel], name: &str) -> Option<&'a InstalledModel> {
    let name = model_name(name);
    models.iter().find(|model| model_name(&model.name) == name)
}

fn loaded_model(models: &[LoadedModel], name: &str, installed: Option<&InstalledModel>) -> bool {
    let name = model_name(name);
    models.iter().any(|model| model_name(&model.name) == name || same_digest(model, installed))
}

fn same_digest(model: &LoadedModel, installed: Option<&InstalledModel>) -> bool {
    let Some(installed) = installed else { return false };
    !installed.digest.is_empty() && installed.digest == model.digest
}

#[derive(Default)]
struct ModelStatus {
    installed: Option<bool>,
    loaded: Option<bool>,
    num_ctx: Option<u64>,
}

async fn collect_model_status(settings: &Settings, ollama_url: &str) -> ModelStatus {
    if !uses_current_ollama(&settings.provider, &settings.base_url, ollama_url) {
        return ModelStatus::default();
    }
    let ollama = OllamaClient::new(ollama_url);
    let (installed, loaded, num_ctx) = tokio::join!(
        bounded(ollama.installed()),
        bounded(ollama.loaded()),
        bounded(ollama.num_ctx(&settings.model)),
    );
    let installed_model = installed.as_ref().and_then(|models| matching_installed(models, &settings.model));
    ModelStatus {
        installed: installed.as_ref().map(|_| installed_model.is_some()),
        loaded: loaded.as_ref().map(|models| loaded_model(models, &settings.model, installed_model)),
        num_ctx: num_ctx.flatten(),
    }
}

async fn collect_inputs(settings: Settings, ollama_url: &str, worker_base: &str, now_ms: u64) -> Result<ClaudeMemInputs, String> {
    let http = reqwest::Client::builder().timeout(STATUS_TIMEOUT).redirect(reqwest::redirect::Policy::none()).build().map_err(|error| error.to_string())?;
    let (models, health, processing) = tokio::join!(
        collect_model_status(&settings, ollama_url),
        worker_json::<Health>(&http, worker_base, "/api/health"),
        worker_json::<ProcessingStatus>(&http, worker_base, "/api/processing-status"),
    );
    Ok(ClaudeMemInputs {
        model_installed: models.installed,
        model_loaded: models.loaded,
        num_ctx: models.num_ctx,
        worker_reachable: health.is_some(),
        last_interaction: health.and_then(|health| health.ai).and_then(|ai| ai.last_interaction),
        queue_depth: processing.and_then(|processing| processing.queue_depth),
        provider: settings.provider,
        base_url: settings.base_url,
        model: settings.model,
        ollama_url: ollama_url.into(),
        now_ms,
    })
}

pub async fn read_status(settings_path: &Path, ollama_url: &str, worker_base: &str, now_ms: u64) -> Result<Option<ClaudeMemStatus>, String> {
    let path = settings_path.to_path_buf();
    let settings = tokio::task::spawn_blocking(move || load_settings(&path)).await.map_err(|error| error.to_string())??;
    let Some(settings) = settings else { return Ok(None) };
    let inputs = collect_inputs(settings, ollama_url, worker_base, now_ms).await?;
    let checks = evaluate_claude_mem(&inputs);
    Ok(Some(ClaudeMemStatus { provider: inputs.provider, base_url: inputs.base_url, model: inputs.model, queue_depth: inputs.queue_depth, checks }))
}
