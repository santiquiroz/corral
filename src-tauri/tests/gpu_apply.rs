use corral_lib::{
    commands::apply_gpu_profile_with,
    config::{load_or_create, validate_gpu_config_change, Config, GpuProfile},
    ollama_gpus::OllamaGpu,
    state::AppState,
};
use std::{path::PathBuf, sync::{atomic::{AtomicBool, AtomicU64, Ordering}, Mutex}};

static NEXT_PATH: AtomicU64 = AtomicU64::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let id = NEXT_PATH.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("corral-gpu-apply-{}-{id}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn config_path(&self) -> PathBuf { self.0.join("config.toml") }
}

impl Drop for TestDir {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}

fn single() -> GpuProfile { GpuProfile::Single { library: "ROCm".into(), filter_id: "0".into() } }

fn detected() -> Vec<OllamaGpu> {
    vec![OllamaGpu { id: "0".into(), filter_id: "0".into(), library: "ROCm".into(), description: "GPU".into(), kind: "discrete".into(), total_mb: Some(16384), dropped: false }]
}

#[tokio::test]
async fn reinicio_gpu_impide_otra_accion_del_ciclo_de_vida_entre_pausa_y_reanudacion() {
    let dir = TestDir::new();
    let state = AppState::new(dir.config_path(), Config::default());
    let gpus = detected();
    let otra_accion_ejecutada = AtomicBool::new(false);
    let (pausa_completada, esperar_pausa) = tokio::sync::oneshot::channel();
    let (otra_accion_iniciada, esperar_otra_accion) = tokio::sync::oneshot::channel();

    let reinicio = apply_gpu_profile_with(&state, single(), false, &gpus, |_| std::future::ready(Ok(())), || async {
        assert!(state.lifecycle.try_lock().is_err(), "la pausa debe conservar el candado del reinicio");
        pausa_completada.send(()).unwrap();
        esperar_otra_accion.await.unwrap();
        tokio::task::yield_now().await;
        assert!(!otra_accion_ejecutada.load(Ordering::SeqCst), "otra accion no puede ejecutarse entre pausa y reanudacion");
        assert!(state.lifecycle.try_lock().is_err(), "la reanudacion debe conservar el mismo candado");
        Ok(())
    });
    let otra_accion = async {
        esperar_pausa.await.unwrap();
        otra_accion_iniciada.send(()).unwrap();
        let _lifecycle = state.lifecycle.lock().await;
        otra_accion_ejecutada.store(true, Ordering::SeqCst);
    };

    let (resultado, ()) = tokio::join!(reinicio, otra_accion);

    resultado.unwrap();
    assert!(otra_accion_ejecutada.load(Ordering::SeqCst), "otra accion debe ejecutarse despues del reinicio");
    assert!(state.lifecycle.try_lock().is_ok(), "el reinicio debe liberar el candado al terminar");
}

#[tokio::test]
async fn aplicar_guarda_config_persiste_todas_las_variables_y_reinicia_en_orden() {
    let dir = TestDir::new();
    let path = dir.config_path();
    let state = AppState::new(path.clone(), Config::default());
    let steps = Mutex::new(Vec::new());

    apply_gpu_profile_with(&state, single(), true, &detected(), |vars| {
        let saved = load_or_create(&path).unwrap();
        assert_eq!(saved.gpu_profile, single());
        assert!(saved.igpu_enabled);
        assert_eq!(vars.len(), 6);
        let visible = if cfg!(windows) { "HIP_VISIBLE_DEVICES" } else { "ROCR_VISIBLE_DEVICES" };
        assert!(vars.contains(&(visible.into(), Some("0".into()))));
        assert!(vars.contains(&("OLLAMA_IGPU_ENABLE".into(), Some("1".into()))));
        steps.lock().unwrap().push("persist");
        std::future::ready(Ok(()))
    }, || async {
        let config = state.config.read().await;
        assert_eq!(config.gpu_profile, single());
        assert!(config.igpu_enabled);
        assert_eq!(*steps.lock().unwrap(), vec!["persist"]);
        steps.lock().unwrap().push("restart");
        Ok(())
    }).await.unwrap();

    assert_eq!(*steps.lock().unwrap(), vec!["persist", "restart"]);
}

#[tokio::test]
async fn gpu_ausente_no_guarda_ni_persiste_ni_reinicia() {
    let dir = TestDir::new();
    let path = dir.config_path();
    let config = Config::default();
    let state = AppState::new(path.clone(), config.clone());

    let result = apply_gpu_profile_with(&state, single(), true, &[], |_| -> std::future::Ready<Result<(), String>> { panic!("no debe persistir") }, || async { panic!("no debe reiniciar") }).await;

    assert!(result.is_err());
    assert!(!path.exists());
    assert_eq!(*state.config.read().await, config);
}

#[tokio::test]
async fn error_al_guardar_no_persiste_ni_reinicia_ni_cambia_config_activa() {
    let dir = TestDir::new();
    let config = Config::default();
    let state = AppState::new(dir.0.clone(), config.clone());

    let result = apply_gpu_profile_with(&state, GpuProfile::Spread, false, &[], |_| -> std::future::Ready<Result<(), String>> { panic!("no debe persistir") }, || async { panic!("no debe reiniciar") }).await;

    assert!(result.is_err());
    assert_eq!(*state.config.read().await, config);
}

#[tokio::test]
async fn error_al_persistir_se_propaga_y_omite_reinicio_con_config_guardada() {
    let dir = TestDir::new();
    let path = dir.config_path();
    let state = AppState::new(path.clone(), Config::default());

    let result = apply_gpu_profile_with(&state, GpuProfile::Spread, true, &[], |_| std::future::ready(Err("registro denegado".into())), || async { panic!("no debe reiniciar") }).await;

    assert_eq!(result, Err("registro denegado".into()));
    assert_eq!(load_or_create(&path).unwrap().gpu_profile, GpuProfile::Spread);
    assert_eq!(state.config.read().await.gpu_profile, GpuProfile::Spread);
}

#[tokio::test]
async fn error_al_reiniciar_se_propaga_despues_de_persistir() {
    let dir = TestDir::new();
    let path = dir.config_path();
    let state = AppState::new(path.clone(), Config::default());
    let persisted = Mutex::new(false);

    let result = apply_gpu_profile_with(&state, GpuProfile::Spread, false, &[], |_| { *persisted.lock().unwrap() = true; std::future::ready(Ok(())) }, || async {
        assert!(*persisted.lock().unwrap());
        Err("Ollama no reinició".into())
    }).await;

    assert_eq!(result, Err("Ollama no reinició".into()));
    assert_eq!(load_or_create(&path).unwrap().gpu_profile, GpuProfile::Spread);
}

#[test]
fn guardado_generico_rechaza_cambios_gpu_y_permite_otros_ajustes() {
    let current = Config::default();
    let next = Config { gpu_profile: GpuProfile::Spread, ..current.clone() };
    assert!(validate_gpu_config_change(&current, &next).is_err());
    let next = Config { igpu_enabled: true, ..current.clone() };
    assert!(validate_gpu_config_change(&current, &next).is_err());
    let next = Config { load_keep_alive: "1h".into(), poll_panel_secs: 4, ..current.clone() };
    assert!(validate_gpu_config_change(&current, &next).is_ok());
    assert!(validate_gpu_config_change(&next, &next).is_ok());
}
