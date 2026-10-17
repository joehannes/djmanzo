<!--
  The whole site: the island, the controls laid over it, and whichever place
  the visitor has walked into. There are no pages; a place opens as a record
  sleeve over the map, and the address bar keeps up so a link to a place
  works.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import Island from "./map/Island.svelte";
  import { camera } from "./map/camera.svelte";
  import TopBar from "./hud/TopBar.svelte";
  import CuePads from "./hud/CuePads.svelte";
  import Platter from "./hud/Platter.svelte";
  import MC from "./hud/MC.svelte";
  import Setlist from "./hud/Setlist.svelte";
  import TourBar from "./hud/TourBar.svelte";
  import LoreCard from "./hud/LoreCard.svelte";
  import Sleeve from "./places/Sleeve.svelte";
  import Muelle from "./places/Muelle.svelte";
  import Cabina from "./places/Cabina.svelte";
  import Tienda from "./places/Tienda.svelte";
  import Tarima from "./places/Tarima.svelte";
  import Faro from "./places/Faro.svelte";
  import Taquilla from "./places/Taquilla.svelte";
  import Muro from "./places/Muro.svelte";
  import Radio from "./places/Radio.svelte";
  import Arena from "./places/Arena.svelte";
  import Gigs from "./places/Gigs.svelte";
  import Plaza from "./places/Plaza.svelte";
  import Pase from "./places/Pase.svelte";
  import Backstage from "./places/Backstage.svelte";
  import GenreIslet from "./places/GenreIslet.svelte";
  import { PLACE_BY_ID, PLACES, type PlaceId } from "./lib/places";
  import { GENRE_BY_ID, GENRES, type Genre } from "./lib/genres";
  import type { Lore } from "./lib/lore";
  import { profile, tourOrder } from "./lib/profile.svelte";
  import { daylight } from "./lib/daylight.svelte";
  import { session } from "./lib/session.svelte";
  import { mix, readable } from "./lib/colour";
  import { isletAt } from "./map/islets";

  type Open = { kind: "place"; id: PlaceId } | { kind: "genre"; id: string } | null;

  let open = $state<Open>(null);
  let lore = $state<{ lore: Lore; x: number; y: number } | null>(null);
  let setlistOpen = $state(false);
  let mcOpen = $state(!profile.p.welcomed);
  let tour = $state<{ order: PlaceId[]; i: number } | null>(null);
  let linkCode = $state<string | null>(null);

  const COMPONENTS = { muelle: Muelle, cabina: Cabina, tienda: Tienda, tarima: Tarima, faro: Faro, taquilla: Taquilla, muro: Muro, radio: Radio, arena: Arena, gigs: Gigs, plaza: Plaza, pase: Pase, backstage: Backstage };

  // --- Theming: the genre's colours, mixed by the hour --------------------------

  $effect(() => {
    const pal = profile.palette;
    const day = daylight.level;
    const root = document.documentElement;
    const bg = mix(pal.bg, mix("#e9f7fb", pal.bg, 0.12), day * 0.88);
    const surface = mix(pal.surface, mix("#ffffff", pal.surface, 0.1), day * 0.9);
    const ink = readable(surface, "#fdf6e3", "#14161c");
    const primary = day > 0.5 ? mix(pal.primary, "#000000", 0.18) : pal.primary;
    const vars: Record<string, string> = {
      "--bg": bg,
      "--surface": surface,
      "--surface-2": mix(surface, ink, 0.08),
      "--primary": primary,
      "--secondary": pal.secondary,
      "--ink": ink,
      "--ink-soft": mix(ink, surface, 0.32),
      "--line": mix(ink, surface, 0.78),
      "--on-primary": readable(primary),
      "--on-secondary": readable(pal.secondary),
      // Signs are painted boards: cream by day, lamplit by night, always dark lettering.
      "--sign-bg": mix("#fdf6e3", pal.primary, 0.12 * (1 - day)),
      "--sign-ink": "#16161c",
    };
    for (const [k, v] of Object.entries(vars)) root.style.setProperty(k, v);
    root.dataset.day = day > 0.5 ? "1" : "0";
    document.querySelector('meta[name="theme-color"]')?.setAttribute("content", bg);
  });

  // The genre's own display face, loaded only once somebody chooses that genre.
  $effect(() => {
    const g = profile.genre;
    const family = g?.display;
    if (!family) {
      document.documentElement.style.setProperty("--display", `"Bungee", system-ui, sans-serif`);
      return;
    }
    const id = `font-${family.replace(/\s+/g, "-")}`;
    if (!document.getElementById(id)) {
      const link = document.createElement("link");
      link.id = id;
      link.rel = "stylesheet";
      link.href = `https://fonts.googleapis.com/css2?family=${encodeURIComponent(family)}&display=swap`;
      document.head.appendChild(link);
    }
    document.documentElement.style.setProperty("--display", `"${family}", "Bungee", system-ui, sans-serif`);
  });

  // --- Going places -----------------------------------------------------------

  async function goPlace(id: PlaceId, fromTour = false) {
    if (!fromTour) tour = null;
    lore = null;
    const p = PLACE_BY_ID[id];
    open = null;
    const narrow = window.innerWidth < 760;
    // Keep the place visible beside (or above) the sleeve.
    const offsetX = narrow ? 0 : 230 / Math.max(camera.z, 0.6);
    const offsetY = narrow ? 180 : 0;
    await camera.flyTo(p.x + offsetX, p.y + offsetY, Math.max(camera.z, narrow ? 0.5 : 0.7));
    camera.tokenX = p.x;
    camera.tokenY = p.y;
    open = { kind: "place", id };
    if (p.stamp) profile.stamp(id);
    history.replaceState(null, "", id === "pase" && location.pathname === "/link" ? location.pathname + location.search : id === "pase" && linkCode ? `/link?code=${linkCode}` : `#/${id}`);
  }

  async function goGenre(g: Genre) {
    tour = null;
    lore = null;
    open = null;
    const at = isletAt(g, profile.p.genres);
    const narrow = window.innerWidth < 760;
    await camera.flyTo(at.x + (narrow ? 0 : 260 / Math.max(camera.z, 0.6)), at.y + (narrow ? 200 : 0), narrow ? 0.45 : 0.6, { token: false });
    history.replaceState(null, "", `#/genre/${g.id}`);
    open = { kind: "genre", id: g.id };
  }

  function close() {
    open = null;
    history.replaceState(null, "", location.pathname === "/link" ? "/" : "#");
  }

  function startTour() {
    mcOpen = false;
    tour = { order: tourOrder(profile.p), i: 0 };
    void goPlace(tour.order[0], true);
  }

  function tourStep(d: number) {
    if (!tour) return;
    const i = tour.i + d;
    if (i < 0 || i >= tour.order.length) {
      tour = null;
      return;
    }
    tour = { ...tour, i };
    void goPlace(tour.order[i], true);
  }

  function fromAddress() {
    if (location.pathname === "/link") {
      linkCode = new URLSearchParams(location.search).get("code");
      mcOpen = false;
      void goPlace("pase");
      return;
    }
    const m = location.hash.match(/^#\/(genre\/)?([a-z-]+)/);
    if (!m) return;
    if (m[1] && GENRE_BY_ID[m[2]]) {
      mcOpen = false;
      void goGenre(GENRE_BY_ID[m[2]]);
    } else if (PLACE_BY_ID[m[2] as PlaceId]) {
      mcOpen = false;
      void goPlace(m[2] as PlaceId);
    }
  }

  function keys(e: KeyboardEvent) {
    if ((e.target as HTMLElement).closest("input, textarea, select, [contenteditable]")) return;
    if (e.key === "Escape") {
      if (lore) lore = null;
      else if (open) close();
      else if (setlistOpen) setlistOpen = false;
      return;
    }
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    const pad = PLACES.find((p) => p.pad === Number(e.key));
    if (pad) {
      e.preventDefault();
      void goPlace(pad.id);
    }
  }

  onMount(() => {
    void session.refresh();
    fromAddress();
    if (new URLSearchParams(location.search).get("signin") === "failed") {
      history.replaceState(null, "", "/");
      void goPlace("pase");
    }
    window.addEventListener("keydown", keys);
    return () => window.removeEventListener("keydown", keys);
  });

  const Current = $derived(open?.kind === "place" ? COMPONENTS[open.id] : null);
  const currentPlace = $derived(open?.kind === "place" ? PLACE_BY_ID[open.id] : null);
  const currentGenre = $derived(open?.kind === "genre" ? GENRE_BY_ID[open.id] : null);
</script>

<Island
  admin={session.admin}
  current={open?.kind === "place" ? open.id : null}
  onplace={(id) => goPlace(id)}
  ongenre={goGenre}
  onlore={(l, x, y) => (lore = { lore: l, x, y })}
/>

<TopBar onplace={(id) => goPlace(id)} onsetlist={() => (setlistOpen = !setlistOpen)} onmc={() => (mcOpen = !mcOpen)} />

{#if tour}
  <TourBar order={tour.order} i={tour.i} onstep={tourStep} onstop={() => (tour = null)} />
{/if}

<CuePads current={open?.kind === "place" ? open.id : null} onplace={(id) => goPlace(id)} />
<Platter />

{#if mcOpen && !open}
  <MC onclose={() => (mcOpen = false)} ontour={startTour} onplace={(id) => goPlace(id)} />
{/if}

{#if setlistOpen}
  <Setlist onplace={(id) => { setlistOpen = false; void goPlace(id); }} ongenre={(g) => { setlistOpen = false; void goGenre(g); }} onclose={() => (setlistOpen = false)} genres={GENRES} admin={session.admin} />
{/if}

{#if lore}
  <LoreCard lore={lore.lore} onclose={() => (lore = null)} />
{/if}

{#if currentPlace && Current}
  {#key currentPlace.id}
    <Sleeve title={currentPlace.name} subtitle={currentPlace.what} emoji={currentPlace.emoji} onclose={close} wide={["taquilla", "arena", "backstage", "cabina"].includes(currentPlace.id)}>
      <Current go={(id: PlaceId) => goPlace(id)} {linkCode} />
    </Sleeve>
  {/key}
{:else if currentGenre}
  {#key currentGenre.id}
    <Sleeve title={currentGenre.name} subtitle={currentGenre.roots} emoji={currentGenre.emoji} onclose={close} tint={currentGenre.palette}>
      <GenreIslet genre={currentGenre} go={(id: PlaceId) => goPlace(id)} />
    </Sleeve>
  {/key}
{/if}
