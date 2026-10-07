import { useEffect, useState } from "react";
import { onPullDone, onPullProgress, pullModel } from "../lib/api";
import type { PullProgress } from "../lib/types";

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

  const percent = progress?.total ? Math.round((progress.completed ?? 0) / progress.total * 100) : null;
  return (
    <form className="row" onSubmit={submit}>
      <input type="text" aria-label="Modelo a descargar" placeholder="ej. qwen3.5:4b" value={name} onChange={(e) => setName(e.target.value)} />
      <button type="submit" disabled={progress !== null}>Descargar</button>
      {progress && (
        <span className="mono">
          {progress.status}{percent !== null ? ` · ${percent} %` : ""}
          {percent !== null ? <span className="progress" role="progressbar" aria-valuenow={percent} aria-valuemin={0} aria-valuemax={100}><span style={{ width: `${percent}%` }} /></span> : null}
        </span>
      )}
      {error && <span role="alert">{error}</span>}
    </form>
  );
}
