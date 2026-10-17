<script lang="ts">
  /**
   * The DJ's account on djmanzo's website, and the licence it holds.
   *
   * Signing in happens on the website, in the DJ's own browser, with the
   * account they bought with; djmanzo opens the site and waits for a yes
   * there, which the browser brings straight back (`dj_app::account`) — or,
   * where that cannot work, shows a short code to check on the site. The
   * licence comes back signed and is checked here, so it keeps working
   * offline for thirty days.
   * In the beta nothing is switched off whatever the licence says, and the
   * block says so rather than letting a DJ wonder.
   */
  import { accountLink, accountRefresh, accountSignOut, accountStatus, type AccountStatus } from "./api";

  let status = $state<AccountStatus | null>(null);
  let error = $state("");
  let busy = $state(false);

  $effect(() => {
    const look = () =>
      void accountStatus()
        .then((now) => (status = now))
        .catch(() => {});
    look();
    const poll = setInterval(look, 2_000);
    return () => clearInterval(poll);
  });

  async function act(f: () => Promise<AccountStatus>) {
    busy = true;
    error = "";
    try {
      status = await f();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  // `dj_core::edition`'s names, as the website sells them.
  const NAME: Record<string, string> = {
    autopilot: "Auto-Pilot",
    hobbyist: "Hobbyist",
    apprentice: "Apprentice",
    karaoke: "Karaoke",
    pro: "Pro DJ",
    full: "Full",
    custom: "Custom",
  };
  const MODULE: Record<string, string> = {
    four_decks: "Four decks",
    stems: "Stems",
    autopilot: "Auto-pilot",
    assistant: "Assistant",
    teacher: "Teacher",
    karaoke: "Karaoke",
    crowd: "Reading the room",
    controllers: "Controllers",
    dvs: "Timecode vinyl",
    video: "Video and visuals",
    streaming: "Going live",
    plugins: "Plugins",
    sources: "Online sources",
    events: "Events",
    replay: "Replay",
  };
  const PLAN: Record<string, string> = { monthly: "monthly", yearly: "yearly", perpetual: "bought once", trial: "free trial" };
  const custom = (edition: string, modules: string[] | undefined) =>
    edition === "custom" && modules?.length ? ` (${modules.map((m) => MODULE[m] ?? m).join(", ")})` : "";
  const daysLeft = (s: number) => Math.max(0, Math.ceil((s * 1000 - Date.now()) / 86_400_000));
  const day = (s: number) => new Date(s * 1000).toLocaleDateString(undefined, { day: "numeric", month: "long", year: "numeric" });
</script>

<div class="account" data-account>
  {#if !status}
    <p class="hint">Reading the account…</p>
  {:else if status.pending && !status.pending.code}
    <p data-account-state="linking">
      Your browser has opened <b>{status.site.replace(/^https?:\/\//, "")}</b>. Sign in there with the account you bought with and say yes — the browser brings the answer straight back here.
    </p>
    <p class="hint">djmanzo is waiting. Didn't open? Go to <span class="mono">{status.pending.address}</span>.</p>
  {:else if status.pending}
    <p data-account-state="linking">
      Your browser has opened <b>{status.site.replace(/^https?:\/\//, "")}</b>. Sign in there with the account you bought with and check that it shows this code:
    </p>
    <p class="code" data-account-code>{status.pending.code}</p>
    <p class="hint">djmanzo is waiting for your yes. Didn't open? Go to <span class="mono">{status.pending.address}</span>.</p>
  {:else if !status.signed_in}
    <p data-account-state="signed-out">
      Sign in to bring your licence into djmanzo. The sign-in happens on djmanzo's website, in your own browser — djmanzo never sees your password.
    </p>
    <button class="primary" onclick={() => act(accountLink)} disabled={busy}>Sign in</button>
  {:else}
    {@const st = status.standing}
    <p data-account-state={st.kind}>
      Signed in as <b>{status.name ?? "you"}</b>{status.email ? ` (${status.email})` : ""} on <b>{status.machine_name}</b>.
    </p>
    {#if st.kind === "licensed"}
      <p class="licence" data-account-licence>
        djmanzo <b>{NAME[st.edition] ?? st.edition}</b>{custom(st.edition, st.modules)}, {PLAN[st.plan] ?? st.plan}{st.until ? `, until ${day(st.until)}` : ""}{st.demo ? " — a demo licence" : ""}.
        {#if st.plan === "trial"}
          <span class="hint" data-account-trial>{daysLeft(st.until ?? st.offline_until)} {daysLeft(st.until ?? st.offline_until) === 1 ? "day" : "days"} of the trial left. What you make in djmanzo stays yours after it ends.</span>
        {:else}
          <span class="hint">Works offline until {day(st.offline_until)}; djmanzo renews it whenever it is online.</span>
        {/if}
      </p>
    {:else if st.kind === "lapsed"}
      <p class="licence lapsed" data-account-licence>{st.reason}</p>
    {:else}
      <p class="hint">No licence on this account yet.</p>
    {/if}
    {#if status.licences.length > 1}
      <ul class="others">
        {#each status.licences as l, i (i)}
          <li>{NAME[l.edition] ?? l.edition}{custom(l.edition, l.modules)}, {PLAN[l.plan] ?? l.plan}{l.here ? " — this computer" : ""}{l.demo ? " (demo)" : ""}</li>
        {/each}
      </ul>
    {/if}
    <div class="row">
      <button onclick={() => act(accountRefresh)} disabled={busy}>Check now</button>
      <button onclick={() => act(accountSignOut)} disabled={busy}>Sign out on this computer</button>
    </div>
  {/if}
  {#if status && !status.enforced}
    <p class="hint" data-account-beta>Beta: every feature is open whatever the licence says.</p>
  {/if}
  {#if status?.note && !status.pending}<p class="hint" data-account-note>{status.note}</p>{/if}
  {#if error}<p class="error">{error}</p>{/if}
</div>

<style>
  .code {
    font-size: 1.6rem;
    font-weight: 800;
    letter-spacing: 0.12em;
    margin: 0.3rem 0;
  }
  .licence.lapsed {
    color: var(--danger, #e5484d);
  }
  .row {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }
  .others {
    margin: 0.2rem 0 0.5rem;
    padding-left: 1.1rem;
  }
</style>
