use crate::config::Hook;
use serde::Serialize;
use std::time::Duration;

const HOOK_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HookResult {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

pub async fn run_hooks(http: &reqwest::Client, hooks: &[Hook]) -> Vec<HookResult> {
    let mut results = Vec::new();
    for hook in hooks.iter().filter(|h| h.enabled) {
        results.push(run_hook(http, hook).await);
    }
    results
}

async fn run_hook(http: &reqwest::Client, hook: &Hook) -> HookResult {
    let sent = http
        .post(&hook.url)
        .header("content-type", "application/json")
        .body(hook.body.clone())
        .timeout(HOOK_TIMEOUT)
        .send()
        .await;
    let (ok, detail) = match sent {
        Ok(response) => (response.status().is_success(), format!("HTTP {}", response.status().as_u16())),
        Err(error) => (false, error.to_string()),
    };
    HookResult { name: hook.name.clone(), ok, detail }
}
