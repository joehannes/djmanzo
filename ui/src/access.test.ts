/**
 * §33's rules, applied to every component rather than to what happened to be
 * on screen.
 *
 * `e2e/access.spec.ts` runs a real audit — axe-core over the shell and every
 * panel — and that is the better test of the two: it checks the tree a browser
 * actually built, with the colours it actually computed. It has one weakness,
 * and it is a large one. **It can only see what it can open.** The audit that
 * found the defects this file guards reached fourteen panels, and djmanzo has
 * forty-odd components; the controller pane needs a MIDI device, the plugin
 * pane needs a `.clap` on disk, and the timecode pane needs a sound card. None
 * of those exist in a container, so none of them were audited, and none of them
 * would be audited by the next person either.
 *
 * So this reads the source. It catches less — it knows nothing about contrast
 * or about computed roles — but it catches it everywhere, including in the
 * panel nobody can open on the machine the tests run on.
 *
 * Two rules, both the general form of something the audit actually found:
 *
 * - **A control the DJ chooses from has a name.** Six unnamed `select`s were
 *   on screen; there were eighteen more behind panels the audit could not open.
 * - **`aria-label` goes on something that can carry it.** ARIA drops the
 *   attribute when the element has no role, so the deck's level meter had been
 *   announcing itself as an empty box for as long as it had existed — the label
 *   was written, and thrown away before anyone heard it.
 */
import { describe, expect, it } from "vitest";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const SRC = join(import.meta.dirname);

function svelteFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((entry) => {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) return svelteFiles(path);
    return path.endsWith(".svelte") ? [path] : [];
  });
}

/** One tag as it was written, with where it started. */
interface Tag {
  name: string;
  attrs: string;
  closing: boolean;
  selfClosing: boolean;
  line: number;
}

/**
 * The tags in a component, in order.
 *
 * Deliberately not a parser. Svelte markup carries arbitrary TypeScript inside
 * `{...}`, and expressions contain `>` — so the scan tracks brace depth and
 * treats a `>` inside one as text. That is enough to know which element a
 * `select` is nested in, which is all either rule needs, and it is a great deal
 * less than a component whose markup this has to keep up with.
 */
function tags(source: string): Tag[] {
  // The script and style blocks are TypeScript and CSS, not markup: a
  // generic like `Map<string, X>` reads as a tag to anything this simple.
  const markup = source
    .replace(/<script[\s\S]*?<\/script>/g, (m) => m.replace(/[^\n]/g, " "))
    .replace(/<style[\s\S]*?<\/style>/g, (m) => m.replace(/[^\n]/g, " "))
    .replace(/<!--[\s\S]*?-->/g, (m) => m.replace(/[^\n]/g, " "));

  const found: Tag[] = [];
  for (let i = 0; i < markup.length; i += 1) {
    if (markup[i] !== "<") continue;
    const closing = markup[i + 1] === "/";
    const start = i + (closing ? 2 : 1);
    const nameMatch = /^[a-zA-Z][a-zA-Z0-9-]*/.exec(markup.slice(start));
    if (!nameMatch) continue;
    let depth = 0;
    let quote = "";
    let end = start + nameMatch[0].length;
    while (end < markup.length) {
      const c = markup[end];
      // Quoted attribute values first, and not as a nicety: a `placeholder`
      // reading `bpm > 120` ends the tag here if they are not skipped, and the
      // rule above then reports an input as nameless because its `aria-label`
      // was written after the `>` this stopped at. It did, once, and the
      // "missing" label was duly added on top of the one already there.
      if (quote) {
        if (c === quote) quote = "";
      } else if (c === '"' || c === "'") quote = c;
      else if (c === "{") depth += 1;
      else if (c === "}") depth -= 1;
      else if (c === ">" && depth === 0) break;
      end += 1;
    }
    const attrs = markup.slice(start + nameMatch[0].length, end);
    found.push({
      name: nameMatch[0],
      attrs,
      closing,
      selfClosing: attrs.trimEnd().endsWith("/"),
      line: markup.slice(0, i).split("\n").length,
    });
    i = end;
  }
  return found;
}

/** Elements that carry a name of their own, so a wrapping `label` is enough. */
const NAMED_BY_A_WRAPPING_LABEL = new Set(["select", "input", "textarea"]);

/** `input`s that are not a control the DJ names anything about. */
const UNNAMEABLE_INPUT = /type=["'](hidden|submit|reset|button|image)["']/;

function hasName(attrs: string): boolean {
  return /(?:^|\s)(aria-label|aria-labelledby)(?:=|\s|$)/.test(attrs);
}

/** The `for=` targets in a component, so `id` counts as a name. */
function labelled(source: string): Set<string> {
  return new Set(
    Array.from(source.matchAll(/<label[^>]*?\sfor=["']([^"']+)["']/g), (m) => m[1]),
  );
}

/** Whatever this element's `id` is, written as the `for=` would be. */
function idOf(attrs: string): string | null {
  return /(?:^|\s)id=["']([^"']+)["']/.exec(attrs)?.[1] ?? null;
}

describe("§33's labels", () => {
  /**
   * **The load-bearing one: every control has a name somewhere.**
   *
   * Either its own `aria-label`, or a `<label>` wrapped round it — which is the
   * better of the two where a caption is already drawn, because then the word
   * on screen and the word spoken are one word and cannot drift.
   *
   * A `select` with no name is announced as "combo box, Peak" — the value and
   * nothing else. §33 lists *keyboard operation* among the five things to
   * support, and a keyboard reaches every one of these; what it cannot do is
   * tell you which of the four on the panel it has landed on.
   */
  it("every select, input and textarea says what it is for", () => {
    const nameless: string[] = [];
    for (const file of svelteFiles(SRC)) {
      const open: string[] = [];
      const source = readFileSync(file, "utf8");
      const targets = labelled(source);
      for (const tag of tags(source)) {
        if (tag.closing) {
          const at = open.lastIndexOf(tag.name);
          if (at !== -1) open.splice(at, 1);
          continue;
        }
        if (!NAMED_BY_A_WRAPPING_LABEL.has(tag.name)) {
          if (!tag.selfClosing) open.push(tag.name);
          continue;
        }
        if (tag.name === "input" && UNNAMEABLE_INPUT.test(tag.attrs)) continue;
        const id = idOf(tag.attrs);
        if (hasName(tag.attrs) || open.includes("label")) continue;
        // A `<label for>` elsewhere in the same component, which is the best of
        // the three: the word on screen and the word spoken are then one word.
        if (id !== null && targets.has(id)) continue;
        nameless.push(`${relative(SRC, file)}:${tag.line} <${tag.name}>`);
      }
    }
    expect(
      nameless,
      "a control with no name is a control a screen reader announces by its " +
        "value and nothing else -- give it an aria-label, point a <label for> " +
        "at it, or wrap the caption already beside it in a <label>",
    ).toEqual([]);
  });

  /**
   * A label goes where something can carry it.
   *
   * ARIA forbids `aria-label` on an element with no role — a bare `div`, a
   * `span` — and forbidding it means *dropping* it: the words are parsed,
   * discarded, and nothing reports that they went. Which is worse than not
   * writing them, because the component reads as though it were handled.
   */
  it("no label is written onto an element that cannot carry one", () => {
    const roleless = new Set(["div", "span", "p", "li", "td", "small", "em", "strong"]);
    const dropped: string[] = [];
    for (const file of svelteFiles(SRC)) {
      for (const tag of tags(readFileSync(file, "utf8"))) {
        if (tag.closing || !roleless.has(tag.name)) continue;
        if (!hasName(tag.attrs)) continue;
        if (/(?:^|\s)role=/.test(tag.attrs)) continue;
        dropped.push(`${relative(SRC, file)}:${tag.line} <${tag.name}>`);
      }
    }
    expect(
      dropped,
      "these elements carry an aria-label that assistive technology throws " +
        "away, because the element has no role to hang it on -- give it a role, " +
        "or move the words somewhere they will be read",
    ).toEqual([]);
  });
});
