# Features

Two halves: **parity** — everything VirtualDJ does that djmanzo must do — and **beyond** —
the reasons to use djmanzo instead.

Every parity row carries the milestone that delivers it. Nothing is listed without a home.
Milestone definitions are in [ROADMAP.md](ROADMAP.md).

---

## 1. Parity matrix

### Decks and transport

| Feature | Milestone |
|---|---|
| 2 decks | M1 |
| 4 decks | M2 |
| 6 decks | M5 — **done** |
| Play / pause / cue (CDJ-style cue behaviour) | M1 |
| Pitch fader, configurable range (±6/8/10/16/25/50/100 %) | M1 |
| Pitch bend (temporary nudge) | M1 |
| Keylock / master tempo | M1 |
| Jog wheel: scratch, bend, search modes | M4 |
| Vinyl mode vs CDJ mode | M4 |
| Sync (tempo + phase) | M2 |
| Quantize to beat / bar | M2 |
| Beat jump | M2 |
| Track load, unload, clone deck, instant doubles | M3 |
| Slip mode | M5 — **done** |
| Reverse / censor | M5 — **done** |
| Elapsed / remaining time, end-of-track warning | M1 |
| **Key shift** — transpose in semitones for harmonic mixing, independent of tempo | M2 |
| **Sandbox** — audition a mix in headphones while the master keeps playing | M5 |

### Mixer

| Feature | Milestone |
|---|---|
| Channel faders, crossfader with selectable curve | M1 |
| Per-channel crossfader assignment (A / thru / B) | M2 |
| 3-band EQ per channel, kill switches | M1 |
| Per-channel filter (low/high-pass sweep) | M1 |
| Gain + auto-gain from EBU R128 analysis | M2 |
| VU meters per channel and master | M1 |
| Headphone cue (PFL) with cue/master blend and split-cue | M1 |
| Booth output with independent level | M1 |
| Master limiter | M1 |
| Microphone / aux input with ducking | M5 — done |
| Microphone effects (reverb, echo, pitch) | M5 |
| Crossfader assign per channel (A / B / thru) | M1 |

### Waveforms and displays

| Feature | Milestone |
|---|---|
| Scrolling waveform, per deck | M1 |
| Stacked / parallel multi-deck waveform view | M2 |
| Overview waveform with position and cue markers | M2 |
| Beat grid overlay, editable (shift, scale, tap) | M2 |
| Saved loops, recalled per track | M3 |
| Cues, grids and loops kept with the track across sessions | M3 |
| Playlists, crates and folders in one tree | M3 |
| Play history, recorded when a track is actually played | M3 |
| Smart folders with a filter language, incl. harmonic matching | M3 |
| Import rekordbox XML, Traktor NML and iTunes XML with cues and grids | M3 |
| Import Serato crates and database (clean-room) | M3 |
| Read Serato hot cues and loops out of the audio files | M3 |
| Duplicate detection across copies of the same audio | M3 |
| Batch tag editing, colour coding and ratings | M3 |
| Session export as a set list | M3 |
| Spectral colouring (bass/mid/high energy) | M2 |
| Loop region and hot cue overlays | M2 |
| Phrase / structure markers | M8 |
| Waveform zoom, per-deck and global | M2 |
| Lyrics on the waveform | M8 |

### Cues, loops, pads

| Feature | Milestone |
|---|---|
| Hot cues (8+, named and colour-coded) | M2 |
| Manual loop in / out, loop adjust | M2 |
| Auto-loop by beat count | M2 |
| Loop roll (momentary, always slipping, down to 1/16 beat) | M5 — **done** |
| Loop move / halve / double | M2 |
| Saved loops | M2 |
| Slicer | M5 — **done** |
| Pad pages: Cues · Loops · Roll · Saved · Sampler · FX | M5 — **done** |
| Pad pages: Slicer · Stems | Slicer page M5 — **done**; Stems page M6 — started |
| Pad colour and LED feedback to hardware | M4 |

### Effects

| Feature | Milestone |
|---|---|
| FX rack, three chained slots per deck and on the master | M5 — **done** |
| Per-deck and master FX routing | M5 — **done** |
| Pre-fader / post-fader chain placement | M5 — **done** |
| Beat-synced FX timing (1/16 to 4 beats, following the pitch fader) | M5 — **done** |
| Core effect set: echo, delay, reverb, gate, crush, flanger, phaser, filter | M5 — **done** |
| Roll (as a loop roll, on its own pad page rather than in a slot) | M5 — **done** |
| Brake and backspin — transport, not signal: they live on the deck | M5 — **done** |
| CLAP plugin hosting (master insert; generic controls, no plugin window) | M5 — done |
| **Per-stem EQ and filter** | **shipped** — three bands and a sweep per stem, composed with the deck's own EQ rather than replacing it |
| Per-stem effects | M6 |
| Effect chain presets | M5 — **done** |

### Sampler

| Feature | Milestone |
|---|---|
| Sampler banks with pad grid | M5 — **done** |
| Trigger modes: one-shot, loop, hold, stutter | M5 — **done** |
| Sync samples to master tempo | M5 — **done** |
| Sample volume / output routing (mix or headphones) | M5 — **done** |
| Record from deck or master into a sample slot | M5 — **done** |

### Browser and library

| Feature | Milestone |
|---|---|
| Folder tree + song list, VirtualDJ layout | M3 |
| **Prepare**, its own dockable surface: Sidelist · Next · Clone · Sampler · Automix | M3 (Karaoke K1) |
| One gesture sets a track aside — `→` on a browser row, and it is in Prepare | Cockpit §21 |
| Search across the whole library, instant | M3 |
| Playlists / crates / smart folders | M3 |
| Columns: BPM, key, energy, rating, play count, last played, comment | M3 |
| Sort, filter, colour-coding | M3 |
| Track info editor, batch tag editing | M3 |
| Import: rekordbox, Serato, Traktor, iTunes, folders | M3 |
| Duplicate detection | M3 |
| Play history and session export | M3 |
| Harmonic (Camelot) key display and compatible-key filtering | M3 |
| Automix with configurable transition style | M5 — done |
| Search across online sources (Spotify, YouTube, Jamendo, Internet Archive) | **shipped** |
| Match an online result to a file you already own | **shipped** |
| Licensed streaming catalogue (Beatsource/Beatport/TIDAL/SoundCloud) | slots shipped; **needs a partnership** — see [SOURCES.md](SOURCES.md) |

### Hardware

| Feature | Milestone |
|---|---|
| Class-compliant MIDI controller support | **shipped** — 7-bit, 14-bit and all three encoder conventions |
| Data-file mappings (no recompile) | **shipped** — TOML, bundled and user files, checked when the file loads |
| Mapping editor (learn a control, bind an action) | **shipped** — press the control, pick the action, save; the file is proved to reload before it is written |
| LED / display feedback to controllers | **shipped** for LEDs and pad colours; segment displays still M4 |
| **Stem swapping across decks** — one deck's vocal over another's mix | **shipped** — `stem_swap vocal 1 2`, latching and undoable to what the DJ had |
| **Lua scripting in mappings** — a shift key, a mode-dependent jog, one knob doing two things | **shipped** — sandboxed: no filesystem, no process, every action through the parser, stopped after 100k instructions |
| HID controller support | **shipped** — 8- and 16-bit fields, both byte orders, level-to-edge conversion, and a learn mode that diffs two reports to name the control that moved |
| **Motorized platter support** — high-res absolute position, motor ramp, torque | **shipped** — absolute angle with wrap handling, motor driven by the transport |
| Controller-specific audio setup presets | **shipped** — an `[audio]` block per mapping; a master that overlaps the cue is refused when the file loads |
| Controllers panel — what is connected, on which mapping, with which outputs | **shipped** |
| **Keyboard as a controller** — same vocabulary and file format as a MIDI mapping, with a live sheet | **shipped** |
| Multi-device audio setup (4-channel, or two devices with drift correction) | M1 |
| **MIDI clock out** — djmanzo as clock master for a drum machine or light desk | **shipped** — 24 PPQN at the room's tempo, on its own thread |
| **MIDI clock in** — follow a drum machine or a second DJ | **shipped** — an external clock outranks every deck as the sync leader |
| **OSC** — TouchOSC, Lemur, QLab | **shipped** — the action grammar is the address space; loopback only, because UDP cannot carry a passphrase |
| Network control API — drive djmanzo from a script, a Stream Deck or a lighting desk | **shipped** — line-delimited JSON over TCP, off by default, loopback by default, passphrase required off-machine |
| **Pro DJ Link** (Pioneer CDJ/XDJ) | M7 |
| **StagelinQ** (Denon Prime) | M7 |
| Network tempo sync (Ableton Link or equivalent) | M7 |
| MIDI clock in/out | M7 |
| DVS timecode vinyl | *deferred — architecture leaves room; see ARCHITECTURE.md §10* |

### Output, recording, broadcast

| Feature | Milestone |
|---|---|
| Record the master to disk (WAV; FLAC/MP3 later) | M5 — **done** |
| **Network tempo sync between djmanzo instances** | **shipped** — announce and follow over UDP, no master, bounded corrections. Not Ableton Link; see ROADMAP |
| Broadcast to Icecast/Shoutcast | M5 |
| **Per-stem outputs for external processing** | **shipped** — one deck as four stereo pairs (vocals 1–2, drums 3–4, bass 5–6, other 7–8), pre-EQ and pre-fader, on an interface with eight outputs |
| **Per-deck outputs for external processing** | **shipped** — each deck on its own stereo pair, pre-fader, no master chain; exclusive with stem out |
| **Phrase detection** | **shipped** — phrase length and phrase anchor, from beat-synchronous novelty in four bands; markers on the waveform, `phrasejump` and `loop_phrase` on the keyboard and the deck panel, persisted across restarts. Verified against synthetic tracks whose structure is arithmetic; **not measured against a corpus of real records** |
| **Next-track suggestions** | **shipped** — harmonic, tempo, loudness and phrase, ranked with typed reasons shown as chips; lift/hold/ease. Deterministic and local. **Energy is approximated by loudness**, which is a proxy and not the same thing |
| **Transition planner** | **shipped** — where to start (phrase boundary with a tail margin), how long, and which style, with typed reasons. Proposes; does not act. **Cannot know where the outro is** — a phrase boundary near the end is a structural guess |
| **Set files and take diffing** | **shipped** — a set saved as readable, diffable text; two takes compared by move and drift |
| **Deterministic replay / re-render** | **shipped** — a set file rendered back to audio, faster than real time, byte-identical run to run. Live inputs (mic, aux, DVS) cannot be reproduced and are stated as such |
| **Genre families** | **shipped** — 31 families across Latin America, North America, Europe, Africa and beyond, encoding felt-vs-written tempo and rhythmic grammar. A working DJ's map, not a musicology; wrong at the edges by design and written down so it can be corrected |
| **Automatic set assembly** | **shipped** — a whole set built from the library with an arc, taste as a tilt, no repeats, no two tracks by one artist in a row, and no blend across a rhythmic grammar |
| **Assistant takeover** | **shipped and wired** — per-control, instant, implicit on any human action; one gesture hands everything back; expires after ten minutes |
| **Assistant posture / occasion** | **shipped** — six postures, nine occasions, seven packs, in the Conduct panel with the next step shown before it happens |
| **Share a set** | **shipped** — a night's tracklist handed to WhatsApp with the message already written, previewed first, and a file for the sets too long to fit in a link. Nothing is posted and no recipient is chosen: djmanzo prepares the share, the person presses send |
| **DJ technique catalogue** | **shipped** — twenty-eight moves with what each needs, when it works and a metaphor to teach it by. Filtered by how the two records blend and by the rig actually present: a controller counts when its mapping is open, and an unanalysed track leaves beatmatching by ear |
| **Learning module** | **shipped** — names what the hand just did, catches one specific error at a time, and sets the easiest thing not yet tried. Reads the action log, so what the DJ did is known rather than inferred; makes no claim about whether it sounded good. Nothing is scored |
| **Every performing control on one screen** | **shipped** — at djmanzo's own default 1280×800 with two records loaded, the waveform, pad grid, deck channel strip, cue, crossfader assignment, crossfader, master gain, headphone cue, split and limiter are all visible without scrolling. The master strip and each deck's channel strip are pinned; the waveform's tail and the loop rows are what scroll on a short window. The crossfader had been below the fold three times, most recently by about 280 px. Enforced by `ui/e2e/budget.spec.ts` |
| **The interface fits the window it is given** | **shipped** — djmanzo picks a density band from the window height (`cockpit::BANDS`, derived from measured deck heights, not chosen), and an explicit density from a layout or workspace overrides it. The pad grid, the SVG faders and knobs and the waveform lane all answer to it; before this they were fixed pixels and the density control moved a deck by 68 px across its whole range |
| **Function tags** | **shipped** — what a record is *for*, which is not what it is: opener, builder, peak, floor reset, singalong, closer, transition tool, safe, risky, emergency. A closed vocabulary of ten fixed in Rust, because the value of the tag is that it means the same thing on every record — free text gives you `opener`, `Opener`, `open`, `warmup` and `warm-up` in the same collection within a month. Set on a selection in the browser; every function is offered even at zero, with a count. Migration 10. Smart folders can filter on them: `for is opener`, `function is peak`, `not for is risky and bpm > 120` |
| **Two panels at once** | **shipped** — surfaces dock beside the decks and along the bottom rather than taking turns in one slot, several open together, each in a titled frame that closes from its own header. Where one lands comes from its own preferred size, so the library runs under the decks and the assistant stands beside them. The arrangement is checked in Rust and survives a restart. This replaced a single variable that allowed exactly one panel — which is why the room and the library could never be looked at together |
| **The deck is drawn from a layout tree** | **shipped** — `Deck.svelte` renders a list of named widgets in the order the resolved tree gives, rather than holding the deck's shape in its own markup (ADR-0008 W3). A golden order in Rust asserts the tree still produces the deck djmanzo draws, so a control cannot move underneath a DJ as a side effect of a format change |
| **Panels that fit the window** | **fixed** — a panel whose content ran past the bottom of the window was cut off silently by `main`'s hidden overflow. At djmanzo's own default 1280×800 with the decks open that was the last rows of every library table and, in the Assistant, the entire chat input |
| **Session journal** | **shipped** — Mark in the top bar takes the moment and what is on the decks; the Journal beside History is where it gets written up. A note belongs to a moment rather than a track, carries what was playing as text so it outlives the record, and its time and context cannot be edited |
| **More like this** | **shipped** — similar records from any track in the browser, ranked with the reason on each row. The table becomes the answer, so sorting, set-aside and load-to-deck all still work |
| **Taste learned from play history** | **shipped** — which genre families you reach for *more than owning them would predict*, on a 180-day half-life. Added to a score and bounded to ±0.75 against a ±3 scale: it breaks ties, it cannot overrule the mixing, and it never learns an avoidance. Silent until about a night's worth of plays, and shown in the Conduct panel so you can disagree with it |
| **Set planning workspace** | **shipped** — pick an arc and a length, see the shape of the night as a strip, steer it up or down without rebuilding, drop or defer single records, then save it as a playlist or hand it to the assistant. The opener is protected from a steer, visibly |
| **Audience requests** | **shipped** — djmanzo serves the room its own page from a QR code or a printed sticker: people type what they want to hear, one entry per song no matter how it is spelled, with a tally of how many asked. English and Spanish, no script and no fetch, so it works on venue wifi with no route out. The DJ reads the list most-wanted first, hands a row to the search box, and a request the deck plays ticks itself off. The page can be closed without being taken away |
| **A printable way in** | **shipped** — `http://djmanzo.local:7331/` is the same at every venue, answered over multicast DNS, which is what makes a sticker printable before anybody knows the venue's router. The caveat is printed rather than hidden: iPhones resolve it, Android has since 12 and not on every build, some browsers never will — so tonight's plain address is offered beside it, and a sheet of twelve stickers can be printed for either |
| **Reading the room** | **shipped** — a camera and a microphone measure how bright the room is, how much of the picture changed, and how loud it is, every two seconds, and djmanzo says whether the floor is stiller or busier *than it has been tonight*. Every reading is judged against the same room earlier the same night, because two lenses pointed at one wall report different numbers and an absolute threshold means nothing. It never names a mood: a camera cannot tell dancing from leaving. The one interpretation offered is a disagreement — the floor is doing something other than the night you set up. **The measuring has never been run against a real camera**: there is none in the machine this was built on, so what is verified is the model, the wiring, and that a machine with no camera says so and says what to do about it |
| **Nothing leaves the window** | **by construction** — the frame is scaled to 64×48, averaged into three numbers and discarded. No image is recorded, saved or sent. The preview exists so you can aim the lens and can be switched off while the measuring carries on |
| **Find it from memory: the words** | **shipped** — search a line you half remember against lyrics fetched for your own collection. Folded on both sides, so "no puedo dormir" finds "Y no puedo dormir," with its capital, its comma and its accent. Words come from LRCLIB (free, no key); a record with none is remembered as asked, so a sweep never asks twice. Browser → **From memory**. **The call to LRCLIB itself has never been made from this machine** — the egress proxy denies the host — so the parser is tested against the documented shape rather than a captured response |
| **Find it from memory: a description** | **shipped** — "bachata with a piano hook, sounds like Aventura, heard it at a beach bar" goes to the assistant, which answers in rows rather than prose; anything that is not a row is dropped, so a model that ignores the format returns nothing instead of a paragraph dressed as a shortlist. Each guess shows its reason and whether you already own it. The only one of the three that can name a record you do not have |
| **Find it from memory: a hum** | **shipped — two searches off one recording, and neither identifies a record you do not own.** The hum is read for key and tempo through djmanzo's own analysis and used to narrow the collection, counting half and double time because people hum the vocal, not the kick; and it is compared *as a melody* against a stored pitch contour for each record. Recognising a recording you do not have needs a licensed fingerprint service with tens of millions of reference melodies; djmanzo has none, and the panel says so next to the button rather than in a help page |
| **Matching the tune itself** | **shipped** — ten pitch points a second, found with YIN because plain autocorrelation peaks at the octave below about as readily as at the fundamental, and an octave error is a semitone error of twelve. Three measurements decided the design. Centring each contour on its own median does *not* make the search key-blind — an eight-second hum's median and a five-minute record's are never the same number, and the phrase sat nine semitones from where the hum thought it was — so the match runs on the differences between consecutive points, where any constant offset cancels exactly. Three octave slips in one hum then flipped the ranking (right record 2.13, wrong one 1.42); clamping barely moved it (0.67 against 0.69) and folding the intervals into an octave took the right record to 0.000. And a nine-kilohertz tone read as fully voiced, because YIN is scale-invariant and attenuating a band it cannot hear does nothing — voicing now requires the energy to be in the melodic band. Searching is subsequence DTW, so the answer is *where in the record*, and the warping is itself what makes it tempo-independent. **What a contour holds is the strongest periodicity, not the melody**: the vocal much of the time, the bassline some of it — so a hummed vocal will not find a record whose loudest line is the bass. That is why it is a shortlist with key and tempo ranked beside it. One bug only came out of *running* it: the sweep handed the matcher an interleaved stereo buffer, which does not fail — it produces a contour twice as long as the record, so every reported timestamp was half the truth and the search still worked. Two channels of a five-minute track are ten minutes of numbers. A test on contour *length* now forbids it. **The hum itself has never been run for real**: there is no microphone in the machine this was built on, so the tests drive synthesised tones and the panel was driven once with a stubbed result. The sweep, by contrast, is verified end-to-end through the interface against real files — 240 contour points for a 24-second record, where the bug gave 480 |
| **Contours are read a sweep at a time** | **shipped** — twenty records a press from the panel, rather than a background pass competing with analysis for the same disk. A contour is a quarter-semitone to the byte, about 3 kB for a five-minute record. The panel shows how many records have one, because a shortlist drawn from a third of a collection should say so |
| **The first screen is for mixing** | **fixed** — measured at djmanzo's own default 1280×800: the crossfader sat about 1,500 px down, two screens below the waveforms it is used against, and the channel faders and EQ about 800 px down. Nothing a DJ touches to perform a mix was on the screen they start on. What was: three empty FX dropdowns per deck and two jog wheels duplicating the waveform's position. No shipped preset fixed it, including the one described as "everything you need and nothing else". The crossfader is now directly under the decks, the EQ, filter, volume and pitch are one side-by-side strip instead of a 530 px stack, the effect rack folds until something is in it, and the cue and crossfader assignment share a line. **Then it grew back.** Re-measured later at the same 1280×800, the deck column had reached 695 px and the crossfader's thumb was 117 px below the fold again — pitch was nominally in the strip but actually in a three-column grid of eight controls, which wrapped the strip onto three lines. Flattening that grid and moving slip, reverse, censor, brake and backspin up to the transport where they belong takes the deck to 539 px and the thumb to y 758, on screen at the size the application opens itself at. The lesson is the regression, not the number: a layout budget with no test drifts back — **and there is now one**, which measured a third instance on the day it landed and is still failing on purpose. See the two rows above |
| **A layout is a tree of named widgets** | **the vocabulary, the format and the loader are built; the interface is not yet rendered from them.** A layout was a struct of thirteen booleans, one per feature, each matched by an `{#if}` — which meant a DJ's layout file could only ever name what the binary already knew, and a file format that can only say what the binary says is decoration. There is now a registry of 33 named widgets (`deck.waveform`, `mixer.crossfader`, `panel.assistant`…), each declaring the slots it may sit in, the slots it offers its children, and every setting with a type, a range and a default. A layout is a tree of those placed into named slots, in JSON, and existing flat layouts are upconverted on load so nobody's file breaks. **What is not done: `Deck.svelte` and `App.svelte` still contain the layout rather than rendering the tree** — that is the refactor ADR-0008 warns is the cost, and it is why the ADR's status is not simply "implemented" |
| **A layout cannot execute** | **enforced by shape, not by a list of forbidden spellings.** A layout is a thing one DJ sends another, so restyling is a closed set of 23 design tokens and each value is checked against the shape that token takes: a colour is a hash and 3, 4, 6 or 8 hex digits, a length is a number and one of `px`/`rem`/`em`, a scale is a bare number. Everything else falls off the end — `url()`, `@import`, a CSS escape spelling `url(`, a comment, a closing brace, a keyword like `red`, `var()` indirection. The tests name those cases, but they are not what makes it safe: a blacklist is a list somebody eventually gets past, and this is a whitelist of three shapes. Mutating the hex-digit check away is what proved the point — without it `#0;url(xy` is exactly eight characters and would have passed, and the first version of the test did not catch it. The audio-driven properties (`--audio-energy`, `--stem-color`) are deliberately not tokens: a layout that could pin them would be a layout lying about the mix |
| **A layout from a newer djmanzo still opens** | **shipped** — an unknown widget, an unknown slot and an unknown token are each skipped and *counted*, never fatal, so a DJ opening their laptop before a set gets an interface rather than a dialog. Skipping in silence would be worse than refusing, because the missing half looks like a bug in the application — so the top bar carries a chip saying how many parts were not shown, with the reasons in its tooltip. Verified by writing a layout file by hand with one unknown widget, one unknown slot and one unknown token: djmanzo drew the rest, applied the accent and radius the file *did* get right, and said "3 not shown" |
| **The first screen is measured, not remembered** | **shipped** — a browser test opens the interface at djmanzo's own 1280×800 and measures where the controls a DJ performs with actually land. Rendered geometry, in a real browser, because a template assertion passes while the crossfader is 900 px off the screen and jsdom does no layout at all. **The fixture it draws is captured from the running application** (`DJMANZO_SNAPSHOT_OUT`), not rebuilt from a parameter registry — that was tried and was wrong twice in ways that both left the test green against a screen no DJ will ever see: a fresh registry is all zeros, so every stem reads as muted and the interface unfolds a 359 px module nobody opened; and nothing is loaded, so the deck draws no pad grid and comes out ~200 px shorter than the real one. A Rust test checks the committed fixture still has the shape the current `Snapshot` type sends |
| **The crossfader is still below the fold, and the test says so** | **found, measured, not yet fixed.** With two records loaded a deck column measures **675 px**; the top bar takes 138 and the master strip 110, so the crossfader's centre lands at **y 877** — 77 px past the window djmanzo opens itself at. **That 77 is a floor, not the figure.** The harness does not draw the pad zone the real deck has — the page strip and the eight-pad grid — so the true gap is larger by whatever that costs, on the order of two hundred pixels judging by a screenshot of the running application. `pad_pages` is asked for and answered with the eight pages the application generates, and the component still draws nothing under the stub; why is unexplained and written down in `ui/e2e/budget.spec.ts` rather than left as a surprise. Master gain is beside it and equally gone. This is the third instance of the same regression and the first two were found by a human with a screenshot. It is recorded as a **running test marked `test.fail()`**, which asserts the failure is *still* there: whoever fixes the deck's height gets a red test telling them to delete the marker, instead of a green suite that quietly forgot. That one is skipped on CI and says why — it is a both-ways pixel assertion, and CI installs a Chromium build with the runner's own font stack that this has never been measured on; a pixel assertion on an unverified renderer is a red build that says nothing about djmanzo. The ratchet prints the runner's deck height on every run, so it can be enabled there on evidence rather than on a guess. Fixing it means finding about 150 px in the deck column — merging the beat-jump and loop rows, folding the overview the way the stems module folds, shortening the channel fader, tightening the row gaps — and which of those to spend is a design decision, so the measurement is recorded and the choice is left open. A second test ratchets the deck at 740 px — deliberately slack, because the regressions on record were +156 and +117 px, not +20 — so the column cannot quietly grow further while that is decided |
| **The master strip was two rows and is now one** | **fixed** — 900×210 in two rows became 1240×110 in one, spending 355 px of horizontal room that was sitting unused beside it. That halves what the strip contributes to the overflow above, and it fixed a bug of its own: with a four-channel cue the SPLIT button was drawn at x 666–760 and the output meters at 740–860 over the same rows, so the meter bars ran *behind* the button. Nobody had seen it because this machine has no cue device and the branch never rendered; it is now caught by a test that opens the shell with the cue flag on. The reading order is what a DJ reaches for in order: what you mix with, how loud, what you hear, what is coming out, what is protecting it |
| **Occasion-aware density** | **shipped** — a control that cannot be undone by pressing it again becomes a 600 ms hold when the occasion says mistakes are costly, and stays a plain click when it does not. The pad itself holds; no second widget. Reversible controls are never made harder |
| **Autopilot** | **shipped** — a half-second tick that stages, levels and mixes according to posture, from an assembled setlist. Never verified against real audio: there is no output device here |
| **Timecode vinyl / DVS** | **shipped** — speed, direction and absolute position drive a deck, in relative or absolute mode, with an input picker and a live calibration reading in Settings. djmanzo writes its own control signal to a WAV, so any turntable, CD deck or phone works without a licensed record. **Not yet run against a pressed record** |
| Video mixing / VJ output | M8 |
| Karaoke | K1 / K2 — see [KARAOKE.md](KARAOKE.md) |

### Interface

| Feature | Milestone |
|---|---|
| Layout presets: Starter · Essentials · Pro · Performance | M3 |
| Skin system (CSS themes + JSON layouts) | M3 |
| Multi-monitor / detachable panels | M5 — done |
| Configurable waveform and jog appearance | M3 |
| Light and dark themes | M1 |
| **A top bar that says what kind of thing each control is** | **shipped** — the row under the readouts carried twelve controls at one weight and three unlike kinds among them: seven panels you open, three controls over what the stage shows, and two acts on the night. It is three named groups now, named for a screen reader as well as for the eye, separated by space rather than a rule so the row still breaks at a group boundary when it wraps. Grouped, not hidden: every control is still one press away, in the same order. Two things fell out of doing it — the watershed was the last unlabelled square in a row whose whole purpose was that squares get names, and `class:on` on the REC button had never matched a rule anywhere, so a running recording looked exactly like a stopped one |

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
- **Art-Net / DMX** output driven by beat and structure data.
- **Community mappings** as data files — a new controller is a pull request, not a release.

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

Moving and restyling individual components — as opposed to showing, hiding and resizing them —
is not in yet. It needs a component-addressing scheme that survives the interface changing
underneath it, which is a design problem rather than a coding one.
[ADR-0008](adr/0008-one-widget-vocabulary.md) is that design: a widget registry, and a layout as
a tree of addressed instances in named slots rather than a struct of feature flags. Decided, not
yet implemented.
