/**
 * §5's Mission Bar.
 *
 * > Always-present but compact. […] It should behave more like an aircraft HUD
 * > than a conventional application toolbar. Do not fill it with menus.
 *
 * What each reading says and whether it is worth a colour is `dj_app::mission`
 * and is tested there against readings this container cannot produce — a
 * failing recording, a limiter being driven, an audio thread out of headroom.
 * These say the interface draws what Rust answered, spends colour only where
 * Rust asked for it, and that the one pressable reading still opens the room.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

const BAR = ".mission";
const item = (slug: string) => `${BAR} [data-mission="${slug}"]`;

/** A bar with everything on it, the way a running night reads. */
const RUNNING = [
  {
    slug: "phase",
    label: "",
    value: "peak",
    level: "quiet",
    about: "The set reads as peak. Certainty: sure.",
  },
  {
    slug: "occasion",
    label: "",
    value: "club",
    level: "quiet",
    about: "You said this is a club night.",
  },
  {
    slug: "posture",
    label: "AI",
    value: "suggest",
    level: "quiet",
    about: "The assistant is on suggest. Offers, with reasons. Never acts.",
  },
  {
    slug: "room",
    label: "ROOM",
    value: "↑",
    level: "quiet",
    about: "The room is busier than it has been for the last twenty minutes. 2 of 3 senses read this way.",
  },
  { slug: "tempo", label: "", value: "128.0 BPM", level: "quiet", about: "The record playing is at 128.0 BPM." },
  { slug: "clock", label: "", value: "1:02:00", level: "quiet", about: "The night has been running 1:02:00." },
  {
    slug: "output",
    label: "OUT",
    value: "clean",
    level: "quiet",
    about: "No dropouts, and the limiter is working within itself.",
  },
  {
    slug: "device",
    label: "",
    value: "48 kHz",
    level: "quiet",
    about: "Playing out of Pioneer DJM at 48 kHz, 5.3 ms.",
  },
  { slug: "health", label: "CPU", value: "22%", level: "quiet", about: "The audio thread is using 22% of its time." },
];

test.describe("§5's mission bar", () => {
  /**
   * **The load-bearing one: every reading is on screen, and none of them
   * shouts.**
   *
   * §5's instruction is as much about restraint as about content — an
   * instrument panel is read in the half second between two other things, and
   * a bar with three amber items on a normal night is a bar a DJ stops
   * reading. So this asserts both halves at once: the readings are all there,
   * and a healthy night spends no colour at all.
   */
  test("a running night reads across the bar without lighting anything up", async ({
    page,
  }) => {
    await openShell(page, "/", {}, { mission_bar: RUNNING });

    for (const reading of RUNNING) {
      await expect(
        page.locator(item(reading.slug)),
        `§5 asks for ${reading.slug} and the bar does not draw it`,
      ).toContainText(reading.value);
    }

    await expect(
      page.locator(`${BAR} [data-level="watch"], ${BAR} [data-level="alarm"]`),
      "a night with nothing wrong with it put colour on the bar -- which is how " +
        "a HUD becomes something nobody looks at",
    ).toHaveCount(0);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * The bar draws the level Rust decided, rather than deciding one.
   *
   * The whole reason the gathering is in Rust: before §5 the load went amber at
   * 0.7 because a line in `App.svelte` said so, and nothing else on the strip
   * had a rule at all — so a recording that had stopped writing was invisible
   * beside a CPU figure a DJ can do nothing about.
   */
  test("a reading that is wrong is marked at the level Rust gave it", async ({
    page,
  }) => {
    await openShell(page, "/", {}, {
      mission_bar: [
        {
          slug: "recording",
          label: "REC",
          value: "42:10",
          level: "alarm",
          about: "The recording stopped writing. What is on disk ends where it stopped.",
        },
        {
          slug: "output",
          label: "OUT",
          value: "-8 dB",
          level: "watch",
          about: "The limiter is taking 8.0 dB off the mix.",
        },
        {
          slug: "health",
          label: "CPU",
          value: "30%",
          level: "quiet",
          about: "The audio thread is using 30% of its time.",
        },
      ],
    });

    await expect(page.locator(item("recording"))).toHaveAttribute("data-level", "alarm");
    await expect(page.locator(item("output"))).toHaveAttribute("data-level", "watch");
    await expect(
      page.locator(item("health")),
      "the bar coloured a reading Rust called quiet, so the threshold is being " +
        "decided in two places again",
    ).toHaveAttribute("data-level", "quiet");
  });

  /**
   * The reading a DJ cannot act on from the bar is the one that opens a panel.
   *
   * §39's *click / expand*, still working now that the chip lives on §5's bar.
   */
  test("the room reading still opens the room", async ({ page }) => {
    await openShell(page, "/", {}, { mission_bar: RUNNING });

    await expect(page.locator('.surface[data-surface="room"]')).toHaveCount(0);
    await page.locator(item("room")).click();
    await expect(page.locator('.surface[data-surface="room"]')).toBeVisible();
  });

  /**
   * A fresh install shows what it can read and nothing else.
   *
   * The default fixture is the bar at its emptiest, which is what every machine
   * looks like before Connect — and what this container looks like always.
   * Four readings: a room nobody is watching, a clean bus, no sound card and an
   * idle machine. No phase, no occasion, no tempo, no clock, because djmanzo
   * has read none of them.
   */
  test("a shell with nothing open says so rather than showing blanks", async ({
    page,
  }) => {
    await openShell(page, "/");

    await expect(page.locator(`${BAR} [data-mission]`)).toHaveCount(4);
    await expect(
      page.locator(item("room")),
      "the room reading is the way into the room panel and has to draw even " +
        "with nothing to report",
    ).toContainText("—");
    await expect(page.locator(item("device"))).toContainText("no device");
    await expect(
      page.locator(item("device")),
      "nothing is open, so nothing on the bar above it means anything yet",
    ).toHaveAttribute("data-level", "watch");
    await expect(
      page.locator(item("phase")),
      "the bar invented a phase for a night nothing has read",
    ).toHaveCount(0);
  });
});
