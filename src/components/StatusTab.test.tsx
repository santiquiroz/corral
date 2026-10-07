import { render, screen, within } from "@testing-library/react";
import StatusTab from "./StatusTab";
import { pausedSnapshot, runningSnapshot, spillingSnapshot } from "../test/fixtures";

test("muestra la GPU con la parte de Ollama, otros y libre", () => {
  render(<StatusTab snapshot={runningSnapshot} onPause={() => {}} onUnload={() => {}} />);
  const gpus = screen.getByRole("region", { name: "Uso de VRAM por GPU" });
  expect(within(gpus).getByText("AMD Radeon RX 7800 XT")).toBeInTheDocument();
  expect(screen.getByLabelText("Ollama 7.6 GB")).toBeInTheDocument();
  expect(screen.getByLabelText("Otros procesos 6.7 GB")).toBeInTheDocument();
  expect(screen.getByLabelText("Libre 1.6 GB")).toBeInTheDocument();
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
});

test("avisa del desborde con el modelo y la memoria compartida", () => {
  render(<StatusTab snapshot={spillingSnapshot} onPause={() => {}} onUnload={() => {}} />);
  expect(screen.getByRole("alert")).toHaveTextContent("qwen3.5-mem:latest está desbordando 729 MB");
});

test("deshabilita la pausa del aviso mientras hay una acción en curso", () => {
  render(<StatusTab snapshot={spillingSnapshot} onPause={() => {}} onUnload={() => {}} busy />);
  expect(screen.getByRole("button", { name: "Pausar Ollama" })).toBeDisabled();
});

test("muestra el runner con su modelo y GPU", () => {
  render(<StatusTab snapshot={runningSnapshot} onPause={() => {}} onUnload={() => {}} />);
  const row = screen.getByRole("row", { name: /qwen3.5-mem:latest/ });
  expect(row).toHaveTextContent("AMD Radeon RX 7800 XT");
  expect(row).toHaveTextContent("7.6 GB");
});

test("lista todas las GPU del runner con memoria dedicada", () => {
  const runner = runningSnapshot.runners.kind === "ok" ? runningSnapshot.runners.value[0] : undefined;
  const adapters = runningSnapshot.adapters.kind === "ok" ? runningSnapshot.adapters.value : [];
  const snap = {
    ...runningSnapshot,
    adapters: { kind: "ok" as const, value: [...adapters, { luid: 2, name: "GPU secundaria", total_mb: 8192, used_mb: 2000, ollama_mb: 1000 }] },
    runners: { kind: "ok" as const, value: [{ ...runner!, gpus: [{ luid: adapters[0].luid, dedicated_mb: 7819, shared_mb: 0 }, { luid: 2, dedicated_mb: 1000, shared_mb: 729 }, { luid: 3, dedicated_mb: 0, shared_mb: 0 }] }] },
  };
  render(<StatusTab snapshot={snap} onPause={() => {}} onUnload={() => {}} />);
  expect(screen.getByRole("row", { name: /qwen3.5-mem:latest/ })).toHaveTextContent("AMD Radeon RX 7800 XT + GPU secundaria");
});

test("sin memoria dedicada en GPU muestra un guion", () => {
  const runner = runningSnapshot.runners.kind === "ok" ? runningSnapshot.runners.value[0] : undefined;
  const snap = { ...runningSnapshot, runners: { kind: "ok" as const, value: [{ ...runner!, gpus: [] }] } };
  render(<StatusTab snapshot={snap} onPause={() => {}} onUnload={() => {}} />);
  expect(screen.getByRole("row", { name: /qwen3.5-mem:latest/ })).toHaveTextContent("—");
});

test("con GPU sin datos explica la razón", () => {
  const snap = { ...runningSnapshot, adapters: { kind: "unavailable" as const, value: "PDH falló" } };
  render(<StatusTab snapshot={snap} onPause={() => {}} onUnload={() => {}} />);
  expect(screen.getByText(/Sin datos de GPU: PDH falló/)).toBeInTheDocument();
});

test("en pausa no hay runners y lo dice", () => {
  render(<StatusTab snapshot={pausedSnapshot} onPause={() => {}} onUnload={() => {}} />);
  expect(screen.getByText("No hay modelos en la GPU.")).toBeInTheDocument();
});
