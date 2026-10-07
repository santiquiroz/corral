import { emit } from "@tauri-apps/api/event";
import { mockIPC } from "@tauri-apps/api/mocks";
import { pausedSnapshot, spillingSnapshot } from "./test/fixtures";

type Calls = { cmd: string; args: unknown }[];

export function installE2eMocks() {
  const calls: Calls = [];
  let current = spillingSnapshot;
  const models = [
    { name: "qwen3.5-mem:latest", digest: "41d7", size_mb: 6289, family: "qwen35", parameter_size: "9.7B", quantization: "Q4_K_M", context_length: 262144, modified_at: "2026-09-30" },
  ];
  mockIPC(
    (cmd, args) => {
      calls.push({ cmd, args });
      switch (cmd) {
        case "get_snapshot": return current;
        case "take_notices": return [];
        case "list_models": return models;
        case "pause_ollama":
          current = pausedSnapshot;
          void emit("snapshot", current);
          return { unloaded: ["qwen3.5-mem:latest"], killed: [1, 2, 3] };
        case "delete_model": return null;
        case "get_config": return { ollama_url: "http://127.0.0.1:11434", ollama_install_dir: "C:\\Ollama", poll_panel_secs: 2, poll_tray_secs: 10, spill_floor_mb: 64, resume_timeout_secs: 30, hooks: [] };
        case "plugin:autostart|is_enabled": return false;
        default: return null;
      }
    },
    { shouldMockEvents: true },
  );
  Object.assign(window, { __corralCalls: calls });
}
