/** The Worker's API, from the island. Errors carry the Worker's own sentence. */

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
    public data: Record<string, unknown> = {},
  ) {
    super(message);
  }
}

export async function api<T = any>(path: string, init: RequestInit & { json?: unknown } = {}): Promise<T> {
  const headers = new Headers(init.headers);
  if (init.json !== undefined) headers.set("content-type", "application/json");
  const res = await fetch(path, { ...init, headers, credentials: "same-origin", body: init.json !== undefined ? JSON.stringify(init.json) : init.body });
  const text = await res.text();
  let data: any = null;
  try {
    data = text ? JSON.parse(text) : null;
  } catch {
    data = { error: text };
  }
  if (!res.ok) throw new ApiError(res.status, data?.error || `The island's server answered ${res.status}.`, data || {});
  return data as T;
}

export const post = <T = any>(path: string, json: unknown = {}) => api<T>(path, { method: "POST", json });
