import { vi } from "vitest";
import { loadModel } from "./api";

const invoke = vi.hoisted(() => vi.fn(() => Promise.resolve()));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

test("cargar invoca el comando con el nombre exacto", async () => {
  await loadModel("hf.co/modelo:Q4_K_M");
  expect(invoke).toHaveBeenCalledWith("load_model", { name: "hf.co/modelo:Q4_K_M" });
});
