import { useCallback, useEffect, useState } from "react";
import { copyModel, deleteModel, listModels, unloadModel } from "../lib/api";
import type { InstalledModel, Snapshot } from "../lib/types";
import { formatMb } from "../lib/format";
import PullForm from "./PullForm";

type Mode = { kind: "idle" } | { kind: "confirm-delete" } | { kind: "copy"; target: string };

function isInternalAlias(name: string) {
  return /^llamacpp:[0-9a-f]{64}$/.test(name);
}

function deletionReason(name: string, duplicate: boolean) {
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
  const alias = isInternalAlias(model.name);
  const reason = deletionReason(model.name, duplicate);
  const act = (action: () => Promise<unknown>) => action().then(onChanged).catch((e) => onError(String(e))).finally(() => setMode({ kind: "idle" }));
  return (
    <tr>
      <ModelName model={model} />
      <td className="mono nowrap">{formatMb(model.size_mb)}</td>
      <td className="mono nowrap">{model.parameter_size} · {model.quantization}</td>
      <td className="mono nowrap">{model.context_length ? `${Math.round(model.context_length / 1024)}k` : "—"}</td>
      <td className="mono nowrap">{model.modified_at.slice(0, 10)}</td>
      <td>{loaded ? <span className="pill pill-running">En GPU</span> : null}</td>
      <td><div className="model-actions">
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

export default function ModelsTab({ snapshot }: { snapshot: Snapshot | null }) {
  const [models, setModels] = useState<InstalledModel[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const refresh = useCallback(() => {
    listModels().then((list) => { setModels(list); setError(null); }).catch((e) => setError(String(e)));
  }, []);
  useEffect(refresh, [refresh]);

  const loadedDigests = new Set(snapshot?.loaded.kind === "ok" ? snapshot.loaded.value.map((m) => m.digest) : []);
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
            {models.map((m) => <ModelRow key={`${m.name}@${m.digest}`} model={m} loaded={!isInternalAlias(m.name) && loadedDigests.has(m.digest)} duplicate={(nameCounts.get(m.name) ?? 0) > 1} onChanged={refresh} onError={setError} />)}
          </tbody>
        </table></div>
      )}
    </div>
  );
}
