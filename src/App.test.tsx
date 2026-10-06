import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { vi } from "vitest";
import App from "./App";
import { pausedSnapshot, runningSnapshot } from "./test/fixtures";

const api = vi.hoisted(() => ({
  pauseOllama: vi.fn(() => Promise.resolve()),
  resumeOllama: vi.fn(() => Promise.resolve()),
}));
const current = vi.hoisted(() => ({ snapshot: null as unknown }));

vi.mock("./lib/api", () => api);
vi.mock("./hooks/useSnapshot", () => ({ useSnapshot: () => current.snapshot }));
vi.mock("./components/ModelsTab", () => ({ default: () => <p>modelos</p> }));
vi.mock("./components/SettingsTab", () => ({ default: () => <p>ajustes</p> }));

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
