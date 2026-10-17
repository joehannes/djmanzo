/**
 * The island's sound: two decks of synthesised loops, a mixer and the effects.
 *
 * Nothing here is a sample. Every kick, güira scrape and requinto pluck is
 * synthesised in the Web Audio graph from the genre's pattern in
 * `genres.ts`, which keeps the site free of anybody's recordings and lets a
 * loop follow any tempo. It is a sketch of a genre's groove, not its sound —
 * the real thing is the visitor's own records in djmanzo.
 *
 * Scheduling follows the usual look-ahead pattern: a timer every 25 ms
 * schedules the notes that fall in the next 120 ms on the audio clock, so
 * timing does not depend on the main thread.
 */

import { GENRE_BY_ID, scaleNotes, type Genre } from "../genres";

export type FxName = "echo" | "reverb" | "filter" | "flanger" | "crush" | "gate";

const PROGRESSIONS: Record<Genre["scale"], number[]> = {
  minor: [0, 8, 3, 10],
  major: [0, 7, 9, 5],
  dorian: [0, 5, 0, 7],
};

function midiRatio(semitones: number): number {
  return Math.pow(2, semitones / 12);
}

class Deck {
  genre: Genre;
  playing = false;
  readonly input: GainNode;
  readonly low: BiquadFilterNode;
  readonly mid: BiquadFilterNode;
  readonly high: BiquadFilterNode;
  readonly fader: GainNode;
  readonly xfade: GainNode;
  readonly analyser: AnalyserNode;
  step = 0;
  nextTime = 0;
  bar = 0;

  constructor(
    private engine: Engine,
    genreId: string,
  ) {
    const ctx = engine.ctx;
    this.genre = GENRE_BY_ID[genreId] ?? GENRE_BY_ID.house;
    this.input = ctx.createGain();
    this.low = ctx.createBiquadFilter();
    this.low.type = "lowshelf";
    this.low.frequency.value = 220;
    this.mid = ctx.createBiquadFilter();
    this.mid.type = "peaking";
    this.mid.frequency.value = 1200;
    this.mid.Q.value = 0.8;
    this.high = ctx.createBiquadFilter();
    this.high.type = "highshelf";
    this.high.frequency.value = 4000;
    this.fader = ctx.createGain();
    this.fader.gain.value = 0.9;
    this.xfade = ctx.createGain();
    this.analyser = ctx.createAnalyser();
    this.analyser.fftSize = 512;
    this.input.connect(this.low).connect(this.mid).connect(this.high).connect(this.fader).connect(this.xfade);
    this.fader.connect(this.analyser);
    this.xfade.connect(engine.bus);
  }

  setGenre(id: string) {
    const g = GENRE_BY_ID[id];
    if (g) this.genre = g;
  }

  /** EQ in dB, −26 (a kill) to +6. */
  eq(band: "low" | "mid" | "high", db: number) {
    this[band].gain.setTargetAtTime(db, this.engine.ctx.currentTime, 0.02);
  }

  start() {
    if (this.playing) return;
    this.playing = true;
    this.step = 0;
    this.bar = 0;
    this.nextTime = this.engine.alignedStart();
  }

  stop() {
    this.playing = false;
  }

  get bpm(): number {
    return this.engine.sync ? this.engine.tempo : (this.genre.bpm[0] + this.genre.bpm[1]) / 2;
  }

  get stepDur(): number {
    // Patterns are written in sixteenths of a bar of four beats.
    return 60 / this.bpm / 4;
  }

  schedule(until: number) {
    const p = this.genre.pattern;
    while (this.playing && this.nextTime < until) {
      const s = this.step % p.steps;
      const swing = s % 2 === 1 ? p.swing * this.stepDur : 0;
      const t = this.nextTime + swing;
      this.play(s, t);
      this.engine.onStep(this, s, t);
      this.nextTime += this.stepDur;
      this.step++;
      if (this.step % p.steps === 0) this.bar++;
    }
  }

  private play(s: number, t: number) {
    const g = this.genre;
    const p = g.pattern;
    const v = this.engine.voices;
    const out = this.input;
    const prog = PROGRESSIONS[g.scale];
    const barOffset = prog[Math.floor(this.step / p.steps) % prog.length] + this.engine.transpose;
    if (p.kick.includes(s)) v.kick(t, out);
    if (p.snare.includes(s)) v.snare(t, out);
    if (p.hat.includes(s)) v.hat(t, out, s % 4 === 2 ? 0.5 : 0.3);
    if (p.perc.includes(s)) v.perc(p.percVoice, t, out);
    const bi = p.bass.indexOf(s);
    if (bi >= 0) {
      const note = p.bassNotes[bi % p.bassNotes.length] + barOffset;
      v.bass(t, out, (g.root / 2) * midiRatio(note), this.stepDur * 2);
    }
    const ci = p.chordSteps.indexOf(s);
    if (ci >= 0) {
      const root = g.root * midiRatio(barOffset);
      if (p.chordVoice === "pluck") {
        // An arpeggio, the way a requinto picks through the chord.
        const n = p.chord[ci % p.chord.length];
        v.pluck(t, out, root * midiRatio(n + (ci % 8 >= 4 ? 12 : 0)));
      } else {
        v.chord(p.chordVoice, t, out, p.chord.map((n) => root * midiRatio(n)), this.stepDur * (p.chordVoice === "pad" ? 8 : 2));
      }
    }
  }
}

/** The instruments. Each schedules one note at `t` into `out`. */
class Voices {
  constructor(private ctx: AudioContext, private noise: AudioBuffer) {}

  private env(t: number, peak: number, decay: number, attack = 0.002): GainNode {
    const g = this.ctx.createGain();
    g.gain.setValueAtTime(0.0001, t);
    g.gain.exponentialRampToValueAtTime(peak, t + attack);
    g.gain.exponentialRampToValueAtTime(0.0001, t + attack + decay);
    return g;
  }

  private noiseSource(t: number, dur: number): AudioBufferSourceNode {
    const s = this.ctx.createBufferSource();
    s.buffer = this.noise;
    s.start(t, Math.random() * 0.5, dur + 0.05);
    return s;
  }

  kick(t: number, out: AudioNode) {
    const o = this.ctx.createOscillator();
    o.frequency.setValueAtTime(150, t);
    o.frequency.exponentialRampToValueAtTime(45, t + 0.12);
    const g = this.env(t, 0.9, 0.38);
    o.connect(g).connect(out);
    o.start(t);
    o.stop(t + 0.45);
  }

  snare(t: number, out: AudioNode) {
    const n = this.noiseSource(t, 0.2);
    const f = this.ctx.createBiquadFilter();
    f.type = "bandpass";
    f.frequency.value = 1900;
    f.Q.value = 0.7;
    n.connect(f).connect(this.env(t, 0.45, 0.17)).connect(out);
    const o = this.ctx.createOscillator();
    o.type = "triangle";
    o.frequency.value = 185;
    o.connect(this.env(t, 0.3, 0.08)).connect(out);
    o.start(t);
    o.stop(t + 0.15);
  }

  hat(t: number, out: AudioNode, level = 0.3) {
    const n = this.noiseSource(t, 0.06);
    const f = this.ctx.createBiquadFilter();
    f.type = "highpass";
    f.frequency.value = 7500;
    n.connect(f).connect(this.env(t, level * 0.5, 0.045)).connect(out);
  }

  perc(voice: Genre["pattern"]["percVoice"], t: number, out: AudioNode) {
    const c = this.ctx;
    const tone = (freq: number, to: number, decay: number, type: OscillatorType = "sine", level = 0.4) => {
      const o = c.createOscillator();
      o.type = type;
      o.frequency.setValueAtTime(freq, t);
      o.frequency.exponentialRampToValueAtTime(to, t + decay);
      o.connect(this.env(t, level, decay)).connect(out);
      o.start(t);
      o.stop(t + decay + 0.05);
    };
    const hiss = (freq: number, q: number, decay: number, level: number, type: BiquadFilterType = "bandpass") => {
      const n = this.noiseSource(t, decay);
      const f = c.createBiquadFilter();
      f.type = type;
      f.frequency.value = freq;
      f.Q.value = q;
      n.connect(f).connect(this.env(t, level, decay, 0.008)).connect(out);
    };
    switch (voice) {
      case "guira":
        // The scrape of a metal güira: a bright, slightly long hiss.
        hiss(6000, 1.5, 0.07, 0.35);
        break;
      case "shaker":
        hiss(5500, 0.8, 0.06, 0.22, "highpass");
        break;
      case "bongo":
        tone(420, 330, 0.12, "sine", 0.35);
        break;
      case "conga":
        tone(230, 190, 0.22, "sine", 0.4);
        break;
      case "log":
        // Amapiano's log drum: a low, gliding, slightly overdriven boom.
        tone(110, 55, 0.45, "sine", 0.6);
        break;
      case "rim":
        tone(1700, 1600, 0.03, "square", 0.12);
        break;
      case "clave":
        tone(2400, 2300, 0.06, "triangle", 0.35);
        break;
      case "cowbell":
        tone(560, 550, 0.2, "square", 0.08);
        tone(840, 830, 0.2, "square", 0.06);
        break;
    }
  }

  bass(t: number, out: AudioNode, freq: number, dur: number) {
    const c = this.ctx;
    const o = c.createOscillator();
    o.type = "sawtooth";
    o.frequency.value = freq;
    const f = c.createBiquadFilter();
    f.type = "lowpass";
    f.frequency.setValueAtTime(900, t);
    f.frequency.exponentialRampToValueAtTime(180, t + dur);
    o.connect(f).connect(this.env(t, 0.32, dur, 0.005)).connect(out);
    o.start(t);
    o.stop(t + dur + 0.05);
  }

  pluck(t: number, out: AudioNode, freq: number) {
    const c = this.ctx;
    const o = c.createOscillator();
    o.type = "triangle";
    o.frequency.value = freq;
    const o2 = c.createOscillator();
    o2.type = "sine";
    o2.frequency.value = freq * 2;
    const g = this.env(t, 0.16, 0.28);
    o.connect(g);
    o2.connect(this.env(t, 0.05, 0.12)).connect(out);
    g.connect(out);
    o.start(t);
    o2.start(t);
    o.stop(t + 0.35);
    o2.stop(t + 0.2);
  }

  chord(voice: Genre["pattern"]["chordVoice"], t: number, out: AudioNode, freqs: number[], dur: number) {
    const c = this.ctx;
    const level = { piano: 0.09, pad: 0.05, stab: 0.07, organ: 0.06, pluck: 0.1 }[voice] ?? 0.06;
    const filter = c.createBiquadFilter();
    filter.type = "lowpass";
    filter.frequency.value = voice === "pad" ? 1400 : voice === "stab" ? 2200 : 3500;
    const g = c.createGain();
    const attack = voice === "pad" ? 0.08 : 0.004;
    const decay = voice === "pad" ? dur : voice === "organ" ? dur * 0.8 : voice === "piano" ? 0.6 : 0.16;
    g.gain.setValueAtTime(0.0001, t);
    g.gain.exponentialRampToValueAtTime(level, t + attack);
    g.gain.exponentialRampToValueAtTime(0.0001, t + attack + decay);
    filter.connect(g).connect(out);
    for (const f of freqs) {
      const types: OscillatorType[] = voice === "pad" ? ["sawtooth", "sawtooth"] : voice === "stab" ? ["square"] : voice === "organ" ? ["sine", "sine"] : ["triangle", "sine"];
      types.forEach((type, i) => {
        const o = c.createOscillator();
        o.type = type;
        o.frequency.value = voice === "organ" && i === 1 ? f * 2 : f;
        if (voice === "pad") o.detune.value = i === 0 ? -8 : 8;
        o.connect(filter);
        o.start(t);
        o.stop(t + attack + decay + 0.05);
      });
    }
  }
}

/**
 * A singing voice: a sawtooth through two formant filters, sliding between
 * vowels. It is the karaoke stage's guide vocal, the thing the voice knob
 * takes out.
 */
export class Singer {
  readonly out: GainNode;
  private osc: OscillatorNode | null = null;
  private f1: BiquadFilterNode;
  private f2: BiquadFilterNode;
  private amp: GainNode;
  private vib: OscillatorNode | null = null;

  constructor(private engine: Engine) {
    const c = engine.ctx;
    this.out = c.createGain();
    this.out.gain.value = 0.6;
    this.amp = c.createGain();
    this.amp.gain.value = 0;
    this.f1 = c.createBiquadFilter();
    this.f1.type = "bandpass";
    this.f1.Q.value = 6;
    this.f2 = c.createBiquadFilter();
    this.f2.type = "bandpass";
    this.f2.Q.value = 8;
    this.amp.connect(this.f1).connect(this.out);
    this.amp.connect(this.f2).connect(this.out);
    this.out.connect(engine.bus);
  }

  private ensure() {
    if (this.osc) return;
    const c = this.engine.ctx;
    this.osc = c.createOscillator();
    this.osc.type = "sawtooth";
    this.vib = c.createOscillator();
    this.vib.frequency.value = 5.5;
    const depth = c.createGain();
    depth.gain.value = 4;
    this.vib.connect(depth).connect(this.osc.frequency);
    this.osc.connect(this.amp);
    this.osc.start();
    this.vib.start();
  }

  /** Sing `freq` on a vowel from `t` for `dur` seconds. */
  note(t: number, freq: number, dur: number, vowel: "a" | "e" | "i" | "o" | "u") {
    this.ensure();
    const F: Record<string, [number, number]> = { a: [800, 1150], e: [400, 1700], i: [300, 2200], o: [450, 800], u: [325, 700] };
    const [a, b] = F[vowel];
    this.osc!.frequency.setTargetAtTime(freq, t, 0.03);
    this.f1.frequency.setTargetAtTime(a, t, 0.03);
    this.f2.frequency.setTargetAtTime(b, t, 0.03);
    this.amp.gain.setTargetAtTime(0.5, t, 0.02);
    this.amp.gain.setTargetAtTime(0, t + dur * 0.92, 0.03);
  }

  level(v: number) {
    this.out.gain.setTargetAtTime(v, this.engine.ctx.currentTime, 0.05);
  }

  silence() {
    this.amp.gain.setTargetAtTime(0, this.engine.ctx.currentTime, 0.02);
  }
}

type StepListener = (deck: 0 | 1, step: number, time: number, genre: Genre) => void;

export class Engine {
  readonly ctx: AudioContext;
  /** Where decks and voices meet before the effects. */
  readonly bus: GainNode;
  readonly master: GainNode;
  readonly analyser: AnalyserNode;
  readonly voices: Voices;
  readonly decks: [Deck, Deck];
  tempo = 124;
  sync = true;
  /** Key shift in semitones for everything the decks play — the karaoke key knob. */
  transpose = 0;
  crossfader = 0.5;
  private filterNode: BiquadFilterNode;
  private fx: Record<Exclude<FxName, "filter" | "gate">, GainNode>;
  private echoDelay: DelayNode;
  private flangeDelay: DelayNode;
  private gateNode: GainNode;
  private gateOn = false;
  private timer: number | null = null;
  private listeners = new Set<StepListener>();
  private freq = new Uint8Array(256);

  constructor() {
    const Ctx = window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
    this.ctx = new Ctx();
    const c = this.ctx;
    const noise = c.createBuffer(1, c.sampleRate * 2, c.sampleRate);
    const data = noise.getChannelData(0);
    for (let i = 0; i < data.length; i++) data[i] = Math.random() * 2 - 1;
    this.voices = new Voices(c, noise);

    this.bus = c.createGain();
    this.filterNode = c.createBiquadFilter();
    this.filterNode.type = "allpass";
    this.gateNode = c.createGain();
    const dry = c.createGain();
    this.master = c.createGain();
    this.master.gain.value = 0.5;
    const limiter = c.createDynamicsCompressor();
    limiter.threshold.value = -6;
    limiter.ratio.value = 12;
    this.analyser = c.createAnalyser();
    this.analyser.fftSize = 512;
    this.bus.connect(this.filterNode).connect(this.gateNode).connect(dry).connect(this.master);

    // Echo: a dotted-eighth delay with feedback.
    this.echoDelay = c.createDelay(2);
    const echoFb = c.createGain();
    echoFb.gain.value = 0.45;
    const echoWet = c.createGain();
    echoWet.gain.value = 0;
    this.gateNode.connect(echoWet).connect(this.echoDelay).connect(this.master);
    this.echoDelay.connect(echoFb).connect(this.echoDelay);

    // Reverb: a convolver over a decaying noise impulse.
    const conv = c.createConvolver();
    const len = c.sampleRate * 2.6;
    const ir = c.createBuffer(2, len, c.sampleRate);
    for (let ch = 0; ch < 2; ch++) {
      const d = ir.getChannelData(ch);
      for (let i = 0; i < len; i++) d[i] = (Math.random() * 2 - 1) * Math.pow(1 - i / len, 2.8);
    }
    conv.buffer = ir;
    const revWet = c.createGain();
    revWet.gain.value = 0;
    this.gateNode.connect(revWet).connect(conv).connect(this.master);

    // Flanger: a short modulated delay with feedback.
    this.flangeDelay = c.createDelay(0.02);
    this.flangeDelay.delayTime.value = 0.004;
    const lfo = c.createOscillator();
    lfo.frequency.value = 0.22;
    const lfoDepth = c.createGain();
    lfoDepth.gain.value = 0.0025;
    lfo.connect(lfoDepth).connect(this.flangeDelay.delayTime);
    lfo.start();
    const flWet = c.createGain();
    flWet.gain.value = 0;
    const flFb = c.createGain();
    flFb.gain.value = 0.6;
    this.gateNode.connect(flWet).connect(this.flangeDelay).connect(this.master);
    this.flangeDelay.connect(flFb).connect(this.flangeDelay);

    // Crush: a stepped wave-shaper, crossfaded against the dry signal.
    const shaper = c.createWaveShaper();
    const curve = new Float32Array(4096);
    for (let i = 0; i < curve.length; i++) {
      const x = (i / curve.length) * 2 - 1;
      curve[i] = Math.round(x * 6) / 6;
    }
    shaper.curve = curve;
    const crWet = c.createGain();
    crWet.gain.value = 0;
    this.gateNode.connect(crWet).connect(shaper).connect(this.master);

    this.fx = { echo: echoWet, reverb: revWet, flanger: flWet, crush: crWet };
    this.master.connect(limiter).connect(this.analyser).connect(c.destination);

    this.decks = [new Deck(this, "house"), new Deck(this, "bachata")];
    this.setCrossfader(0.5);
  }

  /** Start the clock if it is not running. Must follow a user gesture. */
  async wake() {
    if (this.ctx.state !== "running") await this.ctx.resume();
    if (this.timer === null) {
      this.timer = window.setInterval(() => {
        const until = this.ctx.currentTime + 0.12;
        for (const d of this.decks) d.schedule(until);
        if (this.gateOn) this.scheduleGate(until);
      }, 25);
    }
  }

  /** The next beat boundary, so a deck started mid-bar lands in phase with the other. */
  alignedStart(): number {
    const now = this.ctx.currentTime + 0.05;
    const other = this.decks.find((d) => d.playing);
    if (!other || !this.sync) return now;
    const beat = 60 / this.tempo;
    const since = (now - other.nextTime) / beat;
    return other.nextTime + Math.ceil(since) * beat;
  }

  /** Equal-power crossfade: neither side dips in the middle. */
  setCrossfader(x: number) {
    this.crossfader = Math.max(0, Math.min(1, x));
    const t = this.ctx.currentTime;
    this.decks[0].xfade.gain.setTargetAtTime(Math.cos((this.crossfader * Math.PI) / 2), t, 0.015);
    this.decks[1].xfade.gain.setTargetAtTime(Math.sin((this.crossfader * Math.PI) / 2), t, 0.015);
  }

  setTempo(bpm: number) {
    this.tempo = Math.max(60, Math.min(180, bpm));
    this.echoDelay.delayTime.setTargetAtTime((60 / this.tempo) * 0.75, this.ctx.currentTime, 0.05);
  }

  /** −1 (low-pass, closed) through 0 (open) to +1 (high-pass). */
  setFilter(v: number) {
    const f = this.filterNode;
    const t = this.ctx.currentTime;
    if (Math.abs(v) < 0.03) {
      f.type = "allpass";
      return;
    }
    if (v < 0) {
      f.type = "lowpass";
      f.frequency.setTargetAtTime(20000 * Math.pow(0.008, -v), t, 0.03);
      f.Q.value = 6;
    } else {
      f.type = "highpass";
      f.frequency.setTargetAtTime(30 * Math.pow(120, v), t, 0.03);
      f.Q.value = 6;
    }
  }

  setFx(name: FxName, amount: number) {
    if (name === "filter") return this.setFilter(amount);
    if (name === "gate") {
      this.gateOn = amount > 0.01;
      if (!this.gateOn) this.gateNode.gain.setTargetAtTime(1, this.ctx.currentTime, 0.01);
      return;
    }
    const levels = { echo: 0.55, reverb: 0.9, flanger: 0.7, crush: 0.6 };
    this.fx[name].gain.setTargetAtTime(amount * levels[name], this.ctx.currentTime, 0.04);
  }

  private gateNext = 0;
  private scheduleGate(until: number) {
    const sixteenth = 60 / this.tempo / 4;
    if (this.gateNext < this.ctx.currentTime) this.gateNext = this.ctx.currentTime;
    while (this.gateNext < until) {
      this.gateNode.gain.setValueAtTime(1, this.gateNext);
      this.gateNode.gain.setValueAtTime(0.05, this.gateNext + sixteenth * 0.5);
      this.gateNext += sixteenth;
    }
  }

  setVolume(v: number) {
    this.master.gain.setTargetAtTime(v, this.ctx.currentTime, 0.05);
  }

  onStep(deck: Deck, step: number, time: number) {
    const i = deck === this.decks[0] ? 0 : 1;
    for (const l of this.listeners) l(i as 0 | 1, step, time, deck.genre);
  }

  listen(l: StepListener): () => void {
    this.listeners.add(l);
    return () => this.listeners.delete(l);
  }

  /** Energy in three bands, 0–1, for anything that wants to move with the music. */
  levels(analyser: AnalyserNode = this.analyser): { low: number; mid: number; high: number; all: number } {
    if (this.freq.length !== analyser.frequencyBinCount) this.freq = new Uint8Array(analyser.frequencyBinCount);
    analyser.getByteFrequencyData(this.freq);
    const n = this.freq.length;
    const avg = (a: number, b: number) => {
      let s = 0;
      for (let i = a; i < b; i++) s += this.freq[i];
      return s / Math.max(1, b - a) / 255;
    };
    const low = avg(1, Math.floor(n * 0.04));
    const mid = avg(Math.floor(n * 0.04), Math.floor(n * 0.3));
    const high = avg(Math.floor(n * 0.3), n);
    return { low, mid, high, all: (low + mid + high) / 3 };
  }

  get anyPlaying(): boolean {
    return this.decks.some((d) => d.playing);
  }

  scaleOf(deck: 0 | 1): number[] {
    return scaleNotes(this.decks[deck].genre.scale);
  }
}

let engine: Engine | null = null;

/** The one engine, made on first use — which must be inside a click or a key press. */
export function getEngine(): Engine {
  if (!engine) engine = new Engine();
  return engine;
}

export function hasEngine(): boolean {
  return engine !== null;
}
