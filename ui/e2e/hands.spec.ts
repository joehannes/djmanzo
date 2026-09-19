/**
 * §53: the interface knowing what is plugged into it.
 *
 * > The UI should know: number of decks actually controllable, available
 * > physical knobs, jogs, pads, stem controls, mixer channels, displays, LED
 * > feedback. Use that to determine which GUI surfaces deserve prominence.
 * >
 * > Example: if a controller has dedicated stem pads: compact GUI stem panel.
 * > If there are no stem controls: expand stem controls.
 *
 * What a mapping reaches, and how it is counted, is `dj_hid::hands` and is
 * tested there against every mapping djmanzo ships. These say the two things
 * only a browser can — that the reading arrives on screen, and that §53's own
 * worked example actually moves a panel.
 */
import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import { compositions, errorsThrown, openShell } from "./shell";

/** A controller that reaches everything except the stems. */
const NO_STEMS = {
  decks: 2,
  knobs: 8,
  jogs: 2,
  pads: 16,
  stems: false,
  channels: 2,
  leds: 12,
};

/** The same controller with stem pads on it. */
const WITH_STEMS = { ...NO_STEMS, stems: true };

/**
 * A pad-and-fader controller: everything under the hands except a platter.
 *
 * Real hardware, not a contrived reading -- a grid controller mapped for
 * djmanzo reaches its pads, its faders and its EQ and has no jog anywhere on
 * it. `stems: true` so this cannot accidentally pass through the stem
 * adaptation instead.
 */
const NO_JOGS = { ...NO_STEMS, stems: true, jogs: 0 };

const stems = (page: Page) => page.locator("[data-stems-open]").first();

const DECK = "section.deck[data-deck]";

/** The rendered diameter of a deck's wheel, in pixels. */
async function platterWidth(page: Page): Promise<number> {
  const platter = page.locator(`${DECK} .platter`).first();
  await expect(platter).toBeVisible();
  return platter.evaluate((el) => el.getBoundingClientRect().width);
}

test.describe("§53's controller-aware interface", () => {
  /**
   * **The load-bearing one: a controller that cannot reach the stems opens the
   * stem module, and one that can leaves it folded.**
   *
   * §53's own worked example, and the direction is the whole of it. The module
   * is the biggest block on a deck — about 370 pixels — so unfolding it for
   * everybody would be the mistake it was folded to fix, and folding it for the
   * DJ who has no other way to reach the stems hides the feature from the only
   * person who needs it.
   */
  test("no stem controls on the hardware gives the stem module room", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/", {}, { controller_hands: NO_STEMS });
    await expect.poll(async () => stems(page).getAttribute("data-stems-open")).toBe("true");
    expect(thrown).toEqual([]);
  });

  test("a controller with stem pads leaves the module folded", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/", {}, { controller_hands: WITH_STEMS });
    // Held rather than merely read once: the profile is polled, so a panel that
    // unfolded a beat late would pass a single read and fail a DJ.
    await expect(stems(page)).toHaveAttribute("data-stems-open", "false");
    await expect(stems(page)).toHaveAttribute("data-stems-open", "false");
    expect(thrown).toEqual([]);
  });

  /**
   * **And nothing plugged in leaves it folded too.**
   *
   * The half worth asserting separately, because "no controller" and "a
   * controller that reaches nothing" are different answers and the code has to
   * keep them apart. A laptop-only DJ is already the case the interface is
   * designed around; unfolding for them would be §53 answering a question
   * nobody asked.
   */
  test("nothing plugged in is not the same as a controller with no stems", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/");
    await expect(stems(page)).toHaveAttribute("data-stems-open", "false");
    expect(thrown).toEqual([]);
  });

  /**
   * **§53's second prominence judgement: a controller with no platter on it
   * makes the one on screen bigger.**
   *
   * > Use that to determine which GUI surfaces deserve prominence.
   *
   * The deck's wheel is deliberately a small readout -- the waveform above
   * answers *where am I* better than a circle does -- and that reasoning
   * assumes the hand has somewhere better to be. A pad-and-fader controller
   * puts the hands on the pads and the EQ and leaves nudging a record back
   * into time as the one gesture on that deck with no hardware behind it.
   *
   * Measured off the screen rather than off a flag, for the reason §5B's
   * composition test gives: a size can be set, serialised and resolved and
   * still reach nothing.
   */
  test("a controller with no jog gives the on-screen platter room", async ({ page }) => {
    const thrown = errorsThrown(page);

    // Both arms on one page, and compared against each other rather than
    // against a number. Every size on a deck is multiplied by the density
    // token, so a threshold typed here is a threshold about whatever density
    // the shell happens to open at -- the first version of this asserted
    // `> 90` against a wheel that renders at 83, and the adaptation was
    // working perfectly.
    await openShell(page, "/", {}, { controller_hands: NO_STEMS });
    const withPlatter = await platterWidth(page);

    await openShell(page, "/", {}, { controller_hands: NO_JOGS });
    const withoutPlatter = await platterWidth(page);

    expect(
      withoutPlatter,
      "a controller with no jog on it should make the screen's wheel a target rather than a readout",
    ).toBeGreaterThan(withPlatter * 1.2);
    expect(thrown).toEqual([]);
  });

  /**
   * **And the two controls, which are the whole of the direction.**
   *
   * A controller *with* jogs leaves the wheel alone: shrinking it would be
   * contextual demotion, which §3 refuses and §17 is built never to do, and
   * growing it would be the adaptation firing for a DJ who has a platter under
   * their hand. Nothing plugged in leaves it alone too -- a laptop-only DJ is
   * the case the default was chosen for.
   *
   * Both asserted twice, because the profile is polled and a deck that grew a
   * beat late would pass a single read.
   */
  test("nothing plugged in gets the same wheel as a controller with jogs", async ({ page }) => {
    const thrown = errorsThrown(page);

    await openShell(page, "/", {}, { controller_hands: NO_STEMS });
    const withPlatter = await platterWidth(page);

    await openShell(page, "/");
    const laptopOnly = await platterWidth(page);

    // Equal, not merely both small: "no controller" and "a controller with a
    // platter" are different facts that have to land on the same default, and
    // a range would pass an adaptation that fired weakly for the laptop DJ.
    expect(laptopOnly).toBe(withPlatter);

    // Held, because the profile is polled: a deck that grew a beat late would
    // pass a single read and fail a DJ.
    expect(await platterWidth(page)).toBe(withPlatter);
    expect(thrown).toEqual([]);
  });

  /**
   * **An arrangement that asked for a platter keeps it, whatever is plugged
   * in.**
   *
   * The adaptation moves a *default*, not a stated size, and until this test
   * existed nothing said so: every other case here opens the shipped deck,
   * and §5B's composition tests open with no controller, so the two
   * conditions never met. A mutation that let the adaptation win found nothing
   * to fail.
   *
   * The direction is what makes it matter rather than tidy. §5B's scratch
   * composition asks for about 200 px -- hands on the records, a waveform
   * sacrificed for it -- and the adaptation's size is 104. A rule that
   * overrode the tree would *shrink* a scratch deck's platter by half the
   * moment its DJ plugged in a pad controller to go with it, which is §53
   * overruling §5B while claiming to serve the same hands.
   */
  test("a composition that asks for a platter outranks the adaptation", async ({ page }) => {
    const thrown = errorsThrown(page);

    await openShell(page, "/", {}, { layout_tree: compositions.Scratch });
    const asked = await platterWidth(page);

    await openShell(page, "/", {}, {
      layout_tree: compositions.Scratch,
      controller_hands: NO_JOGS,
    });
    expect(
      await platterWidth(page),
      "a stated size is what the DJ asked for; §53 moves the default",
    ).toBe(asked);
    expect(thrown).toEqual([]);
  });

  /**
   * **The panel says what djmanzo found, including the one it cannot see.**
   *
   * §53 opens with *"the UI should know"*, and a DJ who has just plugged
   * something in is entitled to see what djmanzo thinks it can do before
   * finding out mid-set that it disagrees. The screens are named rather than
   * counted, because a controller's displays are driven by its own firmware
   * and are unknowable from a mapping.
   */
  test("the controllers panel reports the reach it read", async ({ page }) => {
    const thrown = errorsThrown(page);
    await openShell(page, "/", {}, {
      controller_hands: NO_STEMS,
      controller_lights: { lit: 0, port: "", unlit: "" },
      control_status: {
        inputs: ["DDJ-SR MIDI 1"],
        // Absent here until the page-error guard started working, and the
        // panel was throwing on it in every run: `status.hid_inputs.length`
        // on an answer that did not carry the field. The layout assertions
        // above passed anyway, because Svelte abandons the rest of a render
        // pass and a panel that stops early still contains what it drew first.
        hid_inputs: [],
        open_port: "DDJ-SR MIDI 1",
        open_mapping: "Pioneer DDJ-SR",
        unavailable: null,
        keyboard: true,
        keyboard_name: "",
      },
      control_mappings: [
        {
          name: "Pioneer DDJ-SR",
          device: "DDJ-SR",
          bindings: 113,
          bundled: true,
          hands: NO_STEMS,
        },
      ],
      // The Controllers panel is a surface, opened the way a DJ opens one.
      palette: {
        because: "",
        entries: [
          {
            label: "Show Controllers",
            about: "What is plugged in, and what it is mapped to.",
            kind: "surface",
            run: "controllers",
            tier: "preparation",
          },
        ],
      },
    });
    await page.keyboard.press("Control+k");
    await page.getByRole("button", { name: /Show Controllers/ }).first().click();

    const reach = page.locator("[data-reach]");
    await expect(reach).toBeVisible();
    await expect(reach).toContainText("Stem controls");
    await expect(reach).toContainText("none");
    await expect(reach).toContainText("Pads");
    // The one it cannot answer, said rather than counted.
    await expect(page.locator('[data-surface="controllers"]')).toContainText(
      "cannot see them",
    );
    expect(thrown).toEqual([]);
  });
});

/**
 * §53's other direction: the controller showing what the interface knows.
 *
 * The lights were the fourth table in this codebase found parsed, validated
 * and read by nothing: a mapping declared its `[[feedback]]` blocks,
 * `FeedbackMap::parse` resolved every parameter name in them, and no byte ever
 * left the machine. The pump is `dj_hid::feedback::Lights` and is tested
 * there, against a sink rather than against hardware — **this container has no
 * MIDI service at all**, so what a browser can prove is the part that matters
 * to a DJ looking at the panel: that a dark board says *which* kind of dark it
 * is, because the three have three different answers.
 */
test.describe("§53's lights", () => {
  const open = (lights: { lit: number; port: string; unlit: string }) => ({
    controller_hands: NO_STEMS,
    controller_lights: lights,
    control_status: {
      inputs: ["DDJ-SR MIDI 1"],
      // The panel draws the HID half too, and an answer missing a field djmanzo
      // always sends is a stub the interface can trip over rather than a state
      // it has to handle.
      hid_inputs: [],
      open_port: "DDJ-SR MIDI 1",
      open_mapping: "Pioneer DDJ-SR",
      unavailable: null,
      keyboard: true,
      keyboard_name: "",
    },
    control_mappings: [
      {
        name: "Pioneer DDJ-SR",
        device: "DDJ-SR",
        bindings: 113,
        bundled: true,
        hands: NO_STEMS,
        lights: 12,
      },
    ],
    palette: {
      because: "",
      entries: [
        {
          label: "Show Controllers",
          about: "What is plugged in, and what it is mapped to.",
          kind: "surface",
          run: "controllers",
          tier: "preparation",
        },
      ],
    },
  });

  async function panel(page: Page, lights: { lit: number; port: string; unlit: string }) {
    await openShell(page, "/", {}, open(lights));
    await page.keyboard.press("Control+k");
    await page.getByRole("button", { name: /Show Controllers/ }).first().click();
    return page.locator("[data-lights]");
  }

  /** **Lit says what is lit, and where it is going.** */
  test("a driven board says how many lights and out to where", async ({ page }) => {
    const said = await panel(page, {
      lit: 12,
      port: "DDJ-SR MIDI 1",
      unlit: "",
    });
    await expect(said).toContainText("driving 12");
    await expect(said).toContainText("DDJ-SR MIDI 1");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **A board that cannot be lit says why, in Rust's words.**
   *
   * The half worth having. "No lights" for a mapping that declares twelve of
   * them reads as djmanzo not supporting feedback; the actual answer — that
   * something else holds the output — is one a DJ can do something about.
   */
  test("a board that cannot be lit gives the reason rather than a blank", async ({
    page,
  }) => {
    const said = await panel(page, {
      lit: 0,
      port: "",
      unlit: 'could not open "DDJ-SR MIDI 1": port is in use',
    });
    await expect(said).toContainText("describes 12");
    await expect(said).toContainText("port is in use");
    await expect(said).not.toContainText("driving");
    expect(errorsThrown(page)).toEqual([]);
  });

  /**
   * **A mapping with no lights in it says only what it describes.**
   *
   * Not a failure and not worth a reason: plenty of mappings bind a hundred
   * controls and declare no feedback at all.
   */
  test("a mapping with nothing to light claims neither", async ({ page }) => {
    const said = await panel(page, { lit: 0, port: "", unlit: "" });
    await expect(said).toContainText("describes 12");
    await expect(said).not.toContainText("driving");
    await expect(said).not.toContainText("cannot send");
    expect(errorsThrown(page)).toEqual([]);
  });
});
