use crate::config::Hook;
use crate::hooks::{run_hooks, HookResult};
use crate::ollama::OllamaClient;
use crate::procs::{kill_order, ProcessSource};
use serde::Serialize;
use std::path::Path;
use std::time::Duration;

const POLL_INTERVAL: Duration = Duration::from_millis(500);
const EXIT_WAIT: Duration = Duration::from_secs(3);
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PauseReport {
    pub unloaded: Vec<String>,
    pub killed: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResumeReport {
    pub launched: bool,
    pub version: String,
    pub hooks: Vec<HookResult>,
}

#[derive(Debug, Clone, PartialEq, Serialize, thiserror::Error)]
pub enum ControlError {
    #[error("Ollama no respondió en {0} s")]
    Timeout(u64),
    #[error("{0}")]
    Launch(String),
    #[error("quedaron procesos de Ollama vivos: {0:?}")]
    Survivors(Vec<u32>),
}

pub trait Launcher: Send + Sync {
    fn launch(&self, install_dir: &Path, env: &[(String, Option<String>)]) -> Result<(), String>;
}

pub struct OllamaLauncher;

impl Launcher for OllamaLauncher {
    fn launch(&self, install_dir: &Path, env: &[(String, Option<String>)]) -> Result<(), String> {
        let mut command = launch_command(install_dir);
        apply_launch_env(&mut command, env);
        command.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        command.spawn().map(|_| ()).map_err(|e| format!("no se pudo arrancar Ollama: {e}"))
    }
}

pub fn apply_launch_env(command: &mut std::process::Command, env: &[(String, Option<String>)]) {
    for (key, value) in env {
        match value {
            Some(value) => {
                command.env(key, value);
            }
            None => {
                command.env_remove(key);
            }
        }
    }
}

fn launch_command(install_dir: &Path) -> std::process::Command {
    let tray_app = install_dir.join("ollama app.exe");
    if tray_app.exists() {
        return std::process::Command::new(tray_app);
    }
    let binary = if cfg!(windows) { "ollama.exe" } else { "ollama" };
    let mut command = std::process::Command::new(install_dir.join(binary));
    command.arg("serve");
    command
}

pub async fn pause(client: &OllamaClient, procs: &dyn ProcessSource, install_dir: &Path) -> Result<PauseReport, ControlError> {
    let unloaded = unload_all(client).await;
    let killed: Vec<u32> = kill_order(&procs.list(), install_dir).into_iter().filter(|target| procs.kill(target)).map(|target| target.pid).collect();
    wait_for_exit(procs, install_dir).await?;
    Ok(PauseReport { unloaded, killed })
}

async fn unload_all(client: &OllamaClient) -> Vec<String> {
    let Ok(models) = client.loaded().await else { return Vec::new() };
    let mut unloaded = Vec::new();
    for model in models {
        if client.unload(&model.name).await.is_ok() {
            unloaded.push(model.name);
        }
    }
    unloaded
}

async fn wait_for_exit(procs: &dyn ProcessSource, install_dir: &Path) -> Result<(), ControlError> {
    let deadline = tokio::time::Instant::now() + EXIT_WAIT;
    loop {
        let survivors = kill_order(&procs.list(), install_dir);
        if survivors.is_empty() {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(ControlError::Survivors(survivors.into_iter().map(|p| p.pid).collect()));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

pub async fn resume(
    client: &OllamaClient,
    launcher: &dyn Launcher,
    install_dir: &Path,
    hooks: &[Hook],
    http: &reqwest::Client,
    env: &[(String, Option<String>)],
    timeout: Duration,
) -> Result<ResumeReport, ControlError> {
    let (launched, version) = tokio::time::timeout(timeout, resume_until_ready(client, launcher, install_dir, env, timeout))
        .await
        .map_err(|_| ControlError::Timeout(timeout.as_secs()))??;
    Ok(ResumeReport { launched, version, hooks: run_hooks(http, hooks).await })
}

async fn resume_until_ready(client: &OllamaClient, launcher: &dyn Launcher, install_dir: &Path, env: &[(String, Option<String>)], timeout: Duration) -> Result<(bool, String), ControlError> {
    match tokio::time::timeout(Duration::from_secs(2), client.version()).await {
        Ok(Ok(version)) => Ok((false, version)),
        _ => {
            launcher.launch(install_dir, env).map_err(ControlError::Launch)?;
            Ok((true, wait_ready(client, timeout).await?))
        }
    }
}

pub async fn wait_ready(client: &OllamaClient, timeout: Duration) -> Result<String, ControlError> {
    tokio::time::timeout(timeout, poll_until_ready(client)).await.map_err(|_| ControlError::Timeout(timeout.as_secs()))
}

async fn poll_until_ready(client: &OllamaClient) -> String {
    loop {
        if let Ok(version) = client.version().await {
            return version;
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}
