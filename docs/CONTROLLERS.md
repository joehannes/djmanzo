# Controllers and the keyboard

How djmanzo listens to hardware, what a mapping file looks like, and why a
mapping from a stranger is safe to open.

## One vocabulary

A pad on a controller, a key on the laptop, a button in the interface, a line
in a script and a suggestion from the assistant all end in the same place: a
line of action text, parsed by `dj_core::Action::parse`, dispatched on the
action bus. That is [ADR-0003](adr/0003-action-bus-and-parameter-registry.md).

The consequence worth stating plainly:

> **A mapping cannot do anything the interface cannot.**

There is no escape hatch — no shell command, no file path, no plugin load. A
mapping file can rebind every control on your controller and cannot invent a
capability, because everything it is able to say has to survive the same
parser the Play button goes through. That is what makes downloading somebody
else's mapping a reasonable thing to do.

And every action in a file is checked **when the file loads**, not when the pad
is pressed. A typo is a message when you choose the mapping. The alternative —
finding out an hour into a set that one pad does nothing — is not a trade
worth making for a slightly shorter startup.

## The keyboard

A laptop keyboard *is* the controller for most people opening djmanzo for the
first time, and for every DJ on a train. It is treated as one: same vocabulary,
same file format, same validation, listed alongside the MIDI mappings.

Press **Keys** in the toolbar for the layout as it currently stands, live — a
key lights while it is held, which is the only way to see that a censor is on
when the deck is doing exactly what you asked and looks like it is playing
normally.

### The shape of it

Two decks, one under each hand, laid out so the two halves mirror each other.
Whatever the left hand does to deck 1, the right hand does the same distance
from home position to deck 2.

```
  1 2 3 4                    7 8 9 0     hot cues
  Q W E R                    U I O P     beat jump, loop
  A S D F        G  H        J K L ;     transport, roll
  Z X C V                    M , . /     EQ kills, brake
```

`Space` plays deck 1, `⇧Space` plays deck 2. The arrow keys are crossfader
cuts — full left, centre, full right — because a keyboard cannot hold a fader
anywhere in between and a nudge-per-press would need thirty presses to cross.

Anything labelled **(hold)** happens while the key is down and undoes itself
when your finger comes up. A censor you have to switch off again is not a
censor.

### Two details that matter more than they look

**Keys are named by physical position.** A binding says `KeyQ`, not `q` — the
key in the position where a US layout has Q. On an AZERTY keyboard that key
produces A and on QWERTZ it still produces Q, but in all three it is *the key
above the left hand's ring finger*, which is what a mapping actually means.
Naming keys by the character they produce would move half the transport
controls under a French DJ's fingers the moment they changed layout.

**Held keys are let go when the window loses focus.** Hold the bass kill, hit
Cmd-Tab, and the operating system delivers the key-up to whatever you switched
to. Without this the deck stays killed until you come back and press the key
again — during a set, in front of a room.

### When it is not listening

The keyboard steps aside for text: typing in the search box gives you letters,
not transport. There is also an explicit off switch on the Keys panel, for
when you want the interface to have every key.

## What a keyboard can reach

`dj-hid` asserts, as a test, that **every deck verb djmanzo has is either on a
key or written down as deliberately keyless with a reason**. The verb list is
read out of the parser's own source rather than kept as a second list, because a
second list is the thing that goes stale — and a verb added to the engine with
no key is invisible: it works, it is tested, and the only person who finds out
it is unreachable is a DJ on a laptop in a booth.

It found three the first time it ran: the **crossfader could not be assigned**,
a **beat grid could not be corrected**, and **sync could be engaged and never
released** — there was no toggle verb at all, so every key and every controller
pad could only turn it on.

Two kinds of thing are legitimately keyless, and the list says which:

- **continuous** — it carries a position, and a key is a switch. A key can kill
  an EQ band; it cannot sweep one.
- **covered by a toggle** — the explicit on/off pair exists for scripts and for
  controllers with two buttons, and the keyboard uses the toggle.

## A MIDI mapping

```toml
name = "Generic 2-deck"
device = "MIDI"          # matched loosely against the port name

[[binding]]
on = "note 1 0x0B"       # channel 1, note 11 -- hex, because manuals use it
press = "deck 1 play_pause"

[[binding]]
on = "note 1 37"
press = "deck 1 censor_on"
release = "deck 1 censor_off"     # both -> momentary; press only -> latching

[[binding]]
on = "cc 1 7"
move = "deck 1 eq_low {value}"
min = 0.0
max = 4.0                # what {value} runs between; 0..=1 by default

[[binding]]
on = "bend 1"            # the pitch wheel, for its fourteen bits
move = "deck 1 pitch {value}"
min = -1.0
max = 1.0

[[binding]]
on = "cc14 1 0x00"       # a fader sent as a *pair* of control changes
move = "deck 2 pitch {value}"
min = -1.0
max = 1.0

[[binding]]
on = "cc 1 0x10"         # a jog wheel: movement, not position
move = "deck 1 jog {value}"
encoding = "offset"
min = -0.02
max = 0.02

[[binding]]
on = "cc 1 24"
encoding = "signed"
turn_up = "deck 1 beatjump 1"
turn_down = "deck 1 beatjump -1"
```

`{value}` is the control's position, scaled into the range the action wants.
It is the same idea as `{deck}` in a preset, and deliberately the same
spelling.

Channel `0` means "any channel", for controllers that can be moved around.

### Faders that arrive in two halves

Most modern controllers -- every Pioneer, and Denon and Native Instruments
besides -- send a fader as a **14-bit** value split across two control changes:
a high byte, and a low byte 32 controllers above it. `cc14` names the high one
and djmanzo pairs them.

The low byte is not written, because MIDI fixes it: controllers 0–31 carry high
bytes and 32–63 carry their partners, so the partner of `n` is always `n + 32`.
A `cc14` naming anything from 32 up is refused when the file loads, because it
names a pair that cannot exist.

Binding the high byte alone with plain `cc` also works, and throws away half the
resolution: 128 steps across a pitch fader is 0.125% a step, which is audible
when beatmatching, and it is the same 128 steps across an EQ kill.

A controller that sends **only** high bytes still reaches both ends of its
travel — until a low byte arrives the control is simply read as the seven bits
it is.

### Encoders: say which convention

Three are in the wild and **the mapping has to declare which**, because the
same byte means opposite things in two of them:

| `encoding` | What a byte means |
|---|---|
| `signed` (default) | `1..=63` that many clicks clockwise, `127..=65` anticlockwise, `0` and `64` still |
| `offset` | `64` is still, above is clockwise, below is anticlockwise |
| `absolute` | a position, not a delta — direction comes from the previous value |

**`encoding` also applies to a `move` binding**, and a jog wheel needs it. A
platter reports how far or how fast it just moved, centred on 64 — not where it
is. Read as a position, 64 out of 127 is 0.504 of the way up a fader, which
lands a hair above zero and drives the deck forwards with nobody touching the
platter; over a set that is a track sliding out of time on its own. With an
`encoding`, the centre is anchored exactly and each direction is scaled to its
own end, so `min` and `max` may differ if one direction should be weightier.

Writing an `encoding` on a fader would be a mistake in the other direction: a
fader has a position, and there is nothing to centre.

An earlier version of this code read the byte and fell back to comparing with
the last value. That is not a shortcut, it is a bug: `30` is thirty clicks
clockwise to a signed encoder and a position below centre to an absolute one,
so turning an absolute encoder *down* from 60 to 30 produced a beat jump
*forward*. Your controller's manual says which it sends.

### Why 14-bit for the pitch fader

A 7-bit control gives 128 steps across ±8%, which is 0.125% per step —
audibly coarse when beatmatching by ear. Controllers that have a good pitch
fader send it on the pitch wheel; use `bend`.

## Where the sound comes out

A controller with a built-in soundcard has a fixed arrangement of sockets, and
its manual says what it is. djmanzo otherwise works it out from the channel
count — master on outputs 1-2, headphones on 3-4, booth on 5-6 — which is
right for most devices and wrong for the ones that do it differently.

Wrong here has one specific meaning: **the room hears what you are cueing.**

So a mapping may state its own arrangement, in the same file as the pads,
because they are the same fact about the same piece of hardware:

```toml
[audio]
device = "DDJ"      # part of the soundcard's name, matched loosely
master = [2, 3]     # outputs 3-4
cue = [0, 1]        # outputs 1-2
booth = [4, 5]      # optional
```

Channels are counted from zero, as the audio buffer indexes them; the panel
shows them from one, as the sockets are labelled.

Three things are checked when the file loads, not when the crowd notices:

- every bus is a **pair** — a mono master is not something a mapping can ask
  for;
- **no two buses share a channel**, which is the rule the whole section exists
  for;
- and when a device is open, whether it has enough outputs. A mapping that
  asks for six on a device with two is reported in the Controllers panel and
  the usual arrangement is used instead — never a silent half-application.

The arrangement is applied when the controller is connected and re-applied
after every audio device change, because opening a device builds a fresh audio
engine that knows nothing about what is plugged in.

## Where mappings live

Bundled mappings are compiled into the application, so a fresh install on a
machine with nothing configured already has a working keyboard and a starting
point for a generic controller.

Your own go in `mappings/` inside the config directory:

| Platform | Path |
|---|---|
| macOS | `~/Library/Application Support/djmanzo/mappings/` |
| Linux | `~/.config/djmanzo/mappings/` |

A file whose `name` matches a bundled one **replaces** it rather than sitting
beside it, so editing a shipped mapping works the way editing a file should.
A keyboard file and a controller file are told apart by what is in them
(`[[key]]` versus `[[binding]]`), not by where they are — name yours whatever
you like.

One broken file is reported and the others still load. A mapping directory is
hand-edited; a typo in one file is normal and should not take the rest down.

## HID

A MIDI controller announces itself: press a pad and a note number arrives, and
that number is the same on every copy of that model. **A HID report is
anonymous bytes.** Where a control lives in them is decided by whoever wrote
the firmware, and nothing in the packet says what just changed.

There is one reason to put up with that, and it is enough: **resolution.** A
7-bit MIDI control gives 128 steps across a jog wheel's whole travel. Sixteen
bits give 65,536. That is the difference between a wheel that scratches and one
that steps.

### The syntax

```toml
on = "hid 1 bit 3.2"      # report 1, byte 3, bit 2 — a button or a switch
on = "hid 1 byte 5"       # an 8-bit fader or knob
on = "hid 1 word 6"       # 16 bits, high byte first
on = "hid 1 word-le 6"    # 16 bits, low byte first
```

The report ID is the first number. Devices that number their reports put it in
the first byte of every packet; devices that do not use `0`, and then the whole
packet is payload. Byte offsets count from the start of the payload, **after**
any report ID — which is how a device's own manual numbers them.

Endianness is declared rather than guessed, for the same reason the encoder
convention is: the two orderings give completely different numbers from the
same two bytes, and a jog wheel read the wrong way round jumps between its
halves instead of turning. If yours does that, change `word-le` to `word`.

### Learning a control

Byte offsets are unwritable by hand for an undocumented controller, so the
editor works them out. It watches two consecutive reports and names the field
that moved:

- one bit → a button;
- several bits of one byte → a fader;
- two adjacent bytes → the 16-bit control HID exists for;
- **three or more bytes → nothing.** That is a DJ brushing two controls on the
  way to the right one, and binding either would be a guess.

Both transports listen while learning, so pressing a pad works without first
knowing whether your controller speaks MIDI or HID — a question about a USB
descriptor, not about music.

### Two things a HID binding cannot be

Refused when the file loads, not discovered mid-set:

- **An encoder.** `turn_up` and `turn_down` describe an event; a HID field is a
  level. There is no honest way to read one from the other.
- **A platter finer than its field.** A 3,600-step platter read out of one byte
  would wrap fourteen times a revolution. Use `word` or `word-le`.

### Lights

A `[[feedback]]` line is three MIDI bytes. Lighting a HID device means writing
an output report of that device's own shape — a different thing, and not one a
feedback line can express. A HID mapping therefore has no lights, and that is
correct rather than missing.

### Permissions on Linux

A controller's HID device node usually belongs to `root` until a udev rule says
otherwise. If the Controllers panel lists your device but refuses to open it,
that is why, and it is fixed with a rule rather than by running djmanzo as
root.

## Lua, where a table cannot describe a control

Every mapping here is a table: this control does that action. That covers
almost everything — and it cannot express **"this pad does one thing normally
and another while shift is held"**, because that is a *decision*, and a
decision needs an `if`.

So a mapping may carry a script. Marked controls go to it; everything else
stays a table entry, so a mapping is not all-or-nothing:

```toml
script = """
local shifted = false

function on_control(control, event, value)
  if control == "note 1 0x3f" then
    shifted = (event == "press")
    return nil
  end
  if event ~= "press" then return nil end
  if shifted then return "deck 1 hotcue_set 1" end
  return "deck 1 hotcue 1"
end
"""

[[binding]]
on = "note 1 0x3f"
script = true

[[binding]]
on = "note 1 0x01"
script = true

# Still an ordinary table entry.
[[binding]]
on = "cc 1 0x08"
move = "crossfader {value}"
min = -1.0
max = 1.0
```

`on_control` receives the binding's own `on` text — so a script can tell its
pads apart — the event (`press`, `release` or `move`) and, for a move, the
value scaled by the binding's own `min`/`max`. It returns nothing, one action,
or a list.

`parameter("deck.1.playing")` reads any parameter by the same stable name the
interface and the network API use, so a script can decide on what the engine is
actually doing. An unknown name is `nil`, not an error.

`crates/dj-hid/mappings/scripted-shift.toml` is a working example, and a test
presses its pads.

### What a script may not do

**A mapping cannot do anything the interface cannot.** That is what makes a
file from a stranger safe to open, and a scripting language is exactly how a
promise like that gets lost. So:

- **Nothing reaches outside the process.** No `io`, no `os`, no `require`, no
  `dofile`, no `load`. `math`, `string` and `table` are there, because a
  mapping computes numbers and builds action text.
- **Actions still go through the parser.** A script returns *text*, and that
  text is checked against the vocabulary exactly as a table entry's is. There
  is no path from Lua to the engine that skips it.
- **It is stopped after a hundred thousand instructions.** A script runs on the
  MIDI thread, where a `while true do end` would take the controller with it.
  The budget is per control event, so a script stopped once still works on the
  next press.

One thing worth knowing if you are reading the source: asking Lua for *no*
standard library is not enough. mlua installs the base library regardless, so
`dofile` and `loadfile` were reachable until they were taken away by name. A
test enumerates them rather than trusting the flag.

## What is not here yet

- Outbound feedback to HID devices — see the note above.

Done since this list was first written: outbound feedback to MIDI (LEDs, pad
colours, ring lights), motorised platters that report an angle rather than a
delta, the in-app learn mode, the audio section above, HID, and Lua.

See [ROADMAP.md](ROADMAP.md#m4--controllers).
