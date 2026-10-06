import { useCallback, useEffect, useState } from "react";
import { copyModel, deleteModel, listModels, unloadModel } from "../lib/api";
import type { InstalledModel, Snapshot } from "../lib/types";
import { formatMb } from "../lib/format";
import PullForm from "./PullForm";

type Mode = { kind: "idle" } | { kind: "confirm-delete" } | { kind: "copy"; target: string };

function ModelRow({ model, loaded, onChanged, onError }: { model: InstalledModel; loaded: boolean; onChanged: () => void; onError: (e: string) => void }) {
  const [mode, setMode] = useState<Mode>({ kind: "idle" });
  const act = (action: () => Promise<unknown>) => action().then(onChanged).catch((e) => onError(String(e))).finally(() => setMode({ kind: "idle" }));
  return (
    <tr>
      <td>{model.name}</td>
      <td className="mono">{formatMb(model.size_mb)}</td>
      <td>{model.parameter_size} · {model.quantization}</td>
      <td className="mono">{model.context_length ? `${Math.round(model.context_length / 1024)}k` : "—"}</td>
      <td>{loaded ? <span className="pill pill-running">En GPU</span> : null}</td>
      <td className="row">
        {loaded && <button onClick={() => act(() => unloadModel(model.name))}>Descargar de memoria</button>}
        {mode.kind === "copy" ? (
          <>
            <input type="text" aria-label="Nombre de la copia" value={mode.target} onChange={(e) => setMode({ kind: "copy", target: e.target.value })} />
            <button disabled={!mode.target.trim()} onClick={() => act(() => copyModel(model.name, mode.target.trim()))}>Crear copia</button>
          </>
        ) : (
          <button onClick={() => setMode({ kind: "copy", target: "" })}>Copiar</button>
        )}
        {mode.kind === "confirm-delete" ? (
          <button className="primary" onClick={() => act(() => deleteModel(model.name))}>Confirmar borrado</button>
        ) : (
          <button onClick={() => setMode({ kind: "confirm-delete" })}>Borrar</button>
        )}
      </td>
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

  const loadedNames = new Set(snapshot?.loaded.kind === "ok" ? snapshot.loaded.value.map((m) => m.name) : []);
  return (
    <div className="tab">
      <PullForm onDone={refresh} />
      {error && <p className="notice">{error}</p>}
      {models === null && !error && <p className="muted">Cargando modelos…</p>}
      {models && (
        <table>
          <thead>
            <tr><th>Modelo</th><th>Tamaño</th><th>Parámetros</th><th>Contexto</th><th>Estado</th><th>Acciones</th></tr>
          </thead>
          <tbody>
            {models.map((m) => <ModelRow key={m.name} model={m} loaded={loadedNames.has(m.name)} onChanged={refresh} onError={setError} />)}
          </tbody>
        </table>
      )}
    </div>
  );
}
