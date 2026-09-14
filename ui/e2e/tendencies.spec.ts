/**
 * §14's behavioural signals, read through §13's rule.
 *
 * §13 is the one that matters and it is about *words* as much as counting:
 *
 * > A DJ raises BPM dramatically once because the crowd suddenly explodes. Do
 * > not conclude "User likes enormous BPM jumps". Instead: "Large jumps
 * > occasionally occur in high-energy contexts."
 *
 * The counting is `dj_app::signals` and is tested there — four occurrences in
 * one phase, never across the night, never without one. What a browser can
 * prove is that the sentence a DJ actually reads keeps §13's shape: it names
 * the part of the night it is about, and it never claims a preference.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

async function openConduct(page: import("@playwright/test").Page) {
  await openShell(page, "/");
  await page.getByRole("button", { name: "Assistant", exact: true }).click();
}

test.describe("what you do, and when", () => {
  /**
   * **Every line says which part of the night it is about.**
   *
   * A gesture without its context is a preference waiting to be learned
   * wrongly, which is the whole of §13 — so a line that dropped the phase
   * would be the failure, not a shorter sentence.
   */
  test("each tendency names the night it was seen in", async ({ page }) => {
    await openConduct(page);
    const list = page.locator(".tendencies li");
    await expect(list).toHaveCount(2);

    await expect(list.first()).toContainText("when the night is at its peak");
    await expect(list.nth(1)).toContainText("when the night is building");
    // And how many times, because a count is the difference between a claim
    // and a guess.
    await expect(list.first()).toContainText("Seen 9 times");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **No line claims a preference.**
   *
   * The directive names the sentence it does not want. This is the guard
   * against it coming back through a well-meaning rewrite of the wording.
   */
  test("nothing on screen says the DJ likes anything", async ({ page }) => {
    await openConduct(page);
    const said = (await page.locator(".tendencies li").allTextContents())
      .join(" ")
      .toLowerCase();

    expect(said.length).toBeGreaterThan(0);
    for (const forbidden of ["likes", "prefers", "always", "never"]) {
      expect(said, `a tendency claims a preference: ${said}`).not.toContain(forbidden);
    }
    // And it does say what it did see.
    expect(said).toContain("sometimes");
    expect(said).toContain("often");
  });
});

/**
 * §11's *technique recommendations*, reading the one context engine.
 *
 * §11 names nine consumers of the context and this was the one that read
 * nothing: the coach narrowed its curriculum by §16's chosen pack and by what
 * the DJ had done in the last two minutes, and had no idea whether they were
 * mid-blend or standing between records.
 *
 * §58 puts *technique advice* in the contextual tier by name, and §18's mixing
 * budget leaves room for the first two tiers and no others — so the rule is two
 * tables that already existed meeting, and Rust owns it. What only a browser
 * can say is that the two blanks look different to a learner.
 */
test.describe("when the coach offers a lesson", () => {
  /**
   * **Withheld says so; it does not go quiet.**
   *
   * A learner shown nothing the moment they start a blend would read it as
   * having finished the curriculum — the one wrong thing a coach can say, and
   * it would say it on every single mix.
   */
  test("a lesson held back for the moment says which moment", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openConduct(page);

    const coach = page.locator(".coach");
    await expect(coach).toBeVisible();
    // `toBeVisible` as well as the text, and the order matters: Playwright reads
    // `textContent` off hidden elements quite happily, so a line the learner
    // cannot see passes a text assertion. A `hidden` attribute on this
    // paragraph survived the first version of this test.
    const waiting = coach.locator(".waiting");
    await expect(waiting).toBeVisible();
    await expect(waiting).toContainText("while you are mixing");
    // And there is no lesson being offered at the same time, which would be
    // the panel saying both things at once.
    await expect(coach.locator(".next")).toHaveCount(0);
    expect(thrown).toEqual([]);
  });

  /**
   * **And the correction is not withheld with it.**
   *
   * The half that makes the rule right rather than merely quiet. "Both records
   * have their bass up" is the one thing a coach exists to say *during* a mix —
   * it sounds fine in the headphones and wrong in the room — and a gate that
   * took it away with the lesson would have made the panel useless at exactly
   * the moment it is worth having.
   */
  test("the correction about the mix in progress is still made", async ({ page }) => {
    await openConduct(page);
    const coach = page.locator(".coach");
    await expect(coach.locator(".note .fix")).toContainText("Pull one low down");
  });
});
