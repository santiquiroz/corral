import { act, cleanup, render, screen, within } from "@testing-library/react";
import { vi } from "vitest";
import StatusTab from "./StatusTab";
import { runningSnapshot } from "../test/fixtures";

const api = vi.hoisted(() => ({ claudeMemStatus: vi.fn() }));
vi.mock("../lib/api", () => api);

const status = {
  provider: "openrouter", base_url: "http://127.0.0.1:11434/v1", model: "qwen3.5-mem:latest", queue_depth: 3,
  checks: [
    { id: "worker", level: "ok", message: "El worker responde" },
    { id: "context", level: "warn", message: "Contexto insuficiente" },
    { id: "model_installed", level: "fail", message: "El modelo no está instalado" },
  ],
};

function renderStatus() {
  return render(<StatusTab snapshot={runningSnapshot} onPause={() => {}} onUnload={() => {}} />);
}

beforeEach(() => api.claudeMemStatus.mockReset().mockResolvedValue(status));
afterEach(() => { cleanup(); vi.useRealTimers(); });

test("muestra proveedor, endpoint, modelo, cola e iconos con texto y borde de error", async () => {
  renderStatus();
  const card = await screen.findByRole("region", { name: "claude-mem" });
  expect(within(card).getByText("openrouter")).toBeInTheDocument();
  expect(within(card).getByText(status.base_url)).toBeInTheDocument();
  expect(within(card).getByText(status.model)).toBeInTheDocument();
  expect(within(card).getByText("Cola: 3")).toBeInTheDocument();
  expect(within(card).getByText("✓ El worker responde")).toBeInTheDocument();
  expect(within(card).getByText("! Contexto insuficiente")).toBeInTheDocument();
  expect(within(card).getByText("✕ El modelo no está instalado")).toBeInTheDocument();
  expect(card.style.borderColor).toBe("var(--bad)");
});

test("sin fallos usa borde normal y cola desconocida explícita", async () => {
  api.claudeMemStatus.mockResolvedValueOnce({ ...status, queue_depth: null, checks: status.checks.slice(0, 2) });
  renderStatus();
  const card = await screen.findByRole("region", { name: "claude-mem" });
  expect(card.style.borderColor).not.toBe("var(--bad)");
  expect(within(card).getByText("Cola: —")).toBeInTheDocument();
});

test("no muestra tarjeta si claude-mem no está instalado", async () => {
  api.claudeMemStatus.mockResolvedValueOnce(null);
  renderStatus();
  await act(async () => {});
  expect(api.claudeMemStatus).toHaveBeenCalledTimes(1);
  expect(screen.queryByRole("region", { name: "claude-mem" })).not.toBeInTheDocument();
});

test("consulta inmediatamente, cada diez segundos y deja de consultar al desmontar", async () => {
  vi.useFakeTimers();
  const view = renderStatus();
  await act(async () => {});
  expect(api.claudeMemStatus).toHaveBeenCalledTimes(1);
  await act(() => vi.advanceTimersByTimeAsync(9999));
  expect(api.claudeMemStatus).toHaveBeenCalledTimes(1);
  await act(() => vi.advanceTimersByTimeAsync(1));
  expect(api.claudeMemStatus).toHaveBeenCalledTimes(2);
  view.unmount();
  await act(() => vi.advanceTimersByTimeAsync(20000));
  expect(api.claudeMemStatus).toHaveBeenCalledTimes(2);
});

test("evita consultas superpuestas e ignora una respuesta pendiente al desmontar", async () => {
  vi.useFakeTimers();
  let finish!: (value: typeof status) => void;
  api.claudeMemStatus.mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
  const view = renderStatus();
  await act(() => vi.advanceTimersByTimeAsync(20000));
  expect(api.claudeMemStatus).toHaveBeenCalledTimes(1);
  view.unmount();
  await act(async () => finish(status));
  expect(screen.queryByRole("region", { name: "claude-mem" })).not.toBeInTheDocument();
  expect(vi.getTimerCount()).toBe(0);
});

test("muestra el error de consulta y lo limpia cuando recupera el estado", async () => {
  vi.useFakeTimers();
  api.claudeMemStatus.mockRejectedValueOnce("No se pudo consultar claude-mem");
  renderStatus();
  await act(async () => {});
  expect(screen.getByText("No se pudo consultar claude-mem")).toBeInTheDocument();
  await act(() => vi.advanceTimersByTimeAsync(10000));
  expect(screen.queryByText("No se pudo consultar claude-mem")).not.toBeInTheDocument();
  expect(screen.getByRole("region", { name: "claude-mem" })).toBeInTheDocument();
});
