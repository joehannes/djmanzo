/**
 * What the small cays out at sea hold: one thing worth knowing each. Technique
 * from djmanzo's own catalogue of moves, the culture behind the genres, and
 * karaoke hosting. The sea is endless and so, by repetition, is this list —
 * the same cay always holds the same note.
 */

export interface Lore {
  kind: "technique" | "culture" | "karaoke" | "djmanzo";
  title: string;
  body: string;
}

export const LORE: Lore[] = [
  { kind: "technique", title: "Mix on the phrase", body: "Most dance records move in phrases of 16 or 32 beats. Start a mix on a phrase boundary and the two records change together, the way a band would. djmanzo marks the phrases on the waveform." },
  { kind: "technique", title: "The bass swap", body: "Two kick drums and two bass lines at once is mud. Take the bass out of one record as you bring the other's in — on the phrase, in one move." },
  { kind: "technique", title: "Echo out", body: "To leave a record without a blend, throw an echo on its last beat and pull the fader. The repeats carry the room into the silence where the next record lands." },
  { kind: "technique", title: "Harmonic mixing", body: "Records in compatible keys blend without clashing. On the Camelot wheel, move one step around or switch between the inner and outer ring. djmanzo shows the key on every record." },
  { kind: "technique", title: "The cut", body: "Not every mix needs a blend. On the downbeat, slam the crossfader across. In dembow, reggaeton and hip-hop the cut is the move." },
  { kind: "technique", title: "Loop the outro", body: "Running out of record before the next one is ready? Loop four or eight bars of the outro and take your time. djmanzo's loop roll keeps the record moving underneath." },
  { kind: "technique", title: "Filter sweep", body: "Sweep a low-pass filter down on the outgoing record while the new one comes in: the old one drifts away rather than stopping." },
  { kind: "technique", title: "Read the floor, not the screen", body: "The screen tells you what is playing; the floor tells you what to play next. djmanzo's Next rail suggests — the room decides." },
  { kind: "technique", title: "Warm-up is a job", body: "The warm-up DJ builds the room without spending the night's energy. Lower tempo, fewer anthems, a mood rather than a peak." },
  { kind: "technique", title: "Stems: the acapella trick", body: "Lift the vocal of one record over the instrumental of another. With djmanzo's stems you can swap a vocal across decks live." },
  { kind: "technique", title: "Beatmatch by ear once", body: "Sync is a gift, but learn to match by ear once: nudge, listen for the flam, nudge again. Then you can hear when sync is wrong." },
  { kind: "technique", title: "Leave headroom", body: "Keep the master out of the red. Loud and distorted is not loud. djmanzo's limiter catches peaks; it is not a volume knob." },
  { kind: "culture", title: "Why bachata was 'música de amargue'", body: "For decades Dominican radio would not play bachata: it was the music of the colmado and the bar, of heartbreak — amargue, bitterness. The bars made it famous anyway." },
  { kind: "culture", title: "The güira", body: "Merengue's metal scraper: a cylinder of punched metal played with a brush of wire. It keeps the whole band in time." },
  { kind: "culture", title: "The dembow riddim", body: "Dembow is named after Shabba Ranks' 1990 'Dem Bow' and its riddim, which became the backbone of reggaeton and of Dominican dembow." },
  { kind: "culture", title: "The picó", body: "On Colombia's Caribbean coast, a picó is a giant hand-painted sound system — artwork and amplifier at once, competing street by street." },
  { kind: "culture", title: "Sound system culture", body: "Kingston in the 1950s: a selector, a toaster on the mic and a stack of speakers made the dance. Hip-hop, jungle and dancehall all grew from it." },
  { kind: "culture", title: "The Warehouse", body: "House music is named after the Warehouse in Chicago, where Frankie Knuckles played from 1977. Record shops sold 'the music they play at the Warehouse'." },
  { kind: "culture", title: "Detroit to Berlin", body: "Techno came from Detroit's Belleville Three; after the Wall fell, Berlin's empty buildings gave it rooms to grow in." },
  { kind: "culture", title: "The Amen break", body: "Six seconds of drumming from the Winstons' 1969 B-side became the most sampled loop in history and the skeleton of jungle." },
  { kind: "culture", title: "Kool Herc's merry-go-round", body: "In 1973 in the Bronx, Kool Herc played the same break on two copies of a record, back to back, so dancers could keep going. That loop became hip-hop." },
  { kind: "culture", title: "Sonideros", body: "In Mexico's cumbia scene the sonidero DJ sends saludos over the music to families, barrios and people far away — the shout-outs are part of the music." },
  { kind: "culture", title: "Konpa dirèk", body: "In 1955 Nemours Jean-Baptiste gave Haiti konpa dirèk: a steady, danceable groove that became the sound of Haitian parties everywhere." },
  { kind: "culture", title: "Amapiano's log drum", body: "The deep, gliding 'log drum' bass of South Africa's amapiano is a synth patch — and the sound a whole generation dances to." },
  { kind: "culture", title: "Disco's mirror ball", body: "Disco records were the first made to be mixed: long intros, extended mixes, twelve-inch singles cut loud for club sound systems." },
  { kind: "karaoke", title: "Start with a brave one", body: "The first singer sets the night. Ask a friend who loves it to go first, and the shy ones will follow." },
  { kind: "karaoke", title: "Key for the singer", body: "Most people sing better a few semitones down from the record. djmanzo shifts the key without changing the tempo." },
  { kind: "karaoke", title: "Break music matters", body: "Between singers, the room still needs music. djmanzo plays a playlist between performances and fades it under the next singer." },
  { kind: "karaoke", title: "Rotation fairness", body: "One song each, in turn, and the regulars wait like everybody else. djmanzo's rotation keeps the order and says who is next." },
  { kind: "karaoke", title: "The voice knob", body: "Take the original singer out of any record you own — stems separate the vocal — and leave a guide voice in for a song someone barely knows." },
  { kind: "karaoke", title: "Let the room help", body: "Guests can put their names down from their phones, and send emoji to the singers' screen. Nobody needs an app." },
  { kind: "djmanzo", title: "It speaks only in actions", body: "djmanzo's assistant acts in actions you could have pressed: it stages a mix, a record or a loop, and you accept, change or reject it. Only what cannot wait is whispered." },
  { kind: "djmanzo", title: "Take it back with a touch", body: "Whatever the assistant is doing, touching a control takes it back. SAFE takes everything back at once." },
  { kind: "djmanzo", title: "A laptop is an instrument", body: "With nothing plugged in, the keyboard is a full controller with the same vocabulary as a MIDI mapping." },
  { kind: "djmanzo", title: "Your set is data", body: "Every action is timestamped, so a set can be replayed, re-rendered at studio quality, or compared with last week's." },
  { kind: "djmanzo", title: "It finds a song you hum", body: "Can't remember the name? Hum it, type a half-remembered line, or describe it. djmanzo searches your own collection." },
  { kind: "djmanzo", title: "No lock-in", body: "djmanzo imports rekordbox, Serato, Traktor and iTunes collections with their cues and grids." },
];
