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
    expect(await exclusiveSecret(store, 'test.', 'token', async () => store.map.get('token'))).toBe('new');
    expect(store.pending.size).toBe(0);
    await expect(exclusiveSecret(store, 'test.', 'token', async () => { store.pending.set('token', {}); })).rejects.toThrow('persisted');
    expect(store.pending.size).toBe(0);
    expect(disk.has('test.token')).toBe(false);
  } finally {
    for (const [key, descriptor] of saved) { if (descriptor) Object.defineProperty(globalThis, key, descriptor); else delete globalThis[key]; }
  }
});


test("failed secret writes cannot overwrite a later realm's rotation", async () => {
  const { exclusiveSecret } = await import('../web/storage-environment.js');
  const saved = ['navigator', 'localStorage'].map(k => [k, Object.getOwnPropertyDescriptor(globalThis, k)]);
  const disk = new Map([['exact.secret.token', 'old']]); let fail = true;
  Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: { getItem: k => disk.get(k) ?? null,
    setItem: (k, v) => { if (fail) throw new Error('quota'); disk.set(k, v); }, removeItem: k => disk.delete(k) } });
  Object.defineProperty(globalThis, 'navigator', { configurable: true, value: { locks: { request: async (_name, work) => work() } } });
  Store.map = new Map([['token', 'old']]); Store.writes = []; Store.pending.clear();
  try {
    Store.writes.push(['token', 'ordinary-stale']); Store.persist();
    disk.set('exact.secret.token', 'another-realm'); fail = false; await Store.persist();
    expect(disk.get('exact.secret.token')).toBe('another-realm');
    await expect(exclusiveSecret(Store, 'exact.secret.', 'token', async () => { fail = true; Store.writes.push(['token', 'rotated-A']); })).rejects.toThrow('persisted');
    expect(Store.pending.size).toBe(0);
    disk.set('exact.secret.token', 'rotated-B'); disk.delete('exact.exclusive.exact.secret.token'); fail = false; Store.persist();
    expect(disk.get('exact.secret.token')).toBe('rotated-B');
    Store.writes.push(['token', 'queued-before-lease']);
    expect(await exclusiveSecret(Store, 'exact.secret.', 'token', async () => Store.get('token'))).toBe('rotated-B');
    expect(disk.get('exact.secret.token')).toBe('rotated-B');
  } finally {
    Store.writes = []; Store.pending.clear();
    for (const [key, descriptor] of saved) { if (descriptor) Object.defineProperty(globalThis, key, descriptor); else delete globalThis[key]; }
  }
});


test("a new secret owner invalidates a credential left by an interrupted update", async () => {
  const { exclusiveSecret } = await import('../web/storage-environment.js');
  const saved = ['navigator', 'localStorage'].map(k => [k, Object.getOwnPropertyDescriptor(globalThis, k)]);
  const disk = new Map([['test.token', 'spent'], ['exact.exclusive.test.token', '1']]);
  Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: { getItem: k => disk.get(k) ?? null, setItem: (k, v) => disk.set(k, v), removeItem: k => disk.delete(k) } });
  Object.defineProperty(globalThis, 'navigator', { configurable: true, value: { locks: { request: async (_name, work) => work() } } });
  const store = { map: new Map([['token', 'spent']]), pending: new Map(), persist() {} };
  try {
    expect(await exclusiveSecret(store, 'test.', 'token', async () => store.map.get('token'))).toBeUndefined();
    expect(disk.size).toBe(0);
    disk.set('test.token', 'current');
    await expect(exclusiveSecret(store, 'test.', 'token', async () => { throw new Error('interrupted'); })).rejects.toThrow('interrupted');
    expect(disk.size).toBe(0);
  } finally {
    for (const [key, descriptor] of saved) { if (descriptor) Object.defineProperty(globalThis, key, descriptor); else delete globalThis[key]; }
  }
});
