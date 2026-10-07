import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { useEffect, useState } from "react";
import { getConfig, saveConfig } from "../lib/api";
import type { Config } from "../lib/types";

type NumberKey = "poll_panel_secs" | "poll_tray_secs" | "spill_floor_mb" | "resume_timeout_secs";
const NUMBER_FIELDS: { key: NumberKey; label: string }[] = [
  { key: "spill_floor_mb", label: "Umbral de desborde (MB)" },
  { key: "poll_panel_secs", label: "Refresco con panel abierto (s)" },
  { key: "poll_tray_secs", label: "Refresco solo bandeja (s)" },
  { key: "resume_timeout_secs", label: "Espera máxima al reanudar (s)" },
];

export default function SettingsTab() {
  const [config, setConfig] = useState<Config | null>(null);
  const [autostart, setAutostart] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    getConfig().then(setConfig).catch((e) => setMessage(String(e)));
    isEnabled().then(setAutostart).catch(() => {});
  }, []);

  if (!config) return <p className="muted">{message ?? "Cargando ajustes…"}</p>;

  const setNumber = (key: NumberKey, value: string) => setConfig({ ...config, [key]: Number(value) });
  const toggleHook = (index: number) =>
    setConfig({ ...config, hooks: config.hooks.map((h, i) => (i === index ? { ...h, enabled: !h.enabled } : h)) });
  const toggleAutostart = async () => {
    try {
      await (autostart ? disable() : enable());
      setAutostart(!autostart);
    } catch (e) {
      setMessage(String(e));
    }
  };
  const save = () => saveConfig(config).then(() => setMessage("Guardado.")).catch((e) => setMessage(String(e)));

  return (
    <div className="tab settings">
      <label className="row">URL de Ollama
        <input type="text" value={config.ollama_url} onChange={(e) => setConfig({ ...config, ollama_url: e.target.value })} />
      </label>
      {NUMBER_FIELDS.map((f) => (
        <label className="row" key={f.key}>{f.label}
          <input type="number" min={1} aria-label={f.label} value={config[f.key]} onChange={(e) => setNumber(f.key, e.target.value)} />
        </label>
      ))}
      <h2>Memoria</h2>
      <label className="row">Mantener modelos cargados
        <select value={config.load_keep_alive} onChange={(e) => setConfig({ ...config, load_keep_alive: e.target.value })}>
          <option value="30m">30 min</option>
          <option value="1h">1 h</option>
          <option value="-1">Siempre</option>
        </select>
      </label>
      <h2>Avisos al reanudar</h2>
      {config.hooks.map((h, i) => (
        <label className="row" key={h.name}>
          <input type="checkbox" aria-label={`Aviso ${h.name}`} checked={h.enabled} onChange={() => toggleHook(i)} />
          <span><strong>{h.name}</strong> <span className="muted mono">POST {h.url}</span></span>
        </label>
      ))}
      <h2>Sistema</h2>
      <label className="row">
        <input type="checkbox" aria-label="Iniciar con Windows" checked={autostart} onChange={toggleAutostart} />
        Iniciar con Windows (oculto en la bandeja)
      </label>
      <div className="row">
        <button className="primary" onClick={save}>Guardar</button>
        {message && <span role="status">{message}</span>}
      </div>
    </div>
  );
}
