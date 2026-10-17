"""Clean the pictures in $WORK/shots and file them in docs/screenshots, with
an index.

    python3 -I finish.py

Cleaning removes one thing: the "UI n fps" warning, which djmanzo shows when
the webview draws without a GPU, as it does under Xvfb. It never appears on a
machine with graphics acceleration, so a picture for the shop should not carry
it. Nothing else in a picture is changed."""
import os, re, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
WORK = os.environ.get("WORK", "/tmp/djmanzo-shots")
OUT = os.path.join(HERE, "../../docs/screenshots")

SCREENS = {
    "dig": "Dig: the library open under the decks, and what could come next",
    "mix": "Mix: two decks and the mixer",
    "perform": "Perform: stems and the sampler",
    "prepare": "Prepare: records on their way to a deck, and the set plan",
    "requests": "Requests: what the room asked for",
    "autopilot": "Autopilot: the assistant mixing, the room, what could come next",
    "practice": "Practice: the practice lab and the headphone deck",
    "karaoke": "Karaoke: the singers' panel and the library",
    "event": "Event: the night prepared, and its plan",
    "launcher": "The launcher (0): every view one key away",
    "palette": "The command palette (Ctrl/Cmd + K)",
    "guides": "The assistant's guides",
    "settings": "Settings",
    "effects": "Both decks' effects open",
}

def group(slug):
    return "skins" if "--" in slug else ("screens" if slug in ("organic-base",) else "themes")

def main():
    files = sorted(f for f in os.listdir(f"{WORK}/shots") if re.match(r"^[a-z]+--[a-z0-9-]+(--[a-z0-9-]+)?\.png$", f))
    index = {}
    for name in files:
        screen, slug = name[:-4].split("--", 1)
        where = group(slug)
        if screen in ("launcher", "palette", "guides", "settings", "effects"):
            where = "screens"
        os.makedirs(f"{OUT}/{where}", exist_ok=True)
        subprocess.run([sys.executable, "-I", f"{HERE}/clean.py", f"{WORK}/shots/{name}", f"{OUT}/{where}/{name}"], check=True)
        index.setdefault(where, []).append((screen, slug, f"{where}/{name}"))
    subprocess.run([sys.executable, "-I", f"{HERE}/showcase.py"], check=True)
    write_index(index)

def title(slug):
    theme, _, skin = slug.partition("--")
    words = theme.replace("-", " ").title().replace("Malecon", "Malecón")
    return f"{words}, {skin.replace('-', ' ')} skin" if skin else words

def write_index(index):
    lines = [open(os.path.join(HERE, "index-head.md")).read().rstrip(), ""]
    shown = sorted(os.listdir(f"{OUT}/showcase")) if os.path.isdir(f"{OUT}/showcase") else []
    if shown:
        lines += ["## Showcase", "", "Grids for a shop page, made from the pictures below (`showcase.py`).", ""]
        lines += [f"[![{f[:-4]}](showcase/{f})](showcase/{f})" + "\n" for f in shown]
    for where, heading in (("screens", "Screens"), ("themes", "Themes"), ("skins", "Skins")):
        if where not in index:
            continue
        lines += [f"## {heading}", "", "| Picture | What it shows | Theme |", "|---|---|---|"]
        for screen, slug, path in index[where]:
            lines.append(f"| [![{screen}]({path})]({path}) | {SCREENS.get(screen, screen)} | {title(slug)} |")
        lines.append("")
    open(f"{OUT}/README.md", "w").write("\n".join(lines))

if __name__ == "__main__":
    main()
