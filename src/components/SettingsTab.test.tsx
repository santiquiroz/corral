import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { vi } from "vitest";
import SettingsTab from "./SettingsTab";

const api = vi.hoisted(() => ({ getConfig: vi.fn(), saveConfig: vi.fn(() => Promise.resolve()) }));
const autostart = vi.hoisted(() => ({ isEnabled: vi.fn(() => Promise.resolve(false)), enable: vi.fn(() => Promise.resolve()), disable: vi.fn(() => Promise.resolve()) }));
vi.mock("../lib/api", () => api);
vi.mock("@tauri-apps/plugin-autostart", () => autostart);

const config = {
  ollama_url: "http://127.0.0.1:11434", ollama_install_dir: "C:\\Ollama", poll_panel_secs: 2, poll_tray_secs: 10,
  spill_floor_mb: 64, resume_timeout_secs: 30, load_keep_alive: "30m",
  hooks: [{ name: "claude-mem", url: "http://127.0.0.1:37777/api/processing", body: "{\"isProcessing\":false}", enabled: true }],
};

beforeEach(() => { vi.clearAllMocks(); api.getConfig.mockResolvedValue(config); });

test.each([["30m", "30 min"], ["1h", "1 h"], ["-1", "Siempre"]])("guarda la duración de carga %s desde Memoria", async (value, label) => {
  render(<SettingsTab />);
  const select = await screen.findByLabelText("Mantener modelos cargados");
  expect(select).toHaveValue("30m");
  expect(screen.getByRole("heading", { name: "Memoria" })).toBeInTheDocument();
  expect(screen.getByRole("option", { name: label })).toHaveValue(value);
  await userEvent.selectOptions(select, value);
  await userEvent.click(screen.getByRole("button", { name: "Guardar" }));
  expect(api.saveConfig).toHaveBeenCalledWith({ ...config, load_keep_alive: value });
});

test("guarda el umbral de desborde cambiado", async () => {
  render(<SettingsTab />);
  const floor = await screen.findByLabelText("Umbral de desborde (MB)");
  await userEvent.clear(floor);
  await userEvent.type(floor, "128");
  await userEvent.click(screen.getByRole("button", { name: "Guardar" }));
  expect(api.saveConfig).toHaveBeenCalledWith({ ...config, spill_floor_mb: 128 });
  expect(await screen.findByText("Guardado.")).toBeInTheDocument();
});

test("desactiva un aviso", async () => {
  render(<SettingsTab />);
  await userEvent.click(await screen.findByLabelText("Aviso claude-mem"));
  await userEvent.click(screen.getByRole("button", { name: "Guardar" }));
  expect(api.saveConfig).toHaveBeenCalledWith({ ...config, hooks: [{ ...config.hooks[0], enabled: false }] });
});

test("activa el inicio con Windows", async () => {
  render(<SettingsTab />);
  await userEvent.click(await screen.findByLabelText("Iniciar con Windows"));
  expect(autostart.enable).toHaveBeenCalled();
});

test("muestra el error de validación del backend", async () => {
  api.saveConfig.mockRejectedValueOnce("las cadencias deben ser de al menos 1 segundo");
  render(<SettingsTab />);
  await userEvent.click(await screen.findByRole("button", { name: "Guardar" }));
  expect(await screen.findByText("las cadencias deben ser de al menos 1 segundo")).toBeInTheDocument();
});

test("si el inicio con Windows falla lo dice y no marca la casilla", async () => {
  autostart.enable.mockRejectedValueOnce("permiso denegado");
  render(<SettingsTab />);
  const box = await screen.findByLabelText("Iniciar con Windows");
  await userEvent.click(box);
  expect(await screen.findByText("permiso denegado")).toBeInTheDocument();
  expect(box).not.toBeChecked();
});
