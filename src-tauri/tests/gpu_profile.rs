use corral_lib::{config::{Config, GpuProfile}, ollama_gpus::{gpu_profile_env, validate_gpu_profile, OllamaGpu}};
use serde_json::json;

const VARIABLES: [&str; 6] = ["OLLAMA_SCHED_SPREAD", "HIP_VISIBLE_DEVICES", "ROCR_VISIBLE_DEVICES", "CUDA_VISIBLE_DEVICES", "GGML_VK_VISIBLE_DEVICES", "OLLAMA_IGPU_ENABLE"];

fn assert_env(profile: &GpuProfile, igpu: bool, os: &str, selected: Option<(&str, &str)>) {
    let vars = gpu_profile_env(profile, igpu, os);
    assert_eq!(vars.len(), VARIABLES.len());
    for name in VARIABLES {
        let expected = if name == "OLLAMA_IGPU_ENABLE" && igpu { Some("1") }
            else { selected.filter(|(variable, _)| *variable == name).map(|(_, value)| value) };
        assert_eq!(vars.iter().filter(|(variable, _)| variable == name).count(), 1);
        assert_eq!(vars.iter().find(|(variable, _)| variable == name).unwrap().1.as_deref(), expected, "{name}, {os}");
    }
}

#[test]
fn perfiles_retornan_todas_las_variables_y_limpian_selecciones_anteriores() {
    for os in ["windows", "linux", "macos"] {
        for igpu in [false, true] {
            assert_env(&GpuProfile::Auto, igpu, os, None);
            assert_env(&GpuProfile::Spread, igpu, os, Some(("OLLAMA_SCHED_SPREAD", "1")));
            for (library, variable) in [("ROCm", if os == "windows" { "HIP_VISIBLE_DEVICES" } else { "ROCR_VISIBLE_DEVICES" }), ("CUDA", "CUDA_VISIBLE_DEVICES"), ("Vulkan", "GGML_VK_VISIBLE_DEVICES")] {
                assert_env(&GpuProfile::Single { library: library.into(), filter_id: "GPU-0".into() }, igpu, os, Some((variable, "GPU-0")));
            }
        }
    }
}

fn gpu(library: &str, filter_id: &str) -> OllamaGpu {
    OllamaGpu { id: "0".into(), filter_id: filter_id.into(), library: library.into(), description: "GPU".into(), kind: "discrete".into(), total_mb: Some(16384), dropped: false }
}

#[test]
fn valida_solo_una_gpu_presente_con_libreria_y_filter_id_coincidentes() {
    assert!(validate_gpu_profile(&GpuProfile::Auto, &[]).is_ok());
    assert!(validate_gpu_profile(&GpuProfile::Spread, &[]).is_ok());
    for library in ["ROCm", "CUDA", "Vulkan"] {
        let profile = GpuProfile::Single { library: library.into(), filter_id: "0".into() };
        assert!(validate_gpu_profile(&profile, &[gpu(library, "0")]).is_ok());
        assert!(validate_gpu_profile(&profile, &[]).is_err());
        assert!(validate_gpu_profile(&profile, &[gpu(library, "1")]).is_err());
        assert!(validate_gpu_profile(&profile, &[gpu("otra", "0")]).is_err());
    }
    let unsupported = GpuProfile::Single { library: "Metal".into(), filter_id: "0".into() };
    assert!(validate_gpu_profile(&unsupported, &[gpu("Metal", "0")]).is_err());
}

#[test]
fn serializa_perfiles_con_kind_y_carga_config_legacy_automatica_sin_igpu() {
    for (profile, value) in [(GpuProfile::Auto, json!({"kind":"auto"})), (GpuProfile::Spread, json!({"kind":"spread"})), (GpuProfile::Single { library:"ROCm".into(), filter_id:"0".into() }, json!({"kind":"single","library":"ROCm","filter_id":"0"}))] {
        assert_eq!(serde_json::to_value(&profile).unwrap(), value);
        assert_eq!(serde_json::from_value::<GpuProfile>(value).unwrap(), profile);
    }
    let config: Config = toml::from_str("spill_floor_mb = 10").unwrap();
    assert_eq!(config.gpu_profile, GpuProfile::Auto);
    assert!(!config.igpu_enabled);
    assert_eq!(Config::default().gpu_profile, GpuProfile::Auto);
    assert!(!Config::default().igpu_enabled);
}

#[test]
fn configuracion_rechaza_gpu_unica_con_libreria_no_soportada_o_filtro_vacio() {
    for (library, filter_id) in [("Metal", "0"), ("ROCm", ""), ("CUDA", "   ")] {
        let config = Config { gpu_profile: GpuProfile::Single { library: library.into(), filter_id: filter_id.into() }, ..Config::default() };
        assert!(corral_lib::config::validate(config).is_err());
    }
    let config = Config { gpu_profile: GpuProfile::Single { library: "ROCm".into(), filter_id: "99".into() }, ..Config::default() };
    assert!(corral_lib::config::validate(config).is_ok());
}
