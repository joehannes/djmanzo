/**
 * §33's audit, which had never been run.
 *
 * > Color alone must never encode critical state. […] Support: high contrast,
 * > reduced motion, daylight, color-blind-safe options, keyboard operation.
 *
 * The section was marked *roles and labels throughout, and the tests query by
 * role rather than by class* — which was true, and was also the reason nobody
 * had looked. Querying by role proves a control *has* one; it proves nothing
 * about whether the role is allowed to carry the label written on it, whether
 * the bar a screen reader finds says which deck it belongs to, or whether the
 * text is readable. The first run of this file found eight kinds of defect, two
 * of which had been shipped since the components were written:
 *
 * - the level meter on every deck, announcing itself as an empty box, because
 *   ARIA drops `aria-label` from an element with no role;
 * - the position bar on every deck, announcing a percentage of nothing;
 * - the label of whichever panel button you were hovering, drawn near-black on
 *   near-black at 1.23:1 — that is, the panel you had just opened;
 * - the mission bar's captions at 3.39:1, under a section that asks for
 *   *daylight* by name;
 * - the browser's two tabs reporting an attribute their role forbids, so
 *   assistive technology dropped it and they announced neither state;
 * - six `select`s with no name at all (the static sweep in
 *   `src/access.test.ts` found eighteen more, behind panels this cannot open);
 * - a table column whose cells belonged to no heading;
 * - and no `h1` anywhere, so the whole booth was one flat run of controls.
 *
 * # What this is and is not
 *
 * axe-core is a rules engine, not a person. It catches what is mechanically
 * checkable — contrast ratios, prohibited attributes, nameless controls — and
 * it cannot tell whether a label is *useful*, whether the tab order makes sense
 * to somebody mixing, or whether a colour-blind DJ can tell an armed deck from
 * a playing one. A clean run is a floor. It is a floor this application was
 * under.
 *
 * The one rule §33 states as an absolute — colour alone never encoding critical
 * state — is not checkable here at all, so it is a type and a test in
 * `dj_app::mission`: `Level::mark`, and the two tests that keep the marks and
 * the stylesheet honest about it.
 */
import { expect, test } from "@playwright/test";
import axe from "axe-core";

import { openShell } from "./shell";

/** The library, as a string, to be evaluated inside the page. */
const AXE = axe.source;

/**
 * Every panel the rail can open, by the name on its button.
 *
 * All of them, rather than a sample. A panel left out of this list is a panel
 * whose next control can be added without a name, which is exactly how six of
 * them came to have none.
 */
const PANELS = [
  "Browse",
  "Prepare",
  "Next",
  "Plan",
  "Pair",
  "Practice",
  "Night",
  "Presets",
  "Booth",
  "Sampler",
  "Assistant",
  "Settings",
  "Keys",
  "Log",
] as const;

interface Violation {
  id: string;
  impact: string | null;
  help: string;
  nodes: { target: string[]; failureSummary: string }[];
}

/** Run the audit against whatever is on screen now. */
async function audit(page: import("@playwright/test").Page): Promise<Violation[]> {
  await page.evaluate(AXE);
  const results = await page.evaluate(async () =>
    // @ts-expect-error `axe` is the global the line above just defined.
    (await axe.run(document, { resultTypes: ["violations"] })) as unknown,
  );
  return (results as { violations: Violation[] }).violations;
}

/** What a failure should print: the rule, and the first few elements. */
function report(violations: Violation[]): string {
  return violations
    .map(
      (v) =>
        `${v.impact ?? "?"}: ${v.id} — ${v.help}\n` +
        v.nodes
          .slice(0, 4)
          .map((n) => `      ${n.target.join(" ")}`)
          .join("\n"),
    )
    .join("\n");
}

test.describe("§33's audit", () => {
  /**
   * **The load-bearing one: the booth, as it opens.**
   *
   * The decks, the crossfader, the mission bar and the panel rail — which is
   * what a DJ looks at for the whole night, and what four of the eight defects
   * were in.
   */
  test("the shell as it opens has nothing wrong with it", async ({ page }) => {
    await openShell(page, "/");
    const violations = await audit(page);
    expect(
      report(violations),
      "the booth a DJ spends the night in fails its own accessibility audit",
    ).toBe("");
  });

  for (const panel of PANELS) {
    test(`the ${panel} panel has nothing wrong with it`, async ({ page }) => {
      await openShell(page, "/");
      await page.getByRole("button", { name: panel, exact: true }).click();
      await expect(page.locator(".surface").first()).toBeVisible();
      const violations = await audit(page);
      expect(report(violations), `${panel} fails its accessibility audit`).toBe("");
    });
  }

  /**
   * §33's *high contrast*, taken from the operating system rather than from a
   * setting inside djmanzo.
   *
   * Asserted on the token rather than on a pixel, because the token is the
   * claim: every caption, hint and unit in the interface is `--text-dim`, and
   * every hairline is `--border`, so moving those two is what "high contrast"
   * means here and a screenshot would only prove it for one of them.
   *
   * The pair is checked in both directions. A rule that fired all the time
   * would be a palette change nobody asked for, and a rule that never fired is
   * §33's row still saying what it said before.
   */
  test("the system asking for contrast gets it, and not otherwise", async ({
    page,
  }) => {
    await openShell(page, "/");
    const tokens = () =>
      page.evaluate(() => {
        const root = getComputedStyle(document.documentElement);
        return {
          dim: root.getPropertyValue("--text-dim").trim(),
          text: root.getPropertyValue("--text").trim(),
          border: root.getPropertyValue("--border").trim(),
          strong: root.getPropertyValue("--border-strong").trim(),
        };
      });

    const normal = await tokens();
    expect(
      normal.dim,
      "the interface has no quiet text at all, so there is nothing for high " +
        "contrast to raise",
    ).not.toBe(normal.text);

    await page.emulateMedia({ contrast: "more" });
    const raised = await tokens();
    expect(
      raised.dim,
      "the system asked for high contrast and every caption in djmanzo stayed " +
        "dim -- §33 lists it among the five things to support",
    ).toBe(raised.text);
    expect(raised.border, "the hairlines stayed hairlines").toBe(raised.strong);

    await page.emulateMedia({ contrast: "no-preference" });
    expect(
      (await tokens()).dim,
      "the contrast rule fires whatever the system says, which is a palette " +
        "change nobody asked for rather than a setting being honoured",
    ).toBe(normal.dim);
  });

  /**
   * §33's *keyboard operation*, as far as a browser can prove it.
   *
   * Not "every control is reachable" — that is what the audit's own rules and
   * the `src/access.test.ts` sweep are for. This is the narrower claim that
   * matters most and that nothing else here makes: **a panel can be opened
   * without a pointer**, and what opens is the panel whose button had focus.
   * A rail that could only be clicked would put every surface in djmanzo behind
   * a mouse, and the cue handles §26 added are already reachable by arrow key
   * from nowhere at all if the panel holding them cannot be opened.
   */
  test("a panel opens from the keyboard, and it is the one that had focus", async ({
    page,
  }) => {
    await openShell(page, "/");
    await expect(page.locator('.surface[data-surface="library"]')).toHaveCount(0);

    const browse = page.getByRole("button", { name: "Browse", exact: true });
    await browse.focus();
    await expect(browse, "the panel rail cannot be focused at all").toBeFocused();
    await page.keyboard.press("Enter");

    await expect(
      page.locator('.surface[data-surface="library"]'),
      "the rail answers a pointer and not a keyboard, so every surface in " +
        "djmanzo is behind a mouse",
    ).toBeVisible();
  });
});
