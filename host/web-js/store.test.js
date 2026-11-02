// The web JS target's store. Run alone, `bun test ./host/web-js/store.test.js`: reorder.test.js mocks rt.js for its process.
import { test, expect, mock } from "bun:test";

// The build copies navigation.js and prompt-glue.js in from host/web and writes names.js.
mock.module("./navigation.js", () => import("../web/navigation.js"));
mock.module("./prompt-glue.js", () => ({}));
mock.module("./alert-message.js", () => ({}));
mock.module("./menu-rows.js", () => ({}));
mock.module("./names.js", () => ({ sourceTypes: {} }));
const { Store } = await import("./rt.js");

// As the runner's `Store::set`.
test("keeping what is already kept is no write", () => {
  Store.map = new Map([["token", "same"]]);
  Store.writes = [];
  Store.dirty = false;
  Store.set("token", "same");
  expect(Store.writes).toEqual([]);
  expect(Store.dirty).toBe(false);
  Store.set("token", "new");
  expect(Store.writes).toEqual([["token", "new"]]);
  Store.set("token", null);
  expect(Store.writes.at(-1)).toEqual(["token", null]);
  expect(Store.get("token")).toBeUndefined();
});

test("a shared-store lock reads another realm's committed value before writing", async () => {
  const { exclusiveSecret } = await import('../web/storage-environment.js');
  const saved = ['navigator', 'localStorage'].map(k => [k, Object.getOwnPropertyDescriptor(globalThis, k)]);
  const disk = new Map([['test.token', 'new']]);
  Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: { getItem: k => disk.get(k) ?? null, setItem: (k, v) => disk.set(k, v), removeItem: k => disk.delete(k) } });
  Object.defineProperty(globalThis, 'navigator', { configurable: true, value: { locks: { request: async (_name, work) => work() } } });
  const store = { map: new Map([['token', 'stale']]), pending: new Map(), persist() {} };
  try {
    expect(await exclusiveSecret(store, 'test.', 'token', async () => store.map.get('token'))).toBe('new');
    store.pending.set('token', {});
    let called = false;
    await expect(exclusiveSecret(store, 'test.', 'token', async () => { called = true; })).rejects.toThrow('persisted');
    expect(called).toBe(false);
  } finally {
    for (const [key, descriptor] of saved) { if (descriptor) Object.defineProperty(globalThis, key, descriptor); else delete globalThis[key]; }
  }
});
