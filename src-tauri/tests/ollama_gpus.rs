use corral_lib::ollama_gpus::parse_ollama_gpus;

#[test]
fn descubre_gpu_util_y_integrada_descartada_en_ultimo_bloque() {
    let log = concat!(
        "time=2026-10-07T08:00:00-05:00 msg=\"inference compute\" id=old filter_id=9 library=CUDA description=\"GPU vieja\" type=discrete total=\"8.0 GiB\"\n",
        "time=2026-10-07T09:00:00-05:00 msg=\"inference compute\" id=0 filter_id=0 library=ROCm compute=gfx1101 name=ROCm0 description=\"AMD Radeon RX 7800 XT\" type=discrete total=\"16.0 GiB\"\n",
        "time=2026-10-07T09:00:01-05:00 msg=\"dropping integrated GPU; to enable, set OLLAMA_IGPU_ENABLE=1\" id=1 library=Vulkan description=\"AMD Radeon(TM) Graphics\"\n",
        "time=2026-10-07T09:30:00-05:00 msg=\"server listening\"\n",
    );

    let gpus = parse_ollama_gpus(log);

    assert_eq!(gpus.len(), 2);
    assert_eq!(gpus[0].id, "0");
    assert_eq!(gpus[0].filter_id, "0");
    assert_eq!(gpus[0].library, "ROCm");
    assert_eq!(gpus[0].description, "AMD Radeon RX 7800 XT");
    assert_eq!(gpus[0].kind, "discrete");
    assert_eq!(gpus[0].total_mb, Some(16384));
    assert!(!gpus[0].dropped);
    assert_eq!(gpus[1].id, "1");
    assert_eq!(gpus[1].library, "Vulkan");
    assert_eq!(gpus[1].description, "AMD Radeon(TM) Graphics");
    assert_eq!(gpus[1].kind, "integrated");
    assert_eq!(gpus[1].total_mb, None);
    assert!(gpus[1].dropped);
}

#[test]
fn ventana_de_diez_segundos_incluye_limite_y_compara_zonas_y_dias() {
    let log = concat!(
        "time=2026-10-06T23:59:49Z msg=\"inference compute\" id=old filter_id=old library=CUDA description=old type=discrete\n",
        "time=2026-10-06T23:59:50Z msg=\"inference compute\" id=boundary filter_id=0 library=CUDA description=boundary type=discrete\n",
        "time=2026-10-06T19:00:00-05:00 msg=\"inference compute\" id=new filter_id=1 library=CUDA description=new type=discrete\n",
    );

    let ids: Vec<_> = parse_ollama_gpus(log).into_iter().map(|gpu| gpu.id).collect();

    assert_eq!(ids, vec!["boundary", "new"]);
}

#[test]
fn convierte_mib_y_gib_y_lee_valores_citados() {
    let log = concat!(
        "time=2026-10-07T09:00:00Z msg=\"inference compute\" id=\"GPU-123\" filter_id=\"GPU-123\" library=CUDA description=\"Tarjeta con espacios\" type=discrete total=\"1536 MiB\"\n",
        "time=2026-10-07T09:00:00Z msg=\"inference compute\" id=2 filter_id=2 library=Vulkan description=integrada type=integrated total=\"1.5 GiB\"\n",
    );

    let gpus = parse_ollama_gpus(log);

    assert_eq!(gpus[0].id, "GPU-123");
    assert_eq!(gpus[0].filter_id, "GPU-123");
    assert_eq!(gpus[0].description, "Tarjeta con espacios");
    assert_eq!(gpus[0].total_mb, Some(1536));
    assert_eq!(gpus[1].total_mb, Some(1536));
    assert_eq!(gpus[1].kind, "integrated");
}

#[test]
fn ignora_lineas_sin_tiempo_o_tiempo_invalido_y_texto_ajeno() {
    let log = concat!(
        "msg=\"inference compute\" id=0 filter_id=0 library=ROCm description=GPU type=discrete\n",
        "time=no-fecha msg=\"inference compute\" id=0 filter_id=0 library=ROCm description=GPU type=discrete\n",
        "time=2026-10-07T09:00:00Z msg=\"inference compute\"\n",
        "time=2026-10-07T09:00:00Z msg=\"otra cosa\" id=0 library=ROCm\n",
    );

    assert!(parse_ollama_gpus(log).is_empty());
    assert!(parse_ollama_gpus("").is_empty());
}

#[test]
fn bloque_reciente_incompleto_no_reutiliza_gpu_de_un_arranque_anterior() {
    let log = concat!(
        "time=2026-10-07T08:00:00Z msg=\"inference compute\" id=0 filter_id=0 library=ROCm description=\"GPU vieja\" type=discrete total=\"16.0 GiB\"\n",
        "time=2026-10-07T09:00:00Z msg=\"inference compute\" description=\"GPU incompleta\"\n",
    );

    assert!(parse_ollama_gpus(log).is_empty());
}
