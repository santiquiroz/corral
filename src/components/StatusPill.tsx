import type { Snapshot } from "../lib/types";

const LABELS: Record<Snapshot["status"], string> = {
  running: "Corriendo",
  spilling: "Desbordando VRAM",
  paused: "En pausa",
  down: "Caído",
};

export default function StatusPill({ snapshot }: { snapshot: Snapshot | null }) {
  const status = snapshot?.status ?? "down";
  const version = snapshot?.version ? ` · v${snapshot.version}` : "";
  return <span className={`pill pill-${status}`} role="status">{snapshot ? `${LABELS[status]}${version}` : "Conectando…"}</span>;
}
