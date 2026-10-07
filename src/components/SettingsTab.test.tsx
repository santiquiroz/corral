import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { vi } from "vitest";
import SettingsTab from "./SettingsTab";

const api = vi.hoisted(() => ({ getConfig: vi.fn(), saveConfig: vi.fn(() => Promise.resolve()), listOllamaGpus: vi.fn(), applyGpuProfile: vi.fn(() => Promise.resolve()) }));
const autostart = vi.hoisted(() => ({ isEnabled: vi.fn(() => Promise.resolve(false)), enable: vi.fn(() => Promise.resolve()), disable: vi.fn(() => Promise.resolve()) }));
vi.mock("../lib/api", () => api);
vi.mock("@tauri-apps/plugin-autostart", () => autostart);

const config = {
  ollama_url: "http://127.0.0.1:11434", ollama_install_dir: "C:\\Ollama", poll_panel_secs: 2, poll_tray_secs: 10,
  spill_floor_mb: 64, resume_timeout_secs: 30, load_keep_alive: "30m",
  gpu_profile: { kind: "auto" }, igpu_enabled: false,
  hooks: [{ name: "claude-mem", url: "http://127.0.0.1:37777/api/processing", body: "{\"isProcessing\":false}", enabled: true }],
};

const gpus = [
  { id: "0", filter_id: "0", library: "ROCm", description: "AMD Radeon RX 7800 XT", kind: "discrete", total_mb: 16384, dropped: false },
  { id: "1", filter_id: "1", library: "Vulkan", description: "AMD Radeon(TM) Graphics", kind: "integrated", total_mb: null, dropped: true },
];

beforeEach(() => { vi.clearAllMocks(); api.getConfig.mockReset().mockResolvedValue(config); api.listOllamaGpus.mockResolvedValue(gpus); });

test("lista las GPUs de Ollama con librería, tipo, VRAM y descarte", async () => {
  render(<SettingsTab />);
  expect(await screen.findByText(/AMD Radeon RX 7800 XT · ROCm · discreta · 16.0 GB/)).toBeInTheDocument();
  expect(screen.getByText(/AMD Radeon\(TM\) Graphics · Vulkan · integrada · — · ignorada/)).toBeInTheDocument();
  expect(screen.getByLabelText("Automático")).toBeChecked();
  expect(screen.getByLabelText("Incluir GPU integrada")).not.toBeChecked();
});

test("aplicar GPU exige confirmación, bloquea mientras reinicia y conserva el perfil aplicado al guardar", async () => {
  let finish!: () => void;
  api.applyGpuProfile.mockImplementationOnce(() => new Promise<void>((resolve) => { finish = resolve; }));
  render(<SettingsTab />);
  await userEvent.click(await screen.findByLabelText("Solo una GPU"));
  await userEvent.selectOptions(screen.getByLabelText("GPU seleccionada"), "ROCm:0");
  await userEvent.click(screen.getByLabelText("Incluir GPU integrada"));
  await userEvent.click(screen.getByRole("button", { name: "Aplicar y reiniciar Ollama" }));
  expect(api.applyGpuProfile).not.toHaveBeenCalled();
  expect(screen.getByText("Ollama se reiniciará; claude-mem conserva su cola")).toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "Confirmar y reiniciar Ollama" }));
  expect(api.applyGpuProfile).toHaveBeenCalledWith({ kind: "single", library: "ROCm", filter_id: "0" }, true);
  expect(screen.getByRole("button", { name: "Reiniciando…" })).toBeDisabled();
  expect(screen.getByLabelText("Automático")).toBeDisabled();
  await act(async () => finish());
  await userEvent.click(screen.getByRole("button", { name: "Guardar" }));
  expect(api.saveConfig).toHaveBeenCalledWith({ ...config, gpu_profile: { kind: "single", library: "ROCm", filter_id: "0" }, igpu_enabled: true });
});

test.each(["Automático", "Repartir entre todas"])("aplica el perfil %s sin permitir que Guardar aplique el borrador GPU", async (label) => {
  render(<SettingsTab />);
  await userEvent.click(await screen.findByLabelText(label));
  await userEvent.click(screen.getByLabelText("Incluir GPU integrada"));
  await userEvent.click(screen.getByRole("button", { name: "Guardar" }));
  expect(api.saveConfig).toHaveBeenCalledWith(config);
  await userEvent.click(screen.getByRole("button", { name: "Aplicar y reiniciar Ollama" }));
  await userEvent.click(screen.getByRole("button", { name: "Confirmar y reiniciar Ollama" }));
  expect(api.applyGpuProfile).toHaveBeenCalledWith({ kind: label === "Automático" ? "auto" : "spread" }, true);
});

test("muestra errores de listado y de aplicar sin perder el perfil guardado", async () => {
  api.listOllamaGpus.mockRejectedValueOnce("No se pudo leer server.log");
  api.applyGpuProfile.mockRejectedValueOnce("No se pudo reiniciar Ollama");
  render(<SettingsTab />);
  expect(await screen.findByText("No se pudo leer server.log")).toBeInTheDocument();
  await userEvent.click(screen.getByLabelText("Solo una GPU"));
  expect(screen.getByRole("button", { name: "Aplicar y reiniciar Ollama" })).toBeDisabled();
  await userEvent.click(screen.getByLabelText("Repartir entre todas"));
  await userEvent.click(screen.getByRole("button", { name: "Aplicar y reiniciar Ollama" }));
  await userEvent.click(screen.getByRole("button", { name: "Confirmar y reiniciar Ollama" }));
  expect(await screen.findByText("No se pudo reiniciar Ollama")).toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "Guardar" }));
  expect(api.saveConfig).toHaveBeenCalledWith(config);
});

test("recupera un perfil GPU guardado y cancela la confirmación al modificarlo", async () => {
  api.getConfig.mockResolvedValueOnce({ ...config, gpu_profile: { kind: "single", library: "Vulkan", filter_id: "1" }, igpu_enabled: true });
  render(<SettingsTab />);
  await waitFor(() => expect(screen.getByLabelText("GPU seleccionada")).toHaveValue("Vulkan:1"));
  expect(screen.getByLabelText("Solo una GPU")).toBeChecked();
  expect(screen.getByLabelText("Incluir GPU integrada")).toBeChecked();
  await userEvent.click(screen.getByRole("button", { name: "Aplicar y reiniciar Ollama" }));
  await userEvent.click(screen.getByLabelText("Automático"));
  expect(screen.queryByRole("button", { name: "Confirmar y reiniciar Ollama" })).not.toBeInTheDocument();
});

test("impide guardar ajustes mientras el perfil GPU se aplica", async () => {
  let finish!: () => void;
  api.applyGpuProfile.mockImplementationOnce(() => new Promise<void>((resolve) => { finish = resolve; }));
  render(<SettingsTab />);
  await userEvent.click(await screen.findByRole("button", { name: "Aplicar y reiniciar Ollama" }));
  await userEvent.click(screen.getByRole("button", { name: "Confirmar y reiniciar Ollama" }));
  expect(screen.getByRole("button", { name: "Guardar" })).toBeDisabled();
  await userEvent.click(screen.getByRole("button", { name: "Guardar" }));
  expect(api.saveConfig).not.toHaveBeenCalled();
  await act(async () => finish());
  expect(screen.getByRole("button", { name: "Guardar" })).toBeEnabled();
});

test("impide aplicar GPU y editar sus controles mientras se guardan ajustes", async () => {
  let finish!: () => void;
  api.saveConfig.mockImplementationOnce(() => new Promise<void>((resolve) => { finish = resolve; }));
  render(<SettingsTab />);
  await userEvent.click(await screen.findByRole("button", { name: "Guardar" }));
  expect(screen.getByRole("button", { name: "Guardando…" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Aplicar y reiniciar Ollama" })).toBeDisabled();
  expect(screen.getByLabelText("Automático")).toBeDisabled();
  await userEvent.click(screen.getByRole("button", { name: "Aplicar y reiniciar Ollama" }));
  expect(api.applyGpuProfile).not.toHaveBeenCalled();
  await act(async () => finish());
  expect(screen.getByRole("button", { name: "Aplicar y reiniciar Ollama" })).toBeEnabled();
});

test("tras un fallo parcial sincroniza solo GPU y conserva ajustes sin guardar", async () => {
  api.applyGpuProfile.mockRejectedValueOnce("Falló el reinicio después de guardar");
  render(<SettingsTab />);
  const floor = await screen.findByLabelText("Umbral de desborde (MB)");
  await userEvent.clear(floor);
  await userEvent.type(floor, "128");
  await userEvent.click(screen.getByLabelText("Repartir entre todas"));
  api.getConfig.mockResolvedValueOnce({ ...config, gpu_profile: { kind: "spread" }, igpu_enabled: true });
  await userEvent.click(screen.getByRole("button", { name: "Aplicar y reiniciar Ollama" }));
  await userEvent.click(screen.getByRole("button", { name: "Confirmar y reiniciar Ollama" }));
  expect(await screen.findByText("Falló el reinicio después de guardar")).toBeInTheDocument();
  await waitFor(() => expect(screen.getByLabelText("Incluir GPU integrada")).toBeChecked());
  await userEvent.click(screen.getByRole("button", { name: "Guardar" }));
  expect(api.saveConfig).toHaveBeenCalledWith({ ...config, spill_floor_mb: 128, gpu_profile: { kind: "spread" }, igpu_enabled: true });
});

test("si recargar el perfil tras un fallo falla también muestra ambos errores", async () => {
  api.applyGpuProfile.mockRejectedValueOnce("Falló el reinicio");
  render(<SettingsTab />);
  await screen.findByRole("button", { name: "Aplicar y reiniciar Ollama" });
  api.getConfig.mockRejectedValueOnce("No se pudo recargar la configuración");
  await userEvent.click(screen.getByRole("button", { name: "Aplicar y reiniciar Ollama" }));
  await userEvent.click(screen.getByRole("button", { name: "Confirmar y reiniciar Ollama" }));
  expect(await screen.findByText(/Falló el reinicio.*No se pudo recargar la configuración/)).toBeInTheDocument();
});

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
