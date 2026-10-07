mod common;

use axum::{extract::State, http::StatusCode, routing::{get, post}, Json, Router};
use corral_lib::config::Hook;
use corral_lib::control::{pause, resume, ControlError, Launcher};
use corral_lib::hooks::run_hooks;
use corral_lib::ollama::OllamaClient;
use corral_lib::procs::{ProcInfo, ProcessSource};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const DIR: &str = r"C:\Ollama";

struct FakeProcs(Mutex<Vec<ProcInfo>>);

impl ProcessSource for FakeProcs {
    fn list(&self) -> Vec<ProcInfo> {
        self.0.lock().unwrap().clone()
    }
    fn kill(&self, target: &ProcInfo) -> bool {
        let mut procs = self.0.lock().unwrap();
        let before = procs.len();
        procs.retain(|p| p.pid != target.pid || p.start_time != target.start_time || p.exe != target.exe);
        procs.len() < before
    }
}

fn proc_at(pid: u32, name: &str, exe: &str, args: &[&str]) -> ProcInfo {
    ProcInfo { pid, start_time: 100, name: name.into(), exe: Some(PathBuf::from(exe)), cmd: args.iter().map(|s| s.to_string()).collect(), cpu_pct: 0.0, ram_mb: 0 }
}

#[tokio::test]
async fn pause_preserves_a_process_that_replaced_the_selected_pid() {
    struct ReplacedPid(FakeProcs);
    impl ProcessSource for ReplacedPid {
        fn list(&self) -> Vec<ProcInfo> {
            let selected = self.0.list();
            let mut live = self.0.0.lock().unwrap();
            live[0].start_time = 200;
            live[0].exe = Some(PathBuf::from(r"C:\foreign\llama-server.exe"));
            selected
        }
        fn kill(&self, target: &ProcInfo) -> bool { self.0.kill(target) }
    }
    let original = proc_at(3, "llama-server.exe", r"C:\Ollama\lib\llama-server.exe", &[]);
    let procs = ReplacedPid(FakeProcs(Mutex::new(vec![original])));
    let report = pause(&OllamaClient::new(&common::closed_port_url()), &procs, Path::new(DIR)).await.unwrap();
    assert!(report.killed.is_empty());
    let survivors = procs.0.list();
    assert_eq!(survivors.len(), 1);
    assert_eq!(survivors[0].start_time, 200);
}

fn ollama_tree() -> FakeProcs {
    FakeProcs(Mutex::new(vec![
        proc_at(3, "llama-server.exe", r"C:\Ollama\lib\llama-server.exe", &["llama-server.exe", "--model", "x"]),
        proc_at(2, "ollama.exe", r"C:\Ollama\ollama.exe", &["ollama.exe", "serve"]),
        proc_at(1, "ollama app.exe", r"C:\Ollama\ollama app.exe", &["ollama app.exe"]),
        proc_at(9, "llama-server.exe", r"C:\llama\llama-server.exe", &["llama-server.exe"]),
    ]))
}

struct FlagLauncher(Arc<AtomicBool>);

impl Launcher for FlagLauncher {
    fn launch(&self, _install_dir: &Path) -> Result<(), String> {
        self.0.store(true, Ordering::SeqCst);
        Ok(())
    }
}

async fn ollama_that_answers_when(up: Arc<AtomicBool>) -> String {
    let router = Router::new()
        .route("/api/version", get(|State(up): State<Arc<AtomicBool>>| async move {
            if up.load(Ordering::SeqCst) { (StatusCode::OK, Json(json!({"version": "0.35.1"}))) } else { (StatusCode::SERVICE_UNAVAILABLE, Json(json!({}))) }
        }))
        .route("/api/ps", get(|| async { Json(json!({"models": [{"name": "qwen3.5-mem:latest", "size": 1, "digest": "d"}]})) }))
        .route("/api/generate", post(|| async { StatusCode::OK }))
        .with_state(up);
    common::spawn(router).await
}

#[tokio::test]
async fn pause_unloads_then_kills_only_ollama_in_order() {
    let base = ollama_that_answers_when(Arc::new(AtomicBool::new(true))).await;
    let procs = ollama_tree();
    let report = pause(&OllamaClient::new(&base), &procs, Path::new(DIR)).await.unwrap();
    assert_eq!(report.unloaded, vec!["qwen3.5-mem:latest".to_string()]);
    assert_eq!(report.killed, vec![1, 2, 3]);
    assert_eq!(procs.list().iter().map(|p| p.pid).collect::<Vec<_>>(), vec![9]);
}

#[tokio::test]
async fn pause_with_ollama_already_down_is_a_noop_success() {
    let procs = FakeProcs(Mutex::new(vec![]));
    let report = pause(&OllamaClient::new(&common::closed_port_url()), &procs, Path::new(DIR)).await.unwrap();
    assert!(report.unloaded.is_empty() && report.killed.is_empty());
}

#[tokio::test]
async fn resume_launches_waits_and_runs_hooks() {
    let up = Arc::new(AtomicBool::new(false));
    let base = ollama_that_answers_when(up.clone()).await;
    let seen: Arc<Mutex<Vec<Value>>> = Arc::default();
    let hook_router = Router::new()
        .route("/api/processing", post(|State(seen): State<Arc<Mutex<Vec<Value>>>>, Json(body): Json<Value>| async move {
            seen.lock().unwrap().push(body);
            StatusCode::OK
        }))
        .with_state(seen.clone());
    let hook_base = common::spawn(hook_router).await;
    let hooks = vec![Hook { name: "claude-mem".into(), url: format!("{hook_base}/api/processing"), body: r#"{"isProcessing":false}"#.into(), enabled: true }];

    let report = resume(&OllamaClient::new(&base), &FlagLauncher(up), Path::new(DIR), &hooks, &reqwest::Client::new(), Duration::from_secs(5)).await.unwrap();

    assert!(report.launched);
    assert_eq!(report.version, "0.35.1");
    assert!(report.hooks[0].ok);
    assert_eq!(seen.lock().unwrap()[0], json!({"isProcessing": false}));
}

#[tokio::test]
async fn resume_when_already_running_does_not_launch_again() {
    let base = ollama_that_answers_when(Arc::new(AtomicBool::new(true))).await;
    let launched = Arc::new(AtomicBool::new(false));
    let report = resume(&OllamaClient::new(&base), &FlagLauncher(launched.clone()), Path::new(DIR), &[], &reqwest::Client::new(), Duration::from_secs(5)).await.unwrap();
    assert!(!report.launched);
    assert!(!launched.load(Ordering::SeqCst));
}

#[tokio::test]
async fn resume_times_out_when_ollama_never_answers() {
    struct NoopLauncher;
    impl Launcher for NoopLauncher {
        fn launch(&self, _: &Path) -> Result<(), String> { Ok(()) }
    }
    let base = ollama_that_answers_when(Arc::new(AtomicBool::new(false))).await;
    let result = resume(&OllamaClient::new(&base), &NoopLauncher, Path::new(DIR), &[], &reqwest::Client::new(), Duration::from_secs(1)).await;
    assert_eq!(result.unwrap_err(), ControlError::Timeout(1));
}

#[tokio::test]
async fn hooks_skip_disabled_and_report_unreachable() {
    let hooks = vec![
        Hook { name: "apagado".into(), url: "http://127.0.0.1:1/x".into(), body: "{}".into(), enabled: false },
        Hook { name: "caido".into(), url: format!("{}/x", common::closed_port_url()), body: "{}".into(), enabled: true },
    ];
    let results = run_hooks(&reqwest::Client::new(), &hooks).await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "caido");
    assert!(!results[0].ok);
}
