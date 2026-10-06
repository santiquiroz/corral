import type { Adapter, Field } from "../lib/types";
import { formatMb } from "../lib/format";

function Segment({ label, mb, total, kind }: { label: string; mb: number; total: number; kind: string }) {
  const width = total > 0 ? `${(Math.max(mb, 0) / total) * 100}%` : "0%";
  return <span className={`seg seg-${kind}`} style={{ width }} aria-label={`${label} ${formatMb(Math.max(mb, 0))}`} title={`${label} ${formatMb(Math.max(mb, 0))}`} />;
}

function AdapterBar({ adapter }: { adapter: Adapter }) {
  const used = adapter.used_mb ?? adapter.ollama_mb;
  const others = adapter.used_mb === null ? 0 : adapter.used_mb - adapter.ollama_mb;
  return (
    <div className="gpu">
      <div className="gpu-head">
        <strong>{adapter.name}</strong>
        <span className="mono">{formatMb(adapter.used_mb)} / {formatMb(adapter.total_mb)}</span>
      </div>
      <div className="bar">
        <Segment label="Ollama" mb={adapter.ollama_mb} total={adapter.total_mb} kind="ollama" />
        <Segment label="Otros procesos" mb={others} total={adapter.total_mb} kind="others" />
        <Segment label="Libre" mb={adapter.total_mb - used} total={adapter.total_mb} kind="free" />
      </div>
    </div>
  );
}

export default function GpuBars({ adapters }: { adapters: Field<Adapter[]> }) {
  if (adapters.kind === "unavailable") return <p className="muted">Sin datos de GPU: {adapters.value}</p>;
  return <section className="gpus" aria-label="Uso de VRAM por GPU">{adapters.value.map((a) => <AdapterBar key={a.luid} adapter={a} />)}</section>;
}
