use crate::collector::BlobCache;
use crate::config::Config;
use crate::control::{Launcher, OllamaLauncher};
use crate::gpu::{default_probe, GpuProbe};
use crate::ollama::OllamaClient;
use crate::procs::{ProcessSource, SysinfoSource};
use crate::snapshot::{OllamaState, PausedBy, Snapshot};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct AppState {
    pub config_path: PathBuf,
    pub config: tokio::sync::RwLock<Config>,
    pub client: tokio::sync::RwLock<OllamaClient>,
    pub http: reqwest::Client,
    pub gpu: Box<dyn GpuProbe>,
    pub procs: Box<dyn ProcessSource>,
    pub launcher: Box<dyn Launcher>,
    pub lifecycle: tokio::sync::Mutex<()>,
    pub gpu_profile_apply: tokio::sync::Mutex<()>,
    pub paused_by: Mutex<Option<PausedBy>>,
    pub notices: Mutex<Vec<String>>,
    pub latest: Mutex<Option<Snapshot>>,
    pub blob_cache: tokio::sync::Mutex<BlobCache>,
    pub wake: tokio::sync::Notify,
}

impl AppState {
    pub fn new(config_path: PathBuf, config: Config) -> Self {
        Self {
            config_path,
            client: tokio::sync::RwLock::new(OllamaClient::new(&config.ollama_url)),
            config: tokio::sync::RwLock::new(config),
            http: reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).build().expect("cliente HTTP de hooks"),
            gpu: default_probe(),
            procs: Box::new(SysinfoSource::new()),
            launcher: Box::new(OllamaLauncher),
            lifecycle: tokio::sync::Mutex::new(()),
            gpu_profile_apply: tokio::sync::Mutex::new(()),
            paused_by: Mutex::new(None),
            notices: Mutex::new(Vec::new()),
            latest: Mutex::new(None),
            blob_cache: tokio::sync::Mutex::new(BlobCache::default()),
            wake: tokio::sync::Notify::new(),
        }
    }

    pub fn latest_state(&self) -> Option<OllamaState> {
        self.latest.lock().unwrap().as_ref().map(|s| s.state)
    }
}
