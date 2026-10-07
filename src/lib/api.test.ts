import { vi } from "vitest";
import { applyGpuProfile, claudeMemStatus, listOllamaGpus, loadModel } from "./api";

const invoke = vi.hoisted(() => vi.fn(() => Promise.resolve()));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

test("cargar invoca el comando con el nombre exacto", async () => {
  await loadModel("hf.co/modelo:Q4_K_M");
  expect(invoke).toHaveBeenCalledWith("load_model", { name: "hf.co/modelo:Q4_K_M" });
});

test("consulta las GPUs detectadas por Ollama", async () => {
  await listOllamaGpus();
  expect(invoke).toHaveBeenCalledWith("list_ollama_gpus");
});

test("consulta el estado de claude-mem", async () => {
  await claudeMemStatus();
  expect(invoke).toHaveBeenCalledWith("claude_mem_status");
});

test("aplica el perfil GPU con argumentos del comando", async () => {
  const profile = { kind: "single" as const, library: "ROCm", filter_id: "0" };
  await applyGpuProfile(profile, true);
  expect(invoke).toHaveBeenCalledWith("apply_gpu_profile", { profile, igpuEnabled: true });
});
