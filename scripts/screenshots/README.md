# Making the screenshots

The pictures in `docs/screenshots/` are made by these scripts from the real
application under Xvfb. Run them in this order, on Linux, with Xvfb, xdotool,
ImageMagick, ffmpeg, Python 3 with numpy and Pillow, and djmanzo built with
the interface bundled:

```sh
npm --prefix ui run build                                  # confirm "✓ built"
cargo build --bin djmanzo --features dj-app/custom-protocol
Xvfb :95 -screen 0 1920x1080x24 &
export WORK=/tmp/djmanzo-shots DISPLAY=:95

python3 -I scripts/screenshots/make_music.py $WORK/music   # the demo collection
python3 -I scripts/screenshots/prime.py                    # a home with it in the library
scripts/screenshots/shots.sh                               # every picture: about an hour
python3 -I scripts/screenshots/finish.py                   # clean, file, index
```

| File | What it does |
|---|---|
| `gen_tracks.py`, `tracks.json` | Six original tracks, synthesised: kick, hats, bass, chords, an arrangement with breakdowns and drops |
| `make_music.py` | Those tracks as tagged MP3s with cover art, and words in time for two of them (karaoke) |
| `prime.py` | A home whose library holds the collection, analysed, with a few playlists, the welcome answered |
| `shoot.py` | One picture per launch, in an activity's arrangement, theme and skin |
| `shots.sh` | The list of pictures |
| `setls.py` | Writes the webview's local storage, where the theme and skin are kept |
| `clean.py` | Removes the "UI n fps" warning (see `docs/screenshots/README.md`) |
| `finish.py`, `index-head.md` | Files the pictures in `docs/screenshots/` and writes its index |
| `showcase.py` | Grids for a shop page (themes, skins, screens) and one large picture, with shadows on a gradient |

## Why it works the way it does

- **One launch per picture.** §31 adapts a theme that is not locked, every
  twenty seconds, and choosing an activity unlocks it. So each picture starts
  djmanzo with the activity's arrangement already stored and the theme locked.
- **Long waits.** Under Xvfb the interface draws at a few frames a second, and
  typing into the palette lags; a shorter wait sends Enter before the palette
  has an answer, and the keys after it land in the palette.
- **The welcome is answered in the home**, so it never covers a screen.
