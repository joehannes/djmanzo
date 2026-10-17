# Karaoke beta — session B notes

What session B built, for folding into the shared documents once both
sessions are merged ([karaoke_beta_release.md §6](../karaoke_beta_release.md#6-after-both-folding-in-and-the-release)).
Only session B edits this file.

## Changelog lines (for CHANGELOG.md, under Unreleased)

- **A guest's song, from Suno to WhatsApp, in djmanzo's own windows.**
  *Fill in Suno's form* (a guest's song card) opens djmanzo's Suno window
  and fills its Custom form with the words chosen and the style, like an
  autofill; the DJ checks them and presses Create — djmanzo never does, as
  Suno's terms forbid it. Not signed in yet? The form fills itself once the
  DJ has signed in and Create opens, leaving anything the DJ typed alone.
  Suno's page says what was filled. Once the DJ opens the finished song in
  that window, its link comes into the card by itself. *In djmanzo's
  window* then opens WhatsApp Web in a window of djmanzo's own at the chat
  with the guest, the message and link written; the DJ presses send. Only
  for a guest who agreed to be contacted. The *WhatsApp* button still opens
  the app. With no prepared event, the song's words take the DJ's own
  genres from the press kit as the night's music.

- **Four karaoke presets for a microphone.** *Stadium* (a long, wide room
  and a little more push, for the big chorus), *Radio voice* (the lows and
  highs cut, the middle forward, squeezed hard and dry — for a spoken verse
  or an intro), *Big echo* (a long echo that repeats, for a ballad's last
  line) and *Duet* (gentler compression and a little less room, so two
  singers stay clear of each other). In each microphone's *is for* menu.

- **The singers' screen, between and within the words.** An instrumental
  break of six seconds or more — the intro, a line with no words, the
  silence after a line — is a thin bar filling to the next line, with the
  seconds left beside it. The word being sung glows softly. In a duet (two
  or more singers on the microphones), each singer's name is shown in their
  microphone's colour and lit while their microphone hears them, and a line
  that begins with a singer's name ("Ana: …") is wiped in their colour,
  without the name.

- **The guide voice, offered for a song new to the singer.** When the
  singer up next has never sung their song in the guest book (by its title,
  however it was written, or by the record), the up-next card offers *Guide
  voice*, ticked: *Load on N* then leaves the original singer quietly
  underneath (a quarter, about −12 dB). Untick it to load as the deck is
  set. A singer who already chose a voice from their phone is not asked
  again. On the singers' screen, before the first line, a short line says
  they will hear the original singer quietly underneath.

- **Skins over a theme.** Faint artwork over the whole interface, in the
  theme's own accent colour: *Palm shadows*, *Waves*, *Papel picado*,
  *Halftone*, *Trash polka* and *Glowing plankton*, two or three offered with
  each theme they suit (every Caribbean theme has some). A skin never takes
  a press, is fainter with reduced motion, off with high contrast, and never
  on the singers' screen. Chosen under *Skin over the theme* in the Singers
  panel.

- **Four Caribbean themes.** *Malecón Night* (Havana's sea wall after dark:
  a night-sea navy, coral and sea glass; its light variant the pastel
  facades), *Carnaval* (the vejigante masks: yellow and green on black, red
  kept for danger), *Bioluminescent Bay* (Vieques at night: a cyan glow on a
  black-blue sea, violet beside it) and *Bachata Rosa* (rose and gold on a
  deep plum). Each in light and dark.

- **Emoji from the room, held back.** With *Emoji from phones on the
  singers' screen* on (Singers panel; off until the host turns it on), the
  room's page shows a row of six emoji — 👏 ❤️ 🔥 😂 🎤 🙌. Each tap shows
  small in a corner of the singers' screen, rises a little and fades within
  three seconds (only fades with reduced motion), never over the words and
  never taking a press; five at most at once, one tap per phone every two
  seconds, six a second for the whole room.

## Status (for DIRECTIVE-STATUS.md §126 / §127, FEATURES.md, KARAOKE.md)

- **B5 built**, as the owner chose on 16 October (asked: prefill and the
  DJ presses Create, not full automation; WhatsApp ready and the DJ sends;
  `GuestSong.svelte` may be edited): `dj_app::suno` (`suno_fill`,
  `suno_link`, `guests_whatsapp_here`; `song_link`, `fill_script`,
  `Pending`, `words_for`), the page script `crates/dj-app/src/suno_fill.js`,
  `guests::whatsapp_web` (sharing `wa.me`'s number rule), `with_night`
  taking the kit's genres when no event names the night's music,
  `ui/src/sunoSend.ts`, and the card in `GuestSong.svelte`. Neither window
  is named by a capability: the pages have no way into djmanzo, and
  djmanzo reads only the Suno window's address, never the page. Held by 6
  Rust tests (`suno::tests` 4, `whatsapp_web_is_the_same_message_on_the_
  same_terms`, the kit case in `the_night_around_a_song_is_the_events_and_
  the_kits`), `e2e/suno-fill.spec.ts` (4: the real script on a stand-in
  Create page built like React's, Custom pressed, filled as typing fills,
  what it could not find said, a waiting fill leaving the DJ's words, never
  pressing Create) and `e2e/song-send.spec.ts` (1); 24 mutants fail them.
- **Driven** (Xvfb, the real suno.com and web.whatsapp.com through this
  container's proxy): *Fill in Suno's form* opened Suno's window, which
  showed Suno's sign-in (no account here); the first drive showed the fill
  lost to Suno's redirect to sign-in and the card claiming the words were
  in, and both were fixed (the fill now waits on suno.com's pages for
  fifteen minutes; the card says the words *went* to Suno's window). The
  page then said "djmanzo is waiting for Suno's song form: sign in and open
  Create, and djmanzo fills it in then." *In djmanzo's window* opened
  WhatsApp Web, which WebKitGTK is served, at its QR sign-in. The same
  drive found both new buttons drawn as blank green bars (a local style
  that coloured their words like their fill); fixed, and a browser check of
  it added.
- Not proved here, and nothing here can prove it: that today's suno.com
  Create form is found by the script's words (*lyric*, *style*/*genre*,
  *title*, a *Custom* switch) once signed in; that the address of a song
  opened in Suno's window is `/song/<id>` or `/s/<id>` (WebKit reports an
  address changed by the page's own routing, which is what `suno_link`
  reads); that WhatsApp Web, signed in, opens the chat from
  `web.whatsapp.com/send`. A Suno redesign breaks the fill quietly: the
  page says it found nothing, and the words are on the clipboard.

- **B4 built**: `dj_vocal::Preset::{Stadium, RadioVoice, BigEcho, Duet}`
  (`radio-voice`, `big-echo` on the wire), chain settings only, inside
  `LIMITS` (the existing `held_settings_stay_inside_the_limits` covers them
  through `Preset::ALL`). Held by `the_karaoke_presets_are_what_they_say`
  (7 mutants fail it) and `e2e/vocal-presets.spec.ts` (the menu, drawn from
  Rust's list, has them; choosing one sends `radio-voice`). Nothing here
  can say how they *sound*: no microphone or speaker in this container.
  Not done: an echo timed to the deck's tempo (an idea in `Resume.md`, not
  in B4).

- **B3 built**: `ui/src/lyricBreak.ts` (`breakAt`, `partOf`; pure, held
  by `lyricBreak.test.ts`, 3) and `SingerScreen.svelte` (the break bar, the
  glow — a `drop-shadow` filter, since a text-shadow would paint over the
  wipe clipped to the letters — and the duet: who is on which microphone
  from `vocals_state`, asked with the rotation every three seconds while
  any input is open). Held by `e2e/lyric-visuals.spec.ts` (4); 4 unit and
  6 interface mutants fail them. Looked at: the theme's two accents were
  two greens a singer could not tell apart; and across all 29 themes an
  amber theme's accent and warning hue are as little as 3° apart. So the
  microphones wear the theme's accent, second accent, warning and danger
  tokens in the order that keeps them farthest apart in hue
  (`ui/src/micColours.ts`, `spread`; `micColours.test.ts`, 2), the first
  always the voice's own accent.
- **Driven** (16 October, the app built with `custom-protocol` under Xvfb,
  a 70 s demo record with a timed `.lrc` beside it): B7's offer on the
  up-next card in Rust's words, ticked; *Load on 2* left deck 2 on *Guide*
  (its header says *Guide vocal*); the singers' screen, playing, showed the
  intro bar counting down with "You'll hear the original singer quietly
  underneath", then the break after a sung line ("4s"). B6's picker said
  no skin is made for the theme in use; B2's switch was there, off. Not
  caught in a driven screenshot: the glow mid-word, and a duet (two singers
  on open microphones) — the browser suite holds both.
- Not yet: a duet whose lines carry no names is shown only by the names
  lit while heard; the words then keep the voice's colour. Marking parts
  in an `.lrc` is by the singer's name, which the host types; "M:"/"F:"
  conventions are not read.

- **B7 built** (§128): `dj_app::guide_offer` (`offer`, `sung_before`,
  `for_up_next`; command `karaoke_guide_offer`) decides; `ui/src/guideOffer.ts`
  asks; `Singers.svelte` draws the offer on the up-next card, ticked, and
  *Load on N* sends `deck N voice 0.25` after Rust's load while it is ticked;
  `SingerScreen.svelte` says it before the first line while the singer's
  deck plays at the guide's level. Held by 2 Rust tests and
  `e2e/guide-offer.spec.ts` (4); 4 Rust and 5 interface mutants fail them.
- Not yet: the plan's "marked *not sure* on the form or phone" — no such
  mark exists on the room's form or the phone page, and adding one is a
  change to the phone's request (`dj_net` form, `karaoke::Request`), larger
  than B7. A phone that asks for the guide (K4, *May ask for the guide
  vocal*) is already applied on load and is not asked again.

- **B6 built** (§129): `ui/src/skins/skins.ts` (six pure-SVG tiles drawn as
  a CSS mask in `--accent` over `body::after`, `pointer-events: none`,
  opacity 0.10–0.18; `prefers-contrast: more` / `forced-colors` off,
  reduced motion ×0.6), worn through `theme.svelte.ts` (`skin`, `skins`,
  `setSkin`, `html[data-skin]`, never with `?panel=singers`) and chosen in
  `SkinPicker.svelte`. Held by `skins.test.ts` (3) and `e2e/skins.spec.ts`
  (4: chosen and drawn faintly, every visible button still the thing the
  pointer finds; never on the singers' screen; fainter with reduced motion;
  only with a theme it suits); 4 mutants fail them. Looked at over four
  themes; papel picado was redrawn as outlines after the first look showed
  its solid flags competing with the controls.

- **B1 built**: four rows in `dj_app::theme::ALL` (now 29; the extras test
  names them), packages `pkg-malecon`, `pkg-carnaval`, `pkg-biobay`,
  `pkg-rosa` in `packages.ts` and their palettes in both modes in
  `colors.ts`. Held by the existing contrast and table tests
  (`themes.test.ts`, `theme::tests`); a Carnaval light accent made too pale
  fails the 4.5:1 test. Looked at in the browser, all four.

- **B2 built** (§126/§127, "live comments of the crowd on the karaoke lyrics
  screen as they input emoticons … don't overdo the visual influences"):
  `dj_net::reactions` (the book: glyphs only, `GAP` 2 s per phone,
  `ROOM_PER_SECOND` 6, `SHOWN` 3 s, `KEPT` 64, `PHONES` 300; `Reacted`
  tags), the room page's row and `POST /react` (`front.rs`, `page.rs`),
  `dj_app::reactions` (`room_reactions`, `room_reactions_set`),
  `RoomEmoji.svelte` on the singers' screen and the host's switch in
  `Singers.svelte`. Held by 6 `dj-net` tests (one over a real socket), 2 in
  `dj-app`, and `e2e/room-emoji.spec.ts` (4); 4 Rust and 4 interface
  mutants fail them. The session that began B2 left four fragments (two
  test modules, the end of the browser spec, the end of the component's
  styles); this was built to match them exactly.
- Driven only as far as the host's switch (see B3's entry); a tap from a
  phone needs the room's page served, which was not driven.
- Not yet: the room's taps are not written to the night's crowd log
  (`crowd::Reaction { source: Room }`) — `crowd.rs` is in neither column.

## For the host (KARAOKE-NIGHT.md)

**Emoji from the room.** In the Singers panel, tick *Emoji from phones on
the singers' screen*. Guests then see a row of emoji on the room's page;
each tap shows small in the corner of the singers' screen and fades within
seconds. It is off until you turn it on.

**The guide voice.** When the song up next is new to the singer, the
up-next card shows *Guide voice*, already ticked. Load as usual and the
original singer stays quietly under them; untick it if they know the song.

**Duets.** Put each singer on their microphone (Singers panel, *Mic 1*,
*Mic 2*). Their names appear on the singers' screen in their own colours,
lit while they sing; a lyric line starting with a name ("Ana: …") is
wiped in that singer's colour.

**Karaoke presets.** Each microphone's *is for* menu now has *Stadium*,
*Radio voice*, *Big echo* and *Duet* beside *Singer*. They change only the
sound of the voice; the fader, pan and where it is sent stay as you set
them.

**A guest's song, sent.** In the guest's record, *Fill in Suno's form*
opens Suno in djmanzo's own window and puts in the words and the style.
Sign in there the first time; check what it filled and press Create
yourself. Open the finished song in that window and its link appears in the
card. Then *In djmanzo's window* opens WhatsApp Web at the guest's chat with
the message written (sign in once with your phone's QR code) — or
*WhatsApp* for the app. You always press send.

## For the next session (HANDOFF.md)

## Needed a file this session does not own

- `ui/src/Settings.svelte` (Appearance) or the theme button: the natural
  home for `<SkinPicker />` is beside the theme picker; for tonight it is in
  the Singers panel. At the merge, place `<SkinPicker />` under Appearance
  too if wanted (it needs no props).

- B1 re-blessed three Rust-generated goldens, `ui/e2e/themes.json`,
  `ui/e2e/leader.json` and `ui/e2e/dashboards.json`, because they list every
  theme. At the merge, if A changed them too, take either side and run
  `DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture` on the merged
  tree.

- `crates/dj-app/src/crowd.rs`, to record the room's taps in the night's
  crowd log. Not done.

- B4 re-blessed `ui/e2e/vocals.json` (the preset list). Same as B1's
  goldens: at the merge, bless again on the merged tree.

- `ui/src/api.ts`: `VocalPreset` names five presets; Rust now sends nine.
  Nothing breaks (the menu is drawn from Rust's list and sends its ids
  back), but the type should name `stadium`, `radio-voice`, `big-echo` and
  `duet` at the merge.

- B5 edited `ui/src/GuestSong.svelte` (in neither column) with the owner's
  leave, asked on 16 October. At the merge, keep its *Fill in Suno's form*,
  the language choice, the caught link and *In djmanzo's window*.

- `docs/karaoke_beta_release.md` (the risks table) says "djmanzo fills
  Suno's form the day its API opens"; with the owner's choice it now fills
  it as an autofill in its own window, and the DJ presses Create. That line
  wants changing at the merge, and `docs/KARAOKE.md`'s §123 with it.
