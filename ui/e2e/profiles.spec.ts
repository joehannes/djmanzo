/**
 * §81, and the one thing it is about.
 *
 * > Do not store one universal DJ profile. Store conditional profiles.
 *
 * A DJ who plays bachata at weddings and techno at clubs, averaged, is a DJ who
 * plays neither — and the average carries twice the evidence of either real
 * answer, so a system offering it would offer it strongly. Everything below
 * checks the interface keeps them apart, says how much each one rests on, and
 * never guesses which kind of night this is.
 *
 * The arithmetic is Rust's and is tested in `dj_app::profile`. What a browser
 * can prove is that the press reaches Rust, the answer comes back, and the
 * panel does not quietly assemble a stronger claim than djmanzo made.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const NIGHT = '.surface[data-surface="night"]';

async function nightOpen(page: import("@playwright/test").Page) {
  await openShell(page, "/");
  await page.getByRole("button", { name: "Night", exact: true }).click();
  await expect(page.locator(NIGHT)).toBeVisible();
}

test.describe("what kind of night this is", () => {
  /**
   * **djmanzo does not guess.**
   *
   * It reads the arc of a night from the music and is right to — energy and
   * tempo are in the signal. Nothing in the signal says *wedding*. So the
   * panel opens with nothing chosen and says what choosing is for, rather than
   * defaulting to one and learning a whole night's habits under the wrong
   * heading.
   */
  test("opens with no kind chosen, and says why to choose one", async ({
    page,
  }) => {
    await nightOpen(page);

    await expect(page.locator(`${NIGHT} .kind button.on`)).toHaveCount(0);
    await expect(page.getByTestId("night-unsaid")).toContainText(
      "averaging your weddings with your club nights",
    );
    expect(errorsThrown(page), "the night panel threw").toEqual([]);
  });

  /** Every setting §81 lists is on offer, and saying one marks it. */
  test("naming the night marks it, and it stays named", async ({ page }) => {
    await nightOpen(page);

    const kinds = page.locator(`${NIGHT} .kind button`);
    await expect(kinds).toHaveCount(6);

    await kinds.filter({ hasText: /^Wedding$/ }).click();
    await expect(page.locator(`${NIGHT} .kind button.on`)).toHaveText("Wedding");
    await expect(page.getByTestId("night-unsaid")).toHaveCount(0);

    // Past the panel's own refresh, which calls back without a setting. That
    // call must not un-say the answer — the rule `Library::note_night`
    // enforces, and the one a DJ would never think to check.
    await page.waitForTimeout(2500);
    await expect(
      page.locator(`${NIGHT} .kind button.on`),
      "a refresh un-said what kind of night it is",
    ).toHaveText("Wedding");
    expect(errorsThrown(page)).toEqual([]);
  });

  /** And a DJ who picked wrong can say so. */
  test("the kind can be corrected", async ({ page }) => {
    await nightOpen(page);
    const kinds = page.locator(`${NIGHT} .kind button`);
    await kinds.filter({ hasText: /^Club$/ }).click();
    await expect(page.locator(`${NIGHT} .kind button.on`)).toHaveText("Club");
    await kinds.filter({ hasText: /^Wedding$/ }).click();
    await expect(page.locator(`${NIGHT} .kind button.on`)).toHaveText("Wedding");
    expect(errorsThrown(page)).toEqual([]);
  });
});

test.describe("how you play, by the kind of night", () => {
  /**
   * **Two settings are two profiles, drawn as two.**
   *
   * The whole of §81 on screen. If these ever merged into one line the
   * interface would be presenting the average — which describes neither
   * evening and reads more confident than either.
   */
  test("draws a profile per setting rather than one for everything", async ({
    page,
  }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Assistant", exact: true }).click();

    const profiles = page.getByTestId("profiles");
    await expect(profiles.locator("li")).toHaveCount(2);
    await expect(profiles).toContainText("Wedding");
    await expect(profiles).toContainText("Club");
    // And they say different things, which is the point.
    await expect(profiles).toContainText("Bachata");
    await expect(profiles).toContainText("Techno");
    expect(errorsThrown(page), "the conduct panel threw").toEqual([]);
  });

  /**
   * **Each one says how much it rests on.**
   *
   * Three nights and thirty are not the same claim, and a profile that hides
   * the difference is asking to be over-trusted. The sentence comes from Rust
   * with the count already in it, so a panel cannot drop it.
   */
  test("every profile names the evidence behind it", async ({ page }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Assistant", exact: true }).click();

    const profiles = page.getByTestId("profiles");
    await expect(profiles).toContainText("3 nights");
    await expect(profiles).toContainText("4 nights");
    expect(errorsThrown(page)).toEqual([]);
  });
});

/**
 * §81's other two: the density and the automation tolerance, acted on.
 *
 * §81 lists five things a conditional profile may differ in — density,
 * technique preferences, genre weights, transition style, automation tolerance
 * — and for a long time only the genre weights reached anything, through §12's
 * rail. The other four were learned, drawn, and acted on by nothing.
 *
 * The rules and the reasons are Rust's (`dj_app::profile::fits`), including
 * the asymmetry that matters: djmanzo may quiet its own assistant on a profile
 * and may only ever *offer* to make it louder. What a browser can prove is
 * that the unasked half actually happens, the asked half actually does not,
 * and a lock is visible as a lock rather than as silence.
 */
test.describe("what a profile fits", () => {
  /** A profile that runs denser than the window fitted, and louder than now. */
  const FITS = {
    night_fits: {
      density: {
        to: "pro-dense",
        name: "Pro Dense",
        doing: "its-own",
        because: "You run Pro Dense over 6 club nights.",
      },
      posture: {
        to: "autopilot",
        name: "autopilot",
        doing: "if-asked",
        because: "You keep the assistant on autopilot over 6 club nights.",
      },
      withheld: [],
    },
  };

  async function named(page: import("@playwright/test").Page, answers = FITS) {
    await openShell(page, "/", {}, answers);
    await page.getByRole("button", { name: "Night", exact: true }).click();
    await expect(page.locator(NIGHT)).toBeVisible();
    await page
      .locator(`${NIGHT} .kind button`)
      .filter({ hasText: /^Club$/ })
      .click();
  }

  /**
   * **The density djmanzo may move, it moves — and then stops offering it.**
   *
   * §78 names *no automatic surface resizing* as one of four freedoms the DJ
   * can withdraw, and with it left on, a profile is better evidence about what
   * this DJ runs at than one window height is. So this is the one of the two
   * that happens without a press, and the proof it happened is the property
   * the whole interface is measured in.
   */
  test("naming the night wears the density that kind of night runs at", async ({
    page,
  }) => {
    await named(page);

    await expect
      .poll(() =>
        page.evaluate(() =>
          document.documentElement.style.getPropertyValue("--density"),
        ),
      )
      .toBe("0.86");
    // And the row is gone, because there is nothing left to offer.
    await expect(page.locator(`${NIGHT} [data-fit="density"]`)).toHaveCount(0);
    expect(errorsThrown(page), "the night panel threw").toEqual([]);
  });

  /**
   * **A louder assistant is offered and never taken.**
   *
   * The load-bearing half. §9's rule is that autonomy above confidence is
   * unsafe, and a profile is evidence about *past* nights — it says nothing
   * about tonight's certainty. A version that turned the autopilot on because
   * of six previous club nights would be handing a machine the mix on
   * yesterday's evidence.
   */
  test("a louder assistant waits for the press that asks for it", async ({
    page,
  }) => {
    await named(page);

    const offer = page.locator(`${NIGHT} [data-fit="posture"]`);
    await expect(offer).toContainText("over 6 club nights");
    // Still there after the panel has polled several times: an offer nobody
    // took is an offer nobody took.
    await page.waitForTimeout(2500);
    await expect(offer).toBeVisible();

    await offer.getByRole("button").click();
    await expect(offer).toHaveCount(0);
    expect(errorsThrown(page), "the night panel threw").toEqual([]);
  });

  /**
   * **A locked density is not moved, and the panel says that is why.**
   *
   * §79's lock, at the one place a profile could walk past it. The silent
   * version is worse than not building it at all: an interface that declined
   * to act and said nothing looks exactly like one with nothing to say.
   */
  test("a withheld fit reads as withheld rather than as silence", async ({
    page,
  }) => {
    await named(page, {
      night_fits: {
        density: null,
        posture: null,
        withheld: [
          "You run Pro Dense over 6 club nights. The density is locked, so djmanzo is leaving it.",
        ],
      },
    });

    const held = page.locator(`${NIGHT} [data-fit="withheld"]`);
    await expect(held).toContainText("The density is locked");
    await expect(held.getByRole("button")).toHaveCount(0);
    // And nothing moved.
    expect(
      await page.evaluate(() =>
        document.documentElement.style.getPropertyValue("--density"),
      ),
    ).not.toBe("0.86");
    expect(errorsThrown(page), "the night panel threw").toEqual([]);
  });

  /**
   * **Most nights have no profile, and the panel shows nothing at all.**
   *
   * There is no profile until three nights of a setting, which is the common
   * case for a long time. A block that drew an empty frame would be the blank
   * panel this whole design argues against.
   */
  test("a night with no profile behind it offers nothing", async ({ page }) => {
    await named(page, {
      night_fits: { density: null, posture: null, withheld: [] },
    });

    await expect(page.getByTestId("night-fits")).toHaveCount(0);
    expect(errorsThrown(page), "the night panel threw").toEqual([]);
  });
});

/**
 * §11's `DJContext`, gathered.
 *
 * > Build an explicit internal concept … The context engine should become the
 * > common input to [nine things]. **Do not duplicate context logic inside
 * > each component.**
 *
 * Five of the eight were already real and were published by five different
 * things on five different schedules — so "the context engine" was a phrase
 * rather than an object, and a panel wanting three of them showed three
 * different moments at once. The eight names and the gathering are Rust's
 * (`dj_app::context`, which holds them against §11's own list in both
 * directions). What a browser can prove is that all eight reach a DJ, and that
 * the ones nothing has measured say so rather than reading as a measurement.
 */
test.describe("what djmanzo has in view", () => {
  test("all eight of §11's fields reach the night panel", async ({ page }) => {
    await nightOpen(page);
    const context = page.getByTestId("dj-context");
    await expect(context).toBeVisible();
    for (const label of [
      "Phase",
      "Occasion",
      "Music",
      "Hardware",
      "Room",
      "You",
      "Attention",
      "Health",
    ]) {
      await expect(context).toContainText(label);
    }
    expect(errorsThrown(page), "the night panel threw").toEqual([]);
  });

  /**
   * **An absence reads as an absence.**
   *
   * The half worth having, and the one a neutral default would destroy. A room
   * with no camera has not been read; a deck with nothing on it has no tempo;
   * a machine with no MIDI service is not a machine with nothing plugged in.
   * All three are true of this container, and all three have to look different
   * from a measurement that happened to come out low.
   */
  test("what nothing has measured says so rather than reading as nought", async ({
    page,
  }) => {
    await nightOpen(page);
    const context = page.getByTestId("dj-context");
    await expect(context).toContainText("not read");
    await expect(context).toContainText("nothing playing");
    await expect(context).toContainText("no MIDI service");
    // And the things that *are* measured are there as numbers.
    await expect(context).toContainText("48 kHz");
    expect(errorsThrown(page), "the night panel threw").toEqual([]);
  });
});
