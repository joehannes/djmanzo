/** An in-memory KV namespace: enough of the interface for the Worker's own code. */
export function memoryKv() {
  const data = new Map<string, { value: string; metadata?: unknown }>();
  const kv = {
    data,
    async get(key: string, type?: string) {
      const v = data.get(key);
      if (!v) return null;
      return type === "json" ? JSON.parse(v.value) : v.value;
    },
    async put(key: string, value: string, opts?: { metadata?: unknown }) {
      if (opts?.metadata && new TextEncoder().encode(JSON.stringify(opts.metadata)).length > 1024) {
        throw new Error(`metadata over 1024 bytes for ${key}`);
      }
      data.set(key, { value, metadata: opts?.metadata });
    },
    async delete(key: string) {
      data.delete(key);
    },
    async list({ prefix = "" }: { prefix?: string; cursor?: string; limit?: number } = {}) {
      const keys = [...data.keys()]
        .filter((k) => k.startsWith(prefix))
        .sort()
        .map((name) => ({ name, metadata: data.get(name)!.metadata }));
      return { keys, list_complete: true, cursor: undefined };
    },
  };
  return kv as unknown as KVNamespace & { data: typeof data };
}

export function testEnv(extra: Record<string, unknown> = {}) {
  return { KV: memoryKv(), DEMO_MODE: "auto", MAJOR_VERSION: "1", SITE_NAME: "djmanzo", ...extra } as never;
}
