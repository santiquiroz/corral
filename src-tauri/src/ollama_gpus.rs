use crate::config::GpuProfile;
use serde::Serialize;
use std::collections::HashMap;

const MANAGED_VARS: [&str; 6] = [
    "OLLAMA_SCHED_SPREAD", "HIP_VISIBLE_DEVICES", "ROCR_VISIBLE_DEVICES",
    "CUDA_VISIBLE_DEVICES", "GGML_VK_VISIBLE_DEVICES", "OLLAMA_IGPU_ENABLE",
];

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OllamaGpu {
    pub id: String,
    pub filter_id: String,
    pub library: String,
    pub description: String,
    pub kind: String,
    pub total_mb: Option<u64>,
    pub dropped: bool,
}

fn log_fields(line: &str) -> HashMap<&str, &str> {
    let mut fields = HashMap::new();
    let mut remaining = line.trim();
    while let Some((key, value)) = remaining.split_once('=') {
        let (value, rest) = field_value(value);
        fields.insert(key.trim(), value);
        remaining = rest.trim_start();
    }
    fields
}

fn field_value(value: &str) -> (&str, &str) {
    if let Some(quoted) = value.strip_prefix('"') {
        return quoted.split_once('"').unwrap_or((quoted, ""));
    }
    value.split_once(char::is_whitespace).unwrap_or((value, ""))
}

fn discovery_line(line: &str) -> Option<(i64, HashMap<&str, &str>)> {
    let fields = log_fields(line);
    let message = *fields.get("msg")?;
    let dropped = message.starts_with("dropping integrated GPU");
    if message != "inference compute" && !dropped {
        return None;
    }
    let time = chrono::DateTime::parse_from_rfc3339(fields.get("time")?).ok()?.timestamp_millis();
    Some((time, fields))
}

fn gpu_from_fields(fields: HashMap<&str, &str>) -> Option<OllamaGpu> {
    let dropped = fields.get("msg")?.starts_with("dropping integrated GPU");
    let id = fields.get("id")?.to_string();
    let gpu = OllamaGpu {
        filter_id: fields.get("filter_id").map(|value| value.to_string()).unwrap_or_else(|| id.clone()),
        id,
        library: fields.get("library")?.to_string(),
        description: fields.get("description").or_else(|| fields.get("name")).copied().unwrap_or("GPU").into(),
        kind: if dropped { "integrated".into() } else { fields.get("type").copied().unwrap_or("discrete").into() },
        total_mb: fields.get("total").and_then(|total| parse_total_mb(total)),
        dropped,
    };
    Some(gpu)
}

fn parse_total_mb(total: &str) -> Option<u64> {
    let (amount, unit) = total.split_once(' ')?;
    let amount = amount.parse::<f64>().ok()?;
    let multiplier = match unit { "GiB" => 1024.0, "MiB" => 1.0, _ => return None };
    let mb = amount * multiplier;
    (mb.is_finite() && mb >= 0.0 && mb < u64::MAX as f64).then_some(mb as u64)
}

pub fn parse_ollama_gpus(log: &str) -> Vec<OllamaGpu> {
    let lines: Vec<_> = log.lines().filter_map(discovery_line).collect();
    let Some(latest) = lines.iter().map(|(time, _)| *time).max() else { return Vec::new() };
    lines.into_iter().filter(|(time, _)| latest - time <= 10_000).filter_map(|(_, fields)| gpu_from_fields(fields)).collect()
}

pub fn list_ollama_gpus() -> Vec<OllamaGpu> {
    let Some(local) = std::env::var_os("LOCALAPPDATA") else { return Vec::new() };
    let path = std::path::PathBuf::from(local).join("Ollama").join("server.log");
    std::fs::read_to_string(path).map(|log| parse_ollama_gpus(&log)).unwrap_or_default()
}

fn visibility_variable(library: &str, os: &str) -> Option<&'static str> {
    match library {
        "ROCm" if os == "windows" => Some("HIP_VISIBLE_DEVICES"),
        "ROCm" => Some("ROCR_VISIBLE_DEVICES"),
        "CUDA" => Some("CUDA_VISIBLE_DEVICES"),
        "Vulkan" => Some("GGML_VK_VISIBLE_DEVICES"),
        _ => None,
    }
}

pub fn gpu_profile_env(profile: &GpuProfile, igpu_enabled: bool, os: &str) -> Vec<(String, Option<String>)> {
    let selected = match profile {
        GpuProfile::Auto => None,
        GpuProfile::Spread => Some(("OLLAMA_SCHED_SPREAD", "1")),
        GpuProfile::Single { library, filter_id } => visibility_variable(library, os).map(|name| (name, filter_id.as_str())),
    };
    MANAGED_VARS.into_iter().map(|name| {
        let value = if name == "OLLAMA_IGPU_ENABLE" && igpu_enabled { Some("1") }
            else { selected.filter(|(variable, _)| *variable == name).map(|(_, value)| value) };
        (name.into(), value.map(str::to_string))
    }).collect()
}

pub fn validate_gpu_profile(profile: &GpuProfile, gpus: &[OllamaGpu]) -> Result<(), String> {
    let GpuProfile::Single { library, filter_id } = profile else { return Ok(()) };
    validate_single_profile(library, filter_id)?;
    if !gpus.iter().any(|gpu| gpu.library == *library && gpu.filter_id == *filter_id) {
        return Err("La GPU seleccionada no está presente en Ollama".into());
    }
    Ok(())
}

pub fn validate_single_profile(library: &str, filter_id: &str) -> Result<(), String> {
    if visibility_variable(library, std::env::consts::OS).is_none() {
        return Err("Librería de GPU no compatible".into());
    }
    if filter_id.trim().is_empty() {
        return Err("La GPU requiere filter_id".into());
    }
    Ok(())
}
