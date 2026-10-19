// The Figma plugin's main thread (tools/figma-tamber/src/code.ts) on an
// in-memory stand-in for the Figma Variables API: a pull writes the Tamber
// collection, an edit in Figma comes back in the push's file, and a file
// whose plan takes fewer modes gets one collection per app tuple.
import { beforeEach, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { tokensOf } from './mapping';

const TEXT = readFileSync(new URL('../test/theme-fixture.export.json', import.meta.url), 'utf8');

/** Shared plugin data, as Figma keeps it: an entry at most 100 KB. */
const data = (into: any) => {
  const kept: Record<string, string> = {};
  into.getSharedPluginData = (_k: string, n: string) => kept[n] ?? '';
  into.setSharedPluginData = (_k: string, n: string, v: string) => { if (Buffer.byteLength(v) > 100_000) throw new Error('shared plugin data over 100 KB'); kept[n] = v; };
  return into;
};

/** Figma, as much as the plugin uses, with `limit` modes a collection. */
function fakeFigma(limit: number) {
  let ids = 0;
  const collections: any[] = [], variables: any[] = [], said: any[] = [], storage: Record<string, any> = {};
  const figma: any = {
    showUI() {},
    ui: { postMessage: (m: any) => said.push(m), onmessage: null as any },
    root: data({}),
    clientStorage: { getAsync: async (k: string) => storage[k], setAsync: async (k: string, v: any) => { storage[k] = v; } },
    variables: {
      getLocalVariableCollectionsAsync: async () => collections,
      getLocalVariablesAsync: async () => variables,
      getVariableByIdAsync: async (id: string) => variables.find(v => v.id === id) ?? null,
      createVariableCollection(name: string) {
        const col: any = data({ id: `c${++ids}`, name, modes: [{ modeId: `m${++ids}`, name: 'Mode 1' }],
          get defaultModeId() { return col.modes[0].modeId; },
          renameMode(id: string, n: string) { col.modes.find((m: any) => m.modeId === id).name = n; },
          removeMode(id: string) { if (col.modes.length < 2) throw new Error('the last mode'); col.modes.splice(col.modes.findIndex((m: any) => m.modeId === id), 1); },
          addMode(n: string) { if (col.modes.length >= limit) throw new Error('in addMode: Limited to ' + limit + ' modes only'); const m = { modeId: `m${++ids}`, name: n }; col.modes.push(m); return m.modeId; } });
        col.remove = () => { collections.splice(collections.indexOf(col), 1); for (const v of variables.filter(v => v.variableCollectionId === col.id)) v.remove(); };
        collections.push(col);
        return col;
      },
      createVariable(name: string, col: any, type: string) {
        const v: any = data({ id: `v${++ids}`, name, resolvedType: type, variableCollectionId: col.id, valuesByMode: {}, setValueForMode(m: string, x: any) { if (!col.modes.some((o: any) => o.modeId === m)) throw new Error('no such mode'); v.valuesByMode[m] = x; } });
        v.remove = () => { const i = variables.indexOf(v); if (i >= 0) variables.splice(i, 1); };
        variables.push(v);
        return v;
      },
    },
  };
  return { figma, collections, variables, said };
}

let f: ReturnType<typeof fakeFigma>;
let asked = 0;
const send = async (msg: any, reply: string) => {
  const id = ++asked;
  await f.figma.ui.onmessage({ ...msg, id });
  const got = f.said.find((m: any) => m.id === id);
  if (got?.type === 'error') throw new Error(got.message);
  expect(got?.type).toBe(reply);
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
  expect(col.modes.map((m: any) => m.name)).toEqual(['dark-style=black, light', 'dark-style=black, dark', 'dark-style=dim, dark', 'dark-style=dim, light']);
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
  expect(f.collections.filter(c => c.name.startsWith('Tamber · ')).map(c => c.modes.map((m: any) => m.name))).toEqual([['Light', 'Dark'], ['Light', 'Dark']]);
  expect(f.variables.every(v => f.collections.some(c => c.id === v.variableCollectionId))).toBe(true);
});

test('a push before any pull says pull first', async () => {
  await load(10);
  await expect(send({ type: 'collect' }, 'collected')).rejects.toThrow(/pull first/);
});

/** The fixture with `f` applied to its document. */
const variant = (f: (doc: any) => void) => { const doc = JSON.parse(TEXT); f(doc); return JSON.stringify(doc); };

test('a re-pull reconciles: a token the design dropped goes, a renamed variable is still its token, a context dropped goes', async () => {
  await load(10);
  await send({ type: 'apply', text: TEXT }, 'applied');
  const fill = f.variables.find(v => v.name === 'accent/fill');
  fill.name = 'brand/primary'; // renamed in Figma
  const col = f.collections[0], light = col.modes.find((m: any) => m.name === 'dark-style=black, light').modeId;
  fill.valuesByMode[light] = { r: 1, g: 0, b: 0, a: 1 };
  expect(tokensOf(JSON.parse((await send({ type: 'collect' }, 'collected')).source))['dark-style=black, light']['accent.fill'].$value.hex).toBe('#ff0000');
  const gone = Object.keys(tokensOf(JSON.parse(TEXT))['dark-style=black, light']).find(t => t !== 'accent.fill' && f.variables.some(v => v.name === t.replaceAll('.', '/')))!;
  const next = variant(doc => {
    delete doc.modifiers.mode.contexts['dark-style=dim, dark'];
    for (const c of Object.values<any>(doc.modifiers.mode.contexts)) {
      const path = gone.split('.'), last = path.pop()!;
      const at = path.reduce((o: any, k) => o?.[k], c[0]);
      if (at) delete at[last];
    }
  });
  const checked = await send({ type: 'check', text: next }, 'checked');
  expect(checked.contexts).toEqual({ added: [], removed: ['dark-style=dim, dark'] });
  await send({ type: 'apply', text: next }, 'applied');
  expect(col.modes.map((m: any) => m.name)).toEqual(['dark-style=black, light', 'dark-style=black, dark', 'dark-style=dim, light']);
  expect(f.variables.some(v => v.getSharedPluginData('exact', 'token') === gone)).toBe(false);
  expect(f.variables.filter(v => v.getSharedPluginData('exact', 'token') === 'accent.fill').map(v => v.name)).toEqual(['brand/primary']);
});

test('a design that outgrows the file\'s modes moves to one collection a tuple; the earlier one goes', async () => {
  await load(4);
  await send({ type: 'apply', text: TEXT }, 'applied');
  expect(f.collections.map(c => c.name)).toEqual(['Tamber']);
  const six = variant(doc => {
    const contexts = doc.modifiers.mode.contexts;
    for (const scheme of ['light', 'dark']) contexts[`dark-style=grey, ${scheme}`] = structuredClone(contexts[`dark-style=dim, ${scheme}`]);
  });
  const applied = await send({ type: 'apply', text: six }, 'applied');
  expect(applied.collections).toEqual(['Tamber · dark-style=black', 'Tamber · dark-style=dim', 'Tamber · dark-style=grey']);
  expect(f.collections.map(c => c.name)).toEqual(applied.collections);
});

test('another error than the mode limit is refused as it is, what the try made taken away', async () => {
  await load(10);
  const create = f.figma.variables.createVariable;
  let n = 0;
  f.figma.variables.createVariable = (...a: any[]) => { if (++n === 3) throw new Error('boom'); return create(...a); };
  await expect(send({ type: 'apply', text: TEXT }, 'applied')).rejects.toThrow('boom');
  expect([f.collections.length, f.variables.length]).toEqual([0, 0]);
});

test('an alias to another collection reads its mode of the same name; a cycle reads nothing', async () => {
  await load(10);
  await send({ type: 'apply', text: TEXT }, 'applied');
  const other = f.figma.variables.createVariableCollection('Brand');
  other.renameMode(other.modes[0].modeId, 'dark-style=dim, light');
  const black = other.addMode('dark-style=black, light');
  const brand = f.figma.variables.createVariable('red', other, 'COLOR');
  brand.setValueForMode(other.modes[0].modeId, { r: 0, g: 0, b: 1, a: 1 });
  brand.setValueForMode(black, { r: 1, g: 0, b: 0, a: 1 });
  const col = f.collections[0], fill = f.variables.find(v => v.name === 'accent/fill');
  const mode = (n: string) => col.modes.find((m: any) => m.name === n).modeId;
  fill.valuesByMode[mode('dark-style=black, light')] = { type: 'VARIABLE_ALIAS', id: brand.id };
  const soft = f.variables.find(v => v.name !== 'accent/fill' && v.resolvedType === 'COLOR' && v.variableCollectionId === col.id);
  fill.valuesByMode[mode('dark-style=dim, dark')] = { type: 'VARIABLE_ALIAS', id: soft.id };
  soft.valuesByMode[mode('dark-style=dim, dark')] = { type: 'VARIABLE_ALIAS', id: fill.id };
  const out = tokensOf(JSON.parse((await send({ type: 'collect' }, 'collected')).source));
  expect(out['dark-style=black, light']['accent.fill'].$value.hex).toBe('#ff0000');
  expect(out['dark-style=dim, dark']['accent.fill']).toEqual(tokensOf(JSON.parse(TEXT))['dark-style=dim, dark']['accent.fill']);
});

test('a pull over 100 KB is kept in parts', async () => {
  await load(10);
  const big = variant(doc => { doc.$description = 'x'.repeat(250_000); });
  await send({ type: 'apply', text: big }, 'applied');
  expect((await send({ type: 'collect' }, 'collected')).baseline).toBe(big);
});
