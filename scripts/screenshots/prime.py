"""Make $WORK/home-pristine: a home whose library holds the demo collection,
analysed, with a few playlists a DJ might have. Every screenshot session
starts from a copy of it.

    python3 -I prime.py            # after make_music.py has filled $WORK/music"""
import os, shutil, sqlite3, subprocess, sys, time

HERE = os.path.dirname(os.path.abspath(__file__))
WORK = os.environ.get("WORK", "/tmp/djmanzo-shots")
BINARY = os.environ.get("DJMANZO", os.path.join(HERE, "../../target/debug/djmanzo"))
MUSIC = "Music/DJ Collection/Demo Night - House, Disco, Reggae and Synthwave"
env = dict(os.environ, DISPLAY=os.environ.get("DISPLAY", ":95"))

home = f"{WORK}/home"
shutil.rmtree(home, ignore_errors=True)
shutil.copytree(f"{WORK}/music", f"{home}/{MUSIC}")
db = f"{home}/.config/app.djmanzo.desktop/library.db"
run_env = dict(env, HOME=home, DJMANZO_NULL_AUDIO="1", DJMANZO_DEMO=f"{home}/{MUSIC}", WEBKIT_DISABLE_COMPOSITING_MODE="1")

def pending():
    try:
        with sqlite3.connect(db) as c:
            return c.execute("select count(*) from pending_files").fetchone()[0] + (6 - c.execute("select count(*) from tracks").fetchone()[0])
    except sqlite3.Error:
        return 99

# Identifying a record decodes and analyses it, a few seconds each; the demo
# stops waiting after thirty, so run until every record is in.
for attempt in range(6):
    app = subprocess.Popen([BINARY], env=run_env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    for _ in range(30):
        time.sleep(10)
        if pending() == 0:
            break
    time.sleep(20)
    app.terminate()
    app.wait()
    if pending() == 0:
        break
else:
    sys.exit("the demo collection was never fully identified")

with sqlite3.connect(db) as c:
    ids = {title: id for id, title in c.execute("select id, title from tracks")}
    c.execute("delete from playlists")
    now = int(time.time())
    def playlist(name, kind, parent=None, query=None, tracks=()):
        row = c.execute("insert into playlists(name, parent_id, kind, query, created_at) values (?,?,?,?,?)", (name, parent, kind, query, now))
        for n, title in enumerate(tracks):
            c.execute("insert into playlist_tracks values (?,?,?)", (row.lastrowid, ids[title], n))
        return row.lastrowid
    gigs = playlist("Gigs", "folder")
    playlist("Saturday at the Beach Bar", "list", gigs, tracks=["Island Frequency", "Coral Lights", "Golden Hour", "Paper Moons"])
    playlist("Rooftop Sunset", "list", gigs, tracks=["Golden Hour", "Coral Lights", "Night Market"])
    playlist("Karaoke Favourites", "list", tracks=["Paper Moons", "Island Frequency"])
    playlist("Peak Time", "smart", query="bpm > 120")

# The welcome is answered, so it does not cover the screens.
with open(f"{home}/.config/app.djmanzo.desktop/welcome.json", "w") as f:
    f.write('{"name":"","nights":[],"genres":[],"bpm_low":0,"bpm_high":0,"favourites":[],"moves":[],"learn":[],"level":"","theme":"","done":true}\n')

shutil.rmtree(f"{WORK}/home-pristine", ignore_errors=True)
shutil.copytree(home, f"{WORK}/home-pristine", symlinks=True)
print("primed", f"{WORK}/home-pristine")
