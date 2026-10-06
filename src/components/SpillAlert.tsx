import type { Runner } from "../lib/types";
import { formatMb } from "../lib/format";

export default function SpillAlert({ runners, onPause }: { runners: Runner[]; onPause: () => void }) {
  const spilling = runners.filter((r) => r.spilling);
  if (spilling.length === 0) return null;
  return (
    <div role="alert" className="spill">
      {spilling.map((r) => (
        <p key={r.pid}>
          <strong>{r.model ?? `runner ${r.pid}`} está desbordando {formatMb(r.shared_mb)}</strong> a la memoria compartida.
          La generación puede ir de 5 a 15 veces más lenta y el escritorio puede trabarse.
        </p>
      ))}
      <button onClick={onPause}>Pausar Ollama</button>
    </div>
  );
}
