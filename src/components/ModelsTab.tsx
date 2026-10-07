import { useCallback, useEffect, useState } from "react";
import { copyModel, deleteModel, listModels, loadModel, unloadModel } from "../lib/api";
import type { InstalledModel, LoadedModel, Snapshot } from "../lib/types";
import { formatMb } from "../lib/format";
import PullForm from "./PullForm";

type Mode = { kind: "idle" } | { kind: "confirm-delete" } | { kind: "copy"; target: string };

function isInternalAlias(name: string) {
  return /^llamacpp:[0-9a-f]{64}$/.test(name);
}

function deletionReason(name: string, duplicate: boolean, loaded: boolean) {
  if (loaded) return "Está cargado en la GPU: libéralo antes de borrarlo";
  if (isInternalAlias(name)) return "Alias interno de Ollama";
  if (duplicate) return "Nombre duplicado en Ollama: revísalo con `ollama list` antes de borrar";
  return undefined;
}

function countModelNames(models: InstalledModel[]) {
  const counts = new Map<string, number>();
  for (const model of models) counts.set(model.name, (counts.get(model.name) ?? 0) + 1);
  return counts;
}

function ModelName({ model }: { model: InstalledModel }) {
  const alias = isInternalAlias(model.name);
  const name = alias ? `llamacpp:${model.name.slice(9, 21)}…` : model.name;
  return (
    <td className="model-cell">
      <span className="model-name" title={alias ? model.name : undefined} aria-label={alias ? model.name : undefined}>{name}</span>
      <span className="model-digest muted mono">{model.digest.slice(0, 12)}</span>
      {alias && <span className="alias-tag muted">alias interno</span>}
    </td>
  );
}

function ModelRow({ model, loaded, duplicate, onChanged, onError }: { model: InstalledModel; loaded: boolean; duplicate: boolean; onChanged: () => void; onError: (e: string) => void }) {
  const [mode, setMode] = useState<Mode>({ kind: "idle" });
  const [loading, setLoading] = useState(false);
  const alias = isInternalAlias(model.name);
  const reason = deletionReason(model.name, duplicate, loaded);
  const act = (action: () => Promise<unknown>) => action().then(onChanged).catch((e) => onError(String(e))).finally(() => setMode({ kind: "idle" }));
  async function load() {
    setLoading(true);
    try {
      await loadModel(model.name);
      onChanged();
    } catch (error) {
      onError(String(error));
    } finally {
      setLoading(false);
    }
  }
  return (
    <tr>
      <ModelName model={model} />
      <td className="mono nowrap">{formatMb(model.size_mb)}</td>
      <td className="mono nowrap">{model.parameter_size} · {model.quantization}</td>
      <td className="mono nowrap">{model.context_length ? `${Math.round(model.context_length / 1024)}k` : "—"}</td>
      <td className="mono nowrap">{model.modified_at.slice(0, 10)}</td>
      <td>{loaded ? <span className="pill pill-running">En GPU</span> : null}</td>
      <td><div className="model-actions">
        {!loaded && !alias && <button className="btn-sm" disabled={loading} onClick={load}>{loading ? "Cargando…" : "Cargar"}</button>}
        {loaded && <button className="btn-sm" onClick={() => act(() => unloadModel(model.name))}>Liberar VRAM</button>}
        {!alias && (mode.kind === "copy" ? (
          <>
            <input type="text" aria-label="Nombre de la copia" value={mode.target} onChange={(e) => setMode({ kind: "copy", target: e.target.value })} />
            <button className="btn-sm" disabled={!mode.target.trim()} onClick={() => act(() => copyModel(model.name, mode.target.trim()))}>Crear copia</button>
          </>
        ) : (
          <button className="btn-sm" onClick={() => setMode({ kind: "copy", target: "" })}>Copiar</button>
        ))}
        {mode.kind === "confirm-delete" ? (
          <button className="btn-sm btn-danger btn-danger-confirm" disabled={!!reason} title={reason} onClick={() => act(() => deleteModel(model.name))}>Confirmar borrado</button>
        ) : (
          <button className="btn-sm btn-danger" disabled={!!reason} title={reason} onClick={() => setMode({ kind: "confirm-delete" })}>Borrar</button>
        )}
      </div></td>
    </tr>
  );
}

function orphanModels(models: InstalledModel[], loaded: LoadedModel[]) {
  const installedDigests = new Set(models.filter((model) => !isInternalAlias(model.name)).map((model) => model.digest));
  return loaded.filter((model) => !installedDigests.has(model.digest));
}

function OrphanRow({ model, onChanged, onError }: { model: LoadedModel; onChanged: () => void; onError: (error: string) => void }) {
  const [busy, setBusy] = useState(false);
  async function unload() {
    setBusy(true);
    try {
      await unloadModel(model.name);
      onChanged();
    } catch (error) {
      onError(String(error));
    } finally {
      setBusy(false);
    }
  }
  return (
    <tr>
      <td className="model-cell">
        <span className="model-name">{model.name}</span>
        <span className="model-digest muted mono">{model.digest.slice(0, 12)}</span>
        <span className="warning-tag" title="Ollama lo tiene cargado pero ya no está instalado; libéralo y vuelve a crearlo o descargarlo">sin manifiesto</span>
      </td>
      <td className="mono nowrap">{formatMb(model.size_mb)}</td>
      <td>—</td>
      <td className="mono nowrap">{model.context_length ? `${Math.round(model.context_length / 1024)}k` : "—"}</td>
      <td>—</td>
      <td><span className="pill pill-running">En GPU</span></td>
      <td><button className="btn-sm" disabled={busy} onClick={unload}>Liberar VRAM</button></td>
    </tr>
  );
}

export default function ModelsTab({ snapshot }: { snapshot: Snapshot | null }) {
  const [models, setModels] = useState<InstalledModel[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const refresh = useCallback(() => {
    listModels().then((list) => { setModels(list); setError(null); }).catch((e) => setError(String(e)));
  }, []);
  useEffect(refresh, [refresh]);

  const loadedModels = snapshot?.loaded.kind === "ok" ? snapshot.loaded.value : [];
  const loadedDigests = new Set(loadedModels.map((model) => model.digest));
  const orphans = orphanModels(models ?? [], loadedModels);
  const nameCounts = countModelNames(models ?? []);
  return (
    <div className="tab">
      <PullForm onDone={refresh} />
      {error && <p className="notice">{error}</p>}
      {models === null && !error && <p className="muted">Cargando modelos…</p>}
      {models && (
        <div className="table-scroll"><table className="models">
          <thead>
            <tr><th>Modelo</th><th>Tamaño</th><th>Parámetros</th><th>Contexto</th><th>Modificado</th><th>Estado</th><th>Acciones</th></tr>
          </thead>
          <tbody>
            {orphans.map((model) => <OrphanRow key={`orphan:${model.name}@${model.digest}`} model={model} onChanged={refresh} onError={setError} />)}
            {models.map((m) => <ModelRow key={`${m.name}@${m.digest}`} model={m} loaded={!isInternalAlias(m.name) && loadedDigests.has(m.digest)} duplicate={(nameCounts.get(m.name) ?? 0) > 1} onChanged={refresh} onError={setError} />)}
          </tbody>
        </table></div>
      )}
    </div>
  );
}
