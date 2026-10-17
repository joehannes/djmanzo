"""Showcase images for the shop, from the filed screenshots: a grid of
themes, the skins, and the karaoke screens, each picture with a soft shadow
on a dark gradient. Written to docs/screenshots/showcase/.

    python3 -I showcase.py

Needs ImageMagick (convert, montage)."""
import os, subprocess, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
SHOTS = os.path.join(HERE, "../../docs/screenshots")
OUT = os.path.join(SHOTS, "showcase")

SETS = {
    "themes-grid": ("themes", ["dig--caribbean", "dig--malecon-night", "dig--bioluminescent-bay",
                               "dig--carnaval", "dig--cyber-trance", "dig--sunset",
                               "dig--karaoke-neon", "dig--aurora", "dig--bachata-rosa"], 3),
    "skins-grid": ("skins", ["dig--caribbean--palms", "dig--bioluminescent-bay--waves", "dig--carnaval--papel-picado",
                             "karaoke--karaoke-neon--halftone", "perform--industrial-techno--trash-polka", "dig--velvet--glowing-plankton"], 3),
    "screens-grid": ("screens", ["dig--organic-base", "perform--organic-base", "autopilot--organic-base",
                                 "karaoke--organic-base", "prepare--organic-base", "launcher--organic-base"], 3),
}

def shadowed(src, dst, width):
    subprocess.run(["convert", src, "-resize", f"{width}x", "(", "+clone", "-background", "black", "-shadow", "60x14+0+10", ")",
                    "+swap", "-background", "none", "-layers", "merge", "+repage", dst], check=True)

def main():
    os.makedirs(OUT, exist_ok=True)
    for name, (folder, files, columns) in SETS.items():
        paths = [os.path.join(SHOTS, folder, f + ".png") for f in files]
        paths = [p for p in paths if os.path.exists(p)]
        if not paths:
            continue
        with tempfile.TemporaryDirectory() as tmp:
            tiles = []
            for i, p in enumerate(paths):
                t = os.path.join(tmp, f"{i}.png")
                shadowed(p, t, 760)
                tiles.append(t)
            grid = os.path.join(tmp, "grid.png")
            subprocess.run(["montage", *tiles, "-tile", f"{columns}x", "-geometry", "+24+24", "-background", "none", grid], check=True)
            size = subprocess.run(["identify", "-format", "%wx%h", grid], capture_output=True, text=True, check=True).stdout
            w, h = (int(v) for v in size.split("x"))
            subprocess.run(["convert", "-size", f"{w + 120}x{h + 120}", "gradient:#1d1b2e-#0b0d12", grid, "-gravity", "center",
                            "-composite", os.path.join(OUT, name + ".png")], check=True)
    # One picture alone, large, for the top of a page.
    hero = os.path.join(SHOTS, "themes", "dig--caribbean.png")
    if os.path.exists(hero):
        with tempfile.TemporaryDirectory() as tmp:
            t = os.path.join(tmp, "hero.png")
            shadowed(hero, t, 1440)
            subprocess.run(["convert", "-size", "1680x980", "gradient:#123b3a-#0b0d12", t, "-gravity", "center", "-composite",
                            os.path.join(OUT, "hero.png")], check=True)

if __name__ == "__main__":
    main()
