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
    fn launch(&self, install_dir: &Path) -> Result<(), String>;
}

pub struct OllamaLauncher;

impl Launcher for OllamaLauncher {
    fn launch(&self, install_dir: &Path) -> Result<(), String> {
        let mut command = launch_command(install_dir);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        command.spawn().map(|_| ()).map_err(|e| format!("no se pudo arrancar Ollama: {e}"))
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
    let killed: Vec<u32> = kill_order(&procs.list(), install_dir).into_iter().filter(|pid| procs.kill(*pid)).collect();
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
            return Err(ControlError::Survivors(survivors));
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
    timeout: Duration,
) -> Result<ResumeReport, ControlError> {
    let (launched, version) = match client.version().await {
        Ok(version) => (false, version),
        Err(_) => {
            launcher.launch(install_dir).map_err(ControlError::Launch)?;
            (true, wait_ready(client, timeout).await?)
        }
    };
    Ok(ResumeReport { launched, version, hooks: run_hooks(http, hooks).await })
}

pub async fn wait_ready(client: &OllamaClient, timeout: Duration) -> Result<String, ControlError> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if let Ok(version) = client.version().await {
            return Ok(version);
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(ControlError::Timeout(timeout.as_secs()));
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}
