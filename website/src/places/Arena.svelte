<!--
  The arena: djmanzo next to the software DJs already use, said fairly.
  Each corner says what the other one does better — that is the part a
  visitor can trust the rest by. Only rows we are sure of are in the table;
  everything else is said in words, with its uncertainty.
-->
<script lang="ts">
  import type { PlaceId } from "../lib/places";
  let { go }: { go: (id: PlaceId) => void; linkCode?: string | null } = $props();

  const RIVALS = [
    {
      name: "VirtualDJ",
      better: "Decades of real gigs, controllers that work out of the box, video mixing, Windows, and streaming catalogues.",
      different: "djmanzo runs on Linux, treats Caribbean music as first-class, and its assistant speaks only in actions you could have pressed — it can run the mix, and gives way the moment a hand touches a control. VirtualDJ is the closest in scope — karaoke included — and the one djmanzo set out to match.",
    },
    {
      name: "Serato DJ Pro",
      better: "The standard in hip-hop and open-format booths, with famously solid control-vinyl (DVS) and a big hardware ecosystem.",
      different: "djmanzo has karaoke built in, runs on Linux, and reads timecode vinyl too (beta). If your club has Serato, Serato is the safe bet tonight.",
    },
    {
      name: "rekordbox",
      better: "Preparing USB sticks for Pioneer CDJs — the club standard. djmanzo cannot export to CDJs.",
      different: "djmanzo is for playing from the laptop: its own library, assistant and karaoke. It imports your rekordbox collection with cues and grids.",
    },
    {
      name: "Traktor Pro 4",
      better: "Deep effects, remix decks, a long history with Native Instruments hardware, and a one-time price.",
      different: "djmanzo's assistant, karaoke and Linux support; it imports Traktor collections.",
    },
    {
      name: "djay Pro",
      better: "Phones and tablets, a polished interface, and streaming services to play from.",
      different: "djmanzo is desktop only (macOS and Linux) and plays files you own; in exchange, karaoke, a set-planning assistant and set replay.",
    },
    {
      name: "Mixxx",
      better: "Free, open source, mature on Linux, with a large community library of controller mappings.",
      different: "djmanzo is built clean-room with permissive licences — which is why it cannot use Mixxx's (GPL) mappings and has fewer mappings today.",
    },
    {
      name: "KaraFun",
      better: "A licensed catalogue of karaoke songs with lyrics, ready to sing. djmanzo comes with no songs.",
      different: "djmanzo makes karaoke out of records you own: stems take the singer out, speech recognition times the words, and the DJ and karaoke halves share one booth.",
    },
  ];

  const ROWS: { label: string; cells: (boolean | string)[] }[] = [
    { label: "Runs on Linux", cells: [true, false, false, false, false, false, true] },
    { label: "Runs on Windows", cells: ["built, not yet run by a person", true, true, true, true, true, true] },
    { label: "Karaoke rotation built in", cells: [true, true, false, false, false, false, false] },
    { label: "Plays from streaming services", cells: ["not yet", true, true, true, true, true, false] },
    { label: "Years of real gigs behind it", cells: ["beta", true, true, true, true, true, true] },
  ];
  const COLS = ["djmanzo", "VirtualDJ", "Serato", "rekordbox", "Traktor", "djay", "Mixxx"];
</script>

<p>The other DJ software is good — most of it has years of real nights behind it. Here is where djmanzo stands, honestly, including what the others do better.</p>

<div class="scroll">
  <table>
    <thead>
      <tr><th></th>{#each COLS as c, i (c)}<th class:us={i === 0}>{c}</th>{/each}</tr>
    </thead>
    <tbody>
      {#each ROWS as r (r.label)}
        <tr>
          <th scope="row">{r.label}</th>
          {#each r.cells as v, i (i)}
            <td class:us={i === 0}>{v === true ? "✓" : v === false ? "—" : v}</td>
          {/each}
        </tr>
      {/each}
    </tbody>
  </table>
</div>
<p class="small muted">Checked October 2026 from each product's public pages. If something here has changed, tell us on <button class="link" onclick={() => go("muro")}>the wall</button> and we'll fix it.</p>

<div class="corners">
  {#each RIVALS as r (r.name)}
    <article class="corner">
      <h3>djmanzo vs {r.name}</h3>
      <p><b>Where {r.name} is stronger:</b> {r.better}</p>
      <p><b>Where djmanzo is different:</b> {r.different}</p>
    </article>
  {/each}
</div>

<h2>Where djmanzo is not ready</h2>
<ul>
  <li>It is <b>beta</b>: tested piece by piece, never through a real gig on real hardware yet.</li>
  <li><b>Windows</b> is built and packaged, but no person has run it yet; no phones or tablets.</li>
  <li><b>No streaming services</b> to play from — the slots exist; the partnerships don't.</li>
  <li><b>Pro DJ Link and StagelinQ</b> (joining a booth of club players) are on the roadmap, not in the app.</li>
  <li><b>Fewer controller mappings</b> than the established applications; the mapping editor learns a controller when you press its buttons.</li>
</ul>
<p>If any of those is a deal-breaker tonight, the others are the better choice tonight. If you want an assistant that learns how you play, karaoke in the same booth, Linux, and a price that is half the usual — <button class="link" onclick={() => go("taquilla")}>the box office is at the harbour</button>.</p>

<style>
  .scroll {
    overflow-x: auto;
  }
  table {
    min-width: 640px;
    font-size: 14px;
  }
  th,
  td {
    text-align: center;
  }
  th[scope="row"] {
    text-align: left;
  }
  .us {
    background: color-mix(in srgb, var(--primary) 18%, transparent);
    font-weight: 800;
  }
  .corners {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
    gap: 12px;
    margin: 14px 0;
  }
  .corner {
    padding: 14px;
    border-radius: 12px;
    border: 2px solid var(--line);
    background: var(--surface-2);
  }
  .corner h3 {
    font-size: 1.05rem;
  }
  .corner p {
    font-size: 14px;
  }
  .link {
    background: none;
    border: 0;
    padding: 0;
    color: var(--primary);
    text-decoration: underline;
    cursor: pointer;
    font: inherit;
  }
</style>
