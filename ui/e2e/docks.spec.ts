/**
 * Phase 2's gate: a new shell with the old functionality intact.
 *
 * `docs/GUI-OVERHAUL.md` §21 states it as *"every surface still reachable"* --
 * no feature may become unreachable during the migration, which is exactly the
 * failure a 1,700-line `App.svelte` invites when its panel slot is rebuilt.
 * This is the measurement of that.
 *
 * It also measures the thing the migration was *for*. The audit's headline
 * finding was structural: one `panel` variable held one of eight names, so a
 * DJ could not look at the room and the library at the same time -- not
 * because anyone decided that, but because the shell was shaped that way. A
 * test that only checked reachability would pass against the old shell too.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

/**
 * Every panel button in the top bar, and the surface it opens.
 *
 * The buttons are named by their labels because that is what a DJ reads. The
 * surface names are djmanzo's own, from `dj_app::cockpit::surfaces()`.
 */
const PANELS: { button: string; surface: string }[] = [
  { button: "Browse", surface: "library" },
  { button: "Prepare", surface: "prepare" },
  { button: "Presets", surface: "presets" },
  { button: "Booth", surface: "booth" },
  { button: "Sampler", surface: "sampler" },
  { button: "Assistant", surface: "assistant" },
  { button: "Settings", surface: "settings" },
  { button: "Keys", surface: "keys" },
  { button: "Log", surface: "log" },
];

test.describe("the dock manager", () => {
  test("every panel the top bar offers still opens", async ({ page }) => {
    await openShell(page, "/");

    for (const { button, surface } of PANELS) {
      await page.getByRole("button", { name: button, exact: true }).click();
      await expect(
        page.locator(`.surface[data-surface="${surface}"]`),
        `pressing ${button} did not open the ${surface} surface -- a feature ` +
          "that was one press away before the docks is now unreachable",
      ).toBeVisible();
      // Closed again, so each is measured on its own rather than in the
      // company of everything opened before it.
      await page.getByRole("button", { name: button, exact: true }).click();
      await expect(page.locator(`.surface[data-surface="${surface}"]`)).toHaveCount(0);
    }
  });

  /**
   * The reason the shell was rebuilt at all.
   *
   * Three things on screen at once -- the decks, the library along the bottom,
   * the assistant beside them. Under the old shell opening the second closed
   * the first, and this assertion would have failed on the deck as well as on
   * the panel.
   */
  test("the library and the assistant can be open together, over the decks", async ({
    page,
  }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Browse", exact: true }).click();
    await page.getByRole("button", { name: "Assistant", exact: true }).click();

    await expect(page.locator('.surface[data-surface="library"]')).toBeVisible();
    await expect(page.locator('.surface[data-surface="assistant"]')).toBeVisible();
    await expect(
      page.locator(".deck").first(),
      "the decks went away when two surfaces were opened, which is the old " +
        "problem wearing a new shape",
    ).toBeVisible();
  });

  /**
   * A wide surface goes along the bottom and a tall one beside the decks.
   *
   * Not a table of special cases: the rule is the surface's own preferred size,
   * which Rust publishes. The library prefers 900x380 and lands under the
   * decks; the assistant is taller than it is wide and stands beside them.
   * Asserted geometrically rather than by class name, because "below" and
   * "beside" are the claims a DJ can actually check.
   */
  test("the library lands below the decks and the assistant beside them", async ({
    page,
  }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Browse", exact: true }).click();
    await page.getByRole("button", { name: "Assistant", exact: true }).click();

    const deck = await page.locator(".deck").first().boundingBox();
    const library = await page.locator('.surface[data-surface="library"]').boundingBox();
    const assistant = await page
      .locator('.surface[data-surface="assistant"]')
      .boundingBox();
    expect(deck).not.toBeNull();
    expect(library).not.toBeNull();
    expect(assistant).not.toBeNull();

    expect(
      library!.y,
      "the library is not below the decks, so it is not in the bottom dock",
    ).toBeGreaterThan(deck!.y);
    expect(
      assistant!.x,
      "the assistant is not to the right of the decks, so it is not in the side dock",
    ).toBeGreaterThan(deck!.x + deck!.width - 1);
  });

  /**
   * The directive's §21: Prepare is first class, and the gesture still works.
   *
   * Prepare used to be a child of the browser -- mounted inside the library
   * panel and handed a track by a prop, so it could only exist where the
   * library was and only while the library was open. Making it a surface of
   * its own is what "first class" means, and the risk of the move is exactly
   * what §21 warns about: an inconsistent Prepare gesture, which is the thing
   * Engine DJ users complain about.
   *
   * Two halves could break. They must open together over the decks, which is
   * the point of moving it; and the handoff must survive the two being
   * siblings rather than parent and child, which is what
   * `prepare.svelte.ts` carries.
   */
  test("the library and Prepare are separate surfaces that open together", async ({
    page,
  }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Browse", exact: true }).click();
    await page.getByRole("button", { name: "Prepare", exact: true }).click();

    await expect(page.locator('.surface[data-surface="library"]')).toBeVisible();
    await expect(page.locator('.surface[data-surface="prepare"]')).toBeVisible();
    await expect(
      page.locator(".deck").first(),
      "the decks went away when the library and Prepare were both opened",
    ).toBeVisible();
  });

  /**
   * **The gesture itself**, which is what §21 is actually about.
   *
   * The two tests around this one prove the surfaces open. They do not prove
   * that pressing → in the browser reaches Prepare, and the difference is not
   * academic: replacing the browser's handler with an empty function broke
   * nothing, because the fixture had no rows to press. A workflow test needs
   * something to act on, so `shell.ts` now answers `library_search` with one
   * record.
   *
   * What is asserted is the command, not a rendered list: the sidelist's
   * contents come from Rust and this harness answers with an empty one, so the
   * honest claim is that the press crossed from the browser to the Prepare
   * space and asked for the right track.
   */
  test("setting a track aside from the browser reaches Prepare", async ({ page }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Browse", exact: true }).click();
    await page.getByRole("button", { name: "Prepare", exact: true }).click();

    await page.evaluate(() => ((window as unknown as Record<string, unknown>).__asked = []));
    await page.getByRole("button", { name: "Set aside Bachata Rosa" }).click();

    await expect
      .poll(
        () =>
          page.evaluate(
            () => (window as unknown as { __asked: string[] }).__asked,
          ),
        {
          message:
            "pressing the browser's set-aside button did not reach the Prepare " +
            "space -- the two are sibling surfaces now, and the handoff between " +
            "them is `prepare.svelte.ts`",
        },
      )
      .toContain("sidelist_add");
  });

  /**
   * Prepare opens on its own, with no library in sight.
   *
   * The thing that was impossible before, and the whole of §21: planning a set
   * is a different activity from browsing, and it should not require the
   * browser to be on screen.
   */
  test("Prepare opens without the library", async ({ page }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Prepare", exact: true }).click();

    await expect(page.locator('.surface[data-surface="prepare"]')).toBeVisible();
    await expect(page.locator('.surface[data-surface="library"]')).toHaveCount(0);
  });

  /** A surface closes from its own header, not only from the top bar. */
  test("a surface closes from its own header", async ({ page }) => {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Browse", exact: true }).click();
    await expect(page.locator('.surface[data-surface="library"]')).toBeVisible();

    await page.getByRole("button", { name: "Close Library" }).click();
    await expect(page.locator('.surface[data-surface="library"]')).toHaveCount(0);
  });

  /** Nothing may throw while the docks are being opened. See `budget.spec.ts`. */
  test("opening every surface throws nothing", async ({ page }) => {
    await openShell(page, "/");
    for (const { button } of PANELS) {
      await page.getByRole("button", { name: button, exact: true }).click();
    }
    await expect(page.locator(".surface").first()).toBeVisible();
    expect(
      errorsThrown(page),
      "a surface threw while it was being opened, so part of the cockpit did " +
        "not finish rendering",
    ).toEqual([]);
  });
});

/**
 * §3's eleven, and the three of them that were fields nothing read.
 *
 * > The UI consists of composable surfaces and zones that can be: docked,
 * > **resized**, **collapsed**, **expanded**, stacked, detached, temporarily
 * > surfaced, **pinned**, contextually promoted, contextually demoted, or
 * > automatically rearranged.
 *
 * `Placement` carried `size`, `collapsed` and `pinned`; Rust stored them,
 * serialised them and resolved them; and this side read none of the three. A
 * DJ could collapse nothing, resize nothing and pin nothing, and the workspace
 * file faithfully recorded all three. The table and its reasons are
 * `dj_app::cockpit::Shaping`; these are the three verbs actually happening.
 */
test.describe("what a surface can be", () => {
  /** What the shell has written, most recent last. */
  async function saved(page: import("@playwright/test").Page) {
    return page.evaluate(
      () => (window as unknown as { __saved?: unknown[] }).__saved ?? [],
    );
  }

  const LIBRARY = '.surface[data-surface="library"]';

  async function openLibrary(page: import("@playwright/test").Page) {
    await openShell(page, "/");
    await page.getByRole("button", { name: "Browse", exact: true }).click();
    await expect(page.locator(LIBRARY)).toBeVisible();
  }

  /**
   * **Collapsed and expanded, which are two of §3's eleven.**
   *
   * A collapsed surface keeps its header — that is the difference from a
   * closed one, and the whole reason §3 lists both verbs: a DJ folding the
   * browser away still wants to know it is there and to get it back without
   * finding the right button in the top bar again.
   */
  test("a surface folds to its own header and comes back", async ({ page }) => {
    await openLibrary(page);
    const body = page.locator(`${LIBRARY} .surface-body`);
    await expect(body).toBeVisible();

    const open = (await page.locator(LIBRARY).boundingBox())!.height;
    await page.getByRole("button", { name: "Collapse Library" }).click();
    await expect(page.locator(LIBRARY)).toHaveAttribute("data-collapsed", "true");
    await expect(body).toBeHidden();
    // **And the panel gives the room back**, which is a different claim from
    // the body being hidden and is the one a DJ actually wants. Found by
    // driving the application: the first version hid the body and kept the
    // dock's eight-rem floor, so folding a panel left a header above a hand's
    // width of empty panel. The attribute was set and the body was hidden, and
    // this test passed.
    await expect
      .poll(async () => (await page.locator(LIBRARY).boundingBox())!.height)
      .toBeLessThan(open / 2);
    // The header is still there, which is what makes this a fold and not a
    // close.
    await expect(page.getByRole("button", { name: "Expand Library" })).toBeVisible();

    await page.getByRole("button", { name: "Expand Library" }).click();
    await expect(body).toBeVisible();
    expect(errorsThrown(page), "the shell threw").toEqual([]);
  });

  /** And the fold is kept, or it is a fold that lasts until the next restart. */
  test("a fold is written to the workspace", async ({ page }) => {
    await openLibrary(page);
    await page.getByRole("button", { name: "Collapse Library" }).click();
    await expect
      .poll(async () => {
        const writes = (await saved(page)) as {
          surfaces: { surface: string; collapsed: boolean }[];
        }[];
        return writes
          .at(-1)
          ?.surfaces.find((p) => p.surface === "library")?.collapsed;
      })
      .toBe(true);
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **Resized, on the dock's own axis.**
   *
   * The handle is on the edge that faces the performance zone, so the left
   * dock grows to the right. A handle on the wrong edge is not a subtle bug:
   * the panel runs away from the pointer.
   */
  test("a surface is resized by dragging its edge, and the size is kept", async ({
    page,
  }) => {
    // The assistant, because it lives in a side dock: a side dock is a column,
    // so its surfaces share one width and what a single one can vary is its
    // height. Choosing the axis deliberately matters — the first version
    // resized the *other* one, and a panel made wider than the dock holding it
    // simply overflows.
    await openShell(page, "/");
    await page.getByRole("button", { name: "Assistant", exact: true }).click();
    const surface = page.locator('.surface[data-surface="assistant"]');
    await expect(surface).toBeVisible();
    const before = (await surface.boundingBox())!;

    const grip = page.getByRole("separator", { name: "Resize Assistant" });
    const handle = (await grip.boundingBox())!;
    await page.mouse.move(handle.x + handle.width / 2, handle.y + handle.height / 2);
    await page.mouse.down();
    // Downwards: the handle is on the surface's trailing edge along the axis
    // its dock stacks on, so dragging away from the surface grows it.
    await page.mouse.move(
      handle.x + handle.width / 2,
      handle.y + handle.height / 2 + 120,
      { steps: 8 },
    );
    await page.mouse.up();

    const after = (await surface.boundingBox())!;
    expect(
      after.height,
      "dragging the edge outwards did not grow the panel",
    ).toBeGreaterThan(before.height + 40);

    await expect
      .poll(async () => {
        const writes = (await saved(page)) as {
          surfaces: { surface: string; size: number | null }[];
        }[];
        return writes.at(-1)?.surfaces.find((p) => p.surface === "assistant")?.size;
      })
      .toBeGreaterThan(before.height + 40);

    // **And the panel wears the size it stored**, which is a different claim
    // from the drag having worked. During a drag the handler writes the width
    // straight onto the element, so a shell that stored the size and never
    // read it back would pass everything above and come up at the default
    // width on the next launch. A fold and an unfold is the cheapest thing
    // that makes the placement re-render from what was kept.
    const dragged = (await surface.boundingBox())!.height;
    await page.getByRole("button", { name: "Collapse Assistant" }).click();
    await expect(surface).toHaveAttribute("data-collapsed", "true");
    await page.getByRole("button", { name: "Expand Assistant" }).click();
    await expect
      .poll(async () => (await surface.boundingBox())!.height)
      .toBeCloseTo(dragged, 0);
    expect(errorsThrown(page), "the shell threw").toEqual([]);
  });

  /**
   * **Pinned, which is the per-surface half of §78's freeze.**
   *
   * "Never moved, resized or closed by adaptation" — and an arrangement is the
   * loudest adaptation there is, because it replaces every placement at once.
   * A DJ who has put a panel where they want it keeps it when they press a
   * preset; that is the whole of what pinning said.
   */
  test("a pinned surface survives an arrangement that does not name it", async ({
    page,
  }) => {
    await openLibrary(page);
    await page.getByRole("button", { name: "Pin Library" }).click();
    await expect(page.locator(LIBRARY)).toHaveAttribute("data-pinned", "true");

    // An arrangement. Whichever djmanzo ships first — the point is that it is
    // not this DJ's own and says nothing about the library.
    const picker = page.getByRole("combobox", { name: /workspace/i }).first();
    const options = await picker.locator("option").allTextContents();
    const other = options.find((o) => o && !/workspace|keep/i.test(o));
    await picker.selectOption({ label: other! });

    await expect(
      page.locator(LIBRARY),
      "an arrangement closed a panel the DJ had pinned",
    ).toBeVisible();
    expect(errorsThrown(page), "the shell threw").toEqual([]);
  });
});
