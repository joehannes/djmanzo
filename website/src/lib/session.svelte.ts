/** Who is signed in, whether they are crew, and which sign-ins the site offers. */

import { api, post } from "./api";
import { profile } from "./profile.svelte";

export interface SessionUser {
  id: string;
  name: string;
  email: string | null;
  avatar: string | null;
  demo: boolean;
  owner: boolean;
  providers: string[];
}

class Session {
  user = $state<SessionUser | null>(null);
  admin = $state(false);
  providers = $state<{ id: string; name: string }[]>([]);
  demo = $state(true);
  loaded = $state(false);

  async refresh() {
    try {
      const me = await api<{ user: SessionUser | null; admin: boolean; providers: { id: string; name: string }[]; demo: boolean }>("/api/me");
      this.user = me.user;
      this.admin = me.admin;
      this.providers = me.providers;
      this.demo = me.demo;
      if (me.user) void this.syncProfile();
    } catch {
      /* the island works without the server; sign-in will say so when tried */
    } finally {
      this.loaded = true;
    }
  }

  /** Copy what the visitor told the island to their account, so djmanzo can read it. */
  async syncProfile() {
    const p = $state.snapshot(profile.p);
    try {
      await api("/api/me/profile", { method: "PUT", json: { name: p.name, role: p.role, venues: p.venues, genres: p.genres, gear: p.gear, experience: p.experience } });
    } catch {
      /* not worth interrupting anyone over */
    }
  }

  signInUrl(provider: string, back = location.pathname + location.search + location.hash): string {
    return `/auth/start/${provider}?return=${encodeURIComponent(back)}`;
  }

  async demoSignIn(name: string) {
    await post("/api/auth/demo", { name });
    await this.refresh();
  }

  async signOut() {
    await post("/api/auth/signout");
    this.user = null;
    this.admin = false;
  }
}

export const session = new Session();
