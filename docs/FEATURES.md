# Features

Two halves: **parity** — everything VirtualDJ does that djmanzo must do — and **beyond** —
the reasons to use djmanzo instead.

Every parity row says where it stands, checked against the code at v0.46.0 (17 October
2026): ✅ built, 🟡 part-built (with what is missing), ⬜ not built. *Untested on hardware*
marks what works against stand-ins and has never met the device. Milestones and their
acceptance criteria are in [ROADMAP.md](ROADMAP.md); what is planned beyond these rows is
§4, [the outlook](#4-outlook).

**At a glance:** of the 187 parity rows, 169 are built, 12 part-built and 6 not built
(waveform zoom, lyrics on the waveform, per-stem effects, Pro DJ Link, StagelinQ, Icecast
broadcast). Beyond the matrix, §2 lists what djmanzo does that VirtualDJ does not, and
[§2's last part](#also-shipped-and-not-in-the-matrix) the fifty-odd things built since this
file was first written.

---

## 1. Parity matrix

### Decks and transport

| Feature | Status |
|---|---|
| 2 decks | ✅ |
| 4 decks | ✅ |
| 6 decks | ✅ |
| Play / pause / cue (CDJ-style cue behaviour) | ✅ |
| Pitch fader, configurable range (±6/8/10/16/25/50/100 %) | 🟡 Fader fixed at ±16% (Deck.svelte zonePitch); no range selector |
| Pitch bend (temporary nudge) | ✅ |
| Keylock / master tempo | ✅ |
| Jog wheel: scratch, bend, search modes | ✅ |
| Vinyl mode vs CDJ mode | ✅ |
| Sync (tempo + phase) | ✅ |
| Quantize to beat / bar | ✅ |
| Beat jump | ✅ |
| Track load, unload, clone deck, instant doubles | 🟡 Load/eject done; no clone-deck or instant doubles (SideView "Clone" only lists decks) |
| Slip mode | ✅ |
| Reverse / censor | ✅ |
| Elapsed / remaining time, end-of-track warning | ✅ |
| **Key shift** — transpose in semitones for harmonic mixing, independent of tempo | ✅ |
| **Sandbox** — audition a mix in headphones while the master keeps playing | 🟡 Headphone-only practice deck (practice_live.rs) and rendered rehearsal (practice.rs); no live two-deck mix audition |

### Mixer

| Feature | Status |
|---|---|
| Channel faders, crossfader with selectable curve | 🟡 Faders/crossfader done; `CrossfaderCurve` exists but no action or UI selects it |
| Per-channel crossfader assignment (A / thru / B) | ✅ |
| 3-band EQ per channel, kill switches | ✅ |
| Per-channel filter (low/high-pass sweep) | ✅ |
| Gain + auto-gain from EBU R128 analysis | ✅ |
| VU meters per channel and master | ✅ |
| Headphone cue (PFL) with cue/master blend and split-cue | ✅ |
| Booth output with independent level | ✅ |
| Master limiter | ✅ |
| Microphone / aux input with ducking | ✅ done |
| Microphone effects (reverb, echo, pitch) | ✅ |

### Waveforms and displays

| Feature | Status |
|---|---|
| Scrolling waveform, per deck | ✅ |
| Stacked / parallel multi-deck waveform view | ✅ |
| Overview waveform with position and cue markers | ✅ |
| Beat grid overlay, editable (shift, scale, tap) | ✅ |
| Saved loops, recalled per track | ✅ |
| Cues, grids and loops kept with the track across sessions | ✅ |
| Playlists, crates and folders in one tree | ✅ |
| Play history, recorded when a track is actually played | ✅ |
| Smart folders with a filter language, incl. harmonic matching | ✅ |
| Import rekordbox XML, Traktor NML and iTunes XML with cues and grids | ✅ |
| Import Serato crates and database (clean-room) | ✅ |
| Read Serato hot cues and loops out of the audio files | ✅ |
| Duplicate detection across copies of the same audio | ✅ |
| Batch tag editing, colour coding and ratings | ✅ |
| Session export as a set list | ✅ |
| Spectral colouring (bass/mid/high energy) | ✅ |
| Loop region and hot cue overlays | ✅ |
| Phrase / structure markers | ✅ |
| Waveform zoom, per-deck and global | ⬜ Lane fixed at 256 frames/px; no zoom control (Pair auto-zooms only) |
| Lyrics on the waveform | ⬜ Timed lyrics exist for karaoke; no waveform lyrics layer |

### Cues, loops, pads

| Feature | Status |
|---|---|
| Hot cues (8+, named and colour-coded) | 🟡 8 cues work; label/colour fields stored but never set or shown |
| Manual loop in / out, loop adjust | ✅ |
| Auto-loop by beat count | ✅ |
| Loop roll (momentary, always slipping, down to 1/16 beat) | ✅ |
| Loop move / halve / double | ✅ |
| Saved loops | ✅ |
| Slicer | ✅ |
| Pad pages: Cues · Loops · Roll · Saved · Sampler · FX | ✅ |
| Pad pages: Slicer · Stems | ✅ Slicer page M5 — **done**; Stems page M6 — started |
| Pad colour and LED feedback to hardware | ✅ *Untested on hardware.* |

### Effects

| Feature | Status |
|---|---|
| FX rack, three chained slots per deck and on the master | ✅ |
| Per-deck and master FX routing | ✅ |
| Pre-fader / post-fader chain placement | ✅ |
| Beat-synced FX timing (1/16 to 4 beats, following the pitch fader) | ✅ |
| Core effect set: echo, delay, reverb, gate, crush, flanger, phaser, filter | ✅ |
| Roll (as a loop roll, on its own pad page rather than in a slot) | ✅ |
| Brake and backspin — transport, not signal: they live on the deck | ✅ |
| CLAP plugin hosting (master insert; generic controls, no plugin window) | ✅ done |
| **Per-stem EQ and filter** | ✅ three bands and a sweep per stem, composed with the deck's own EQ rather than replacing it |
| Per-stem effects | ⬜ No effect slot per stem; rack is per deck/master only |
| Effect chain presets | ✅ |

### Sampler

| Feature | Status |
|---|---|
| Sampler banks with pad grid | ✅ |
| Trigger modes: one-shot, loop, hold, stutter | ✅ |
| Sync samples to master tempo | ✅ |
| Sample volume / output routing (mix or headphones) | ✅ |
| Record from deck or master into a sample slot | ✅ |

### Browser and library

| Feature | Status |
|---|---|
| Folder tree + song list, VirtualDJ layout | ✅ |
| **Prepare**, its own dockable surface: Sidelist · Next · Clone · Sampler · Automix | ✅ (Karaoke K1) |
| One gesture sets a track aside — `→` on a browser row, and it is in Prepare | ✅ Cockpit §21 |
| **Next**, a rail of up to eight candidates for what follows the playing deck, each on one line of deltas with a confidence bar | ✅ Cockpit §22 |
| **Set plan**, its own surface: the night as a sequence, with the seam between each pair of records and a count of the joins that need a cut | ✅ Cockpit §20 |
| **Pair**, its own surface: two records side by side and the seam between them — the deltas, the key relation, where the mix starts and ends, and the reasons | ✅ Cockpit §20 |
| **The transition is an object**: djmanzo holds the mix you set up, you move it, shorten it or change its style, and the reasons are re-derived over what you changed | ✅ Cockpit §68 |
| **Command palette** on `Ctrl/Cmd + K` — every verb the parser accepts, every surface, and anything you type that parses | ✅ Cockpit §51 |
| Search across the whole library, instant | ✅ |
| Playlists / crates / smart folders | ✅ |
| Columns: BPM, key, energy, rating, play count, last played, comment | 🟡 All present in crates/dj-app/src/columns.rs except a comment column |
| Sort, filter, colour-coding | ✅ |
| Track info editor, batch tag editing | 🟡 Batch bar sets genre/colour/rating only; no per-track info editor |
| Import: rekordbox, Serato, Traktor, iTunes, folders | ✅ |
| Duplicate detection | ✅ |
| Play history and session export | ✅ |
| Harmonic (Camelot) key display and compatible-key filtering | ✅ |
| Automix with configurable transition style | ✅ done |
| Search across online sources (Spotify, YouTube, Jamendo, Internet Archive) | ✅ |
| Match an online result to a file you already own | ✅ |
| Licensed streaming catalogue (Beatsource/Beatport/TIDAL/SoundCloud) | 🟡 Provider slots in partner.rs; no catalogue access without partnership |

### Hardware

| Feature | Status |
|---|---|
| Class-compliant MIDI controller support | ✅ 7-bit, 14-bit and all three encoder conventions *Untested on hardware.* |
| Data-file mappings (no recompile) | ✅ TOML, bundled and user files, checked when the file loads |
| Mapping editor (learn a control, bind an action) | ✅ press the control, pick the action, save; the file is proved to reload before it is written |
| LED / display feedback to controllers | 🟡 LEDs/pad colours done (feedback.rs); segment displays not built |
| **Stem swapping across decks** — one deck's vocal over another's mix | ✅ `stem_swap vocal 1 2`, latching and undoable to what the DJ had |
| **Lua scripting in mappings** — a shift key, a mode-dependent jog, one knob doing two things | ✅ sandboxed: no filesystem, no process, every action through the parser, stopped after 100k instructions |
| HID controller support | ✅ 8- and 16-bit fields, both byte orders, level-to-edge conversion, and a learn mode that diffs two reports to name the control that moved *Untested on hardware.* |
| **Motorized platter support** — high-res absolute position, motor ramp, torque | ✅ absolute angle with wrap handling, motor driven by the transport *Untested on hardware.* |
| Controller-specific audio setup presets | ✅ an `[audio]` block per mapping; a master that overlaps the cue is refused when the file loads |
| Controllers panel — what is connected, on which mapping, with which outputs | ✅ |
| **Keyboard as a controller** — same vocabulary and file format as a MIDI mapping, with a live sheet | ✅ |
| Multi-device audio setup (4-channel, or two devices with drift correction) | ✅ |
| **MIDI clock out** — djmanzo as clock master for a drum machine or light desk | ✅ 24 PPQN at the room's tempo, on its own thread |
| **MIDI clock in** — follow a drum machine or a second DJ | ✅ an external clock outranks every deck as the sync leader |
| **OSC** — TouchOSC, Lemur, QLab | ✅ the action grammar is the address space; loopback only, because UDP cannot carry a passphrase |
| Network control API — drive djmanzo from a script, a Stream Deck or a lighting desk | ✅ line-delimited JSON over TCP, off by default, loopback by default, passphrase required off-machine |
| **Pro DJ Link** (Pioneer CDJ/XDJ) | ⬜ No protocol code anywhere in crates/ |
| **StagelinQ** (Denon Prime) | ⬜ No protocol code anywhere in crates/ |
| Network tempo sync (Ableton Link or equivalent) | 🟡 djmanzo-to-djmanzo sync only (peersync.rs, dj_net::peer); no Link interop |
| DVS timecode vinyl | ✅ |

### Output, recording, broadcast

| Feature | Status |
|---|---|
| Record the master to disk (WAV; FLAC/MP3 later) | ✅ |
| **Network tempo sync between djmanzo instances** | ✅ announce and follow over UDP, no master, bounded corrections. Not Ableton Link; see ROADMAP |
| Broadcast to Icecast/Shoutcast | ⬜ No Icecast/Shoutcast code in crates/ |
| **Now playing on a live stream** | ✅ (§108) — for a set streamed from OBS to Twitch, YouTube, Instagram, TikTok or Kick: a transparent overlay page for a Browser source, served on this machine only (`http://127.0.0.1:7332/`), and a `now-playing.txt` for a Text source set to *Read from file*. It names the record the room hears most — channel fader times the crossfader's side — and changes hands once per blend, not back and forth. Off until switched on in Settings → *Going live* |
| **Per-stem outputs for external processing** | ✅ one deck as four stereo pairs (vocals 1–2, drums 3–4, bass 5–6, other 7–8), pre-EQ and pre-fader, on an interface with eight outputs |
| **Per-deck outputs for external processing** | ✅ each deck on its own stereo pair, pre-fader, no master chain; exclusive with stem out *Untested on hardware.* |
| **Phrase detection** | ✅ phrase length and phrase anchor, from beat-synchronous novelty in four bands; markers on the waveform, `phrasejump` and `loop_phrase` on the keyboard and the deck panel, persisted across restarts. Verified against synthetic tracks whose structure is arithmetic; **not measured against a corpus of real records** |
| **Next-track suggestions** | ✅ harmonic, tempo, loudness and phrase, ranked with typed reasons shown as chips; lift/hold/ease. Deterministic and local. **Energy is approximated by loudness**, which is a proxy and not the same thing |
| **Transition planner** | ✅ where to start (phrase boundary with a tail margin), how long, and which style, with typed reasons. Proposes; does not act. **Cannot know where the outro is** — a phrase boundary near the end is a structural guess |
| **Set files and take diffing** | ✅ a set saved as readable, diffable text; two takes compared by move and drift |
| **Deterministic replay / re-render** | ✅ a set file rendered back to audio, faster than real time, byte-identical run to run. Live inputs (mic, aux, DVS) cannot be reproduced and are stated as such |
| **Genre families** | ✅ 31 families across Latin America, North America, Europe, Africa and beyond, encoding felt-vs-written tempo and rhythmic grammar. A working DJ's map, not a musicology; wrong at the edges by design and written down so it can be corrected |
| **Automatic set assembly** | ✅ a whole set built from the library with an arc, taste as a tilt, no repeats, no two tracks by one artist in a row, and no blend across a rhythmic grammar |
| **Assistant takeover** | ✅ **shipped and wired** — per-control, instant, implicit on any human action; one gesture hands everything back; expires after ten minutes |
| **Assistant posture / occasion** | ✅ six postures, nine occasions, seven packs, in the Conduct panel with the next step shown before it happens |
| **Share a set** | ✅ a night's tracklist handed to WhatsApp, X, Bluesky or Threads with the message already written and cut to what the network takes (a post on X holds a handful of records and says how many more), previewed first, and a file for the sets too long for any of them. Nothing is posted and no recipient is chosen: djmanzo prepares the share, the person presses send (§108) |
| **DJ technique catalogue** | ✅ twenty-eight moves with what each needs, when it works and a metaphor to teach it by. Filtered by how the two records blend and by the rig actually present: a controller counts when its mapping is open, and an unanalysed track leaves beatmatching by ear |
| **Learning module** | ✅ names what the hand just did, catches one specific error at a time, and sets the easiest thing not yet tried. Reads the action log, so what the DJ did is known rather than inferred; makes no claim about whether it sounded good. Nothing is scored |
| **Every performing control on one screen** | ✅ at djmanzo's own default 1280×800 with two records loaded, the waveform, pad grid, deck channel strip, cue, crossfader assignment, crossfader, master gain, headphone cue, split and limiter are all visible without scrolling. The master strip and each deck's channel strip are pinned; the waveform's tail and the loop rows are what scroll on a short window. The crossfader had been below the fold three times, most recently by about 280 px. Enforced by `ui/e2e/budget.spec.ts` |
| **The interface fits the window it is given** | ✅ djmanzo picks a density band from the window height (`cockpit::BANDS`, derived from measured deck heights, not chosen), and an explicit density from a layout or workspace overrides it. The pad grid, the SVG faders and knobs and the waveform lane all answer to it; before this they were fixed pixels and the density control moved a deck by 68 px across its whole range |
| **Function tags** | ✅ what a record is *for*, which is not what it is: opener, builder, peak, floor reset, singalong, closer, transition tool, safe, risky, emergency. A closed vocabulary of ten fixed in Rust, because the value of the tag is that it means the same thing on every record — free text gives you `opener`, `Opener`, `open`, `warmup` and `warm-up` in the same collection within a month. Set on a selection in the browser; every function is offered even at zero, with a count. Migration 10. Smart folders can filter on them: `for is opener`, `function is peak`, `not for is risky and bpm > 120` |
| **Two panels at once** | ✅ surfaces dock beside the decks and along the bottom rather than taking turns in one slot, several open together, each in a titled frame that closes from its own header. Where one lands comes from its own preferred size, so the library runs under the decks and the assistant stands beside them. The arrangement is checked in Rust and survives a restart. This replaced a single variable that allowed exactly one panel — which is why the room and the library could never be looked at together |
| **The deck is drawn from a layout tree** | ✅ `Deck.svelte` renders a list of named widgets in the order the resolved tree gives, rather than holding the deck's shape in its own markup (ADR-0008 W3). A golden order in Rust asserts the tree still produces the deck djmanzo draws, so a control cannot move underneath a DJ as a side effect of a format change |
| **Panels that fit the window** | ✅ **fixed** — a panel whose content ran past the bottom of the window was cut off silently by `main`'s hidden overflow. At djmanzo's own default 1280×800 with the decks open that was the last rows of every library table and, in the Assistant, the entire chat input |
| **Session journal** | ✅ Mark in the top bar takes the moment and what is on the decks; the Journal beside History is where it gets written up. A note belongs to a moment rather than a track, carries what was playing as text so it outlives the record, and its time and context cannot be edited |
| **More like this** | ✅ similar records from any track in the browser, ranked with the reason on each row. The table becomes the answer, so sorting, set-aside and load-to-deck all still work |
| **Taste learned from play history** | ✅ which genre families you reach for *more than owning them would predict*, on a 180-day half-life. Added to a score and bounded to ±0.75 against a ±3 scale: it breaks ties, it cannot overrule the mixing, and it never learns an avoidance. Silent until about a night's worth of plays, and shown in the Conduct panel so you can disagree with it |
| **Set planning workspace** | ✅ pick an arc and a length, see the shape of the night as a strip, steer it up or down without rebuilding, drop or defer single records, then save it as a playlist or hand it to the assistant. The opener is protected from a steer, visibly |
| **Audience requests** | ✅ djmanzo serves the room its own page from a QR code or a printed sticker: people type what they want to hear, one entry per song no matter how it is spelled, with a tally of how many asked. English and Spanish, no script and no fetch, so it works on venue wifi with no route out. The DJ reads the list most-wanted first, hands a row to the search box, and a request the deck plays ticks itself off. The page can be closed without being taken away |
| **A printable way in** | ✅ `http://djmanzo.local:7331/` is the same at every venue, answered over multicast DNS, which is what makes a sticker printable before anybody knows the venue's router. The caveat is printed rather than hidden: iPhones resolve it, Android has since 12 and not on every build, some browsers never will — so tonight's plain address is offered beside it, and a sheet of twelve stickers can be printed for either |
| **Reading the room** | ✅ a camera and a microphone measure how bright the room is, how much of the picture changed, and how loud it is, every two seconds, and djmanzo says whether the floor is stiller or busier *than it has been tonight*. Every reading is judged against the same room earlier the same night, because two lenses pointed at one wall report different numbers and an absolute threshold means nothing. It never names a mood: a camera cannot tell dancing from leaving. The one interpretation offered is a disagreement — the floor is doing something other than the night you set up. **The measuring has never been run against a real camera**: there is none in the machine this was built on, so what is verified is the model, the wiring, and that a machine with no camera says so and says what to do about it |
| **Nothing leaves the window** | ✅ **by construction** — the frame is scaled to 64×48, averaged into three numbers and discarded. No image is recorded, saved or sent. The preview exists so you can aim the lens and can be switched off while the measuring carries on |
| **Find it from memory: the words** | ✅ search a line you half remember against lyrics fetched for your own collection. Folded on both sides, so "no puedo dormir" finds "Y no puedo dormir," with its capital, its comma and its accent. Words come from LRCLIB (free, no key); a record with none is remembered as asked, so a sweep never asks twice. Browser → **From memory**. **The call to LRCLIB was first made from here on 16 October**, when the egress allowed it: djmanzo's own client got timed lyrics back, and the answer's shape as served (fields beyond the documented ones, a space after each stamp) is now held by `dj_sources::lyrics` and `dj_library::lrc` tests; `the_live_service_answers_as_read` asks the real service when run with `--ignored` |
| **Find it from memory: a description** | ✅ "bachata with a piano hook, sounds like Aventura, heard it at a beach bar" goes to the assistant, which answers in rows rather than prose; anything that is not a row is dropped, so a model that ignores the format returns nothing instead of a paragraph dressed as a shortlist. Each guess shows its reason and whether you already own it. The only one of the three that can name a record you do not have |
| **Find it from memory: a hum** | ✅ **shipped — two searches off one recording, and neither identifies a record you do not own.** The hum is read for key and tempo through djmanzo's own analysis and used to narrow the collection, counting half and double time because people hum the vocal, not the kick; and it is compared *as a melody* against a stored pitch contour for each record. Recognising a recording you do not have needs a licensed fingerprint service with tens of millions of reference melodies; djmanzo has none, and the panel says so next to the button rather than in a help page |
| **Matching the tune itself** | ✅ ten pitch points a second, found with YIN because plain autocorrelation peaks at the octave below about as readily as at the fundamental, and an octave error is a semitone error of twelve. Three measurements decided the design. Centring each contour on its own median does *not* make the search key-blind — an eight-second hum's median and a five-minute record's are never the same number, and the phrase sat nine semitones from where the hum thought it was — so the match runs on the differences between consecutive points, where any constant offset cancels exactly. Three octave slips in one hum then flipped the ranking (right record 2.13, wrong one 1.42); clamping barely moved it (0.67 against 0.69) and folding the intervals into an octave took the right record to 0.000. And a nine-kilohertz tone read as fully voiced, because YIN is scale-invariant and attenuating a band it cannot hear does nothing — voicing now requires the energy to be in the melodic band. Searching is subsequence DTW, so the answer is *where in the record*, and the warping is itself what makes it tempo-independent. **What a contour holds is the strongest periodicity, not the melody**: the vocal much of the time, the bassline some of it — so a hummed vocal will not find a record whose loudest line is the bass. That is why it is a shortlist with key and tempo ranked beside it. One bug only came out of *running* it: the sweep handed the matcher an interleaved stereo buffer, which does not fail — it produces a contour twice as long as the record, so every reported timestamp was half the truth and the search still worked. Two channels of a five-minute track are ten minutes of numbers. A test on contour *length* now forbids it. **The hum itself has never been run for real**: there is no microphone in the machine this was built on, so the tests drive synthesised tones and the panel was driven once with a stubbed result. The sweep, by contrast, is verified end-to-end through the interface against real files — 240 contour points for a 24-second record, where the bug gave 480 |
| **Contours are read a sweep at a time** | ✅ twenty records a press from the panel, rather than a background pass competing with analysis for the same disk. A contour is a quarter-semitone to the byte, about 3 kB for a five-minute record. The panel shows how many records have one, because a shortlist drawn from a third of a collection should say so |
| **The first screen is for mixing** | ✅ **fixed** — measured at djmanzo's own default 1280×800: the crossfader sat about 1,500 px down, two screens below the waveforms it is used against, and the channel faders and EQ about 800 px down. Nothing a DJ touches to perform a mix was on the screen they start on. What was: three empty FX dropdowns per deck and two jog wheels duplicating the waveform's position. No shipped preset fixed it, including the one described as "everything you need and nothing else". The crossfader is now directly under the decks, the EQ, filter, volume and pitch are one side-by-side strip instead of a 530 px stack, the effect rack folds until something is in it, and the cue and crossfader assignment share a line. **Then it grew back.** Re-measured later at the same 1280×800, the deck column had reached 695 px and the crossfader's thumb was 117 px below the fold again — pitch was nominally in the strip but actually in a three-column grid of eight controls, which wrapped the strip onto three lines. Flattening that grid and moving slip, reverse, censor, brake and backspin up to the transport where they belong takes the deck to 539 px and the thumb to y 758, on screen at the size the application opens itself at. The lesson is the regression, not the number: a layout budget with no test drifts back — **and there is now one**, which measured a third instance on the day it landed and is still failing on purpose. See the two rows above |
| **A layout is a tree of named widgets** | 🟡 Deck.svelte renders the tree; App.svelte and W4 (network/assistant addressing) do not |
| **A layout cannot execute** | ✅ **enforced by shape, not by a list of forbidden spellings.** A layout is a thing one DJ sends another, so restyling is a closed set of 23 design tokens and each value is checked against the shape that token takes: a colour is a hash and 3, 4, 6 or 8 hex digits, a length is a number and one of `px`/`rem`/`em`, a scale is a bare number. Everything else falls off the end — `url()`, `@import`, a CSS escape spelling `url(`, a comment, a closing brace, a keyword like `red`, `var()` indirection. The tests name those cases, but they are not what makes it safe: a blacklist is a list somebody eventually gets past, and this is a whitelist of three shapes. Mutating the hex-digit check away is what proved the point — without it `#0;url(xy` is exactly eight characters and would have passed, and the first version of the test did not catch it. The audio-driven properties (`--audio-energy`, `--stem-color`) are deliberately not tokens: a layout that could pin them would be a layout lying about the mix |
| **A layout from a newer djmanzo still opens** | ✅ an unknown widget, an unknown slot and an unknown token are each skipped and *counted*, never fatal, so a DJ opening their laptop before a set gets an interface rather than a dialog. Skipping in silence would be worse than refusing, because the missing half looks like a bug in the application — so the top bar carries a chip saying how many parts were not shown, with the reasons in its tooltip. Verified by writing a layout file by hand with one unknown widget, one unknown slot and one unknown token: djmanzo drew the rest, applied the accent and radius the file *did* get right, and said "3 not shown" |
| **The first screen is measured, not remembered** | ✅ a browser test opens the interface at djmanzo's own 1280×800 and measures where the controls a DJ performs with actually land. Rendered geometry, in a real browser, because a template assertion passes while the crossfader is 900 px off the screen and jsdom does no layout at all. **The fixture it draws is captured from the running application** (`DJMANZO_SNAPSHOT_OUT`), not rebuilt from a parameter registry — that was tried and was wrong twice in ways that both left the test green against a screen no DJ will ever see: a fresh registry is all zeros, so every stem reads as muted and the interface unfolds a 359 px module nobody opened; and nothing is loaded, so the deck draws no pad grid and comes out ~200 px shorter than the real one. A Rust test checks the committed fixture still has the shape the current `Snapshot` type sends |
| **The crossfader is still below the fold, and the test says so** | ✅ |
| **The master strip was two rows and is now one** | ✅ **fixed** — 900×210 in two rows became 1240×110 in one, spending 355 px of horizontal room that was sitting unused beside it. That halves what the strip contributes to the overflow above, and it fixed a bug of its own: with a four-channel cue the SPLIT button was drawn at x 666–760 and the output meters at 740–860 over the same rows, so the meter bars ran *behind* the button. Nobody had seen it because this machine has no cue device and the branch never rendered; it is now caught by a test that opens the shell with the cue flag on. The reading order is what a DJ reaches for in order: what you mix with, how loud, what you hear, what is coming out, what is protecting it |
| **Occasion-aware density** | ✅ a control that cannot be undone by pressing it again becomes a 600 ms hold when the occasion says mistakes are costly, and stays a plain click when it does not. The pad itself holds; no second widget. Reversible controls are never made harder |
| **Autopilot** | ✅ a half-second tick that stages, levels and mixes according to posture, from an assembled setlist. Never verified against real audio: there is no output device here |
| **Timecode vinyl / DVS** | ✅ speed, direction and absolute position drive a deck, in relative or absolute mode, with an input picker and a live calibration reading in Settings. djmanzo writes its own control signal to a WAV, so any turntable, CD deck or phone works without a licensed record. **Not yet run against a pressed record** |
| Video mixing / VJ output | ✅ |
| Karaoke | 🟡 K1–K8 built; K9 duet parts and phone microphones remain |

### Interface

| Feature | Status |
|---|---|
| Layout presets: Starter · Essentials · Pro · Performance | ✅ |
| Skin system (CSS themes + JSON layouts) | ✅ |
| Multi-monitor / detachable panels | ✅ done |
| Configurable waveform and jog appearance | ✅ |
| Light and dark themes | ✅ |
| **A top bar that says what kind of thing each control is** | ✅ the row under the readouts carried twelve controls at one weight and three unlike kinds among them: seven panels you open, three controls over what the stage shows, and two acts on the night. It is three named groups now, named for a screen reader as well as for the eye, separated by space rather than a rule so the row still breaks at a group boundary when it wraps. Grouped, not hidden: every control is still one press away, in the same order. Two things fell out of doing it — the watershed was the last unlabelled square in a row whose whole purpose was that squares get names, and `class:on` on the REC button had never matched a rule anywhere, so a running recording looked exactly like a stopped one |
| **The night, read and shown** | ✅ `dj_core::ContextEngine` places the last few minutes in the whole night's own range of loudness and tempo, alongside whatever the DJ's occasion declares, and answers with a phase, a certainty and what produced it. It says nothing for the first six minutes and nothing at all on a night that has not varied, because a position inside a distribution that has not spread out is arithmetic rather than news. Where the two sources disagree the DJ wins, djmanzo says which way its own reading points, and the autopilot stops mixing unasked until it resolves. The **Night** surface shows all of it, including what it does not yet know |
| **The assistant asks before it acts** | ✅ a non-trivial move is a transaction: the whole next transition staged as one thing with Accept, Modify and Reject, in a strip that costs the decks no height until there is something to decide. Modify is a checkbox, because "load it and cue it, but I will bring it in myself" is the realistic change. A move your posture refuses is shown greyed and named rather than dropped, so the obvious fix is visible. Accepting runs the same code the automatic tick does, and partial success is reported as partial |
| **An override matrix, and three gates** | ✅ the directive's ten capabilities against six postures, verbatim. Before anything moves djmanzo asks three separate questions: has a hand claimed this control, does the posture permit this kind of thing, is the night sure enough to act on. Three rather than one because "it did nothing" deserves an answer. Configurable both ways, except that Off and Watch cannot be widened |
| **SAFE** | ✅ takes every control back, drops anything staged, clears every rack, flattens the tone, restores master gain and the limiter. It never stops a record, moves a fader or touches the crossfader: an emergency control that silences the floor is worse than the emergency. `safe` on the action bus, so it is on a controller pad and in a script too |
| **The assistant can ask for a panel** | ✅ `ui show prepare`, `ui pin room`, `ui focus 2`: a second closed vocabulary generated from the panels that exist, so one djmanzo does not have cannot be asked for. Never JavaScript, never the DOM. Your override matrix decides whether it may, and it is in the command palette for you too — pinning a panel had no gesture at all before this |
| **Grab the mix point** | ✅ the transition's start is a handle on the outgoing waveform. Drag it, or focus it and use the arrow keys. It moves in whole beats, because a mix point between two beats is not on the grid, and djmanzo re-derives what it now means: drag it off a phrase boundary and it stops claiming to land on one |
| **The mix you set up is the mix that happens** | ✅ hand over to the automix, or let the assistant run it, and it performs the transition djmanzo is holding: where you put it, over the length you chose, in the style you picked. Without one the handover is the end of the file minus the transition, which is wrong for any record with applause on the end. The panel says which of the two promises is in force, and dims the settings that are now about the *next* mix rather than this one |
| **The waveform says how long you have** | ✅ a wash over the last thirty seconds of a record, and the stretch a mix covers drawn as a region rather than two lines. Both are named layers in djmanzo's own inventory of the twenty §25 asks for: eleven exist, nine are named and not yet built, and a test refuses two of them sharing a colour meaning |
| **djmanzo remembers which records you put together** | ✅ *keep* on any of tonight's mixes saves that transition, directionally, with what the mix was; keeping it again says so more strongly. The Next rail then answers "why do I keep seeing these two together?" with the reason first on the line — strong enough to lift a candidate, capped so it cannot pin one to the top |
| **What you do, and when** | ✅ the gestures tonight's log contains, read through a rule that refuses to turn one surprising night into a preference: nothing generalises on fewer than four occurrences in one phase of the night, and every sentence names the phase it is about. "You sometimes sweep the filter when the night is building", never "you like filter sweeps" |
| **At hand** | ✅ the four to eight controls that matter on whichever deck your hands are on, changing with what you are doing: scratching, playing the stems, mixing against another record, or getting one ready. Each is an ordinary action, so a press is the same event as typing it or mapping a controller to it, and a latched control offers the other half rather than sitting beside it |
| **The collection as cards, with its own artwork** | ✅ sleeves read out of the files' tags and served as images on their own scheme, each card carrying its operations: deck, set aside, more like this, favourite, and why it was suggested. A record with no cover draws its key and tempo over a colour from the Camelot hour rather than a placeholder repeated down the grid |
| **The night knows its own mixes** | ✅ Tonight's mixes lists every handover in the set: what went into what, over how many beats, and whether it was a cut, a fade, a blend, an echo or a vocal drop. Derived from the action log rather than recorded beside it, so a set from before the feature existed has its mixes too, and one rule covers every way of doing it — the crossfader, the channel faders, or both |
| **One mix can be heard back on its own** | ✅ hear it again on any row of Tonight's mixes re-renders that handover to a WAV, with eight seconds of run-up and four of tail, from the set being played. Not a seek: the set is replayed up to the mix and discarded, because the engine's state at a moment is everything before it — so it takes a while on a long night, and the control says so |
| **The waveform says where the record can be left** | ✅ the window in which a mix out can begin, from the last beat where the longest transition djmanzo proposes still fits to the last beat where the shortest one does. Drawn over the whole record in the overview, where it can be seen from the moment the record loads, and to scale in the lane. Both edges come from the planner's own lengths and tail margin, so the band and its "rushed" warning cannot disagree |
| **The waveform says when the grid is a guess** | ✅ a hatch under a record whose beat grid the analyser is unsure of, over the record's own length. The rasteriser already draws the grid at the weight it deserves; at overview zoom it is suppressed for density, so faint and absent look the same, and this is the part that says which |

### Deliberately not planned

- **DRM'd content** of any kind.
- **Mixing Spotify or YouTube Music audio.** Not a scope decision — Spotify's policy forbids
  mixing their catalogue, and YouTube Music exposes no API that permits it at any tier. Both are
  integrated for search and planning instead. [SOURCES.md](SOURCES.md) has the detail.
- **Cloud library sync between machines.** VirtualDJ has it; it needs hosted infrastructure and
  an account system, which is a different kind of project. The session record is local and
  portable, so nothing precludes it later.

---

## 2. Beyond VirtualDJ


**Where §2 is not yet true** (the rest of it is built):

- *Stems*: no per-stem lanes on the waveform yet; the Stem Performance layout and the stem chips stand in.
- *The universal hardware bridge*: **Pro DJ Link and StagelinQ are not started**, so walking into a club with CDJs and being in phase is not yet possible; network tempo sync is djmanzo to djmanzo, not Ableton Link. Both are planned in [§4](#more-hardware).
- *Re-render*: rendering a whole set again offline exists as a command with no button; a single mix renders from the Mixes panel.
- *Structure*: phrases, builds, drops, breakdowns and the vocal's entry are found; intro and outro are not named. Suggestions rank energy by loudness, not yet by the energy analysis.
- *Linux*: audio goes through ALSA's default host, which PipeWire serves; a JACK host choice is not built. CI runs Ubuntu, not Xubuntu itself, and the WebKitGTK benchmark is measured, not gated.
- *Talking to it*: push-to-talk works; a wake phrase does not exist. Plans are built in the Plan panel, not parsed from a spoken sentence.
- *Extension surface*: the control API is OSC and line-delimited JSON over TCP; WebSocket is not built. Art-Net/DMX is not built ([§4](#more-hardware)).
Parity is the floor. These are the reasons to switch.

### Stem engine done right

VirtualDJ separates on the GPU in realtime. We separate **outward from the playhead** into a
persistent cache, which means the audio path adds exactly zero latency, the second load of a
track is instant, and the quality ceiling is set by the best available model rather than by
what fits in a 5 ms budget. On top of that:

- per-stem EQ and filter, not just per-stem volume — **shipped**, and composed with the deck's channel strip rather than fighting it;
- **stem-aware transitions** — drop the incoming vocal over the outgoing instrumental, planned
  and executed as one action rather than four hands;
- **stem swapping across decks** — take deck 1's vocal onto deck 2's instrumental as a
  first-class operation;
- stem isolation as a mixing surface, with the beat grid and phrase markers to align it;
- **the stems as four physical outputs** — one deck's parts on four stereo
  pairs, ahead of djmanzo's own EQ and fader, so an external mixer or a DAW
  gets the separation rather than the separation-plus-our-opinion-of-it.

The best separation model is a **download rather than part of the package** --
it is tens of megabytes and carries its own licence, so it is the DJ's to
accept. But an application whose headline feature only works after finding and
installing a 60 MB file does not have that feature on the night, so djmanzo
ships a **built-in separator** that needs no model, no runtime and no download:
harmonic/percussive separation over an FFT, split by band and by how centred a
sound is. Stems work out of the box; the panel names which separator is running
and says what a model would improve.

Separated audio reaches the deck without the audio thread ever taking a lock:
the worker publishes an immutable table of chunks and swaps it in atomically.
That is not incidental tidiness — the earlier lock-based handoff meant a muted
stem came back every time the worker appended, which is exactly the moment a
DJ would notice.

djmanzo also does not start ONNX Runtime speculatively to find out whether it
is there, because a missing runtime takes the process down at exit rather than
returning an error.

### A platter that behaves like a platter

A jog wheel is the control a DJ touches most, so djmanzo is precise about what
it does. One turn moves one turn of a record. A hand on the top in vinyl mode
stops the music and drives it; the same hand in CDJ mode only nudges the tempo.
The rim always bends. A paused deck searches, with sound, because that is how a
cue point is found by ear.

The two halves are deliberately different kinds of control: a scratch is a
*position* and is applied the instant it arrives, while a bend is a *speed* and
is estimated over time. That is why a scratch feels attached to the hand and a
bend does not jump — and why neither changes when you pick a different audio
buffer size.

### Motorised platters, without the revolution

A motorised platter — a Rane Twelve, a Denon SC6000M — reports its **angle**,
not its movement, and that angle wraps at zero. Treating it like an ordinary
jog wheel plays a whole revolution of audio backwards every time the record
goes round, which is the kind of bug that sounds like the software is broken
because it is.

djmanzo treats it as its own kind of control, with the number of steps in a
revolution declared in the mapping because every device counts differently. It
takes the short way round two readings, which is not a guess: at playing speed
a platter covers three thousandths of a turn between reports, so the long way
is physically impossible. And when a reading is too far to believe — a dropped
packet, a cable knocked — it reports nothing rather than lurching the record,
because the truth is that nobody knows how far it went.

### A controller you can map yourself

Every DJ application claims that mappings are files you can edit. djmanzo means
it in both directions: the files are plain TOML with the same action grammar
the interface and the assistant use, *and* you can make one without reading a
manual — press a control, choose what it does, save.

Two things make that safe rather than merely convenient. Learning suppresses
whatever the control already does, so mapping a play button does not start the
deck. And a binding is checked against the engine's vocabulary the moment it is
made, so a mistake is a sentence while you are still looking at the control
rather than a pad that quietly does nothing an hour into a set.

### The universal hardware bridge

No other application speaks Pro DJ Link *and* StagelinQ *and* network tempo sync *and* MIDI
clock. djmanzo does: walk into a club with CDJs already running, join the link network, and be
in phase — as a peer, not as a replacement. This is the feature that makes djmanzo usable in
rooms it does not control.

### Deterministic set replay and offline re-render

The action bus is an ordered, timestamped log, so a performance is fully described by data,
not just by its audio. That gives:

- **replay** — watch or hear a set exactly as it was played;
- **offline re-render** — regenerate the master at studio quality with no realtime deadline, so
  the recording is better than the live output;
- **practice loops** — isolate a 30-second transition and rehearse it against the same
  starting state, repeatedly;
- **diffing takes** — see what you did differently the second time.

This falls out of the architecture rather than being built on top of it. See
[ARCHITECTURE.md §1.1](ARCHITECTURE.md#11-one-action-bus-one-parameter-registry).

### Structure-aware assistance

Phrase and section detection (intro / build / drop / breakdown / outro) drives:

- phrase-locked looping and beat jumping — jump to the *next 16 bars*, not the next 16 beats;
- transition planning that suggests *where* as well as *what*;
- next-track suggestions ranked by harmonic compatibility, energy trajectory and phrase fit —
  and which **explain their reasoning** rather than presenting an opaque score.

### First-class Linux

PipeWire and JACK are supported properly, not as PulseAudio with extra steps. Xubuntu is a
tested target in CI, and waveform performance under WebKitGTK is a benchmarked requirement from
M1 rather than a late discovery. Linux DJs are currently served by exactly one application; we
intend to be the second one that actually works.

### A DJ you can talk to

Voice control with a wake phrase and a push-to-talk fallback, understanding
Spanish and English and the mix of the two people actually speak. Session
planning in the terms a DJ thinks in -- "half an hour of warm-up bachata, then
build through merengue into dembow" -- with live steering that adjusts the
remaining plan instead of discarding it. A domain pack for Dominican and
Caribbean repertoire that knows what a bachata-to-merengue transition really
costs. Songs generated on request for the people in the room. Full design in
[ASSISTANT.md](ASSISTANT.md).

The assistant holds no privileged access: it emits the same action text a
controller does, so everything it does is visible, reversible and replayable.

### An open extension surface

- **CLAP** effect hosting — a genuinely permissive plugin standard with a real ecosystem.
- **WebSocket + OSC control API** speaking the same Actions and Parameters the UI uses, so
  lighting rigs, OBS overlays, stage automation and phone remotes are all first-class clients.
- **Art-Net / DMX** output driven by beat and structure data. *Planned, not built.*
- **Community mappings** as data files — a new controller is a pull request, not a release.

### Also shipped, and not in the matrix

Built since this file was first written, and not named above. Release numbers are the
[CHANGELOG](../CHANGELOG.md)'s.

#### Video, visuals and looks
- **Video files on a deck**: a deck plays a video's sound as a record and its picture on the projector; H.264 on Linux (v0.38, v0.40). `crates/dj-app/src/video.rs`, `ui/src/Video.svelte`
- **MilkDrop visuals as a layer**, plus the DJ's own MilkDrop presets (v0.39). `crates/dj-app/src/milk.rs`, `ui/src/milkdrop.ts`, `MilkdropPresets.svelte`
- **Looks**: layered visuals behind the singers' words, and a look instead of black on the projector (v0.39). `crates/dj-app/src/looks.rs`, `ui/src/visualStack.ts`
- **The AI picks a look for each record** (v0.39). `crates/dj-assistant/src/looks.rs`, `ui/src/LookByAi.svelte`
- **Artist photos** behind the words and on the screen (v0.39). `crates/dj-app/src/artists.rs`, `ui/src/artistShow.ts`
- **Waveform in the colours of light, reacting to the EQ** (v0.20). `ui/src/eqLight.ts`, `crates/dj-render`
- **Rhythm line under the waveform** (v0.22) and **a blend's bass handover drawn on it** (v0.42). `ui/src/rhythm.ts`, `crates/dj-app/src/ghost.rs`

#### Streaming and going live
- **djmanzo drives OBS**: changes scene on the night's moments (v0.37). `crates/dj-app/src/obs.rs`, `crates/dj-net/src/obs.rs`, `ui/src/Obs.svelte`
- **Cameras for the stream**: the computer's webcam, and a phone as a camera from djmanzo's own page (v0.38). `crates/dj-app/src/phone_camera.rs`, `ui/src/Camera.svelte`, `PhoneCamera.svelte`
- **YouTube live chat read into the crowd log** (v0.38). `crates/dj-app/src/youtube.rs`, `ui/src/YouTube.svelte`
- **Chapters for a recorded mix** (v0.22). `crates/dj-app/src/chapters.rs`

#### Reading the room and the crowd (beyond the three basic readings)
- **Twelve more floor signals**, each on its own line with its blind spot: three sound bands, applause-like clicks, room over the music, optical-flow movement, occupancy, moving light, voices above the music (v0.46). `crates/dj-app/src/floor.rs`, `RoomSense.svelte`; tested only against drawn footage and generated sound
- **What the crowd said, on the music**, and where past crowds reacted on a record (v0.28, v0.32). `crates/dj-app/src/crowd.rs`, `ui/src/Crowd.svelte`
- **What the room did after a mix, across nights** (§37). `crates/dj-app/src/response.rs`
- **The assistant told how the room is doing** while it is being watched (v0.32). `crates/dj-app/src/sight.rs`

#### The assistant and what djmanzo learns
- **Choose your AI in Settings**: six providers, including a local model with no key (v0.26). `crates/dj-assistant/src/provider.rs`, `ui/src/AiSetup.svelte`, `crates/dj-secrets`
- **Spoken commands and karaoke by voice** (v0.42). `crates/dj-app/src/voice.rs`, `crates/dj-assistant/src/intent.rs`
- **Background assistant ("whisper line")** that watches the booth and proposes without interrupting; quiet in the warm-up (v0.22, v0.46). `crates/dj-app/src/whisper.rs`, `ui/src/Staged.svelte`
- **The assistant's guides**, one per topic, and a guide for a waiting request (v0.28, v0.42). `crates/dj-app/src/guide.rs`, `ui/src/Guides.svelte`
- **A decision when a record is running out** (v0.28). `crates/dj-app/src/decide.rs`, `ui/src/Decision.svelte`
- **The assistant's full briefing**: music, machine and DJ-behaviour context, attention budget, health (v0.46). `crates/dj-app/src/context.rs`
- **djmanzo learns your hands**: tempos per kind of night, key moves, loop size, mix length, effect pairs, pad habits, staged-move verdicts, across nights (v0.33, v0.46). `crates/dj-app/src/profile.rs`, `crates/dj-app/src/signals.rs`, `persona.rs`
- **Knowledge packs (21)** and each pack's tempo tolerance, set shape and preferred joins; the set plan offers the pack's shape (v0.37, v0.46). `crates/dj-assistant/src/pack.rs`
- **Learning a move / Rehearse it** in the style the automix performs (v0.42, v0.45). `crates/dj-app/src/practice.rs`, `ui/src/Coach.svelte`
- **The energy of the night** (v0.42). `crates/dj-app/src/night.rs`, `ui/src/Night.svelte`
- **Effects and stems come forward at the peak** (v0.46). `ui/src/peak.svelte.ts`

#### Practice and activities
- **Practice deck in the headphones** while the set plays (v0.46). `crates/dj-app/src/practice_live.rs`, `ui/src/PracticeLive.svelte`
- **Activity mode**: Prepare / Mix / Practice screens, switchable from a controller button (v0.20, v0.29). `crates/dj-app/src/activity.rs`, `ui/src/ActivityStrip.svelte`
- **Functional presets**: one gesture sets workspace, posture, layers, packs and more (§54). `crates/dj-app/src/setup.rs`
- **Laptop mode**: on a weak machine it gives up visuals before control, and control before audio (§48). `crates/dj-app/src/thrift.rs`

#### Events, the DJ's business and the welcome
- **Prepare an event and play the night you prepared**; fact sheet for the job (v0.27, v0.42). `crates/dj-app/src/gig.rs`, `ui/src/Event.svelte`
- **Press kit**, filled from the DJ's own pages (v0.28, v0.43). `crates/dj-app/src/kit.rs`, `ui/src/PressKit.svelte`
- **Your own logo** in place of the wordmark. `crates/dj-app/src/brand.rs`
- **A welcome** that gets to know the DJ and sets djmanzo up (v0.28). `crates/dj-app/src/welcome.rs`, `ui/src/Welcome.svelte`

#### Interface: navigation, windows and looks
- **Space and its leader-key guide** (which-key style), with your own keys under Space (v0.24). `crates/dj-app/src/leader.rs`, `ui/src/MyKeys.svelte`
- **Launcher (`0`)** and **boards**: whole views that replace the decks, boards of your own arranged by hand (v0.24, v0.30, v0.42). `crates/dj-app/src/dashboard.rs`, `boards.rs`, `ui/src/Dashboard.svelte`, `BoardFlow.svelte`
- **Every panel pops out into a window of its own and stays live** (v0.31, v0.34, v0.37). `ui/src/Detached.svelte`, `crates/dj-app/src/monitors.rs`. (FEATURES mentions only "detachable panels M5 — done".)
- **Panels you move, zoom and lift over the decks**; docks you size (v0.27, v0.30). `ui/src/App.svelte`, `crates/dj-app/src/cockpit.rs`
- **One forgiving search for everything**, typo-tolerant (v0.30). `ui/src/find.svelte.ts`
- **Themes worn whole, in parts or as colours alone**; colour that carries meaning; the theme follows the night slowly (v0.21, v0.30). `ui/src/ThemeParts.svelte`, `crates/dj-app/src/mood.rs`, `crates/dj-app/src/theme.rs`
- **Controls shaped like what they do**: an effect knob draws its effect, a sampler slot shows its sound, a stem chip shows where its part plays, the platter turns with the record (v0.21, v0.32). `crates/dj-app/src/outline.rs`, `ui/src/stemPresence.ts`, `ui/src/controls/`
- **The window's edge says what is happening** (v0.30). `ui/src/App.svelte`
- **Pads in one row of symbols; scroll across a stem to fade it; one-press stem mute** (v0.25, v0.29). `ui/src/Pads.svelte`, `ui/src/Stems.svelte`

#### Library, music in and out
- **Find it to buy, and filed when you do**: watched downloads folder, filed by tag or by how it sounds, `.zip` albums unpacked (v0.21, v0.23). `crates/dj-app/src/downloads.rs`, `crates/dj-sources/src/stores.rs`, `ui/src/BuyLinks.svelte`
- **Preset packs to make, share and browse from indexes** (v0.40). `crates/dj-app/src/packs.rs`, `packindex.rs`, `ui/src/Packs.svelte`
- **Opus playback** (v0.40). `crates/dj-decode/src/opus.rs`
- **Every preset one search away** (v0.22). `ui/src/Presets.svelte`, `crates/dj-presets`
- **A night on one line, and nights side by side** (v0.38). `ui/src/Journal.svelte`, `crates/dj-app/src/session.rs`

#### Karaoke pieces not in the karaoke row
- **The melody line** a song's voice sings, drawn (v0.21). `ui/src/melody.ts`, `crates/dj-library/src/vocal_line.rs`
- **WhisperX word timing** as an outside option beside whisper.cpp, and English words landing on the syllable (v0.31, v0.34). `crates/dj-app/src/whispercpp.rs` and `wordtimes.rs`, `ui/src/WordTiming.svelte`
- **Versus teams and the room's verdict** (v0.42). `crates/dj-app/src/karaoke.rs`
- **A singer's photo while they sing** (v0.41). `crates/dj-net/src/photo.rs`
- **Karaoke Night preset pack** (v0.43). `crates/dj-presets/src/builtin.rs`

#### Development and hardware stand-ins
- **Stand-in turntable and controller in the running app** (`DJMANZO_STANDIN_TIMECODE`, `DJMANZO_STANDIN_CONTROLLER`) (v0.46). `crates/dj-app/src/standin.rs`, `crates/dj-hid/src/standin.rs`, `crates/dj-dvs/src/turntable.rs`
- **Neural stems through ONNX on Windows and Apple Silicon**, HT-Demucs working (v0.24, v0.30). `crates/dj-stems/src/model.rs`

---

## 3. Interface and workflow map

The layout follows VirtualDJ's, because that is the handling we are cloning. Our own assets,
our own code.

```
┌──────────────────────────────────────────────────────────────────────────┐
│ TOP BAR   master level · CPU/latency · recording · layout preset · config │
├──────────────────────────────────────────────────────────────────────────┤
│ WAVEFORM ZONE                                                            │
│   stacked or parallel scrolling waveforms · beat grid · cues · loops     │
│   phrase markers · playhead centred, tracks move                         │
├───────────────────────┬──────────────────────┬───────────────────────────┤
│ DECK A                │ MIXER                │ DECK B                    │
│  overview waveform    │  channel faders      │  overview waveform        │
│  title/artist/key/BPM │  3-band EQ + filter  │  title/artist/key/BPM     │
│  jog / platter        │  gain · VU · PFL     │  jog / platter            │
│  play · cue · sync    │  crossfader + curve  │  play · cue · sync        │
│  pitch fader          │  master · booth      │  pitch fader              │
│  loop controls        │                      │  loop controls            │
├───────────────────────┴──────────────────────┴───────────────────────────┤
│ PAD ZONE   page selector: Cues · Loops · Roll · Slicer · Sampler ·        │
│            Stems · FX          8 pads per deck, colour-coded              │
├──────────────────────────────────────────────────────────────────────────┤
│ FX ZONE    slots · beat-synced timing · routing · presets                 │
├───────────────────┬───────────────────────────────────┬──────────────────┤
│ FOLDER TREE       │ SONG LIST                         │ PREPARE          │
│  local library    │  sortable columns, instant search │  a surface of its│
│  playlists/crates │  BPM · key · energy · rating      │  own, docked     │
│  smart folders    │  colour-coded, drag to deck       │  beside or below │
│  imported sources │  `→` sets a track aside ─────────▶│  Sidelist · Next │
│                   │                                   │  Clone · Sampler │
│                   │                                   │  Automix         │
└───────────────────┴───────────────────────────────────┴──────────────────┘
```

**Layout presets** trade complexity for screen space, as VirtualDJ's do:

| Preset | Shows |
|---|---|
| Starter | 2 decks, 160 px waveforms, no pads, loops, filter or keylock — for learning |
| Essentials | 2 decks, 120 px waveforms, cues, loops and the EQ |
| Pro | 4 decks, everything on screen, browser open |
| Performance | 4 decks, 72 px waveforms, density 0.85 — for a controller-driven set |

**Skinning** is CSS themes plus JSON layout definitions. Layout presets are just built-in
skins, which keeps one mechanism instead of two: the four above are ordinary `Layout` values
that happen to ship, and a DJ's own is JSON read from `layouts/` in the config directory by the
same code. A layout says which components are on screen, how tall the waveform lane is, and
one overall density; **it cannot execute code**, reach a file, or change what any control does,
which is what makes one somebody sent you safe to load. Every field has a default, so a file
names only what it changes; out-of-range values are clamped rather than refused; a malformed
file is skipped with a warning rather than costing the DJ their other layouts mid-set. The
choice is stored by name and restored at start-up, so editing your own layout file takes
effect.

Since this was written, panels move, zoom, lift over the decks and pop out into windows of
their own that stay live, and boards are arranged by hand (v0.27–v0.42).
[ADR-0008](adr/0008-one-widget-vocabulary.md)'s widget registry is built (`dj_app::widgets`):
the deck is drawn from a tree of addressed widgets, and the rest of the interface is not yet.
Besides the four presets above, Scratch, Stem Performance and Custom ship, and *skinning*
now also means themes worn whole or in parts and §129's artwork skins over a theme.

---

## 4. Outlook

What is planned, beyond finishing the rows above that are not yet done. Each
item names where it is planned in full.

### Editions, one download, and a trial

djmanzo will be sold as **Auto-Pilot, Hobbyist, Apprentice, Karaoke, Pro DJ,
Full and Custom**, all one download, the parts unlocked by a licence the DJ's
social sign-in fetches (Google, Apple, GitHub, Microsoft, Discord). A **free
7-day trial** of any edition, Full included, once per account. Prices at half
of what the professional competitors charge on average, with regional and
student discounts. A licence never runs out mid-set, and playing never needs
a connection. [EDITIONS.md](EDITIONS.md), [ADR-0011](adr/0011-licence-by-sign-in.md).

Still to build for the editions themselves: the **Auto-Pilot surface** (style
the night, fill the playlist, start, take over), the **Apprentice
curriculum** (from the first beatmatch to the first gig, with exercises the
teacher checks), and the quiet cards that show a Hobbyist what more there is.

### A free marketplace

Themes, skins, visuals, controller mappings, knowledge packs, lessons,
karaoke screens, audio plugins, and **DJ techniques as performances** that
the autopilot can perform and the teacher can teach, through the same gates
as a person's hands. Everything in it free, so that the owner runs no shop
for others; creators may link to their own pages. Built on the signed
`.djpack` and the pack indexes djmanzo already reads.
[MARKETPLACE.md](MARKETPLACE.md), [ADR-0012](adr/0012-a-free-marketplace.md).

### More hardware

Every mapping is written from the maker's published MIDI or HID
documentation or from the device in hand, never from Mixxx's GPL mappings
(ADR-0002), and none is called working until someone has played it.

| Area | Planned | What it needs |
|---|---|---|
| **Pioneer DJ controllers** | DDJ-FLX4, DDJ-400, DDJ-FLX6, DDJ-REV1, DDJ-REV5, DDJ-REV7, DDJ-1000; XDJ-RX3, XDJ-XZ and Opus Quad in controller mode | Pioneer publishes MIDI message lists; the jog screens of the REV7 need the device |
| **Numark** | Mixtrack Pro FX and Platinum FX, Party Mix II, NS4FX | Published MIDI charts |
| **Hercules** | DJControl Inpulse 200, 300 and 500, Starlight | Published MIDI charts |
| **Native Instruments** | Traktor Kontrol S2, S3 and S4 MK3, Z1, X1 | HID, so the device in hand |
| **Denon DJ and Rane** | MC7000, SC Live 2 and 4, Prime Go and Prime 4+ in controller mode, Rane One, Rane Seventy-Two | Published MIDI charts; the Rane One's motorised platter rides on the platter support already built |
| **Reloop, Roland, Allen & Heath** | Mixon 8, Ready, Buddy; DJ-202, DJ-505, DJ-707M; Xone:K2 and K3 | Published MIDI charts |
| **Pads and extra controls** | Novation Launchpad and Launch Control, Akai APC and MPD, DJ TechTools Midi Fighter Twister and 3D, foot switches, Elgato Stream Deck | MIDI charts; Stream Deck over HID |
| **Club players** | **Pro DJ Link** (Pioneer CDJ and XDJ players: tempo and beat, what is loaded, on air from the mixer) and **StagelinQ** (Denon Prime players) | Built against a simulated peer first; neither is proved until it meets a real player |
| **Tempo with other software** | **Ableton Link** | Ableton's licence on its own terms: the GPL one cannot be used (ADR-0002) |
| **Mixers as sound cards** | Pioneer DJM-S and DJM-A series, Allen & Heath Xone:96 and Xone:92, Rane Seventy and Seventy-Two: their channels named and routed as the mixer's panel is | The mixer's channel layout, from its manual |
| **Lighting** | **Art-Net and sACN** over the network and USB DMX interfaces (Enttec), scenes on the beat and on the phrase, the drop cued from the structure djmanzo already finds | A DMX interface or a network node to test against |
| **MIDI 2.0** | High-resolution controls and devices that describe themselves | A MIDI 2.0 device and the platforms' MIDI 2.0 support |
| **Phones and tablets** | A remote for the DJ on the same secure page the singers' phones use, and phones as karaoke microphones (K9) | A secure page on the local network |
| **Microphones and headsets** | Wireless microphone systems and USB headsets recognised and set up for karaoke and talkover | The devices |

### Platforms

The karaoke build for older Macs becomes one more target any edition can be
built for. **Windows** is built, tested in CI and packaged with every release,
and has never been run by a person (see [BUILDING.md](BUILDING.md)); running
it on a Windows machine is what makes it a promised platform beside macOS
and Linux.

### A real gig

Nothing here has played in front of a crowd. The next step that matters
most is a night on real hardware, with what it finds written down here.
