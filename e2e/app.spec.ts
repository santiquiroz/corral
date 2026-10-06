import { expect, test } from "@playwright/test";

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
  const row = page.getByRole("row", { name: /qwen3.5-mem:latest/ });
  await row.getByRole("button", { name: "Borrar" }).click();
  await row.getByRole("button", { name: "Confirmar borrado" }).click();
  const calls = await page.evaluate(() => (window as unknown as { __corralCalls: { cmd: string; args: { name?: string } }[] }).__corralCalls);
  expect(calls.find((c) => c.cmd === "delete_model")?.args.name).toBe("qwen3.5-mem:latest");
});
