import { useEffect, useState } from "react";
import { onPullDone, onPullProgress, pullModel } from "../lib/api";
import type { PullProgress } from "../lib/types";
import { formatPct } from "../lib/format";

export default function PullForm({ onDone }: { onDone: () => void }) {
  const [name, setName] = useState("");
  const [progress, setProgress] = useState<PullProgress | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const stopProgress = onPullProgress(setProgress);
    const stopDone = onPullDone((done) => {
      setProgress(null);
      setError(done.error);
      if (!done.error) onDone();
    });
    return () => {
      stopProgress.then((stop) => stop());
      stopDone.then((stop) => stop());
    };
  }, [onDone]);

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    if (!name.trim()) return;
    setError(null);
    try {
      await pullModel(name.trim());
    } catch (e) {
      setError(String(e));
    }
  }

  const pct = progress?.total ? formatPct(progress.completed ?? 0, progress.total) : null;
  return (
    <form className="row" onSubmit={submit}>
      <input type="text" aria-label="Modelo a descargar" placeholder="ej. qwen3.5:4b" value={name} onChange={(e) => setName(e.target.value)} />
      <button type="submit" disabled={progress !== null}>Descargar</button>
      {progress && (
        <span className="mono">
          {progress.status}{pct ? ` · ${pct}` : ""}
          {progress.total ? <span className="progress"><span style={{ width: pct ?? "0%" }} /></span> : null}
        </span>
      )}
      {error && <span role="alert">{error}</span>}
    </form>
  );
}
