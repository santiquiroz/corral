import { expect, test } from "@playwright/test";

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
