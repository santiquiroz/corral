import { emit } from "@tauri-apps/api/event";
import { mockIPC } from "@tauri-apps/api/mocks";
import type { Config, GpuProfile, Snapshot } from "./lib/types";
import { pausedSnapshot, spillingSnapshot } from "./test/fixtures";

type Calls = { cmd: string; args: unknown }[];

export function installE2eMocks() {
  const calls: Calls = [];
  const loadedDigest = "1d171a12578e05912e54f89617a52d0c808b000c1af2fdb70abc2c6533cf7426";
  let current: Snapshot = {
    ...spillingSnapshot,
    version: "0.40",
    loaded: { kind: "ok" as const, value: [{ name: "qwen3.5-mem:latest", digest: loadedDigest, size_mb: 6289, vram_mb: 6289, context_length: 32768, expires_at: "" }] },
  };
  const models = [
    { name: "qwen3.5-mem:latest", digest: loadedDigest, size_mb: 6289, family: "qwen35", parameter_size: "9.7B", quantization: "Q4_K_M", context_length: 262144, modified_at: "2026-09-30" },
    { name: "qwen3.5-mem:latest", digest: "f779ea1ead1e" + "0".repeat(52), size_mb: 6289, family: "qwen35", parameter_size: "9.7B", quantization: "Q4_K_M", context_length: 262144, modified_at: "2026-09-30" },
    { name: `llamacpp:${loadedDigest}`, digest: loadedDigest, size_mb: 6289, family: "qwen35", parameter_size: "9.7B", quantization: "Q4_K_M", context_length: 262144, modified_at: "2026-09-30" },
    { name: "hf.co/openbmb/MiniCPM5-2B-GGUF:Q4_K_M", digest: "e680", size_mb: 1526, family: "minicpm", parameter_size: "2.5B", quantization: "Q4_K_M", context_length: 131072, modified_at: "2026-10-05" },
  ];
  const installedModels = new URLSearchParams(window.location.search).get("scenario") === "orphan-loaded"
    ? models.filter((model) => model.name !== "qwen3.5-mem:latest")
    : models;
  let config: Config = { ollama_url: "http://127.0.0.1:11434", ollama_install_dir: "C:\\Ollama", poll_panel_secs: 2, poll_tray_secs: 10, spill_floor_mb: 64, resume_timeout_secs: 30, load_keep_alive: "30m", gpu_profile: { kind: "auto" }, igpu_enabled: false, hooks: [] };

  async function loadMockModel(name: string) {
    await new Promise((resolve) => setTimeout(resolve, 350));
    const model = installedModels.find((installed) => installed.name === name);
    if (!model) throw new Error("El modelo no está instalado");
    const loaded = current.loaded.kind === "ok" ? current.loaded.value : [];
    current = { ...current, loaded: { kind: "ok", value: [...loaded, { name, digest: model.digest, size_mb: model.size_mb, vram_mb: model.size_mb, context_length: model.context_length, expires_at: "" }] } };
    await emit("snapshot", current);
    return null;
  }
  mockIPC(
    (cmd, args) => {
      calls.push({ cmd, args });
      switch (cmd) {
        case "get_snapshot": return current;
        case "take_notices": return [];
        case "list_models": return installedModels;
        case "load_model": return loadMockModel((args as { name: string }).name);
        case "pause_ollama":
          current = pausedSnapshot;
          void emit("snapshot", current);
          return { unloaded: ["qwen3.5-mem:latest"], killed: [1, 2, 3] };
        case "delete_model": return null;
        case "get_config": return config;
        case "list_ollama_gpus": return [
          { id: "0", filter_id: "0", library: "ROCm", description: "AMD Radeon RX 7800 XT", kind: "discrete", total_mb: 16384, dropped: false },
          { id: "1", filter_id: "1", library: "Vulkan", description: "AMD Radeon(TM) Graphics", kind: "integrated", total_mb: null, dropped: true },
        ];
        case "apply_gpu_profile": {
          const { profile, igpuEnabled } = args as { profile: GpuProfile; igpuEnabled: boolean };
          config = { ...config, gpu_profile: profile, igpu_enabled: igpuEnabled };
          return null;
        }
        case "save_config": config = (args as { config: typeof config }).config; return null;
        case "plugin:autostart|is_enabled": return false;
        default: return null;
      }
    },
    { shouldMockEvents: true },
  );
  Object.assign(window, { __corralCalls: calls });
}
