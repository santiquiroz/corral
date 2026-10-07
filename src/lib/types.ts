export type Field<T> = { kind: "ok"; value: T } | { kind: "unavailable"; value: string };
export type PausedBy = "user" | "rule";
export type OllamaState = { kind: "running" } | { kind: "paused"; by: PausedBy } | { kind: "down" };
export type TrayStatus = "running" | "spilling" | "paused" | "down";

export interface Adapter { luid: number; name: string; total_mb: number; used_mb: number | null; ollama_mb: number }
export interface RunnerGpu { luid: number; dedicated_mb: number | null; shared_mb: number | null }
export interface Runner {
  pid: number; model: string | null; gpus: RunnerGpu[]; dedicated_mb: number | null;
  shared_mb: number | null; cpu_pct: number; ram_mb: number; spilling: boolean;
}
export interface LoadedModel { name: string; digest: string; size_mb: number; vram_mb: number; context_length: number; expires_at: string }
export interface Snapshot {
  taken_at_ms: number; state: OllamaState; status: TrayStatus; version: string | null;
  adapters: Field<Adapter[]>; runners: Field<Runner[]>; loaded: Field<LoadedModel[]>;
}
export interface InstalledModel {
  name: string; digest: string; size_mb: number; family: string; parameter_size: string;
  quantization: string; context_length: number | null; modified_at: string;
}
export interface PullProgress { name: string; status: string; completed: number | null; total: number | null }
export interface PullDone { name: string; error: string | null }
export interface Hook { name: string; url: string; body: string; enabled: boolean }
export interface Config {
  ollama_url: string; ollama_install_dir: string; poll_panel_secs: number; poll_tray_secs: number;
  spill_floor_mb: number; resume_timeout_secs: number; load_keep_alive: string; hooks: Hook[];
}
