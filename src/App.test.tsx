import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { vi } from "vitest";
import App from "./App";
import { pausedSnapshot, runningSnapshot } from "./test/fixtures";
import type { Runner } from "./lib/types";

const runner = runningSnapshot.runners.kind === "ok" ? runningSnapshot.runners.value[0] : {} as Runner;

const api = vi.hoisted(() => ({
  claudeMemStatus: vi.fn(() => Promise.resolve(null)),
  pauseOllama: vi.fn(() => Promise.resolve()),
  resumeOllama: vi.fn(() => Promise.resolve()),
  unloadModel: vi.fn((_name: string) => Promise.resolve()),
  takeNotices: vi.fn(() => Promise.resolve([] as string[])),
  onNotice: vi.fn((_cb: (message: string) => void) => Promise.resolve(() => {})),
}));
const current = vi.hoisted(() => ({ snapshot: null as unknown }));

vi.mock("./lib/api", () => api);
vi.mock("./hooks/useSnapshot", () => ({ useSnapshot: () => current.snapshot }));
vi.mock("./components/ModelsTab", () => ({ default: () => <p>modelos</p> }));
vi.mock("./components/SettingsTab", () => ({ default: () => <p>ajustes</p> }));

beforeEach(() => {
  vi.clearAllMocks();
  api.takeNotices.mockResolvedValue([]);
});

test("muestra los avisos pendientes al abrir el panel y permite cerrarlos", async () => {
  api.takeNotices.mockResolvedValueOnce(["Config inválida: cadencia cero"]);
  render(<App />);
  expect(await screen.findByText("Config inválida: cadencia cero")).toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "Cerrar" }));
  expect(screen.queryByText("Config inválida: cadencia cero")).not.toBeInTheDocument();
});

test("muestra los avisos recibidos por evento", async () => {
  render(<App />);
  await waitFor(() => expect(api.onNotice).toHaveBeenCalled());
  act(() => api.onNotice.mock.calls[0][0]("El aviso claude-mem falló: HTTP 500"));
  expect(screen.getByText("El aviso claude-mem falló: HTTP 500")).toBeInTheDocument();
});

test("muestra el nombre del producto", () => {
  render(<App />);
  expect(screen.getByRole("heading", { name: "Corral" })).toBeInTheDocument();
});

test("con Ollama corriendo el botón pausa", async () => {
  current.snapshot = runningSnapshot;
  render(<App />);
  await userEvent.click(screen.getByRole("button", { name: "Pausar Ollama" }));
  expect(api.pauseOllama).toHaveBeenCalled();
});

test("en pausa el botón reanuda", async () => {
  current.snapshot = pausedSnapshot;
  render(<App />);
  await userEvent.click(screen.getByRole("button", { name: "Reanudar Ollama" }));
  expect(api.resumeOllama).toHaveBeenCalled();
});

test("un error de la acción se muestra", async () => {
  current.snapshot = runningSnapshot;
  api.pauseOllama.mockRejectedValueOnce("quedaron procesos de Ollama vivos: [3]");
  render(<App />);
  await userEvent.click(screen.getByRole("button", { name: "Pausar Ollama" }));
  expect(await screen.findByText("quedaron procesos de Ollama vivos: [3]")).toBeInTheDocument();
});

test("Estado libera todos los nombres del runner y deshabilita acciones mientras espera", async () => {
  let finish!: () => void;
  api.unloadModel.mockImplementationOnce(() => new Promise<void>((resolve) => { finish = resolve; }));
  current.snapshot = { ...runningSnapshot, runners: { kind: "ok", value: [{ ...runner, model: "qwen3.5-mem:latest / otra:latest" }] } };
  render(<App />);
  const button = screen.getByRole("button", { name: "Liberar VRAM" });
  await userEvent.click(button);
  expect(button).toBeDisabled();
  expect(screen.getByRole("button", { name: "Pausar Ollama" })).toBeDisabled();
  expect(api.unloadModel).toHaveBeenCalledWith("qwen3.5-mem:latest");
  await act(async () => finish());
  await waitFor(() => expect(api.unloadModel).toHaveBeenCalledWith("otra:latest"));
  expect(button).toBeEnabled();
});

test("Estado muestra errores al liberar VRAM como avisos", async () => {
  current.snapshot = { ...runningSnapshot, runners: { kind: "ok", value: [{ ...runner, model: "qwen3.5-mem:latest / otra:latest" }] } };
  api.unloadModel.mockRejectedValueOnce("No se pudo liberar el modelo");
  render(<App />);
  await userEvent.click(screen.getByRole("button", { name: "Liberar VRAM" }));
  expect(await screen.findByText(/No se pudo liberar el modelo/)).toBeInTheDocument();
  expect(api.unloadModel).toHaveBeenCalledWith("otra:latest");
});

test("Estado no permite liberar un runner sin modelo conocido", () => {
  current.snapshot = { ...runningSnapshot, runners: { kind: "ok", value: [{ ...runner, model: null }] } };
  render(<App />);
  expect(screen.getByRole("columnheader", { name: "Acciones" })).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Liberar VRAM" })).not.toBeInTheDocument();
});
