use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Field<T> {
    Ok(T),
    Unavailable(String),
}

impl<T> Field<T> {
    pub fn from_result(result: Result<T, String>) -> Self {
        match result {
            Ok(value) => Field::Ok(value),
            Err(reason) => Field::Unavailable(reason),
        }
    }

    pub fn as_ok(&self) -> Option<&T> {
        match self {
            Field::Ok(value) => Some(value),
            Field::Unavailable(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PausedBy {
    User,
    Rule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OllamaState {
    Running,
    Paused { by: PausedBy },
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TrayStatus {
    Running,
    Spilling,
    Paused,
    Down,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Adapter {
    pub luid: u64,
    pub name: String,
    pub total_mb: u64,
    pub used_mb: Option<u64>,
    pub ollama_mb: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RunnerGpu {
    pub luid: u64,
    pub dedicated_mb: Option<u64>,
    pub shared_mb: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Runner {
    pub pid: u32,
    pub model: Option<String>,
    pub gpus: Vec<RunnerGpu>,
    pub dedicated_mb: Option<u64>,
    pub shared_mb: Option<u64>,
    pub cpu_pct: f32,
    pub ram_mb: u64,
    pub spilling: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LoadedModel {
    pub name: String,
    pub digest: String,
    pub size_mb: u64,
    pub vram_mb: u64,
    pub context_length: u64,
    pub expires_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Snapshot {
    pub taken_at_ms: u64,
    pub state: OllamaState,
    pub status: TrayStatus,
    pub version: Option<String>,
    pub adapters: Field<Vec<Adapter>>,
    pub runners: Field<Vec<Runner>>,
    pub loaded: Field<Vec<LoadedModel>>,
}

pub fn is_spilling(shared_mb: Option<u64>, floor_mb: u64) -> bool {
    shared_mb.is_some_and(|shared| shared > floor_mb)
}

pub fn ollama_state(version_ok: bool, paused_by: Option<PausedBy>) -> OllamaState {
    match (version_ok, paused_by) {
        (true, _) => OllamaState::Running,
        (false, Some(by)) => OllamaState::Paused { by },
        (false, None) => OllamaState::Down,
    }
}

pub fn tray_status(state: OllamaState, runners: &Field<Vec<Runner>>) -> TrayStatus {
    match state {
        OllamaState::Down => TrayStatus::Down,
        OllamaState::Paused { .. } => TrayStatus::Paused,
        OllamaState::Running if any_spilling(runners) => TrayStatus::Spilling,
        OllamaState::Running => TrayStatus::Running,
    }
}

fn any_spilling(runners: &Field<Vec<Runner>>) -> bool {
    runners.as_ok().is_some_and(|list| list.iter().any(|r| r.spilling))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn runner(spilling: bool) -> Runner {
        Runner { pid: 1, model: None, gpus: vec![], dedicated_mb: Some(100), shared_mb: None, cpu_pct: 0.0, ram_mb: 10, spilling }
    }

    #[test]
    fn field_serializes_with_kind_and_value() {
        assert_eq!(serde_json::to_value(Field::Ok(3)).unwrap(), json!({"kind": "ok", "value": 3}));
        assert_eq!(serde_json::to_value(Field::<u8>::Unavailable("x".into())).unwrap(), json!({"kind": "unavailable", "value": "x"}));
    }

    #[test]
    fn state_serializes_as_tagged_object() {
        assert_eq!(serde_json::to_value(OllamaState::Paused { by: PausedBy::User }).unwrap(), json!({"kind": "paused", "by": "user"}));
        assert_eq!(serde_json::to_value(OllamaState::Down).unwrap(), json!({"kind": "down"}));
        assert_eq!(serde_json::to_value(TrayStatus::Spilling).unwrap(), json!("spilling"));
    }

    #[test]
    fn spill_needs_more_than_the_floor() {
        assert!(!is_spilling(None, 64));
        assert!(!is_spilling(Some(64), 64));
        assert!(is_spilling(Some(65), 64));
    }

    #[test]
    fn state_prefers_running_when_ollama_answers() {
        assert_eq!(ollama_state(true, Some(PausedBy::User)), OllamaState::Running);
        assert_eq!(ollama_state(false, Some(PausedBy::Rule)), OllamaState::Paused { by: PausedBy::Rule });
        assert_eq!(ollama_state(false, None), OllamaState::Down);
    }

    #[test]
    fn tray_status_derives_spilling_only_while_running() {
        let spilling = Field::Ok(vec![runner(false), runner(true)]);
        assert_eq!(tray_status(OllamaState::Running, &spilling), TrayStatus::Spilling);
        assert_eq!(tray_status(OllamaState::Running, &Field::Ok(vec![runner(false)])), TrayStatus::Running);
        assert_eq!(tray_status(OllamaState::Running, &Field::Unavailable("x".into())), TrayStatus::Running);
        assert_eq!(tray_status(OllamaState::Paused { by: PausedBy::User }, &spilling), TrayStatus::Paused);
        assert_eq!(tray_status(OllamaState::Down, &spilling), TrayStatus::Down);
    }
}
