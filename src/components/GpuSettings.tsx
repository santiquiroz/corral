import { useEffect, useState } from "react";
import { applyGpuProfile, getConfig, listOllamaGpus } from "../lib/api";
import { formatMb } from "../lib/format";
import type { GpuProfile, OllamaGpu } from "../lib/types";

const PROFILE_OPTIONS = [
  { kind: "auto", label: "Automático" },
  { kind: "spread", label: "Repartir entre todas" },
  { kind: "single", label: "Solo una GPU" },
] as const;

function gpuKey(gpu: { library: string; filter_id: string }) {
  return `${gpu.library}:${gpu.filter_id}`;
}

function gpuDescription(gpu: OllamaGpu) {
  const kind = gpu.kind === "integrated" ? "integrada" : "discreta";
  const memory = gpu.total_mb === null ? "—" : formatMb(gpu.total_mb);
  return `${gpu.description} · ${gpu.library} · ${kind} · ${memory}${gpu.dropped ? " · ignorada" : ""}`;
}

function selectedProfile(kind: GpuProfile["kind"], gpu: OllamaGpu | undefined): GpuProfile {
  if (kind !== "single") return { kind };
  return { kind, library: gpu?.library ?? "", filter_id: gpu?.filter_id ?? "" };
}

export default function GpuSettings({ profile, igpuEnabled, onApplied, onBusy, disabled }: {
  profile: GpuProfile; igpuEnabled: boolean; onApplied: (profile: GpuProfile, enabled: boolean) => void;
  onBusy: (busy: boolean) => void; disabled: boolean;
}) {
  const [gpus, setGpus] = useState<OllamaGpu[]>([]);
  const [draft, setDraft] = useState(profile);
  const [integrated, setIntegrated] = useState(igpuEnabled);
  const [confirm, setConfirm] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    listOllamaGpus().then(setGpus).catch((error) => setMessage(String(error)));
  }, []);

  function changeProfile(profile: GpuProfile) {
    setDraft(profile);
    setConfirm(false);
  }

  function changeIntegrated(enabled: boolean) {
    setIntegrated(enabled);
    setConfirm(false);
  }

  async function apply() {
    setBusy(true);
    onBusy(true);
    setMessage(null);
    try {
      await applyGpuProfile(draft, integrated);
      onApplied(draft, integrated);
      setMessage("Perfil GPU aplicado.");
    } catch (error) {
      await refreshSavedProfile(String(error));
    } finally {
      setBusy(false);
      onBusy(false);
      setConfirm(false);
    }
  }

  async function refreshSavedProfile(applyError: string) {
    try {
      const saved = await getConfig();
      setDraft(saved.gpu_profile);
      setIntegrated(saved.igpu_enabled);
      onApplied(saved.gpu_profile, saved.igpu_enabled);
      setMessage(applyError);
    } catch (error) {
      setMessage(`${applyError}; ${String(error)}`);
    }
  }

  const selection = draft.kind === "single" ? gpus.find((gpu) => gpuKey(gpu) === gpuKey(draft)) : undefined;
  const invalid = draft.kind === "single" && !selection;
  return (
    <section aria-label="Configuración de GPU">
      <h2>GPU</h2>
      <ul>{gpus.map((gpu) => <li key={`${gpuKey(gpu)}:${gpu.id}`}>{gpuDescription(gpu)}</li>)}</ul>
      <fieldset disabled={busy || disabled}>
        <legend>Perfil GPU</legend>
        {PROFILE_OPTIONS.map((option) => (
          <label className="row" key={option.kind}>
            <input type="radio" name="gpu-profile" checked={draft.kind === option.kind} onChange={() => changeProfile(selectedProfile(option.kind, gpus[0]))} />
            {option.label}
          </label>
        ))}
        {draft.kind === "single" && <label className="row">GPU seleccionada
          <select value={selection ? gpuKey(selection) : ""} onChange={(event) => changeProfile(selectedProfile("single", gpus.find((gpu) => gpuKey(gpu) === event.target.value)))}>
            <option value="" disabled>Selecciona una GPU</option>
            {gpus.map((gpu) => <option key={gpuKey(gpu)} value={gpuKey(gpu)}>{gpu.description} ({gpu.library})</option>)}
          </select>
        </label>}
        <label className="row"><input type="checkbox" checked={integrated} onChange={(event) => changeIntegrated(event.target.checked)} />Incluir GPU integrada</label>
      </fieldset>
      <p className="muted">Ollama se reiniciará; claude-mem conserva su cola</p>
      <button disabled={busy || disabled || invalid} onClick={confirm ? apply : () => setConfirm(true)}>
        {busy ? "Reiniciando…" : confirm ? "Confirmar y reiniciar Ollama" : "Aplicar y reiniciar Ollama"}
      </button>
      {message && <p className="notice" role="status">{message}</p>}
    </section>
  );
}
