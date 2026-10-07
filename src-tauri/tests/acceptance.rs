#![cfg(windows)]

use corral_lib::config::{claude_mem_hook, default_install_dir};
use corral_lib::control::{pause, resume, OllamaLauncher};
use corral_lib::gpu::{default_probe, GpuProbe};
use corral_lib::ollama::OllamaClient;
use corral_lib::procs::SysinfoSource;
use std::time::{Duration, Instant};

fn ollama_vram_mb(probe: &dyn GpuProbe, pids: &[u32]) -> u64 {
    let reading = probe.read().unwrap();
    reading.processes.iter().filter(|p| pids.contains(&p.pid)).filter_map(|p| p.dedicated_mb).sum()
}

#[tokio::test]
#[ignore = "requiere Ollama con un modelo cargado y claude-mem en el PC de referencia"]
async fn pause_frees_vram_under_five_seconds_and_resume_wakes_claude_mem() {
    let client = OllamaClient::new("http://127.0.0.1:11434");
    let procs = SysinfoSource::new();
    let probe = default_probe();
    let install_dir = default_install_dir();
    let runner_pids: Vec<u32> = corral_lib::procs::kill_order(&corral_lib::procs::ProcessSource::list(&procs), &install_dir).into_iter().map(|p| p.pid).collect();
    assert!(ollama_vram_mb(probe.as_ref(), &runner_pids) > 0, "debe haber un modelo cargado antes de la prueba");

    let started = Instant::now();
    pause(&client, &procs, &install_dir).await.unwrap();
    let elapsed = started.elapsed();
    assert_eq!(ollama_vram_mb(probe.as_ref(), &runner_pids), 0);
    assert!(elapsed < Duration::from_secs(5), "la pausa tardó {elapsed:?}");

    let report = resume(&client, &OllamaLauncher, &install_dir, &[claude_mem_hook()], &reqwest::Client::new(), &[], Duration::from_secs(30)).await.unwrap();
    assert!(report.launched);
    assert_eq!(report.hooks.len(), 1, "debe recibirse exactamente el resultado del aviso claude-mem");
    assert!(report.hooks[0].ok, "claude-mem no recibió el aviso: {}", report.hooks[0].detail);
}
