# Corral: diseño

**Fecha:** 2026-10-06
**Estado:** aprobado en conversación, pendiente de revisión escrita
**Repo:** `santiquiroz/corral` (local: `C:\personal\corral`)
**Licencia:** AGPL-3.0

## 1. Problema

Ollama corre en segundo plano y nadie ve qué hace con la GPU. En el PC de referencia
(RX 7800 XT de 16 GB, Windows 11) se midió el 2026-10-05:

- claude-mem usa Ollama de forma continua (`qwen3.5-mem`, 6.6 GB de VRAM) y la cola
  nunca se vacía, así que el modelo nunca se descarga.
- Con la VRAM llena (escritorio, Edge, DesktopMate, fondo animado) el runner desborda
  a memoria compartida: el 2026-10-06 tenía 7.8 GB dedicados más 729 MB compartidos.
- Cuando desborda, la generación cae de 60-170 tok/s a 7-11 tok/s y el escritorio se
  traba. Nada lo avisa.
- Pausar requería un script propio (`ollama-gpu-toggle.ps1`), y reanudar exigía
  despertar a mano la cola de claude-mem.

Los monitores que existen (ElBruno.OllamaMonitor, jbrink90/ollama-monitor,
kol4s/ollama-manager, ollama-tray) miden la GPU solo con `nvidia-smi`, no detectan el
desborde, no dicen quién usa Ollama y no tienen una pausa segura. La app oficial de
Ollama es solo un chat.

## 2. Objetivo

Corral es una app de bandeja liviana que muestra el estado real de Ollama y su GPU, y
deja gestionar modelos y pausar o reanudar sin perder trabajo de los clientes.

Criterios de éxito:

1. De un vistazo (ícono de bandeja) se sabe si Ollama corre, está pausado, caído o
   desbordando VRAM.
2. El panel muestra, por GPU, cuánta VRAM usa Ollama, cuánta otros procesos y cuánta
   está libre, sin depender del fabricante de la GPU.
3. Pausar libera la VRAM en menos de 5 s; reanudar deja Ollama respondiendo y avisa a
   los clientes configurados (claude-mem incluido de fábrica).
4. En reposo, Corral usa menos de 60 MB de RAM y menos del 1 % de CPU.

## 3. Fuera de alcance

Chat (para eso ya están la app de Ollama y Open WebUI), buscador del catálogo de
ollama.com, gestión de varios servidores remotos, macOS y otros backends (LM Studio,
llama-server suelto). Se reconsideran solo si alguien los pide con un caso real.

## 4. Plataformas

| Plataforma | Fase | Telemetría de GPU |
|---|---|---|
| Windows 10/11 | v0.1 | DXGI (adaptadores) + contadores PDH (VRAM por adaptador y por proceso) |
| Linux | v0.3 | sysfs de amdgpu (`mem_info_vram_used`/`total`) y `nvidia-smi` |
| macOS | fuera de alcance | memoria unificada, otro modelo mental |

## 5. Arquitectura

App Tauri 2. Un núcleo en Rust hace todo el trabajo; la interfaz es React + TypeScript
dentro del webview; el ícono de bandeja es nativo de Tauri.

```
            ┌──────────────── núcleo Rust ────────────────┐
 Ollama ◀──▶│ ollama  gpu  procesos  clientes             │
 :11434     │        └──────┬──────┘                      │
            │          collector ──▶ Snapshot (inmutable) │──▶ evento Tauri ──▶ UI React
            │                         │                   │──▶ bandeja (ícono + menú)
            │ control ◀── reglas      │                   │──▶ api local (JSON, /metrics)
            │    └──▶ avisos          ▼                   │
            └─────────────────── config.toml ─────────────┘
```

### 5.1 Módulos del núcleo

Cada módulo hace una cosa, expone funciones puras donde se puede y recibe sus
dependencias de forma explícita (cliente HTTP, reloj, lector de contadores) para poder
probarlo con dobles.

**`ollama`**: cliente HTTP de la API de Ollama. Base configurable (por defecto
`http://127.0.0.1:11434`, o lo que diga `OLLAMA_HOST`).
- `version()` → `GET /api/version`
- `loaded()` → `GET /api/ps`: nombre, digest, tamaño, `size_vram`, contexto, vencimiento
- `installed()` → `GET /api/tags`: nombre, digest, tamaño, familia, cuantización, parámetros, fecha
- `show(name)` → `POST /api/show`: Modelfile y parámetros
- `pull(name)` → `POST /api/pull` en streaming, emite progreso
- `delete(name)`, `copy(src, dst)`
- `create_variant(base, num_ctx)` → `POST /api/create` con `from: base` y
  `parameters: {num_ctx}`; nombre `<base>-<ctx/1024>k`
- `unload(name)` → `POST /api/generate` con `keep_alive: 0` y sin prompt

**`gpu`**: telemetría sin depender del fabricante.
- Windows: `IDXGIFactory1::EnumAdapters1` da nombre, LUID y VRAM dedicada total de cada
  adaptador. Los contadores PDH `\GPU Adapter Memory(*)\Dedicated Usage` dan el uso
  total por adaptador, y `\GPU Process Memory(*)\Dedicated Usage` y `\Shared Usage` el
  uso por proceso. Las instancias se llaman `pid_<pid>_luid_0x<hi>_0x<lo>_phys_<n>` y se
  cruzan con el LUID de DXGI.
- Saneamiento: un valor por proceso mayor que la VRAM total de su adaptador se descarta
  (`None`), porque se sabe que `dwm.exe` reporta valores absurdos en Radeon. El total
  por adaptador siempre sale del contador de adaptador, nunca de sumar procesos.
- Se excluyen adaptadores sin VRAM dedicada o virtuales (por ejemplo `IddSampleDriver`).
- Un trait `GpuProbe` con dos implementaciones (Windows en v0.1, Linux en v0.3).

**`procesos`**: identifica el servidor y los runners de Ollama.
- Servidor: `ollama.exe` / `ollama` con `serve` en la línea de comandos. App de
  bandeja de Ollama: `ollama app.exe`.
- Runners: `llama-server` cuya ruta está dentro del directorio de instalación de Ollama
  (en Windows `%LOCALAPPDATA%\Programs\Ollama`). Así nunca se confunde con un
  llama-server ajeno (por ejemplo el de bipolar-code).
- Runner → modelo: el argumento `--model <ruta>/blobs/sha256-<hash>` se cruza con los
  digests de `installed()`.
- CPU y RAM por proceso con la crate `sysinfo`.

**`clientes`**: quién usa Ollama ahora.
- Conexiones TCP establecidas con puerto remoto igual al de Ollama (tabla TCP del
  sistema; en Windows `GetExtendedTcpTable` vía la crate `netstat2`), agrupadas por PID.
- Para cada PID: nombre de ejecutable y una etiqueta corta derivada de la línea de
  comandos (por ejemplo `bun … claude-mem … worker-service.cjs` → "claude-mem").
  Las etiquetas salen de una tabla de patrones en la config, con claude-mem incluido.

**`collector`**: arma una `Snapshot` inmutable combinando los módulos anteriores. Un
fallo de un módulo deja su campo en `None` con la razón; nunca aborta la snapshot
entera. Cadencia: cada 2 s con el panel abierto, cada 10 s solo con la bandeja.

**`control`**: acciones que cambian el estado.
- `pause()`: 1) `unload()` de cada modelo cargado; 2) cierra `ollama app` (si no, vuelve
  a levantar el servidor); 3) termina el servidor y sus runners, identificados por ruta.
  Pausar nunca borra modelos ni configuración.
- `resume()`: arranca `ollama app.exe` si existe; si no, `ollama serve` oculto. Espera a
  que `version()` responda (tope de 30 s) y después ejecuta los avisos.
- `paused_by`: `user` o `rule`, para que las reglas solo reanuden lo que pausaron.

**`avisos`**: al reanudar, envía un POST HTTP a cada aviso configurado. Viene uno
preconfigurado y activo si el puerto responde:
`POST http://127.0.0.1:37777/api/processing` con `{"isProcessing": false}`, que hace
que claude-mem retome su cola (verificado el 2026-10-05: durante la pausa, claude-mem
conserva la cola con "preserving buffered work"). Un aviso que falla se registra y no
bloquea la reanudación.

**`reglas`** (v0.2): pausa automática por programas. La config lista ejecutables (por
ejemplo `blender.exe`, `Resolve.exe`, juegos). Si alguno corre y Ollama no está en
pausa, pausa con `paused_by = rule`. Cuando ninguno corre durante 30 s seguidos y la
pausa la hizo una regla, reanuda. Una pausa manual nunca la deshace una regla.

**`api`** (v0.2): servidor HTTP local solo en `127.0.0.1:47115`.
- `GET /status`: la snapshot en JSON.
- `GET /metrics`: formato Prometheus (VRAM por adaptador y por runner, desborde,
  modelos cargados, clientes, estado).
- `POST /pause`, `POST /resume`: requieren el token de la config en `Authorization`.
- CLI `corral status|pause|resume`: el mismo ejecutable con argumentos habla con la
  instancia viva (plugin single-instance de Tauri) o, si no hay, con la API.

### 5.2 Modelo de datos (`Snapshot`)

```rust
struct Snapshot {
    taken_at: SystemTime,
    state: OllamaState,               // Running | Paused { by } | Down
    version: Option<String>,
    adapters: Field<Vec<Adapter>>,     // Field<T> = Ok(T) | Unavailable(String)
    runners: Field<Vec<Runner>>,
    loaded: Field<Vec<LoadedModel>>,
    clients: Field<Vec<Client>>,       // se agrega en v0.2
}
struct Adapter { luid: u64, name: String, total_mb: u64, used_mb: u64, ollama_mb: u64 }
struct Runner  { pid: u32, model: Option<String>, luid: Option<u64>,
                 dedicated_mb: Option<u64>, shared_mb: Option<u64>,
                 cpu_pct: f32, ram_mb: u64 }
struct Client  { pid: u32, exe: String, label: Option<String>, connections: u32 }
```

**Desborde:** un runner desborda si `shared_mb > spill_floor_mb` (por defecto 64 MB, para
ignorar el ruido). "Desbordando" no es un valor de `OllamaState`: se deriva de
`state == Running` y algún runner desbordando, y es lo que pinta el ícono de amarillo.

### 5.3 Interfaz

**Bandeja:** ícono verde (corriendo), amarillo (desbordando), gris (pausado), rojo
(caído). El tooltip muestra los modelos cargados y la VRAM. El menú tiene los modelos
cargados con su VRAM y botón de descargar de memoria, Pausar/Reanudar y Abrir panel.

**Panel** (ventana que se abre desde la bandeja, se oculta al cerrar):
- **Estado:** una barra por GPU (Ollama / otros / libre), alerta de desborde que dice
  qué runner desborda y cuánto, CPU/RAM de los procesos y, desde v0.2, los clientes
  conectados.
- **Modelos:** tabla de instalados (tamaño, cuantización, parámetros, contexto, fecha,
  si está cargado). Acciones: descargar de memoria, borrar (con confirmación), copiar y
  descargar un modelo nuevo con barra de progreso. Desde v0.2, crear variante con
  contexto acotado.
- **Reglas y avisos** (v0.2): editar la lista de programas y los avisos.
- **Ajustes:** URL de Ollama, cadencias, umbral de desborde, inicio con Windows.

### 5.4 Configuración

`%APPDATA%\corral\config.toml` (Linux: `~/.config/corral/config.toml`). Se crea con
valores por defecto en el primer arranque. El token de la API se genera ahí, aleatorio,
la primera vez.

## 6. Errores

- Ollama caído es un estado (`Down`), no un error: el ícono pasa a rojo y el panel
  ofrece "Reanudar".
- Cada fuente de datos falla por separado y su campo queda `Unavailable(razón)`; el
  panel muestra "sin datos" con la razón.
- Las acciones devuelven errores con tipo (`OllamaUnreachable`, `ProcessKillFailed`,
  `Timeout`…) que la UI muestra como mensaje breve.
- `resume()` que no logra respuesta en 30 s deja el estado `Down` y lo informa; no
  reintenta en bucle.

## 7. Seguridad

- La API local escucha solo en `127.0.0.1`; las acciones requieren token.
- Corral nunca mata procesos fuera del directorio de instalación de Ollama.
- Los avisos solo van a URLs de la config; no se descargan avisos remotos.

## 8. Pruebas

- **Unitarias en Rust** para todo lo puro: parseo de nombres de instancia PDH, LUID,
  saneamiento de valores, runner → modelo por digest, cálculo de desborde, nombres de
  variantes, evaluación de reglas con reloj falso, render de `/metrics`.
- **Integración en Rust** contra un Ollama falso (servidor `axum` en un puerto libre)
  para el cliente y para `pause`/`resume` con procesos de prueba.
- **UI:** Vitest + Testing Library para componentes, con la snapshot como prop.
- **Punta a punta:** Playwright contra la UI en modo dev con los comandos de Tauri
  simulados.
- **Prueba manual por release** en el PC de referencia: pausa con claude-mem activo,
  verificación de que la cola se conserva y se reanuda.
- Cobertura objetivo del núcleo: 80 %.

## 9. CI y releases

GitHub Actions en `windows-latest` (y `ubuntu-latest` desde v0.3): `cargo test`,
`cargo clippy`, `npm test`, build de Tauri. Cada release adjunta el instalador `.msi` y
el `.exe` (NSIS); en v0.3 también AppImage. Un release sin binario adjunto no está
terminado.

Nota de build local: en el PC de referencia el toolset MSVC de VS18 Community está roto;
hay que compilar con la DevShell del install de BuildTools (14.50.35717).

## 10. Fases y criterios de aceptación

**v0.1 (Windows)**
- Bandeja con los cuatro estados y el panel con las pestañas Estado y Modelos.
- Telemetría de GPU por adaptador y por runner, con desborde detectado.
- Modelos: listar, descargar de memoria, borrar, copiar y descargar uno nuevo con progreso.
- Pausa y reanudación con el aviso de claude-mem; reemplaza `ollama-gpu-toggle.ps1`.
- Aceptación: en el PC de referencia, con claude-mem activo, pausar libera la VRAM del
  runner en menos de 5 s y reanudar devuelve la cola a cero sin pérdidas.

**v0.2**
- Clientes conectados, reglas de pausa automática, variantes con contexto acotado,
  API local con `/metrics` y CLI.
- Aceptación: abrir un programa de la lista pausa Ollama, y cerrarlo reanuda tras 30 s.

**v0.3 (Linux)**
- `GpuProbe` para amdgpu y NVIDIA, procesos y clientes vía `/proc`, AppImage.

## 11. Riesgos

| Riesgo | Mitigación |
|---|---|
| Los contadores PDH reportan valores absurdos (bug conocido de `dwm` en Radeon) | Saneamiento contra la VRAM total; los totales salen del contador de adaptador |
| Ollama cambia el formato de los runners (hoy `llama-server --model …/sha256-…`) | Si no se puede cruzar, `model: None`; la VRAM se sigue mostrando por proceso |
| La app de bandeja de Ollama relanza el servidor al matarlo | `pause()` cierra primero `ollama app` |
| Matar un runner a mitad de una petición | Los clientes ven conexión cortada; claude-mem lo trata como transitorio y conserva la cola (verificado) |
| El toolset MSVC roto en el PC de referencia | Documentado en §9 |

## 12. Decisiones tomadas

- Stack Tauri 2 (Rust + React/TS) en lugar de Python+PyInstaller (60-80 MB residente) o
  .NET WPF (solo Windows y ya existe).
- Nombre "Corral" (corral de llamas); se evita "Ollama" en el nombre por la marca.
- AGPL-3.0, como Argos, Blindside y Lumina.
- Windows primero porque es la plataforma del PC de referencia y la que no tiene
  ninguna herramienta con telemetría AMD.
