"""Take screenshots of djmanzo, running for real under Xvfb.

    python3 -I shoot.py THEME_ID SKIN|- SLUG SHOT...

Each SHOT is `name=activity[;step;step...]`. **Every shot is its own
launch**, straight into that activity's arrangement (from
`ui/e2e/activities.json`, which Rust writes) with the theme locked: §31
adapts any theme that is not locked, and choosing an activity unlocks it, so a
picture taken after switching activities can catch the theme changing back.

Steps, after the launch: a key (xdotool's names: `0`, `ctrl+k`, `Escape`),
`pal:words` (run a palette entry), `type:words` (type into the palette and
leave it open), `click:X:Y`, `sleep:SECONDS`. The picture is
`$WORK/shots/NAME--SLUG.png`.

Every session starts from `$WORK/home-pristine` (prime.py). Environment: WORK
(default /tmp/djmanzo-shots), DISPLAY (default :95), W and H (window size,
default 1600x900), DJMANZO (the binary, built with
`--features dj-app/custom-protocol`)."""
import json, os, shutil, subprocess, sys, time

HERE = os.path.dirname(os.path.abspath(__file__))
WORK = os.environ.get("WORK", "/tmp/djmanzo-shots")
W, H = os.environ.get("W", "1600"), os.environ.get("H", "900")
BINARY = os.environ.get("DJMANZO", os.path.join(HERE, "../../target/debug/djmanzo"))
MUSIC = "Music/DJ Collection/Demo Night - House, Disco, Reggae and Synthwave"
ACTIVITIES = {a["slug"]: a["workspace"] for a in json.load(open(os.path.join(HERE, "../../ui/e2e/activities.json")))["activities"]}
env = dict(os.environ, DISPLAY=os.environ.get("DISPLAY", ":95"))

def x(*args):
    subprocess.run(["xdotool", *args], env=env, check=False)

def key(keys):
    x("key", "--delay", "80", *keys.split(","))

def shot(name):
    os.makedirs(f"{WORK}/shots", exist_ok=True)
    subprocess.run(["import", "-window", "root", "-crop", f"{W}x{H}+0+0", "+repage", f"{WORK}/shots/{name}.png"], env=env, check=True)

def palette(words, keep_open=False):
    # Typing is slow where the webview draws without a GPU, so every wait is long.
    x("mousemove", "800", "560", "click", "1")
    key("ctrl+k")
    time.sleep(1.5)
    x("type", "--delay", "90", words)
    time.sleep(7)
    if not keep_open:
        key("Return")
        time.sleep(5)

def launch(theme, skin, activity):
    subprocess.run(["pkill", "-x", "djmanzo"], check=False)
    time.sleep(1)
    home = f"{WORK}/home"
    shutil.rmtree(home, ignore_errors=True)
    shutil.copytree(f"{WORK}/home-pristine", home, symlinks=True)
    workspace = dict(ACTIVITIES[activity], locked=["theme"])
    with open(f"{home}/.config/app.djmanzo.desktop/workspace.json", "w") as f:
        json.dump(workspace, f)
    store = f"{home}/.local/share/app.djmanzo.desktop/localstorage/tauri_localhost_0.localstorage"
    subprocess.run([sys.executable, "-I", f"{HERE}/setls.py", store, f"djmanzo.themePackage={theme}",
                    f"djmanzo.skin={'' if skin == '-' else skin}", "djmanzo.theme=dark"], check=True)
    run_env = dict(env, HOME=home, DJMANZO_NULL_AUDIO="1", DJMANZO_DEMO=f"{home}/{MUSIC}", WEBKIT_DISABLE_COMPOSITING_MODE="1")
    subprocess.Popen([BINARY], env=run_env, stdout=open(f"{WORK}/app.log", "w"), stderr=subprocess.STDOUT)
    for _ in range(40):
        if subprocess.run(["xdotool", "search", "--name", "djmanzo"], env=env, capture_output=True).returncode == 0:
            break
        time.sleep(1)
    time.sleep(2)
    for window in subprocess.run(["xdotool", "search", "--name", "djmanzo"], env=env, capture_output=True, text=True).stdout.split():
        x("windowmove", window, "0", "0", "windowsize", window, W, H)
    # The demo loads and plays two records once the interface asks for them.
    time.sleep(24)
    x("mousemove", "800", "560", "click", "1")
    time.sleep(2)

def main(theme, skin, slug, shots):
    for spec in shots:
        name, rest = spec.split("=", 1)
        activity, *steps = rest.split(";")
        launch(theme, skin, activity)
        for step in steps:
            if step.startswith("pal:"):
                palette(step[4:])
            elif step.startswith("type:"):
                palette(step[5:], keep_open=True)
            elif step.startswith("sleep:"):
                time.sleep(float(step[6:]))
            elif step.startswith("click:"):
                px, py = step[6:].split(":")
                x("mousemove", px, py, "click", "1")
                time.sleep(2)
            elif step:
                key(step)
                time.sleep(3)
        # Out of the way, so no tooltip is in the picture.
        x("mousemove", str(int(W) - 2), str(int(H) - 2))
        time.sleep(3)
        shot(f"{name}--{slug}")
        print("shot", name, slug, flush=True)

if __name__ == "__main__":
    main(*sys.argv[1:4], sys.argv[4:])
