import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "e2e",
  use: { baseURL: "http://localhost:1420", channel: "msedge" },
  webServer: {
    command: "npx vite --port 1420 --strictPort",
    url: "http://localhost:1420",
    env: { VITE_E2E: "1" },
    reuseExistingServer: !process.env.CI,
  },
});
