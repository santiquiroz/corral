import type { Snapshot } from "../lib/types";
import GpuBars from "./GpuBars";
import RunnersTable from "./RunnersTable";
import SpillAlert from "./SpillAlert";

export default function StatusTab({ snapshot, onPause }: { snapshot: Snapshot; onPause: () => void }) {
  const runners = snapshot.runners.kind === "ok" ? snapshot.runners.value : [];
  const adapters = snapshot.adapters.kind === "ok" ? snapshot.adapters.value : [];
  return (
    <div className="tab">
      <SpillAlert runners={runners} onPause={onPause} />
      <GpuBars adapters={snapshot.adapters} />
      <h2>Modelos en ejecución</h2>
      <RunnersTable runners={runners} adapters={adapters} />
    </div>
  );
}
