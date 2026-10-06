import type { Adapter, Runner } from "../lib/types";
import { formatMb } from "../lib/format";

export default function RunnersTable({ runners, adapters }: { runners: Runner[]; adapters: Adapter[] }) {
  if (runners.length === 0) return <p className="muted">No hay modelos en la GPU.</p>;
  const gpuName = (luid: number | null) => adapters.find((a) => a.luid === luid)?.name ?? "—";
  return (
    <table className="runners">
      <thead>
        <tr><th>Modelo</th><th>GPU</th><th>VRAM</th><th>Compartida</th><th>CPU</th><th>RAM</th></tr>
      </thead>
      <tbody>
        {runners.map((r) => (
          <tr key={r.pid} className={r.spilling ? "is-spilling" : undefined}>
            <td>{r.model ?? `pid ${r.pid}`}</td>
            <td>{gpuName(r.luid)}</td>
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
