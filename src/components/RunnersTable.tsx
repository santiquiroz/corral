import type { Adapter, Runner } from "../lib/types";
import { formatMb } from "../lib/format";

export default function RunnersTable({ runners, adapters }: { runners: Runner[]; adapters: Adapter[] }) {
  if (runners.length === 0) return <p className="muted">No hay modelos en la GPU.</p>;
  const gpuNames = (runner: Runner) => runner.gpus
    .filter((g) => (g.dedicated_mb ?? 0) > 0)
    .map((g) => adapters.find((a) => a.luid === g.luid)?.name ?? "—")
    .join(" + ") || "—";
  return (
    <table className="runners">
      <thead>
        <tr><th>Modelo</th><th>GPU</th><th>VRAM</th><th>Compartida</th><th>CPU</th><th>RAM</th></tr>
      </thead>
      <tbody>
        {runners.map((r) => (
          <tr key={r.pid} className={r.spilling ? "is-spilling" : undefined}>
            <td>{r.model ?? `pid ${r.pid}`}</td>
            <td>{gpuNames(r)}</td>
            <td className="mono">{formatMb(r.dedicated_mb)}</td>
            <td className="mono">{formatMb(r.shared_mb)}</td>
            <td className="mono">{r.cpu_pct.toFixed(1)} %</td>
            <td className="mono">{formatMb(r.ram_mb)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
