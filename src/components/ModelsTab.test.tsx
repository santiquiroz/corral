import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { vi } from "vitest";
import ModelsTab from "./ModelsTab";
import { runningSnapshot } from "../test/fixtures";
import type { PullProgress } from "../lib/types";

const api = vi.hoisted(() => ({
  listModels: vi.fn(),
  unloadModel: vi.fn(() => Promise.resolve()),
  deleteModel: vi.fn(() => Promise.resolve()),
  copyModel: vi.fn(() => Promise.resolve()),
  pullModel: vi.fn(() => Promise.resolve()),
  onPullProgress: vi.fn((_cb: (progress: PullProgress) => void) => Promise.resolve(() => {})),
  onPullDone: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("../lib/api", () => api);

const installed = [
  { name: "qwen3.5-mem:latest", digest: "41d7", size_mb: 6289, family: "qwen35", parameter_size: "9.7B", quantization: "Q4_K_M", context_length: 262144, modified_at: "2026-09-30" },
  { name: "hf.co/openbmb/MiniCPM5-2B-GGUF:Q4_K_M", digest: "e680", size_mb: 1526, family: "minicpm", parameter_size: "2.5B", quantization: "Q4_K_M", context_length: 131072, modified_at: "2026-10-05" },
];

beforeEach(() => {
  vi.clearAllMocks();
  api.listModels.mockResolvedValue(installed);
});

test("lista los modelos y marca el cargado", async () => {
  render(<ModelsTab snapshot={runningSnapshot} />);
  const row = await screen.findByRole("row", { name: /qwen3.5-mem:latest/ });
  expect(within(row).getByText("En GPU")).toBeInTheDocument();
  expect(within(row).getByText("6.1 GB")).toBeInTheDocument();
  expect(within(row).getByRole("button", { name: "Descargar de memoria" })).toBeInTheDocument();
});

test("borrar pide confirmación en dos pasos y envía el nombre exacto", async () => {
  render(<ModelsTab snapshot={runningSnapshot} />);
  const row = await screen.findByRole("row", { name: /MiniCPM5/ });
  await userEvent.click(within(row).getByRole("button", { name: "Borrar" }));
  expect(api.deleteModel).not.toHaveBeenCalled();
  await userEvent.click(within(row).getByRole("button", { name: "Confirmar borrado" }));
  expect(api.deleteModel).toHaveBeenCalledWith("hf.co/openbmb/MiniCPM5-2B-GGUF:Q4_K_M");
});

test("copiar usa el nombre nuevo escrito", async () => {
  render(<ModelsTab snapshot={runningSnapshot} />);
  const row = await screen.findByRole("row", { name: /MiniCPM5/ });
  await userEvent.click(within(row).getByRole("button", { name: "Copiar" }));
  await userEvent.type(within(row).getByLabelText("Nombre de la copia"), "minicpm-32k");
  await userEvent.click(within(row).getByRole("button", { name: "Crear copia" }));
  expect(api.copyModel).toHaveBeenCalledWith("hf.co/openbmb/MiniCPM5-2B-GGUF:Q4_K_M", "minicpm-32k");
});

test("descargar un modelo nuevo llama a pull con el nombre", async () => {
  render(<ModelsTab snapshot={runningSnapshot} />);
  await userEvent.type(screen.getByLabelText("Modelo a descargar"), "qwen3.5:4b");
  await userEvent.click(screen.getByRole("button", { name: "Descargar" }));
  expect(api.pullModel).toHaveBeenCalledWith("qwen3.5:4b");
});

test("el progreso de descarga muestra porcentaje y ancho válidos", async () => {
  render(<ModelsTab snapshot={runningSnapshot} />);
  await screen.findByRole("row", { name: /qwen3.5-mem:latest/ });
  await waitFor(() => expect(api.onPullProgress).toHaveBeenCalled());
  const onProgress = api.onPullProgress.mock.calls[0][0];
  act(() => onProgress({ name: "x", status: "downloading", completed: 50, total: 100 }));
  const bar = screen.getByRole("progressbar");
  expect(bar).toHaveAttribute("aria-valuenow", "50");
  expect(bar).toHaveAttribute("aria-valuemin", "0");
  expect(bar).toHaveAttribute("aria-valuemax", "100");
  expect(bar.querySelector("span")).toHaveStyle({ width: "50%" });
  expect(screen.getByText(/downloading · 50 %/)).toBeInTheDocument();
});

test("si Ollama no responde lo dice", async () => {
  api.listModels.mockRejectedValueOnce("Ollama no responde: conexión rechazada");
  render(<ModelsTab snapshot={null} />);
  expect(await screen.findByText(/Ollama no responde/)).toBeInTheDocument();
});
