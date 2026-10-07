import type { Snapshot } from "../lib/types";

const RX = 0x1685a;

export const runningSnapshot: Snapshot = {
  taken_at_ms: 1,
  state: { kind: "running" },
  status: "running",
  version: "0.35.1",
  adapters: { kind: "ok", value: [{ luid: RX, name: "AMD Radeon RX 7800 XT", total_mb: 16368, used_mb: 14691, ollama_mb: 7819 }] },
  runners: { kind: "ok", value: [{ pid: 7, model: "qwen3.5-mem:latest", gpus: [{ luid: RX, dedicated_mb: 7819, shared_mb: 0 }], dedicated_mb: 7819, shared_mb: 0, cpu_pct: 3.2, ram_mb: 900, spilling: false }] },
  loaded: { kind: "ok", value: [{ name: "qwen3.5-mem:latest", digest: "41d7", size_mb: 6279, vram_mb: 6279, context_length: 32768, expires_at: "" }] },
};

export const spillingSnapshot: Snapshot = {
  ...runningSnapshot,
  status: "spilling",
  runners: { kind: "ok", value: [{ pid: 7, model: "qwen3.5-mem:latest", gpus: [{ luid: RX, dedicated_mb: 7819, shared_mb: 729 }], dedicated_mb: 7819, shared_mb: 729, cpu_pct: 3.2, ram_mb: 900, spilling: true }] },
};

export const pausedSnapshot: Snapshot = {
  ...runningSnapshot,
  state: { kind: "paused", by: "user" },
  status: "paused",
  version: null,
  runners: { kind: "ok", value: [] },
  loaded: { kind: "unavailable", value: "Ollama no está corriendo" },
};
