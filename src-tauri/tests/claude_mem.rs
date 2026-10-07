use corral_lib::claude_mem::{evaluate_claude_mem, CheckLevel, ClaudeMemInputs, LastInteraction};

fn healthy() -> ClaudeMemInputs {
    ClaudeMemInputs { provider: "openrouter".into(), base_url: "http://127.0.0.1:11434/v1".into(), model: "memory:latest".into(), ollama_url: "http://127.0.0.1:11434".into(), worker_reachable: true, model_installed: Some(true), model_loaded: Some(true), last_interaction: Some(LastInteraction { timestamp_ms: 7_200_000, success: true }), queue_depth: Some(0), num_ctx: Some(32768), now_ms: 7_200_000 }
}

fn level(inputs: &ClaudeMemInputs, id: &str) -> CheckLevel {
    evaluate_claude_mem(inputs).into_iter().find(|check| check.id == id).unwrap().level
}

#[test]
fn estado_saludable_retornan_seis_verificaciones_con_texto_y_nivel_ok() {
    let checks = evaluate_claude_mem(&healthy());
    assert_eq!(checks.len(), 6);
    for id in ["worker", "endpoint", "model_installed", "model_loaded", "last_interaction", "context"] {
        let check = checks.iter().find(|check| check.id == id).unwrap();
        assert_eq!(check.level, CheckLevel::Ok);
        assert!(!check.message.is_empty());
    }
    assert_eq!(serde_json::to_value(CheckLevel::Warn).unwrap(), "warn");
    assert_eq!(serde_json::to_value(CheckLevel::Fail).unwrap(), "fail");
    assert_eq!(serde_json::to_value(CheckLevel::Ok).unwrap(), "ok");
}

#[test]
fn worker_inaccesible_falla_con_aviso_explicito() {
    let checks = evaluate_claude_mem(&ClaudeMemInputs { worker_reachable: false, ..healthy() });
    let worker = checks.iter().find(|check| check.id == "worker").unwrap();
    assert_eq!(worker.level, CheckLevel::Fail);
    assert_eq!(worker.message, "El worker de claude-mem no responde");
}

#[test]
fn endpoint_compara_proveedor_host_y_puerto_no_la_ruta() {
    assert_eq!(level(&ClaudeMemInputs { base_url: "http://127.0.0.1:11434/otra/ruta".into(), ..healthy() }, "endpoint"), CheckLevel::Ok);
    for base_url in ["http://127.0.0.1:11435/v1", "http://localhost:11434/v1", "https://openrouter.ai/api/v1", "sin-url"] {
        let inputs = ClaudeMemInputs { base_url: base_url.into(), ..healthy() };
        assert_eq!(level(&inputs, "endpoint"), CheckLevel::Warn);
    }
    assert_eq!(level(&ClaudeMemInputs { provider: "claude".into(), ..healthy() }, "endpoint"), CheckLevel::Warn);
    let inputs = ClaudeMemInputs { base_url: "http://example.com/v1".into(), ollama_url: "http://example.com:80".into(), ..healthy() };
    assert_eq!(level(&inputs, "endpoint"), CheckLevel::Ok);
}

#[test]
fn modelo_ausente_falla_y_estado_desconocido_advierte() {
    let inputs = ClaudeMemInputs { model_installed: Some(false), ..healthy() };
    let checks = evaluate_claude_mem(&inputs);
    let installed = checks.iter().find(|check| check.id == "model_installed").unwrap();
    assert_eq!(installed.level, CheckLevel::Fail);
    assert_eq!(installed.message, "El modelo memory:latest no está instalado: claude-mem fallará cuando se descargue de memoria");
    assert_eq!(level(&ClaudeMemInputs { model_installed: None, ..healthy() }, "model_installed"), CheckLevel::Warn);
}

#[test]
fn modelo_no_cargado_advierte_proxima_observacion_y_desconocido_advierte() {
    let checks = evaluate_claude_mem(&ClaudeMemInputs { model_loaded: Some(false), ..healthy() });
    let loaded = checks.iter().find(|check| check.id == "model_loaded").unwrap();
    assert_eq!(loaded.level, CheckLevel::Warn);
    assert!(loaded.message.contains("se cargará con la próxima observación"));
    assert_eq!(level(&ClaudeMemInputs { model_loaded: None, ..healthy() }, "model_loaded"), CheckLevel::Warn);
}

#[test]
fn interaccion_fallida_falla_aunque_sea_reciente_y_ausente_advierte() {
    let inputs = ClaudeMemInputs { last_interaction: Some(LastInteraction { timestamp_ms: 7_200_000, success: false }), ..healthy() };
    assert_eq!(level(&inputs, "last_interaction"), CheckLevel::Fail);
    assert_eq!(level(&ClaudeMemInputs { last_interaction: None, ..healthy() }, "last_interaction"), CheckLevel::Warn);
}

#[test]
fn interaccion_vieja_advierte_solo_con_cola_pendiente_y_tolera_futuro() {
    let stale = ClaudeMemInputs { last_interaction: Some(LastInteraction { timestamp_ms: 3_599_999, success: true }), queue_depth: Some(1), ..healthy() };
    assert_eq!(level(&stale, "last_interaction"), CheckLevel::Warn);
    assert_eq!(level(&ClaudeMemInputs { queue_depth: Some(0), ..stale.clone() }, "last_interaction"), CheckLevel::Ok);
    assert_eq!(level(&ClaudeMemInputs { queue_depth: None, ..stale }, "last_interaction"), CheckLevel::Ok);
    let boundary = ClaudeMemInputs { last_interaction: Some(LastInteraction { timestamp_ms: 3_600_000, success: true }), queue_depth: Some(1), ..healthy() };
    assert_eq!(level(&boundary, "last_interaction"), CheckLevel::Ok);
    let future = ClaudeMemInputs { last_interaction: Some(LastInteraction { timestamp_ms: 8_000_000, success: true }), queue_depth: Some(1), ..healthy() };
    assert_eq!(level(&future, "last_interaction"), CheckLevel::Ok);
}

#[test]
fn contexto_minimo_es_16384_y_ausente_o_menor_advierte() {
    assert_eq!(level(&ClaudeMemInputs { num_ctx: Some(16384), ..healthy() }, "context"), CheckLevel::Ok);
    for num_ctx in [Some(0), Some(8192), Some(16383), None] {
        assert_eq!(level(&ClaudeMemInputs { num_ctx, ..healthy() }, "context"), CheckLevel::Warn);
    }
    let checks = evaluate_claude_mem(&ClaudeMemInputs { num_ctx: Some(8192), ..healthy() });
    assert_eq!(checks.iter().find(|check| check.id == "context").unwrap().message, "claude-mem recicla su conversación cerca de 16k tokens; con menos contexto Ollama recorta el prompt");
}
