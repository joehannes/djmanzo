"""Synthesise original demo tracks for screenshots: kick, hats, bass, chords,
arrangement with intro / breakdown / drop. Writes WAV; ffmpeg tags them."""
import numpy as np, wave, sys, json, os
SR = 44100
rng = np.random.default_rng(7)
NOTE = {n:i for i,n in enumerate("C C# D D# E F F# G G# A A# B".split())}
def hz(midi): return 440*2**((midi-69)/12)
def env(n, a=0.005, d=0.2, s=0.0, r=0.05):
    t = np.arange(n)/SR
    e = np.where(t<a, t/a, np.exp(-(t-a)/max(d,1e-4))*(1-s)+s)
    return e
def kick(n):
    t=np.arange(n)/SR; f=50+120*np.exp(-t*35)
    return np.sin(2*np.pi*np.cumsum(f)/SR)*np.exp(-t*7)
def hat(n):
    return rng.standard_normal(n)*np.exp(-np.arange(n)/SR*60)*0.25
def clap(n):
    return rng.standard_normal(n)*np.exp(-np.arange(n)/SR*22)*0.35
def saw(f, n):
    t=np.arange(n)/SR; ph=(t*f)%1.0
    return 2*ph-1
def lowpass(x, a):
    y=np.empty_like(x); acc=0.0
    # one-pole, vectorised via cumulative approach is hard; do in chunks
    for i in range(len(x)):
        acc += a*(x[i]-acc); y[i]=acc
    return y
def make(spec):
    bpm=spec["bpm"]; beat=60/bpm; bars=sum(n for n,_ in spec["sections"]); spb=int(beat*SR)
    total=bars*4*spb; out=np.zeros(total)
    root=NOTE[spec["key"][:-1] if spec["key"][-1] in "mM" else spec["key"]]+36
    minor=spec["key"].endswith("m")
    prog=spec["prog"]  # semitone offsets per bar cycle
    sections=spec["sections"]  # list of (bars, flags)
    bar0=0
    k=kick(int(0.4*SR)); h=hat(int(0.08*SR)); c=clap(int(0.25*SR))
    for nb, flags in sections:
        for b in range(bar0, bar0+nb):
            for q in range(4):
                s=(b*4+q)*spb
                if "k" in flags: out[s:s+len(k)]+=k[:total-s]*0.9
                if "h" in flags:
                    o=s+spb//2; out[o:o+len(h)]+=h[:max(0,total-o)]
                if "c" in flags and q in (1,3): out[s:s+len(c)]+=c[:total-s]
                if spec.get("skank") and "p" in flags:
                    o=s+spb//2; n=int(0.12*SR)
                    deg=prog[b%len(prog)]
                    chord=[root+24+deg, root+24+deg+(3 if minor else 4), root+24+deg+7]
                    seg=sum(np.sin(2*np.pi*hz(m)*np.arange(n)/SR) for m in chord)*env(n,0.002,0.05)*0.12
                    out[o:o+n]+=seg[:max(0,total-o)]
            deg=prog[b%len(prog)]
            bs=b*4*spb; n=4*spb
            if "b" in flags:
                for e8 in range(8):
                    o=bs+e8*spb//2; m=int(spb/2)
                    f=hz(root+deg+(12 if e8%2 and spec.get("octave") else 0))
                    seg=saw(f,m)*env(m,0.003,0.12,0.3)*0.28
                    out[o:o+m]+=seg[:max(0,total-o)]
            if "p" in flags and not spec.get("skank"):
                chord=[root+24+deg, root+24+deg+(3 if minor else 4), root+24+deg+7, root+36+deg]
                t=np.arange(n)/SR
                pad=sum(np.sin(2*np.pi*hz(m)*t+0.3*np.sin(2*np.pi*0.3*t)) for m in chord)
                out[bs:bs+n]+=pad*np.minimum(1,t/0.4)*0.05
            if "r" in flags:
                t=np.arange(n)/SR
                out[bs:bs+n]+=rng.standard_normal(n)*(t/t[-1])**2*0.15
        bar0+=nb
    out=np.tanh(out*1.2); out/=np.max(np.abs(out))*1.05
    # light stereo: slight delay on right
    right=np.concatenate([np.zeros(90), out[:-90]])
    return np.stack([out,right],1)
def write(path, x):
    with wave.open(path,"wb") as w:
        w.setnchannels(2); w.setsampwidth(2); w.setframerate(SR)
        w.writeframes((x*32000).astype(np.int16).tobytes())
if __name__=="__main__":
    specs=json.load(open(sys.argv[1])); outdir=sys.argv[2]
    for s in specs:
        p=os.path.join(outdir, s["file"]+".wav")
        if os.path.exists(p): continue
        write(p, make(s)); print("wrote", p)
