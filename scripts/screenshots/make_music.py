"""Make the demo collection the screenshots are taken with: six original
tracks synthesised here (gen_tracks.py), each with cover art, tags, and for
two of them words in time (.lrc) for the karaoke screens. Nothing in it is
anyone else's music.

    python3 -I make_music.py OUT_DIR

Needs numpy, ImageMagick (convert) and ffmpeg."""
import json, os, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
COLOURS = {
    "golden-hour": ("#ff9a3c", "#7a1f5c"), "island-frequency": ("#00c2a8", "#ffd23f"),
    "night-market": ("#1b1b3a", "#e63946"), "coral-lights": ("#ff6f91", "#2a9d8f"),
    "paper-moons": ("#c3b1e1", "#2b2d42"), "overdrive-heart": ("#ff2a6d", "#05d9e8"),
}
LYRICS = {
    "paper-moons": (112, 8, [
        "We cut the sky from paper", "and hung it on the wall", "you said the moon was crooked", "I said it's best of all",
        "Paper moons, paper moons", "shining just for two", "the whole wide night is folded", "in a little room with you",
        "Turn the music louder", "the neighbours sing along", "a paper moon is all we need", "to carry us till dawn",
        "Paper moons, paper moons", "shining just for two", "the whole wide night is folded", "in a little room with you"]),
    "island-frequency": (92, 8, [
        "Tune the radio to the ocean", "let the morning drift in slow", "every wave is on the rhythm", "every palm tree on the go",
        "Island frequency", "calling over the sea", "turn it up and let it play", "the sun is singing back to me",
        "Bring your people to the shoreline", "bring the drum and bring the bass", "we got one more song to dance to", "and a smile on every face",
        "Island frequency", "calling over the sea", "turn it up and let it play", "the sun is singing back to me"]),
}

def main(out):
    os.makedirs(out, exist_ok=True)
    specs = json.load(open(os.path.join(HERE, "tracks.json")))
    with tempfile.TemporaryDirectory() as wav:
        subprocess.run([sys.executable, "-I", os.path.join(HERE, "gen_tracks.py"), os.path.join(HERE, "tracks.json"), wav], check=True)
        for s in specs:
            a, b = COLOURS[s["file"]]
            cover = os.path.join(wav, s["file"] + ".jpg")
            subprocess.run(["convert", "-size", "600x600", f"gradient:{a}-{b}", "-swirl", "60",
                "(", "-size", "600x600", "xc:none", "-fill", "rgba(255,255,255,0.18)", "-draw", "circle 300,300 300,120", ")", "-composite",
                "-font", "DejaVu-Sans-Bold", "-fill", "white", "-pointsize", "46", "-gravity", "south", "-annotate", "+0+90", s["title"],
                "-font", "DejaVu-Sans", "-pointsize", "26", "-annotate", "+0+50", s["artist"], cover], check=True)
            subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", os.path.join(wav, s["file"] + ".wav"), "-i", cover,
                "-map", "0:a", "-map", "1:v", "-c:a", "libmp3lame", "-b:a", "256k", "-c:v", "mjpeg", "-id3v2_version", "3",
                "-disposition:v", "attached_pic", "-metadata", f"title={s['title']}", "-metadata", f"artist={s['artist']}",
                "-metadata", f"genre={s['genre']}", "-metadata", "album=djmanzo demo", "-metadata", f"TBPM={s['bpm']}",
                os.path.join(out, s["file"] + ".mp3")], check=True)
    for name, (bpm, start_bar, lines) in LYRICS.items():
        bar = 4 * 60 / bpm
        t = start_bar * bar
        rows = [f"[ti:{name.replace('-', ' ').title()}]", "[ar:demo]"]
        for i, line in enumerate(lines):
            rows.append("[%02d:%05.2f]%s" % (int(t // 60), t % 60, line))
            t += 2 * bar
            if i % 4 == 3:
                t += 2 * bar
        open(os.path.join(out, name + ".lrc"), "w").write("\n".join(rows) + "\n")

if __name__ == "__main__":
    main(sys.argv[1])
