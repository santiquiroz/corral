use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hook {
    pub name: String,
    pub url: String,
    pub body: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub ollama_url: String,
    pub ollama_install_dir: PathBuf,
    pub poll_panel_secs: u64,
    pub poll_tray_secs: u64,
    pub spill_floor_mb: u64,
    pub resume_timeout_secs: u64,
    pub load_keep_alive: String,
    pub hooks: Vec<Hook>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ollama_url: normalize_ollama_host(std::env::var("OLLAMA_HOST").ok().as_deref()),
            ollama_install_dir: default_install_dir(),
            poll_panel_secs: 2,
            poll_tray_secs: 10,
            spill_floor_mb: 64,
            resume_timeout_secs: 30,
            load_keep_alive: "30m".into(),
            hooks: vec![claude_mem_hook()],
        }
    }
}

pub fn claude_mem_hook() -> Hook {
    Hook {
        name: "claude-mem".into(),
        url: "http://127.0.0.1:37777/api/processing".into(),
        body: r#"{"isProcessing":false}"#.into(),
        enabled: true,
    }
}

pub fn normalize_ollama_host(raw: Option<&str>) -> String {
    let raw = raw.map(str::trim).filter(|s| !s.is_empty()).unwrap_or("127.0.0.1:11434");
    let (scheme, rest) = split_scheme(raw);
    let rest = rest.trim_end_matches('/');
    // 0.0.0.0 es una dirección de escucha, no un destino
    let rest = rest.strip_prefix("0.0.0.0").map(|tail| format!("127.0.0.1{tail}")).unwrap_or_else(|| rest.to_string());
    let rest = if rest.starts_with(':') { format!("127.0.0.1{rest}") } else { rest };
    let with_port = if has_port(&rest) { rest } else { format!("{rest}:{}", default_port(scheme)) };
    format!("{scheme}://{with_port}")
}

fn split_scheme(raw: &str) -> (&str, &str) {
    match raw.split_once("://") {
        Some((scheme, rest)) => (scheme, rest),
        None => ("http", raw),
    }
}

fn has_port(host: &str) -> bool {
    host.rsplit_once(':').is_some_and(|(_, port)| !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()))
}

fn default_port(scheme: &str) -> u16 {
    if scheme == "https" { 443 } else { 11434 }
}

pub fn default_install_dir() -> PathBuf {
    match std::env::var_os("LOCALAPPDATA") {
        Some(local) => PathBuf::from(local).join("Programs").join("Ollama"),
        None => PathBuf::from("/usr/local/lib/ollama"),
    }
}

pub fn config_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("corral").join("config.toml")
}

pub fn load_or_create(path: &Path) -> Result<Config, String> {
    if !path.exists() {
        let config = Config::default();
        save(path, &config)?;
        return Ok(config);
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("no se pudo leer {}: {e}", path.display()))?;
    let config = toml::from_str(&text).map_err(|e| format!("config inválida en {}: {e}", path.display()))?;
    validate(config)
}

pub fn save(path: &Path, config: &Config) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("no se pudo crear {}: {e}", dir.display()))?;
    }
    let text = toml::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| format!("no se pudo escribir {}: {e}", path.display()))
}

pub fn validate(config: Config) -> Result<Config, String> {
    if !matches!(config.load_keep_alive.as_str(), "30m" | "1h" | "-1") {
        return Err("load_keep_alive debe ser 30m, 1h o -1".into());
    }
    if config.poll_panel_secs == 0 || config.poll_tray_secs == 0 {
        return Err("las cadencias deben ser de al menos 1 segundo".into());
    }
    if config.resume_timeout_secs == 0 {
        return Err("el tope de reanudación debe ser de al menos 1 segundo".into());
    }
    Ok(Config { ollama_url: normalize_ollama_host(Some(&config.ollama_url)), ..config })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_forms_resolve_to_connectable_urls() {
        let cases = [
            (None, "http://127.0.0.1:11434"),
            (Some(""), "http://127.0.0.1:11434"),
            (Some("0.0.0.0"), "http://127.0.0.1:11434"),
            (Some("0.0.0.0:11500"), "http://127.0.0.1:11500"),
            (Some(":11434"), "http://127.0.0.1:11434"),
            (Some("192.168.1.5"), "http://192.168.1.5:11434"),
            (Some("http://localhost:8080/"), "http://localhost:8080"),
            (Some("https://ollama.example.com"), "https://ollama.example.com:443"),
            (Some("[::1]"), "http://[::1]:11434"),
        ];
        for (raw, expected) in cases {
            assert_eq!(normalize_ollama_host(raw), expected, "entrada {raw:?}");
        }
    }

    #[test]
    fn defaults_include_claude_mem_hook() {
        let config = Config::default();
        assert_eq!(config.poll_panel_secs, 2);
        assert_eq!(config.poll_tray_secs, 10);
        assert_eq!(config.spill_floor_mb, 64);
        assert_eq!(config.resume_timeout_secs, 30);
        assert_eq!(config.hooks, vec![claude_mem_hook()]);
        assert_eq!(claude_mem_hook().url, "http://127.0.0.1:37777/api/processing");
        assert_eq!(claude_mem_hook().body, r#"{"isProcessing":false}"#);
    }

    #[test]
    fn creates_file_on_first_load_and_round_trips() {
        let dir = std::env::temp_dir().join(format!("corral-cfg-{}", std::process::id()));
        let path = dir.join("config.toml");
        let _ = std::fs::remove_dir_all(&dir);
        let created = load_or_create(&path).unwrap();
        assert!(path.exists());
        let changed = Config { spill_floor_mb: 128, ..created };
        save(&path, &changed).unwrap();
        assert_eq!(load_or_create(&path).unwrap(), changed);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn missing_fields_take_defaults_and_garbage_is_an_error() {
        let partial: Config = toml::from_str("spill_floor_mb = 10").unwrap();
        assert_eq!(partial.spill_floor_mb, 10);
        assert_eq!(partial.poll_tray_secs, 10);
        assert!(toml::from_str::<Config>("spill_floor_mb = \"mucho\"").is_err());
    }

    #[test]
    fn validate_rejects_zero_cadences_and_normalizes_url() {
        assert!(validate(Config { poll_panel_secs: 0, ..Config::default() }).is_err());
        assert!(validate(Config { resume_timeout_secs: 0, ..Config::default() }).is_err());
        let fixed = validate(Config { ollama_url: "0.0.0.0".into(), ..Config::default() }).unwrap();
        assert_eq!(fixed.ollama_url, "http://127.0.0.1:11434");
    }

    #[test]
    fn load_rejects_a_config_file_with_zero_panel_cadence() {
        let dir = std::env::temp_dir().join(format!("corral-invalid-cfg-{}", std::process::id()));
        let path = dir.join("config.toml");
        save(&path, &Config { poll_panel_secs: 0, ..Config::default() }).unwrap();
        let result = load_or_create(&path);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(result.unwrap_err(), "las cadencias deben ser de al menos 1 segundo");
    }

    #[test]
    fn valor_predeterminado_de_load_keep_alive_es_30m() {
        assert_eq!(Config::default().load_keep_alive, "30m");
    }

    #[test]
    fn configuracion_legacy_sin_load_keep_alive_usa_30m() {
        let partial: Config = toml::from_str("spill_floor_mb = 10").unwrap();
        assert_eq!(partial.load_keep_alive, "30m");
    }

    #[test]
    fn validate_acepta_valores_validos_de_load_keep_alive() {
        for value in ["30m", "1h", "-1"] {
            assert!(
                validate(Config { load_keep_alive: value.into(), ..Config::default() }).is_ok(),
                "valor {value:?}"
            );
        }
    }

    #[test]
    fn validate_rechaza_valores_invalidos_de_load_keep_alive() {
        for value in ["", "0", "15m", "arbitrario"] {
            let error = validate(Config { load_keep_alive: value.into(), ..Config::default() }).unwrap_err();
            assert!(error.contains("load_keep_alive"), "valor {value:?}: {error}");
        }
    }
}
