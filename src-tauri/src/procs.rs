use std::path::{Path, PathBuf};
use std::sync::Mutex;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

#[derive(Debug, Clone, PartialEq)]
pub struct ProcInfo {
    pub pid: u32,
    pub start_time: u64,
    pub name: String,
    pub exe: Option<PathBuf>,
    pub cmd: Vec<String>,
    pub cpu_pct: f32,
    pub ram_mb: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OllamaRole {
    TrayApp,
    Server,
    Runner,
}

#[cfg(windows)]
pub fn is_under(exe: &Path, dir: &Path) -> bool {
    let normalize = |p: &Path| {
        p.to_string_lossy()
            .replace('/', "\\")
            .trim_end_matches('\\')
            .to_lowercase()
    };
    normalize(exe).starts_with(&format!("{}\\", normalize(dir)))
}

#[cfg(not(windows))]
pub fn is_under(exe: &Path, dir: &Path) -> bool {
    exe != dir && exe.starts_with(dir)
}

pub fn classify(process: &ProcInfo, install_dir: &Path) -> Option<OllamaRole> {
    if !is_under(process.exe.as_deref()?, install_dir) {
        return None;
    }
    let name = process.name.to_lowercase();
    let has_arg = |arg: &str| process.cmd.iter().any(|a| a == arg);
    if name.starts_with("ollama app") {
        return Some(OllamaRole::TrayApp);
    }
    if name.starts_with("llama-server") || (name.starts_with("ollama") && has_arg("runner")) {
        return Some(OllamaRole::Runner);
    }
    if name.starts_with("ollama") && has_arg("serve") {
        return Some(OllamaRole::Server);
    }
    None
}

pub fn model_arg(cmd: &[String]) -> Option<&str> {
    let separate = cmd
        .iter()
        .position(|a| a == "--model")
        .and_then(|i| cmd.get(i + 1))
        .map(String::as_str);
    separate.or_else(|| cmd.iter().find_map(|a| a.strip_prefix("--model=")))
}

pub fn blob_hash(path: &str) -> Option<String> {
    let file = path.rsplit(['\\', '/']).next()?;
    file.strip_prefix("sha256-")
        .filter(|hash| !hash.is_empty())
        .map(str::to_string)
}

pub fn kill_order(procs: &[ProcInfo], install_dir: &Path) -> Vec<ProcInfo> {
    let mut ranked: Vec<(u8, ProcInfo)> = procs
        .iter()
        .filter_map(|p| classify(p, install_dir).map(|role| (kill_rank(role), p.clone())))
        .collect();
    ranked.sort_by_key(|(rank, process)| (*rank, process.pid));
    ranked.into_iter().map(|(_, process)| process).collect()
}

// La app de bandeja de Ollama relanza el servidor si se mata primero el servidor
fn kill_rank(role: OllamaRole) -> u8 {
    match role {
        OllamaRole::TrayApp => 0,
        OllamaRole::Server => 1,
        OllamaRole::Runner => 2,
    }
}

pub trait ProcessSource: Send + Sync {
    fn list(&self) -> Vec<ProcInfo>;
    fn kill(&self, target: &ProcInfo) -> bool;
}

pub struct SysinfoSource {
    system: Mutex<System>,
    cores: f32,
}

impl SysinfoSource {
    pub fn new() -> Self {
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1) as f32;
        Self {
            system: Mutex::new(System::new()),
            cores,
        }
    }
}

impl Default for SysinfoSource {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessSource for SysinfoSource {
    fn list(&self) -> Vec<ProcInfo> {
        let mut system = self.system.lock().unwrap();
        let refresh = ProcessRefreshKind::nothing()
            .with_cpu()
            .with_memory()
            .with_exe(UpdateKind::OnlyIfNotSet)
            .with_cmd(UpdateKind::OnlyIfNotSet);
        system.refresh_processes_specifics(ProcessesToUpdate::All, true, refresh);
        system
            .processes()
            .iter()
            .map(|(pid, p)| ProcInfo {
                pid: pid.as_u32(),
                start_time: p.start_time(),
                name: p.name().to_string_lossy().into_owned(),
                exe: p.exe().map(Path::to_path_buf),
                cmd: p
                    .cmd()
                    .iter()
                    .map(|s| s.to_string_lossy().into_owned())
                    .collect(),
                cpu_pct: p.cpu_usage() / self.cores,
                ram_mb: p.memory() / (1024 * 1024),
            })
            .collect()
    }

    fn kill(&self, target: &ProcInfo) -> bool {
        let mut system = self.system.lock().unwrap();
        let pid = Pid::from_u32(target.pid);
        system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
        let Some(process) = system.process(pid) else { return false };
        if process.start_time() != target.start_time || process.exe() != target.exe.as_deref() {
            return false;
        }
        process.kill()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIR: &str = r"C:\Users\u\AppData\Local\Programs\Ollama";

    fn proc_at(pid: u32, name: &str, exe: &str, cmd: &[&str]) -> ProcInfo {
        ProcInfo {
            pid,
            start_time: 100,
            name: name.into(),
            exe: Some(PathBuf::from(exe)),
            cmd: cmd.iter().map(|s| s.to_string()).collect(),
            cpu_pct: 0.0,
            ram_mb: 0,
        }
    }

    fn sample() -> Vec<ProcInfo> {
        vec![
            proc_at(
                30,
                "llama-server.exe",
                r"C:\Users\u\AppData\Local\Programs\Ollama\lib\ollama\llama-server.exe",
                &[
                    "llama-server.exe",
                    "--model",
                    r"C:\m\blobs\sha256-dec52a",
                    "--port",
                    "51081",
                ],
            ),
            proc_at(
                20,
                "ollama.exe",
                r"C:\Users\u\AppData\Local\Programs\Ollama\ollama.exe",
                &["ollama.exe", "serve"],
            ),
            proc_at(
                10,
                "ollama app.exe",
                r"C:\Users\u\AppData\Local\Programs\Ollama\ollama app.exe",
                &["ollama app.exe"],
            ),
            proc_at(
                40,
                "llama-server.exe",
                r"C:\llama\llama-server.exe",
                &["llama-server.exe", "--port", "4002"],
            ),
            proc_at(
                50,
                "ollama.exe",
                r"C:\Users\u\AppData\Local\Programs\Ollama\ollama.exe",
                &["ollama.exe", "list"],
            ),
        ]
    }

    #[test]
    fn classifies_only_processes_inside_the_install_dir() {
        let dir = Path::new(DIR);
        let roles: Vec<_> = sample().iter().map(|p| classify(p, dir)).collect();
        assert_eq!(
            roles,
            vec![
                Some(OllamaRole::Runner),
                Some(OllamaRole::Server),
                Some(OllamaRole::TrayApp),
                None,
                None
            ]
        );
    }

    #[test]
    fn old_style_runner_and_missing_exe() {
        let dir = Path::new(DIR);
        let old = proc_at(
            60,
            "ollama.exe",
            r"C:\Users\u\AppData\Local\Programs\Ollama\ollama.exe",
            &["ollama.exe", "runner", "--model", "x"],
        );
        assert_eq!(classify(&old, dir), Some(OllamaRole::Runner));
        let no_exe = ProcInfo { exe: None, ..old };
        assert_eq!(classify(&no_exe, dir), None);
    }

    #[test]
    fn install_dir_match_is_case_and_separator_insensitive_on_windows() {
        if cfg!(windows) {
            assert!(is_under(
                Path::new(r"c:\users\u\appdata\local\programs\ollama\ollama.exe"),
                Path::new(DIR)
            ));
            assert!(is_under(
                Path::new("C:/Users/u/AppData/Local/Programs/Ollama/lib/x.exe"),
                Path::new(DIR)
            ));
            assert!(!is_under(
                Path::new(r"C:\Users\u\AppData\Local\Programs\OllamaEvil\x.exe"),
                Path::new(DIR)
            ));
        }
    }

    #[test]
    fn extracts_model_argument_and_blob_hash() {
        let cmd: Vec<String> = ["x", "--model", r"C:\m\blobs\sha256-dec52a", "--port", "1"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(model_arg(&cmd), Some(r"C:\m\blobs\sha256-dec52a"));
        let eq_form: Vec<String> = vec!["x".into(), "--model=/m/blobs/sha256-abc".into()];
        assert_eq!(model_arg(&eq_form), Some("/m/blobs/sha256-abc"));
        assert_eq!(model_arg(&["x".to_string()]), None);
        assert_eq!(
            blob_hash(r"C:\m\blobs\sha256-dec52a").as_deref(),
            Some("dec52a")
        );
        assert_eq!(blob_hash("/m/blobs/sha256-abc").as_deref(), Some("abc"));
        assert_eq!(blob_hash(r"C:\m\model.gguf"), None);
    }

    #[test]
    fn kill_order_is_tray_then_server_then_runners_and_skips_foreign() {
        let sample = sample();
        assert_eq!(kill_order(&sample, Path::new(DIR)), vec![sample[2].clone(), sample[1].clone(), sample[0].clone()]);
    }

    #[test]
    fn sysinfo_rejects_stale_start_time_and_changed_executable() {
        let source = SysinfoSource::new();
        let current = source.list().into_iter().find(|p| p.pid == std::process::id()).unwrap();
        let stale = ProcInfo { start_time: current.start_time + 1, ..current.clone() };
        assert!(!source.kill(&stale));
        let changed = ProcInfo { exe: Some(PathBuf::from(r"C:\foreign\other.exe")), ..current };
        assert!(!source.kill(&changed));
        assert!(source.list().iter().any(|p| p.pid == std::process::id()));
    }
}
