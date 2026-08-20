# Five minutes with djmanzo

For somebody sitting down in front of it for the first time, with a laptop and
no controller.

## Before you start

The beta builds are **unsigned and un-notarised**, because signing needs an
Apple Developer ID and that is an account question rather than a code one.

- **macOS** will refuse to open the app on first launch. Right-click it in
  Applications and choose **Open**, then confirm. Once. After that it opens
  normally. (If you prefer the terminal:
  `xattr -dr com.apple.quarantine /Applications/djmanzo.app`.)
- **Xubuntu / Debian**: `sudo apt install ./djmanzo_0.1.0_amd64.deb`. The
  package pulls in what it needs.

## It should already be making sound

djmanzo opens your default output on launch and remembers whichever sound card
you pick after that. The top bar names the device, the sample rate and the
buffer latency. If your interface was not plugged in when you started, the
interface says so rather than quietly using the laptop speakers.

## Get some music in

Open **Browse**. On a first run it offers to scan your Music folder in one
click. Otherwise **Add folder…** and point it anywhere.

Tracks appear as they are identified, and analysis — BPM, key, beat grid,
loudness — runs in the background. A track is playable before it has finished
analysing; sync and quantize start working once it has.

Load a track with the **1** / **2** buttons on its row, or the **Load** button
on a deck.

## Play it from the keyboard

Press **Keys** for the full sheet. The short version, with the two hands
mirroring each other:

| | Deck 1 | Deck 2 |
|---|---|---|
| Play / pause | `A` (or `Space`) | `J` (or `⇧Space`) |
| Cue | `S` | `K` |
| Sync | `D` | `L` |
| Hot cues 1–4 | `1` `2` `3` `4` | `7` `8` `9` `0` |
| Set a hot cue | `⇧1`… | `⇧7`… |
| Loop 4 beats / off | `E` / `R` | `O` / `P` |
| Bass / mid / treble kill | `Z` `X` `C` | `M` `,` `.` |
| Censor | `F` | `;` |
| Loop roll | `G` | `H` |
| Brake | `V` | `/` |

Crossfader cuts are on the arrow keys: `←` full deck 1, `→` full deck 2, `↓`
centre.

Everything marked **(hold)** on the sheet happens while the key is down and
undoes itself when you let go. Try holding `Z` on a playing track — that is the
bass kill, and it is the first half of most transitions you have ever heard.

The keyboard steps aside while you are typing in the search box, and there is an
off switch on the Keys panel.

## A first mix, in about ninety seconds

1. Load a track on deck 1, press `A`, bring the crossfader to the left with `←`.
2. Load a second track on deck 2.
3. Press `L` — deck 2 syncs to deck 1's tempo and phrase.
4. Press `K` to cue it, `J` to start it.
5. Hold `M` to kill deck 2's bass, then drag the crossfader across, then let go
   of `M` as deck 1's bass comes out with `Z`.

That is a bass swap, and it is the move most of a set is built from.

## If you have a controller

Plug it in before starting. **Settings** lists the MIDI inputs it can see and
the mappings available. A generic two-deck mapping is bundled as a starting
point; it is meant to be copied and edited, because the note and control
numbers are in your controller's manual and nowhere else.

Mappings are TOML files in `mappings/` inside the config directory
(`~/Library/Application Support/djmanzo/` on macOS, `~/.config/djmanzo/` on
Linux). See [CONTROLLERS.md](CONTROLLERS.md).

## Things to know about this beta

- **The interface warns if it is running slowly.** On a machine without
  hardware-accelerated compositing the waveform will not scroll smoothly. The
  audio engine is unaffected — the warning exists because a webview that falls
  back to software rendering says nothing and the app just looks broken.
- **The headphone cue** can run on a second sound card; two cards means two
  clocks and a resampler between them, so it is worth staying on one device
  when that device has the channels.
- **Recording** (the REC button) writes a 16-bit WAV beside your settings. If
  the disk cannot keep up it says how many samples were lost rather than
  stalling the audio thread.
- Nothing here has been through a real gig yet. That is what a beta is.
