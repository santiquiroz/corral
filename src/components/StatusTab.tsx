import type { Snapshot } from "../lib/types";
import GpuBars from "./GpuBars";
import RunnersTable from "./RunnersTable";
import SpillAlert from "./SpillAlert";

export default function StatusTab({ snapshot, onPause, onUnload, busy = false }: { snapshot: Snapshot; onPause: () => void; onUnload: (model: string) => void; busy?: boolean }) {
  const runners = snapshot.runners.kind === "ok" ? snapshot.runners.value : [];
  const adapters = snapshot.adapters.kind === "ok" ? snapshot.adapters.value : [];
  return (
    <div className="tab">
      <SpillAlert runners={runners} onPause={onPause} busy={busy} />
      <GpuBars adapters={snapshot.adapters} />
      <h2>Modelos en ejecución</h2>
      <div className="table-scroll">
        <RunnersTable runners={runners} adapters={adapters} onUnload={onUnload} busy={busy} />
      </div>
    </div>
  );
}
