import { defineConfig } from "@playwright/test";
const external = Boolean(process.env.FINDIT_E2E_BASE_URL);
export default defineConfig({
  testDir: "./tests/e2e",
  fullyParallel: false,
  workers: 1,
  timeout: 60000,
  expect: { timeout: 15000 },
  reporter: "list",
  use: {
    baseURL: process.env.FINDIT_E2E_BASE_URL ?? "http://127.0.0.1:3000",
    headless: true,
    channel: process.platform === "win32" ? "msedge" : undefined,
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  webServer: external
    ? undefined
    : {
        command: "npm run dev -- --port 3000",
        url: "http://127.0.0.1:3000/about",
        reuseExistingServer: !process.env.CI,
        timeout: 120000,
      },
});
