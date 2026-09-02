/**
 * The rendered-geometry budget's runner.
 *
 * Chromium only, and the pre-installed one rather than a downloaded browser:
 * this measures layout, and one engine's layout is enough to catch a control
 * drifting hundreds of pixels. Running the same assertions on three engines
 * would triple the time and find the same failure three times.
 *
 * It is worth saying plainly that this engine is **not the one djmanzo ships
 * on** -- the application runs in WebKitGTK. That is a real limitation, it is
 * why every assertion here carries slack, and it is recorded in `e2e/shell.ts`
 * beside the number.
 */
import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  // The budget is a fact about the layout, so a retry that passes would be
  // hiding something rather than smoothing over a flake.
  retries: 0,
  fullyParallel: true,
  reporter: process.env.CI ? [["list"], ["html", { open: "never" }]] : [["list"]],
  use: {
    baseURL: "http://127.0.0.1:4173",
    // Kept for a failure, discarded for a pass: a screenshot of the moment the
    // crossfader went off the screen is the whole of the bug report.
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
  },
  projects: [
    {
      name: "chromium",
      use: {
        ...devices["Desktop Chrome"],
        // CI installs the browser Playwright ships with, which is the right
        // default: the version is then pinned by the lockfile and nobody has to
        // think about it. A machine that already has a Chromium -- a container
        // that pre-installed one, a developer who does not want a second copy
        // -- points at it with `DJMANZO_CHROMIUM` instead. The layout being
        // measured does not change between Chromium builds; only the download
        // does.
        ...(process.env.DJMANZO_CHROMIUM
          ? { launchOptions: { executablePath: process.env.DJMANZO_CHROMIUM } }
          : {}),
      },
    },
  ],
  webServer: {
    // The built bundle, not the dev server: what is measured should be what
    // ships, and Vite's dev transform can change how stylesheets land.
    //
    // `--host 127.0.0.1` is not decoration. Vite's preview binds `localhost`,
    // and on a machine where that resolves to `::1` first it listens on IPv6
    // while Playwright polls the IPv4 address below -- which presents as the
    // server simply never coming up, sixty seconds of nothing and a timeout
    // that names no cause. It cost a red CI run to find. Binding and polling
    // the same literal address removes the question.
    command: "npm run preview -- --host 127.0.0.1 --port 4173 --strictPort",
    url: "http://127.0.0.1:4173",
    reuseExistingServer: !process.env.CI,
    // Piped rather than swallowed, so the next failure of this kind says what
    // the server thought it was doing.
    stdout: "pipe",
    stderr: "pipe",
    timeout: 60_000,
  },
});
