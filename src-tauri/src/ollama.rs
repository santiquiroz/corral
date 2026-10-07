use crate::snapshot::LoadedModel;
use futures_util::StreamExt;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;

const READ_TIMEOUT: Duration = Duration::from_secs(5);
const ACTION_TIMEOUT: Duration = Duration::from_secs(30);
const MB: u64 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum OllamaError {
    #[error("Ollama no responde: {0}")]
    Unreachable(String),
    #[error("Ollama respondió {status}: {body}")]
    Http { status: u16, body: String },
    #[error("respuesta inesperada de Ollama: {0}")]
    Decode(String),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InstalledModel {
    pub name: String,
    pub digest: String,
    pub size_mb: u64,
    pub family: String,
    pub parameter_size: String,
    pub quantization: String,
    pub context_length: Option<u64>,
    pub modified_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PullProgress {
    pub status: String,
    pub completed: Option<u64>,
    pub total: Option<u64>,
}

#[derive(Deserialize)]
struct VersionResponse {
    version: String,
}

#[derive(Deserialize)]
struct PsResponse {
    models: Vec<PsModel>,
}

#[derive(Deserialize)]
struct PsModel {
    name: String,
    #[serde(default)]
    digest: String,
    size: u64,
    #[serde(default)]
    size_vram: u64,
    #[serde(default)]
    context_length: u64,
    #[serde(default)]
    expires_at: String,
}

#[derive(Deserialize)]
struct TagsResponse {
    models: Vec<TagModel>,
}

#[derive(Deserialize)]
struct TagModel {
    name: String,
    digest: String,
    size: u64,
    #[serde(default)]
    modified_at: String,
    #[serde(default)]
    details: TagDetails,
}

#[derive(Deserialize, Default)]
struct TagDetails {
    #[serde(default)]
    family: String,
    #[serde(default)]
    parameter_size: String,
    #[serde(default)]
    quantization_level: String,
    context_length: Option<u64>,
}

#[derive(Deserialize)]
struct ShowResponse {
    #[serde(default)]
    modelfile: String,
}

#[derive(Deserialize)]
struct PullLine {
    #[serde(default)]
    status: String,
    completed: Option<u64>,
    total: Option<u64>,
    error: Option<String>,
}

#[derive(Clone)]
pub struct OllamaClient {
    base: String,
    http: reqwest::Client,
}

impl OllamaClient {
    pub fn new(base: &str) -> Self {
        Self { base: base.trim_end_matches('/').to_string(), http: reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).build().expect("cliente HTTP de Ollama") }
    }

    pub async fn version(&self) -> Result<String, OllamaError> {
        let response = self.send(self.http.get(self.url("/api/version")).timeout(READ_TIMEOUT)).await?;
        Ok(decode::<VersionResponse>(response).await?.version)
    }

    pub async fn loaded(&self) -> Result<Vec<LoadedModel>, OllamaError> {
        let response = self.send(self.http.get(self.url("/api/ps")).timeout(READ_TIMEOUT)).await?;
        Ok(decode::<PsResponse>(response).await?.models.into_iter().map(to_loaded).collect())
    }

    pub async fn installed(&self) -> Result<Vec<InstalledModel>, OllamaError> {
        let response = self.send(self.http.get(self.url("/api/tags")).timeout(READ_TIMEOUT)).await?;
        Ok(decode::<TagsResponse>(response).await?.models.into_iter().map(to_installed).collect())
    }

    pub async fn blob_path(&self, model: &str) -> Result<Option<String>, OllamaError> {
        let request = self.http.post(self.url("/api/show")).json(&json!({ "model": model })).timeout(READ_TIMEOUT);
        let show = decode::<ShowResponse>(self.send(request).await?).await?;
        Ok(parse_from_blob(&show.modelfile))
    }

    pub async fn unload(&self, model: &str) -> Result<(), OllamaError> {
        let request = self.http.post(self.url("/api/generate")).json(&json!({ "model": model, "keep_alive": 0 }));
        self.send(request.timeout(ACTION_TIMEOUT)).await.map(|_| ())
    }

    pub async fn delete(&self, model: &str) -> Result<(), OllamaError> {
        let request = self.http.delete(self.url("/api/delete")).json(&json!({ "model": model }));
        self.send(request.timeout(ACTION_TIMEOUT)).await.map(|_| ())
    }

    pub async fn copy(&self, source: &str, destination: &str) -> Result<(), OllamaError> {
        let request = self.http.post(self.url("/api/copy")).json(&json!({ "source": source, "destination": destination }));
        self.send(request.timeout(ACTION_TIMEOUT)).await.map(|_| ())
    }

    pub async fn pull<F: FnMut(PullProgress)>(&self, model: &str, mut on_progress: F) -> Result<(), OllamaError> {
        let request = self.http.post(self.url("/api/pull")).json(&json!({ "model": model, "stream": true }));
        let mut stream = self.send(request).await?.bytes_stream();
        let mut pending: Vec<u8> = Vec::new();
        while let Some(chunk) = stream.next().await {
            pending.extend_from_slice(&chunk.map_err(|e| OllamaError::Unreachable(e.to_string()))?);
            emit_complete_lines(&mut pending, &mut on_progress)?;
        }
        emit_line(&pending, &mut on_progress)
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    async fn send(&self, request: reqwest::RequestBuilder) -> Result<reqwest::Response, OllamaError> {
        let response = request.send().await.map_err(|e| OllamaError::Unreachable(e.to_string()))?;
        if response.status().is_success() {
            return Ok(response);
        }
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        Err(OllamaError::Http { status, body })
    }
}

async fn decode<T: DeserializeOwned>(response: reqwest::Response) -> Result<T, OllamaError> {
    response.json::<T>().await.map_err(|e| OllamaError::Decode(e.to_string()))
}

fn emit_complete_lines<F: FnMut(PullProgress)>(pending: &mut Vec<u8>, on_progress: &mut F) -> Result<(), OllamaError> {
    while let Some(newline) = pending.iter().position(|b| *b == b'\n') {
        let line: Vec<u8> = pending.drain(..=newline).collect();
        emit_line(&line, on_progress)?;
    }
    Ok(())
}

fn emit_line<F: FnMut(PullProgress)>(raw: &[u8], on_progress: &mut F) -> Result<(), OllamaError> {
    let text = String::from_utf8_lossy(raw);
    if let Some(progress) = parse_pull_line(text.trim())? {
        on_progress(progress);
    }
    Ok(())
}

fn to_loaded(model: PsModel) -> LoadedModel {
    LoadedModel {
        name: model.name,
        digest: model.digest,
        size_mb: model.size / MB,
        vram_mb: model.size_vram / MB,
        context_length: model.context_length,
        expires_at: model.expires_at,
    }
}

fn to_installed(model: TagModel) -> InstalledModel {
    InstalledModel {
        name: model.name,
        digest: model.digest,
        size_mb: model.size / MB,
        family: model.details.family,
        parameter_size: model.details.parameter_size,
        quantization: model.details.quantization_level,
        context_length: model.details.context_length,
        modified_at: model.modified_at,
    }
}

pub fn parse_from_blob(modelfile: &str) -> Option<String> {
    modelfile
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("FROM "))
        .map(|path| path.trim().to_string())
        .filter(|path| path.contains("sha256-"))
}

pub fn parse_pull_line(line: &str) -> Result<Option<PullProgress>, OllamaError> {
    if line.is_empty() {
        return Ok(None);
    }
    let parsed: PullLine = serde_json::from_str(line).map_err(|e| OllamaError::Decode(e.to_string()))?;
    if let Some(error) = parsed.error {
        return Err(OllamaError::Http { status: 200, body: error });
    }
    Ok(Some(PullProgress { status: parsed.status, completed: parsed.completed, total: parsed.total }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_line_gives_blob_path() {
        let modelfile = "# comentario\nFROM C:\\Users\\x\\.ollama\\models\\blobs\\sha256-abc123\nPARAMETER num_ctx 32768\n";
        assert_eq!(parse_from_blob(modelfile).as_deref(), Some("C:\\Users\\x\\.ollama\\models\\blobs\\sha256-abc123"));
        assert_eq!(parse_from_blob("FROM qwen3.5:9b\n"), None);
        assert_eq!(parse_from_blob(""), None);
    }

    #[test]
    fn pull_lines_parse_progress_and_errors() {
        assert_eq!(parse_pull_line("").unwrap(), None);
        assert_eq!(
            parse_pull_line(r#"{"status":"downloading","completed":10,"total":100}"#).unwrap(),
            Some(PullProgress { status: "downloading".into(), completed: Some(10), total: Some(100) })
        );
        assert!(matches!(parse_pull_line(r#"{"error":"pull model manifest: file does not exist"}"#), Err(OllamaError::Http { .. })));
        assert!(matches!(parse_pull_line("no es json"), Err(OllamaError::Decode(_))));
    }
}
