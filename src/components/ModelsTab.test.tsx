import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { vi } from "vitest";
import ModelsTab from "./ModelsTab";
import { runningSnapshot } from "../test/fixtures";
import type { PullProgress, Snapshot } from "../lib/types";

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
  { name: "qwen3.5-mem:latest", digest: "41d7", size_mb: 6289, family: "qwen35", parameter_size: "9.7B", quantization: "Q4_K_M", context_length: 262144, modified_at: "2026-09-30T18:20:00Z" },
  { name: "hf.co/openbmb/MiniCPM5-2B-GGUF:Q4_K_M", digest: "e680", size_mb: 1526, family: "minicpm", parameter_size: "2.5B", quantization: "Q4_K_M", context_length: 131072, modified_at: "2026-10-05" },
];

const loadedDigest = "1d171a12578e05912e54f89617a52d0c808b000c1af2fdb70abc2c6533cf7426";
const staleDigest = "f779ea1ead1e" + "0".repeat(52);
const aliasName = `llamacpp:${loadedDigest}`;
const duplicateModels = [
  { ...installed[0], digest: loadedDigest },
  { ...installed[0], digest: staleDigest },
  { ...installed[0], name: aliasName, digest: loadedDigest },
  installed[1],
];
const duplicateSnapshot: Snapshot = {
  ...runningSnapshot,
  loaded: { kind: "ok", value: [{ name: installed[0].name, digest: loadedDigest, size_mb: 6289, vram_mb: 6289, context_length: 32768, expires_at: "" }] },
};

async function renderDuplicates() {
  api.listModels.mockResolvedValueOnce(duplicateModels);
  render(<ModelsTab snapshot={duplicateSnapshot} />);
  await screen.findByRole("row", { name: /MiniCPM5/ });
  return screen.getAllByRole("row").slice(1);
}

beforeEach(() => {
  vi.clearAllMocks();
  api.listModels.mockResolvedValue(installed);
});

test("lista los modelos y marca el cargado", async () => {
  render(<ModelsTab snapshot={runningSnapshot} />);
  const row = await screen.findByRole("row", { name: /qwen3.5-mem:latest/ });
  expect(within(row).getByText("En GPU")).toBeInTheDocument();
  expect(within(row).getByText("6.1 GB")).toBeInTheDocument();
  expect(within(row).getByRole("button", { name: "Liberar VRAM" })).toBeInTheDocument();
});

test("muestra la fecha de modificación sin la hora", async () => {
  render(<ModelsTab snapshot={runningSnapshot} />);
  const row = await screen.findByRole("row", { name: /qwen3.5-mem:latest/ });
  expect(screen.getByRole("columnheader", { name: "Modificado" })).toBeInTheDocument();
  expect(within(row).getByRole("cell", { name: "2026-09-30" })).toBeInTheDocument();
  expect(screen.queryByText("2026-09-30T18:20:00Z")).not.toBeInTheDocument();
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


test("solo el digest cargado del modelo visible aparece en GPU", async () => {
  const rows = await renderDuplicates();
  expect(screen.getAllByText("En GPU")).toHaveLength(1);
  expect(within(rows[0]).getByText("En GPU")).toBeInTheDocument();
  expect(within(rows[1]).queryByText("En GPU")).not.toBeInTheDocument();
  expect(within(rows[2]).queryByText("En GPU")).not.toBeInTheDocument();
  expect(within(rows[0]).getByText(loadedDigest.slice(0, 12))).toBeInTheDocument();
  expect(within(rows[1]).getByText(staleDigest.slice(0, 12))).toBeInTheDocument();
});

test("impide borrar nombres duplicados y alias internos sin ocultar las copias normales", async () => {
  const rows = await renderDuplicates();
  const duplicateReason = "Nombre duplicado en Ollama: revísalo con `ollama list` antes de borrar";
  for (const row of rows.slice(0, 2)) {
    expect(within(row).getByRole("button", { name: "Borrar" })).toBeDisabled();
    expect(within(row).getByRole("button", { name: "Borrar" })).toHaveAttribute("title", duplicateReason);
    expect(within(row).getByRole("button", { name: "Copiar" })).toBeEnabled();
  }
  expect(within(rows[2]).getByRole("button", { name: "Borrar" })).toBeDisabled();
  expect(within(rows[2]).getByRole("button", { name: "Borrar" })).toHaveAttribute("title", "Alias interno de Ollama");
  expect(within(rows[2]).queryByRole("button", { name: "Copiar" })).not.toBeInTheDocument();
  expect(within(rows[3]).getByRole("button", { name: "Borrar" })).toBeEnabled();
});

test("abrevia el alias interno y conserva su nombre completo accesible", async () => {
  const rows = await renderDuplicates();
  const alias = within(rows[2]).getByText("llamacpp:1d171a12578e…");
  expect(alias).toHaveAttribute("title", aliasName);
  expect(alias).toHaveAttribute("aria-label", aliasName);
  expect(within(rows[2]).getByText("alias interno")).toBeInTheDocument();
});

test("liberar VRAM envía el nombre exacto del digest cargado", async () => {
  const rows = await renderDuplicates();
  await userEvent.click(within(rows[0]).getByRole("button", { name: "Liberar VRAM" }));
  expect(api.unloadModel).toHaveBeenCalledWith("qwen3.5-mem:latest");
  expect(within(rows[1]).queryByRole("button", { name: "Liberar VRAM" })).not.toBeInTheDocument();
});

test("los modelos duplicados no producen advertencias de claves React", async () => {
  const error = vi.spyOn(console, "error").mockImplementation(() => {});
  try {
    await renderDuplicates();
    expect(error.mock.calls.some((args) => args.some((arg) => String(arg).includes("same key")))).toBe(false);
  } finally {
    error.mockRestore();
  }
});
