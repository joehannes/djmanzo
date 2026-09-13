/**
 * §43: the assistant learns how much assistance the DJ tolerates.
 *
 * > A DJ who ignores 20 consecutive suggestions should cause the suggestion
 * > rate to fall. Do not spam.
 *
 * The rule, the number and the sentence are `dj_assistant::fatigue`, tested
 * there against a whole night of loads this container cannot perform. What is
 * here is the half that is only true on screen: that a rail which has gone
 * quiet **says so**, and says it in a way a DJ can see without reading.
 *
 * That is not decoration. The obvious implementation of *do not spam* is to go
 * quiet and say nothing, and a DJ whose rail has thinned then has no way to
 * tell whether djmanzo has taken the hint, crashed, or run out of library — so
 * the feature reads as a bug and the honest behaviour gets reported as one.
 */
import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const LINE = ".conduct .appetite";

async function openAssistant(page: Page) {
  await page.getByRole("button", { name: "Assistant", exact: true }).click();
  await expect(page.locator('.surface[data-surface="assistant"]')).toBeVisible();
}

test.describe("§43's suggestion fatigue", () => {
  /**
   * **The load-bearing one: a rail that has gone quiet says why, and says the
   * way back.**
   *
   * Both halves. A line that reported the streak without the remedy would tell
   * a DJ the machine had given up on them and leave them there; one that said
   * "play a suggested record" without the count would be advice with no reason
   * attached.
   */
  test("an assistant that has gone quiet says why, and how to undo it", async ({
    page,
  }) => {
    await openShell(page, "/", {}, {
      assistant_appetite: {
        appetite: "quarter",
        ignored_in_a_row: 41,
        offers: 44,
        taken: 3,
        says:
          "Quieter: 41 records in a row that were not suggested. Play one that " +
          "is and this goes back to full.",
      },
    });
    await openAssistant(page);

    const line = page.locator(LINE);
    await expect(
      line,
      "the rail thinned and nothing on screen says why -- which is how a " +
        "working feature gets reported as a bug",
    ).toBeVisible();
    await expect(line).toContainText("41 records in a row");
    await expect(
      line,
      "a DJ told the assistant has gone quiet, and not how to undo it, has no " +
        "way back",
    ).toContainText("back to full");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * §33, on this line: the state is not said in colour alone.
   *
   * A quieter assistant is marked by a border as well as a hue, and the step
   * itself is on the element — so the difference between "offering everything"
   * and "offering a quarter" survives a hazer, a sunlit window and
   * deuteranopia, and a test can ask about it without matching on words.
   */
  test("a quieter assistant is marked, not only coloured", async ({ page }) => {
    await openShell(page, "/", {}, {
      assistant_appetite: {
        appetite: "least",
        ignored_in_a_row: 63,
        offers: 70,
        taken: 1,
        says: "Quieter: 63 records in a row that were not suggested. Play one that is and this goes back to full.",
      },
    });
    await openAssistant(page);

    await expect(page.locator(LINE)).toHaveAttribute("data-appetite", "least");
    expect(
      await page.locator(LINE).evaluate((el) => getComputedStyle(el).borderLeftWidth),
      "the only thing separating a quiet assistant from a loud one is its " +
        "colour, which §33 states as the one thing never to do",
    ).not.toBe("0px");
  });

  /**
   * A night nobody has ignored anything in says so plainly, and quietly.
   *
   * Drawn at full as well, so a DJ who notices a short rail has somewhere to
   * look: a line that only appears once the assistant has gone quiet is a line
   * nobody knows to look for. It just does not shout.
   */
  test("a fresh night draws the line without spending colour on it", async ({
    page,
  }) => {
    await openShell(page, "/");
    await openAssistant(page);

    const line = page.locator(LINE);
    await expect(line).toBeVisible();
    await expect(line).toHaveAttribute("data-appetite", "full");
    await expect(line).toContainText("Nothing offered yet");
    expect(
      await line.evaluate((el) => getComputedStyle(el).borderLeftWidth),
      "a night with nothing wrong with it marked the assistant as tired",
    ).toBe("0px");
  });
});
