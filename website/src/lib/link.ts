/**
 * djmanzo asking, through the browser, to be linked to an account.
 *
 * Two shapes of `/link`, matching the Worker's two ways (`worker/device.ts`):
 * `?code=ABCD-EFGH` for the device flow, where the DJ checks a code; and
 * `?port=…&challenge=…&state=…&machine=…` for the loopback, where the DJ says
 * yes and the browser carries the answer back to djmanzo on that port.
 */

export interface LoopbackAsk {
  port: number;
  challenge: string;
  state: string;
  machine: string;
}

/** The loopback request in a query string, if it is a whole one. */
export function loopbackFrom(search: string): LoopbackAsk | null {
  const q = new URLSearchParams(search);
  const port = Number(q.get("port"));
  const challenge = q.get("challenge") ?? "";
  const state = q.get("state") ?? "";
  if (!Number.isInteger(port) || port < 1024 || port > 65535) return null;
  if (!/^[A-Za-z0-9_-]{43}$/.test(challenge) || !/^[A-Za-z0-9_-]{8,64}$/.test(state)) return null;
  return { port, challenge, state, machine: (q.get("machine") ?? "").slice(0, 60) || "A computer" };
}
