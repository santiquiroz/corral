use crate::gpu::{pdh_names::sanitize_mb, GpuProbe, GpuReading, ProcessGpuMem};
use crate::ollama::OllamaClient;
use crate::procs::{blob_hash, classify, model_arg, OllamaRole, ProcInfo, ProcessSource};
use crate::snapshot::{is_spilling, ollama_state, tray_status, Adapter, Field, LoadedModel, PausedBy, Runner, Snapshot};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct CollectInputs {
    pub now_ms: u64,
    pub version: Result<String, String>,
    pub loaded: Result<Vec<LoadedModel>, String>,
    pub model_hashes: HashMap<String, String>,
    pub gpu: Result<GpuReading, String>,
    pub procs: Vec<ProcInfo>,
    pub install_dir: PathBuf,
    pub paused_by: Option<PausedBy>,
    pub spill_floor_mb: u64,
}

pub fn assemble(inputs: &CollectInputs) -> Snapshot {
    let state = ollama_state(inputs.version.is_ok(), inputs.paused_by);
    let runners = Field::Ok(build_runners(inputs));
    let adapters = build_adapters(&runners, &inputs.gpu);
    Snapshot {
        taken_at_ms: inputs.now_ms,
        status: tray_status(state, &runners),
        state,
        version: inputs.version.clone().ok(),
        adapters,
        runners,
        loaded: Field::from_result(inputs.loaded.clone()),
    }
}

fn build_runners(inputs: &CollectInputs) -> Vec<Runner> {
    inputs
        .procs
        .iter()
        .filter(|p| classify(p, &inputs.install_dir) == Some(OllamaRole::Runner))
        .map(|p| build_runner(p, inputs))
        .collect()
}

fn build_runner(process: &ProcInfo, inputs: &CollectInputs) -> Runner {
    let memory = inputs.gpu.as_ref().ok().and_then(|g| primary_gpu_memory(g, process.pid));
    let shared_mb = memory.and_then(|m| m.shared_mb);
    Runner {
        pid: process.pid,
        model: model_arg(&process.cmd).and_then(blob_hash).and_then(|h| models_for_hash(&h, &inputs.model_hashes)),
        luid: memory.map(|m| m.luid),
        dedicated_mb: memory.and_then(|m| m.dedicated_mb),
        shared_mb,
        cpu_pct: process.cpu_pct,
        ram_mb: process.ram_mb,
        spilling: is_spilling(shared_mb, inputs.spill_floor_mb),
    }
}

fn primary_gpu_memory(reading: &GpuReading, pid: u32) -> Option<&ProcessGpuMem> {
    reading.processes.iter().filter(|m| m.pid == pid).max_by_key(|m| m.dedicated_mb.unwrap_or(0))
}

fn models_for_hash(hash: &str, model_hashes: &HashMap<String, String>) -> Option<String> {
    let mut names: Vec<&str> = model_hashes.iter().filter(|(_, h)| h.as_str() == hash).map(|(name, _)| name.as_str()).collect();
    names.sort_unstable();
    (!names.is_empty()).then(|| names.join(" / "))
}

fn build_adapters(runners: &Field<Vec<Runner>>, gpu: &Result<GpuReading, String>) -> Field<Vec<Adapter>> {
    let reading = match gpu {
        Ok(reading) => reading,
        Err(reason) => return Field::Unavailable(reason.clone()),
    };
    let runners = runners.as_ok().map(Vec::as_slice).unwrap_or(&[]);
    Field::Ok(
        reading
            .adapters
            .iter()
            .filter(|a| is_visible_adapter(reading, a.luid))
            .map(|a| Adapter {
                luid: a.luid,
                name: a.name.clone(),
                total_mb: a.total_mb,
                used_mb: reading.adapter_used_mb.get(&a.luid).and_then(|used| sanitize_mb(*used, a.total_mb)),
                ollama_mb: runners.iter().filter(|r| r.luid == Some(a.luid)).filter_map(|r| r.dedicated_mb).sum(),
            })
            .collect(),
    )
}

fn is_visible_adapter(reading: &GpuReading, luid: u64) -> bool {
    // DXGI puede enumerar adaptadores fantasma que los contadores PDH no conocen
    reading.adapter_used_mb.is_empty() || reading.adapter_used_mb.contains_key(&luid)
}

#[derive(Default)]
pub struct BlobCache(HashMap<String, (String, String)>);

pub async fn resolve_hashes(client: &OllamaClient, loaded: &[LoadedModel], cache: &mut BlobCache) -> HashMap<String, String> {
    let mut hashes = HashMap::new();
    for model in loaded {
        if let Some(hash) = cached_or_fetched_hash(client, model, cache).await {
            hashes.insert(model.name.clone(), hash);
        }
    }
    hashes
}

async fn cached_or_fetched_hash(client: &OllamaClient, model: &LoadedModel, cache: &mut BlobCache) -> Option<String> {
    if let Some((digest, hash)) = cache.0.get(&model.name) {
        if *digest == model.digest {
            return Some(hash.clone());
        }
    }
    let hash = client.blob_path(&model.name).await.ok().flatten().as_deref().and_then(blob_hash)?;
    cache.0.insert(model.name.clone(), (model.digest.clone(), hash.clone()));
    Some(hash)
}

#[allow(clippy::too_many_arguments)]
pub async fn gather(
    client: &OllamaClient,
    gpu: &dyn GpuProbe,
    procs: &dyn ProcessSource,
    cache: &mut BlobCache,
    install_dir: &Path,
    paused_by: Option<PausedBy>,
    spill_floor_mb: u64,
    now_ms: u64,
) -> CollectInputs {
    let version = client.version().await.map_err(|e| e.to_string());
    let loaded = match &version {
        Ok(_) => client.loaded().await.map_err(|e| e.to_string()),
        Err(_) => Err("Ollama no está corriendo".to_string()),
    };
    let model_hashes = match &loaded {
        Ok(models) => resolve_hashes(client, models, cache).await,
        Err(_) => HashMap::new(),
    };
    CollectInputs {
        now_ms,
        version,
        loaded,
        model_hashes,
        gpu: gpu.read(),
        procs: procs.list(),
        install_dir: install_dir.to_path_buf(),
        paused_by,
        spill_floor_mb,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gpu::{AdapterInfo, ProcessGpuMem};
    use crate::snapshot::{OllamaState, TrayStatus};

    const DIR: &str = r"C:\Ollama";
    const RX: u64 = 0x1685a;

    fn runner_proc(pid: u32, hash: &str) -> ProcInfo {
        ProcInfo {
            pid,
            name: "llama-server.exe".into(),
            exe: Some(PathBuf::from(format!(r"{DIR}\lib\llama-server.exe"))),
            cmd: vec!["llama-server.exe".into(), "--model".into(), format!(r"C:\m\blobs\sha256-{hash}")],
            cpu_pct: 3.0,
            ram_mb: 900,
        }
    }

    fn reading(dedicated: Option<u64>, shared: u64) -> GpuReading {
        GpuReading {
            adapters: vec![AdapterInfo { luid: RX, name: "AMD Radeon RX 7800 XT".into(), total_mb: 16368 }],
            adapter_used_mb: HashMap::from([(RX, 14691)]),
            processes: vec![ProcessGpuMem { pid: 7, luid: RX, dedicated_mb: dedicated, shared_mb: Some(shared) }],
        }
    }

    fn inputs() -> CollectInputs {
        CollectInputs {
            now_ms: 1,
            version: Ok("0.35.1".into()),
            loaded: Ok(vec![]),
            model_hashes: HashMap::from([("qwen3.5-mem:latest".into(), "dec52a".into())]),
            gpu: Ok(reading(Some(7819), 0)),
            procs: vec![runner_proc(7, "dec52a")],
            install_dir: PathBuf::from(DIR),
            paused_by: None,
            spill_floor_mb: 64,
        }
    }

    #[test]
    fn runner_gets_model_gpu_and_adapter_totals() {
        let snap = assemble(&inputs());
        assert_eq!(snap.state, OllamaState::Running);
        assert_eq!(snap.status, TrayStatus::Running);
        let runner = &snap.runners.as_ok().unwrap()[0];
        assert_eq!(runner.model.as_deref(), Some("qwen3.5-mem:latest"));
        assert_eq!((runner.luid, runner.dedicated_mb, runner.spilling), (Some(RX), Some(7819), false));
        let adapter = &snap.adapters.as_ok().unwrap()[0];
        assert_eq!((adapter.used_mb, adapter.ollama_mb), (Some(14691), 7819));
    }

    #[test]
    fn shared_memory_above_floor_marks_spilling() {
        let snap = assemble(&CollectInputs { gpu: Ok(reading(Some(7819), 729)), ..inputs() });
        assert!(snap.runners.as_ok().unwrap()[0].spilling);
        assert_eq!(snap.status, TrayStatus::Spilling);
    }

    #[test]
    fn models_sharing_a_blob_are_all_named() {
        let hashes = HashMap::from([("qwen3.5:9b".into(), "dec52a".into()), ("qwen3.5-mem:latest".into(), "dec52a".into())]);
        let snap = assemble(&CollectInputs { model_hashes: hashes, ..inputs() });
        assert_eq!(snap.runners.as_ok().unwrap()[0].model.as_deref(), Some("qwen3.5-mem:latest / qwen3.5:9b"));
    }

    #[test]
    fn gpu_failure_keeps_runners_and_marks_adapters_unavailable() {
        let snap = assemble(&CollectInputs { gpu: Err("PDH falló".into()), ..inputs() });
        assert_eq!(snap.adapters, Field::Unavailable("PDH falló".into()));
        let runner = &snap.runners.as_ok().unwrap()[0];
        assert_eq!((runner.luid, runner.dedicated_mb), (None, None));
    }

    #[test]
    fn absurd_adapter_usage_becomes_unknown() {
        let mut gpu = reading(Some(7819), 0);
        gpu.adapter_used_mb.insert(RX, 67_481);
        let snap = assemble(&CollectInputs { gpu: Ok(gpu), ..inputs() });
        assert_eq!(snap.adapters.as_ok().unwrap()[0].used_mb, None);
    }

    #[test]
    fn foreign_llama_server_is_not_a_runner() {
        let foreign = ProcInfo { exe: Some(PathBuf::from(r"C:\llama\llama-server.exe")), ..runner_proc(9, "zzz") };
        let snap = assemble(&CollectInputs { procs: vec![foreign], ..inputs() });
        assert!(snap.runners.as_ok().unwrap().is_empty());
    }

    #[test]
    fn unreachable_ollama_is_paused_or_down() {
        let paused = assemble(&CollectInputs { version: Err("x".into()), paused_by: Some(PausedBy::User), ..inputs() });
        assert_eq!(paused.status, TrayStatus::Paused);
        let down = assemble(&CollectInputs { version: Err("x".into()), ..inputs() });
        assert_eq!((down.status, down.version), (TrayStatus::Down, None));
    }

    #[test]
    fn phantom_dxgi_adapters_without_pdh_counters_are_hidden() {
        let mut gpu = reading(Some(7819), 0);
        gpu.adapters.push(AdapterInfo { luid: 160223, name: "AMD Radeon RX 7800 XT".into(), total_mb: 16177 });
        let snap = assemble(&CollectInputs { gpu: Ok(gpu), ..inputs() });
        assert_eq!(snap.adapters.as_ok().unwrap().len(), 1);
    }

    #[test]
    fn without_any_pdh_adapter_counters_all_adapters_are_kept() {
        let mut gpu = reading(Some(7819), 0);
        gpu.adapter_used_mb.clear();
        gpu.adapters.push(AdapterInfo { luid: 160223, name: "AMD Radeon RX 7800 XT".into(), total_mb: 16177 });
        let snap = assemble(&CollectInputs { gpu: Ok(gpu), ..inputs() });
        assert_eq!(snap.adapters.as_ok().unwrap().len(), 2);
    }
}
