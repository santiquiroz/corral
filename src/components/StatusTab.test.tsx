import { render, screen, within } from "@testing-library/react";
import StatusTab from "./StatusTab";
import { pausedSnapshot, runningSnapshot, spillingSnapshot } from "../test/fixtures";

test("muestra la GPU con la parte de Ollama, otros y libre", () => {
  render(<StatusTab snapshot={runningSnapshot} onPause={() => {}} />);
  const gpus = screen.getByRole("region", { name: "Uso de VRAM por GPU" });
  expect(within(gpus).getByText("AMD Radeon RX 7800 XT")).toBeInTheDocument();
  expect(screen.getByLabelText("Ollama 7.6 GB")).toBeInTheDocument();
  expect(screen.getByLabelText("Otros procesos 6.7 GB")).toBeInTheDocument();
  expect(screen.getByLabelText("Libre 1.6 GB")).toBeInTheDocument();
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
});

test("avisa del desborde con el modelo y la memoria compartida", () => {
  render(<StatusTab snapshot={spillingSnapshot} onPause={() => {}} />);
  expect(screen.getByRole("alert")).toHaveTextContent("qwen3.5-mem:latest está desbordando 729 MB");
});

test("muestra el runner con su modelo y GPU", () => {
  render(<StatusTab snapshot={runningSnapshot} onPause={() => {}} />);
  const row = screen.getByRole("row", { name: /qwen3.5-mem:latest/ });
  expect(row).toHaveTextContent("AMD Radeon RX 7800 XT");
  expect(row).toHaveTextContent("7.6 GB");
});

test("con GPU sin datos explica la razón", () => {
  const snap = { ...runningSnapshot, adapters: { kind: "unavailable" as const, value: "PDH falló" } };
  render(<StatusTab snapshot={snap} onPause={() => {}} />);
  expect(screen.getByText(/Sin datos de GPU: PDH falló/)).toBeInTheDocument();
});

test("en pausa no hay runners y lo dice", () => {
  render(<StatusTab snapshot={pausedSnapshot} onPause={() => {}} />);
  expect(screen.getByText("No hay modelos en la GPU.")).toBeInTheDocument();
});
