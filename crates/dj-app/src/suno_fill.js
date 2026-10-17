// Karaoke beta B5: fill Suno's Custom form with a guest's words and style,
// in djmanzo's own Suno window, for the DJ to check and press Create.
//
// Run by `dj_app::suno` with `window.__djmanzoSuno = { lyrics, style, title }`
// set first. It fills what it finds the way typing would (the native value
// setter, then input and change events, so the page's own code sees it), and
// says on the page what it did. It never presses Create and never reads the
// page back to djmanzo: Suno's terms do not allow a robot to make the song,
// and the window has no way into djmanzo.
(() => {
  const data = window.__djmanzoSuno;
  if (!data) return;
  const BANNER = "djmanzo-suno-banner";

  const said = (el) =>
    [el.getAttribute("placeholder"), el.getAttribute("aria-label"), el.getAttribute("name"), el.id]
      .filter(Boolean)
      .join(" ");
  const visible = (el) => el.offsetParent !== null || el.getClientRects().length > 0;
  const field = (pattern, tags) =>
    Array.from(document.querySelectorAll(tags)).find(
      (el) => visible(el) && !el.disabled && !el.readOnly && pattern.test(said(el)),
    ) || null;
  const lyricsBox = () => field(/lyric/i, "textarea");
  const styleBox = () => field(/style|genre/i, "textarea, input[type=text], input:not([type])");
  const titleBox = () => field(/title/i, "input[type=text], input:not([type])");

  const type = (el, value) => {
    // A fill that waited for the form leaves what the DJ wrote alone.
    if (!data.overwrite && el.value) return false;
    const proto = el instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
    const setter = Object.getOwnPropertyDescriptor(proto, "value").set;
    el.focus();
    setter.call(el, value);
    el.dispatchEvent(new Event("input", { bubbles: true }));
    el.dispatchEvent(new Event("change", { bubbles: true }));
    el.blur();
    return true;
  };

  const tell = (text, ok) => {
    let banner = document.getElementById(BANNER);
    if (!banner) {
      banner = document.createElement("div");
      banner.id = BANNER;
      banner.setAttribute("role", "status");
      banner.style.cssText =
        "position:fixed;left:50%;bottom:18px;transform:translateX(-50%);z-index:2147483647;" +
        "max-width:min(90vw,560px);padding:10px 14px;border-radius:10px;font:14px/1.35 system-ui,sans-serif;" +
        "box-shadow:0 6px 24px rgba(0,0,0,.35);pointer-events:none";
      document.body.appendChild(banner);
    }
    banner.style.background = ok ? "#123b2a" : "#4a2a10";
    banner.style.color = "#fff";
    banner.textContent = text;
    setTimeout(() => banner.remove(), 12000);
  };

  let tries = 0;
  let pressedCustom = false;
  const attempt = () => {
    tries += 1;
    if (!lyricsBox() && !pressedCustom) {
      // The words box is Custom mode's; Simple mode shows only a description.
      const custom = Array.from(document.querySelectorAll("button, [role=switch], [role=tab], label")).find(
        (el) => visible(el) && /^\s*custom\s*$/i.test(el.textContent || ""),
      );
      if (custom) {
        custom.click();
        pressedCustom = true;
      }
    }
    const lyrics = lyricsBox();
    const style = styleBox();
    if (!lyrics && !style) {
      // About twenty seconds for Suno's page to draw its form.
      if (tries < (data.tries || 40)) return void setTimeout(attempt, 500);
      tell(
        data.waits
          ? "djmanzo is waiting for Suno's song form: sign in and open Create, and djmanzo fills it in then."
          : "djmanzo could not find Suno's song form. Sign in, open Create, then press “Fill in Suno's form” again in djmanzo.",
        false,
      );
      return;
    }
    const filled = [];
    const missing = [];
    const kept = [];
    if (lyrics) (type(lyrics, data.lyrics) ? filled : kept).push("the words");
    else missing.push("the words box");
    if (style) (type(style, data.style) ? filled : kept).push("the style");
    else missing.push("the style box");
    const title = titleBox();
    if (title && data.title && !title.value) type(title, data.title);
    if (filled.length === 0 && missing.length === 0) {
      tell(`djmanzo left ${kept.join(" and ")} as you wrote them. Press Create when ready.`, true);
    } else if (missing.length === 0) {
      tell(`djmanzo filled in ${filled.join(" and ")}. Check them, then press Create.`, true);
    } else {
      tell(
        `djmanzo filled in ${filled.join(" and ")} but could not find ${missing.join(" or ")}: paste it from djmanzo. Then press Create.`,
        false,
      );
    }
  };
  attempt();
})();
