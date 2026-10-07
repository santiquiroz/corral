import { expect, test } from "@playwright/test";

test("Estado detecta un modelo de claude-mem ausente con icono y texto", async ({ page }) => {
  await page.goto("/?scenario=claude-mem-missing");
  const card = page.getByRole("region", { name: "claude-mem" });
  await expect(card.getByText("openrouter", { exact: true })).toBeVisible();
  await expect(card.getByText("Cola: 4", { exact: true })).toBeVisible();
  await expect(card.getByText("✓ El worker responde", { exact: true })).toBeVisible();
  await expect(card.getByText("✕ El modelo qwen3.5-mem:latest no está instalado: claude-mem fallará cuando se descargue de memoria", { exact: true })).toBeVisible();
  const colors = await card.evaluate((element) => ({ border: getComputedStyle(element).borderColor, error: getComputedStyle(element).getPropertyValue("--bad").trim() }));
  expect(colors.border).not.toBe("");
  expect(await card.evaluate((element) => (element as HTMLElement).style.borderColor)).toBe("var(--bad)");
  await page.getByRole("tab", { name: "Modelos" }).click();
  await expect(card).toHaveCount(0);
});

test("aplicar una GPU requiere confirmar el reinicio y guarda la selección", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("tab", { name: "Ajustes" }).click();
  await expect(page.getByText(/AMD Radeon RX 7800 XT · ROCm · discreta/)).toBeVisible();
  await page.getByLabel("Solo una GPU", { exact: true }).check();
  await page.getByLabel("GPU seleccionada").selectOption("ROCm:0");
  await page.getByLabel("Incluir GPU integrada").check();
  await page.getByRole("button", { name: "Aplicar y reiniciar Ollama", exact: true }).click();
  await expect(page.getByText("Ollama se reiniciará; claude-mem conserva su cola")).toBeVisible();
  expect(await page.evaluate(() => (window as unknown as { __corralCalls: { cmd: string }[] }).__corralCalls.filter((call) => call.cmd === "apply_gpu_profile"))).toHaveLength(0);
  await page.getByRole("button", { name: "Confirmar y reiniciar Ollama" }).click();
  await expect(page.getByText("Perfil GPU aplicado.")).toBeVisible();
  const calls = await page.evaluate(() => (window as unknown as { __corralCalls: { cmd: string; args: unknown }[] }).__corralCalls);
  expect(calls.find((call) => call.cmd === "apply_gpu_profile")?.args).toEqual({ profile: { kind: "single", library: "ROCm", filter_id: "0" }, igpuEnabled: true });
});

test("cargar un modelo usa la duración guardada y actualiza su estado", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("tab", { name: "Ajustes" }).click();
  await page.getByLabel("Mantener modelos cargados").selectOption("1h");
  await page.getByRole("button", { name: "Guardar", exact: true }).click();
  await page.getByRole("tab", { name: "Modelos" }).click();
  const row = page.getByRole("row", { name: /MiniCPM5/ });
  await row.getByRole("button", { name: "Cargar", exact: true }).click();
  await expect(row.getByRole("button", { name: "Cargando…" })).toBeDisabled();
  await expect(row.getByText("En GPU", { exact: true })).toBeVisible();
  const calls = await page.evaluate(() => (window as unknown as { __corralCalls: { cmd: string; args: { name?: string; config?: { load_keep_alive: string } } }[] }).__corralCalls);
  expect(calls.find((call) => call.cmd === "save_config")?.args.config?.load_keep_alive).toBe("1h");
  expect(calls.find((call) => call.cmd === "load_model")?.args.name).toBe("hf.co/openbmb/MiniCPM5-2B-GGUF:Q4_K_M");
});

test("Modelos permite liberar un modelo cargado sin manifiesto", async ({ page }) => {
  await page.goto("/?scenario=orphan-loaded");
  await page.getByRole("tab", { name: "Modelos" }).click();
  const orphan = page.getByRole("row", { name: /qwen3\.5-mem:latest/ });
  await expect(orphan.getByText("sin manifiesto")).toBeVisible();
  await expect(orphan.getByText("En GPU", { exact: true })).toBeVisible();
  await expect(orphan.getByRole("button", { name: "Liberar VRAM" })).toBeVisible();
  await expect(orphan.getByRole("button", { name: "Borrar" })).toHaveCount(0);
  await expect(page.getByRole("row", { name: /llamacpp:/ }).getByText("En GPU", { exact: true })).toHaveCount(0);
});

test("muestra la alerta de desborde y la GPU AMD", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("alert")).toContainText("qwen3.5-mem:latest está desbordando 729 MB");
  await expect(page.getByRole("region", { name: "Uso de VRAM por GPU" }).getByText("AMD Radeon RX 7800 XT")).toBeVisible();
});

test("pausar desde la alerta deja el panel en pausa", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("alert").getByRole("button", { name: "Pausar Ollama" }).click();
  await expect(page.getByRole("status").first()).toHaveText("En pausa");
  await expect(page.getByRole("button", { name: "Reanudar Ollama" })).toBeVisible();
});

test("borrar un modelo pasa por la confirmación", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("tab", { name: "Modelos" }).click();
  const row = page.getByRole("row", { name: /MiniCPM5/ });
  await row.getByRole("button", { name: "Borrar" }).click();
  await row.getByRole("button", { name: "Confirmar borrado" }).click();
  const calls = await page.evaluate(() => (window as unknown as { __corralCalls: { cmd: string; args: { name?: string } }[] }).__corralCalls);
  expect(calls.find((c) => c.cmd === "delete_model")?.args.name).toBe("hf.co/openbmb/MiniCPM5-2B-GGUF:Q4_K_M");
});


test("Modelos contiene el desborde horizontal y mantiene Liberar VRAM en una línea", async ({ page }) => {
  await page.setViewportSize({ width: 1100, height: 720 });
  await page.goto("/");
  await page.getByRole("tab", { name: "Modelos" }).click();
  await expect(page.getByRole("row", { name: /MiniCPM5/ })).toBeVisible();
  const pageFits = await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth);
  expect(pageFits).toBe(true);
  const button = page.getByRole("button", { name: "Liberar VRAM" }).first();
  await expect(button).toBeVisible();
  const bounds = await button.boundingBox();
  expect(bounds).not.toBeNull();
  expect(bounds!.height).toBeLessThan(40);
});
