/**
 * The stem controls say which separator is doing the work, and keep saying
 * it while a model loads.
 *
 * In v0.23.0 the HT-Demucs model loaded, the panel named it, and every chunk
 * then failed — so the controls moved gains on stems that never arrived and
 * nothing said why. Rust now tries a model before it trusts it and loads it
 * off the startup path, with the built-in separator playing meanwhile
 * (`dj_app::state::AppState::open_stems`, `load_stems_model`, and
 * `dj-stems/tests/onnx_contract.rs` against real ONNX Runtime). What the
 * browser holds: the panel says the model is loading, and names it once it
 * has taken over, without being reopened.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const LOADING = {
  available: true,
  backend: "built-in (harmonic/percussive)",
  reason: "the HTDemucs model is loading; the built-in separator plays until it is ready",
  loading: true,
};
const READY = { available: true, backend: "HTDemucs (ONNX)", reason: null, loading: false };

test("the stems panel follows the model from loading to in use", async ({ page }) => {
  await openShell(page, "/", {}, {
    stems_status: [LOADING, LOADING, READY],
    __sequences: ["stems_status"],
  });

  const fold = page.locator(".stem-fold").first();
  await fold.locator("summary").click();
  const reason = fold.locator(".stems-reason");
  await expect(reason).toContainText("Using the built-in (harmonic/percussive) separator");
  await expect(reason).toContainText("model is loading");
  // Every stem pad is usable meanwhile: the built-in separator is playing.
  await expect(fold.locator(".stem-pad").first()).toBeEnabled();

  // Asked again while loading, and the model has taken over: nothing left
  // to explain.
  await expect(reason).toHaveCount(0, { timeout: 8_000 });
  await expect(fold.locator(".stem-pad").first()).toBeEnabled();
  expect(await errorsThrown(page)).toEqual([]);
});
