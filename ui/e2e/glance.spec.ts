/**
 * §39's compact indicator, and the panel it opens.
 *
 * The section is a warning before it is a feature: *do not create a giant
 * analytics dashboard during a set*. What ships is one mark — `ROOM ↑`,
 * `ROOM ↓`, `ROOM STABLE` — and the six comparisons in a panel that opens when
 * the DJ asks for it.
 *
 * The mark lives on §5's Mission Bar, which is where §5 asks for *room status*
 * to be. The arrow itself is `dj_assistant::room::Room::glance` and the
 * wording is `dj_app::mission`; both are tested in Rust against readings this
 * container cannot produce. These say the interface draws what came back, that
 * a room nobody is watching is not given a direction, and that pressing it
 * reaches the dashboard.
 */
import { expect, test } from "@playwright/test";

import { openShell } from "./shell";

const CHIP = '.mission [data-mission="room"]';
const ROOM = '.surface[data-surface="room"]';
/** `RoomSense.svelte`'s own root, wherever it is mounted. */
const SENSOR = "div.room";

/** A bar carrying one room reading and nothing else that matters here. */
const reading = (value: string, about: string) => [
  { slug: "room", label: "ROOM", value, level: "quiet", about },
  {
    slug: "health",
    label: "CPU",
    value: "20%",
    level: "quiet",
    about: "The audio thread is using 20% of its time.",
  },
];

test.describe("the room indicator", () => {
  /**
   * The load-bearing one: the mark, the evidence, and the way in.
   *
   * All three are §39 in one line — a compact indicator, with confidence, that
   * expands. A mark that could not be pressed would leave the dashboard
   * reachable only from the assistant, which is where it was and is the thing
   * this section is fixing.
   */
  test("the arrow reads what Rust said and opens the room", async ({ page }) => {
    await openShell(page, "/", {}, {
      mission_bar: reading(
        "↑",
        "The room is busier and louder than it has been for the last twenty minutes. " +
          "2 of 3 senses read this way.",
      ),
    });

    const chip = page.locator(CHIP);
    await expect(chip, "no indicator appeared for a room that is reading").toBeVisible();
    await expect(chip).toContainText("ROOM");
    await expect(chip).toContainText("↑");
    await expect(
      chip,
      "the arrow shipped without its evidence -- §39 asks for confidence, and " +
        "this project states it as a count rather than a percentage",
    ).toHaveAttribute("title", /2 of 3 senses/);

    await expect(page.locator(ROOM), "the room panel was already open").toHaveCount(0);
    await chip.click();
    await expect(
      page.locator(ROOM),
      "§39's other half is click-to-expand and the indicator is inert",
    ).toBeVisible();
  });

  /**
   * A room djmanzo cannot place says so, rather than inventing a direction.
   *
   * This is the state of every machine with no camera, this container
   * included, so it is the state the indicator is in almost all the time. A
   * mark reading STABLE over a room nobody is looking at would be a claim
   * about a floor djmanzo has never seen — and one that vanished entirely
   * would take the only way into the panel with it, since nothing can be
   * watching until somebody has opened it and aimed a camera.
   */
  test("a room nothing is watching says so and is still the way in", async ({
    page,
  }) => {
    await openShell(page, "/");

    const chip = page.locator(CHIP);
    await expect(chip).toBeVisible();
    await expect(
      chip,
      "the indicator invented a direction for a room nothing has looked at",
    ).not.toContainText(/↑|↓|STABLE/);
    await expect(chip).toHaveAttribute("title", /Nothing is watching the room/);

    await chip.click();
    await expect(
      page.locator(ROOM),
      "with no reading and no panel button, the room would be reachable from " +
        "nowhere at all",
    ).toBeVisible();
  });

  /** A falling room is marked as one, and differently. */
  test("a falling room is marked differently from a rising one", async ({ page }) => {
    await openShell(page, "/", {}, {
      mission_bar: reading(
        "↓",
        "The room is stiller than it has been for the last twenty minutes. " +
          "1 of 1 sense reads this way.",
      ),
    });

    await expect(page.locator(CHIP)).toContainText("↓");
    await expect(
      page.locator(CHIP),
      "a floor coming off and a floor picking up read the same",
    ).not.toContainText("↑");
  });

  /**
   * The room is a surface of its own, not a fold inside the assistant.
   *
   * §39's list — activity trend, movement, audio response, recent transitions,
   * response timing, historical comparison — is a panel, and it was nested two
   * levels inside the assistant where it could only exist where the assistant
   * did. Promoted rather than duplicated: it is gone from the assistant, so
   * there is one of it.
   */
  test("the room is its own panel and there is only one of it", async ({ page }) => {
    await openShell(page, "/");

    await page.getByRole("button", { name: "Assistant", exact: true }).click();
    await expect(page.locator('.surface[data-surface="assistant"]')).toBeVisible();
    await expect(
      page.locator(SENSOR),
      "the room sensor is still mounted inside the assistant, so opening it " +
        "from the indicator would give a DJ two of them measuring one camera",
    ).toHaveCount(0);

    await page.locator(CHIP).click();
    await expect(page.locator(ROOM)).toBeVisible();
    await expect(page.locator(SENSOR)).toHaveCount(1);
  });
});
