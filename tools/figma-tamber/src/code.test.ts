// The Figma plugin's main thread (tools/figma-tamber/src/code.ts) on an
// in-memory stand-in for the Figma Variables API: a pull writes the Tamber
// collection, an edit in Figma comes back in the push's file, and a file
// whose plan takes fewer modes gets one collection per app tuple.
import { beforeEach, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { tokensOf } from './mapping';

const TEXT = readFileSync(new URL('../test/theme-fixture.export.json', import.meta.url), 'utf8');

/** Figma, as much as the plugin uses, with `limit` modes a collection. */
function fakeFigma(limit: number) {
  let ids = 0;
  const collections: any[] = [], variables: any[] = [], said: any[] = [], shared: Record<string, string> = {}, storage: Record<string, any> = {};
  const figma: any = {
    showUI() {},
    ui: { postMessage: (m: any) => said.push(m), onmessage: null as any },
    root: { getSharedPluginData: (_k: string, n: string) => shared[n] ?? '', setSharedPluginData: (_k: string, n: string, v: string) => { shared[n] = v; } },
    clientStorage: { getAsync: async (k: string) => storage[k], setAsync: async (k: string, v: any) => { storage[k] = v; } },
    variables: {
      getLocalVariableCollectionsAsync: async () => collections,
      getLocalVariablesAsync: async () => variables,
      getVariableByIdAsync: async (id: string) => variables.find(v => v.id === id) ?? null,
      createVariableCollection(name: string) {
        const col: any = { id: `c${++ids}`, name, modes: [{ modeId: `m${++ids}`, name: 'Mode 1' }],
          renameMode(id: string, n: string) { col.modes.find((m: any) => m.modeId === id).name = n; },
          addMode(n: string) { if (col.modes.length >= limit) throw new Error('Limited to ' + limit + ' modes'); const m = { modeId: `m${++ids}`, name: n }; col.modes.push(m); return m.modeId; } };
        col.remove = () => { collections.splice(collections.indexOf(col), 1); };
        collections.push(col);
        return col;
      },
      createVariable(name: string, col: any, type: string) {
        const v: any = { id: `v${++ids}`, name, type, variableCollectionId: col.id, valuesByMode: {}, setValueForMode(m: string, x: any) { v.valuesByMode[m] = x; }, setSharedPluginData() {} };
        variables.push(v);
        return v;
      },
    },
  };
  return { figma, collections, variables, said };
}

let f: ReturnType<typeof fakeFigma>;
const send = async (msg: any, reply: string) => {
  const before = f.said.length;
  await f.figma.ui.onmessage(msg);
  const got = f.said.slice(before).find((m: any) => m.type === reply || m.type === 'error');
  if (got?.type === 'error') throw new Error(got.message);
  return got;
};
const load = async (limit: number) => {
  f = fakeFigma(limit);
  (globalThis as any).figma = f.figma;
  (globalThis as any).__html__ = '';
  await import(`./code.ts?${Math.random()}`);
};

test('a pull writes one collection, a mode per context; an edit in Figma is in the push', async () => {
  await load(10);
  const applied = await send({ type: 'apply', text: TEXT }, 'applied');
  expect(applied.collections).toEqual(['Tamber']);
  const col = f.collections[0];
  expect(col.modes.map((m: any) => m.name)).toEqual(['dark-style=black, dark', 'dark-style=black, light', 'dark-style=dim, dark', 'dark-style=dim, light']);
  const fill = f.variables.find(v => v.name === 'accent/fill');
  const dim = col.modes.find((m: any) => m.name === 'dark-style=dim, light').modeId;
  fill.valuesByMode[dim] = { r: 1, g: 0, b: 0, a: 1 };
  const { source, baseline } = await send({ type: 'collect' }, 'collected');
  expect(baseline).toBe(TEXT);
  expect(tokensOf(JSON.parse(source))['dark-style=dim, light']['accent.fill'].$value.hex).toBe('#ff0000');
  expect(tokensOf(JSON.parse(source))['dark-style=black, light']['accent.fill'].$value.hex).toBe('#1d9bf0');
});

test('a plan of two modes a collection gets one collection per app tuple, Light and Dark', async () => {
  await load(2);
  const applied = await send({ type: 'apply', text: TEXT }, 'applied');
  expect(applied.collections).toEqual(['Tamber · dark-style=black', 'Tamber · dark-style=dim']);
  expect(f.collections.map(c => c.name)).toEqual(['Tamber · dark-style=black', 'Tamber · dark-style=dim']); // the refused try's one is gone
  expect(f.collections.filter(c => c.name.startsWith('Tamber · ')).map(c => c.modes.map((m: any) => m.name))).toEqual([['Dark', 'Light'], ['Dark', 'Light']]);
});

test('a push before any pull says pull first', async () => {
  await load(10);
  await expect(send({ type: 'collect' }, 'collected')).rejects.toThrow(/pull first/);
});
