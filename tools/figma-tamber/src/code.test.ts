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
  into.kept = {} as Record<string, string>;
  into.getSharedPluginData = (_k: string, n: string) => into.kept[n] ?? '';
  into.setSharedPluginData = (_k: string, n: string, v: string) => { if (Buffer.byteLength(v) > 100_000) throw new Error('shared plugin data over 100 KB'); into.kept[n] = v; };
  return into;
};

/** Figma, as much as the plugin uses, with `limit` modes a collection. */
function fakeFigma(limit: number) {
  const raise = (n: number) => { limit = n; };
  let ids = 0;
  const collections: any[] = [], variables: any[] = [], said: any[] = [], storage: Record<string, any> = {}, library: any[] = [], libraryCollections: any[] = [];
  let undo: (() => void) | null = null;
  const snapshot = () => {
    const cols = collections.map(c => [c, c.name, structuredClone(c.modes), { ...c.kept }] as const);
    const vars = variables.map(v => [v, v.name, v.resolvedType, { ...v.valuesByMode }, { ...v.kept }] as const);
    const root = { ...figma.root.kept };
    return () => {
      collections.splice(0, collections.length, ...cols.map(([c, name, modes, kept]) => Object.assign(c, { name, modes, kept })));
      variables.splice(0, variables.length, ...vars.map(([v, name, resolvedType, valuesByMode, kept]) => Object.assign(v, { name, resolvedType, valuesByMode, kept })));
      figma.root.kept = root;
    };
  };
  const figma: any = {
    showUI() {},
    ui: { postMessage: (m: any) => said.push(m), onmessage: null as any },
    root: data({}),
    // Undo, as a plugin has it: back to the last commit, whatever was made, changed or removed since.
    commitUndo() { undo = snapshot(); },
    triggerUndo() { undo?.(); },
    clientStorage: { getAsync: async (k: string) => storage[k], setAsync: async (k: string, v: any) => { storage[k] = v; } },
    variables: {
      getLocalVariableCollectionsAsync: async () => collections,
      getLocalVariablesAsync: async () => variables,
      getVariableByIdAsync: async (id: string) => variables.find(v => v.id === id) ?? library.find(v => v.id === id) ?? null,
      getVariableCollectionByIdAsync: async (id: string) => collections.find(c => c.id === id) ?? libraryCollections.find(c => c.id === id) ?? null,
      createVariableCollection(name: string) {
        const col: any = data({ id: `c${++ids}`, name, modes: [{ modeId: `m${++ids}`, name: 'Mode 1' }],
          get defaultModeId() { return col.modes[0].modeId; },
          renameMode(id: string, n: string) { if (col.modes.some((m: any) => m.name === n && m.modeId !== id)) throw new Error(`a mode named ${n} already`); col.modes.find((m: any) => m.modeId === id).name = n; },
          removeMode(id: string) { if (col.modes.length < 2 || id === col.modes[0].modeId) throw new Error('cannot remove the default mode'); const i = col.modes.findIndex((m: any) => m.modeId === id); if (i < 0) throw new Error('no such mode'); col.modes.splice(i, 1); },
          addMode(n: string) { if (col.modes.some((m: any) => m.name === n)) throw new Error(`a mode named ${n} already`); if (col.modes.length >= limit) throw new Error('in addMode: Limited to ' + limit + ' modes only'); const m = { modeId: `m${++ids}`, name: n }; col.modes.push(m); return m.modeId; } });
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
  return { figma, collections, variables, said, storage, library, libraryCollections, raise };
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

test('round 2: a refused re-pull is undone whole: the earlier collection, its modes, variables and kept export as they were', async () => {
  await load(10);
  await send({ type: 'apply', text: TEXT }, 'applied');
  const col = f.collections[0], modes = structuredClone(col.modes), names = f.variables.map(v => v.name).sort();
  const before = (await send({ type: 'collect' }, 'collected')).baseline;
  const renamed = variant(doc => {
    const contexts = doc.modifiers.mode.contexts, all = Object.keys(contexts);
    for (const c of all) { contexts[c.replace('dark-style', 'mood')] = contexts[c]; delete contexts[c]; }
    doc.modifiers.mode.default = doc.modifiers.mode.default.replace('dark-style', 'mood');
  });
  const create = f.figma.variables.createVariable;
  f.figma.variables.createVariable = () => { throw new Error('boom'); };
  // A type change forces a variable to be made again, and that is refused.
  const fill = f.variables.find(v => v.name === 'accent/fill');
  fill.resolvedType = 'FLOAT';
  await expect(send({ type: 'apply', text: renamed }, 'applied')).rejects.toThrow('boom');
  f.figma.variables.createVariable = create;
  fill.resolvedType = 'COLOR';
  expect([f.collections.length, col.modes, f.variables.map(v => v.name).sort()]).toEqual([1, modes, names]);
  expect((await send({ type: 'collect' }, 'collected')).baseline).toBe(before);
});

test('round 2: a collection named Tamber that the plugin did not make is refused, not taken over; a renamed mode is still its context', async () => {
  await load(10);
  f.figma.variables.createVariableCollection('Tamber');
  await expect(send({ type: 'apply', text: TEXT }, 'applied')).rejects.toThrow(/did not make/);
  await load(10);
  await send({ type: 'apply', text: TEXT }, 'applied');
  const col = f.collections[0], m = col.modes.find((x: any) => x.name === 'dark-style=dim, light');
  m.name = 'Dim (light)';
  const fill = f.variables.find(v => v.name === 'accent/fill');
  fill.valuesByMode[m.modeId] = { r: 1, g: 0, b: 0, a: 1 };
  expect(tokensOf(JSON.parse((await send({ type: 'collect' }, 'collected')).source))['dark-style=dim, light']['accent.fill'].$value.hex).toBe('#ff0000');
  await send({ type: 'apply', text: TEXT }, 'applied');
  expect(col.modes.find((x: any) => x.modeId === m.modeId).name).toBe('dark-style=dim, light');
});

test('round 2: an alias to a library variable reads it; one that goes nowhere is reported, not sent', async () => {
  await load(10);
  await send({ type: 'apply', text: TEXT }, 'applied');
  const lib = { id: 'lib1', name: 'Brand', modes: [{ modeId: 'lm1', name: 'dark-style=black, light' }], defaultModeId: 'lm1' };
  f.libraryCollections.push(lib);
  f.library.push({ id: 'lv1', name: 'red', variableCollectionId: 'lib1', valuesByMode: { lm1: { r: 1, g: 0, b: 0, a: 1 } } });
  const col = f.collections[0], fill = f.variables.find(v => v.name === 'accent/fill');
  const mode = (n: string) => col.modes.find((m: any) => m.name === n).modeId;
  fill.valuesByMode[mode('dark-style=black, light')] = { type: 'VARIABLE_ALIAS', id: 'lv1' };
  fill.valuesByMode[mode('dark-style=dim, light')] = { type: 'VARIABLE_ALIAS', id: 'nowhere' };
  const got = await send({ type: 'collect' }, 'collected');
  expect(tokensOf(JSON.parse(got.source))['dark-style=black, light']['accent.fill'].$value.hex).toBe('#ff0000');
  expect(got.unread.map((u: any) => u.mode)).toEqual(['dark-style=dim, light']);
});

test('round 2: the server and its token are kept as one entry', async () => {
  await load(10);
  await send({ type: 'save', server: 'http://127.0.0.1:1', token: 't' }, 'saved');
  expect(f.storage).toEqual({ paired: { server: 'http://127.0.0.1:1', token: 't' } });
  expect((await send({ type: 'load' }, 'loaded'))).toMatchObject({ server: 'http://127.0.0.1:1', token: 't' });
});

test('round 3: a re-pull that drops the default mode\'s context parks the default mode; every surviving context keeps its own', async () => {
  await load(10);
  await send({ type: 'apply', text: TEXT }, 'applied');
  const col = f.collections[0], first = col.modes[0].modeId, kept = Object.fromEntries(col.modes.map((m: any) => [m.name, m.modeId]));
  const next = variant(doc => { delete doc.modifiers.mode.contexts['dark-style=black, light']; doc.modifiers.mode.default = 'dark-style=dim, light'; });
  await send({ type: 'apply', text: next }, 'applied');
  expect(col.modes[0]).toEqual({ modeId: first, name: 'unused: dark-style=black, light' });
  for (const name of ['dark-style=black, dark', 'dark-style=dim, dark', 'dark-style=dim, light']) expect(col.modes.find((m: any) => m.name === name)?.modeId).toBe(kept[name]);
  const out = tokensOf(JSON.parse((await send({ type: 'collect' }, 'collected')).source));
  expect(Object.keys(out).sort()).toEqual(['dark-style=black, dark', 'dark-style=dim, dark', 'dark-style=dim, light']);
});

test('round 3: a mode name is cut to 40 characters; a 1-mode file says it needs Light and Dark', async () => {
  await load(10);
  const long = variant(doc => {
    const contexts = doc.modifiers.mode.contexts;
    for (const c of Object.keys(contexts)) { contexts[c.replace('dark-style', 'appearance-of-the-darker-scheme')] = contexts[c]; delete contexts[c]; }
    doc.modifiers.mode.default = doc.modifiers.mode.default.replace('dark-style', 'appearance-of-the-darker-scheme');
  });
  await send({ type: 'apply', text: long }, 'applied');
  expect(f.collections[0].modes.every((m: any) => m.name.length <= 40)).toBe(true);
  expect(new Set(f.collections[0].modes.map((m: any) => m.name)).size).toBe(4);
  expect(Object.keys(tokensOf(JSON.parse((await send({ type: 'collect' }, 'collected')).source))).length).toBe(4);
  await load(1);
  await expect(send({ type: 'apply', text: TEXT }, 'applied')).rejects.toThrow(/at least Light and Dark/);
  expect(f.collections.length).toBe(0);
});

test('round 3: modes are kept by context, not by their shown names: long names that trade places, and two modes trading names', async () => {
  await load(10);
  const axis = 'appearance-of-the-darker-scheme';
  const long = (first: string) => variant(doc => {
    const contexts = doc.modifiers.mode.contexts;
    for (const c of Object.keys(contexts)) { contexts[c.replace('dark-style', axis)] = contexts[c]; delete contexts[c]; }
    doc.modifiers.mode.default = first;
  });
  await send({ type: 'apply', text: long(`${axis}=black, light`) }, 'applied');
  const col = f.collections[0], fill = f.variables.find(v => v.name === 'accent/fill');
  const plan = () => JSON.parse(f.figma.root.kept['plan#0']);
  const ids = plan().collections[0].contextIds;
  // The default moves to Dark: the cut names would trade places; each context keeps its mode.
  await send({ type: 'apply', text: long(`${axis}=black, dark`) }, 'applied');
  expect(plan().collections[0].contextIds).toEqual(ids);
  // Two modes renamed in Figma to each other's names: each still its context, named back.
  const [a, b] = col.modes;
  [a.name, b.name] = [b.name, a.name];
  fill.valuesByMode[a.modeId] = { r: 1, g: 0, b: 0, a: 1 };
  const context = Object.entries<string>(ids).find(([, id]) => id === a.modeId)![0];
  expect(tokensOf(JSON.parse((await send({ type: 'collect' }, 'collected')).source))[context]['accent.fill'].$value.hex).toBe('#ff0000');
  await send({ type: 'apply', text: long(`${axis}=black, dark`) }, 'applied');
  expect(plan().collections[0].contextIds).toEqual(ids);
});

test('round 3: a refused pull with nothing changed undoes nothing; a deleted collection is reported, not read by name', async () => {
  await load(10);
  let undos = 0;
  const trigger = f.figma.triggerUndo;
  f.figma.triggerUndo = () => { undos++; trigger(); };
  f.figma.variables.createVariableCollection('Tamber');
  await expect(send({ type: 'apply', text: TEXT }, 'applied')).rejects.toThrow(/did not make/);
  expect(undos).toBe(0);
  await load(10);
  await send({ type: 'apply', text: TEXT }, 'applied');
  f.collections[0].remove();
  const other = f.figma.variables.createVariableCollection('Tamber');
  f.figma.variables.createVariable('accent/fill', other, 'COLOR');
  const got = await send({ type: 'collect' }, 'collected');
  expect(got.unread.map((u: any) => u.why)).toEqual(['the collection is gone from this file']);
  expect(JSON.parse(got.source)).toEqual(JSON.parse(TEXT)); // nothing of the stranger's sent
});

test('round 4: a collection renamed to a new tuple\'s name stays its own tuple\'s; one collection each', async () => {
  await load(2);
  await send({ type: 'apply', text: TEXT }, 'applied');
  const black = f.collections.find(c => c.name === 'Tamber · dark-style=black');
  black.name = 'Tamber · dark-style=grey';
  const grey = variant(doc => {
    const contexts = doc.modifiers.mode.contexts;
    for (const scheme of ['light', 'dark']) contexts[`dark-style=grey, ${scheme}`] = structuredClone(contexts[`dark-style=dim, ${scheme}`]);
  });
  await send({ type: 'apply', text: grey }, 'applied');
  expect(f.collections.map(c => c.name).sort()).toEqual(['Tamber · dark-style=black', 'Tamber · dark-style=dim', 'Tamber · dark-style=grey']);
  const ids = JSON.parse(f.figma.root.kept['plan#0']).collections.map((c: any) => c.id);
  expect(new Set(ids).size).toBe(3);
  expect(black.name).toBe('Tamber · dark-style=black');
});

test('round 4: a parked default mode comes back only to its own context', async () => {
  await load(10);
  await send({ type: 'apply', text: TEXT }, 'applied');
  const col = f.collections[0], first = col.modes[0].modeId;
  const plan = () => JSON.parse(f.figma.root.kept['plan#0']).collections[0];
  const dim = variant(doc => {
    for (const s of ['light', 'dark']) delete doc.modifiers.mode.contexts[`dark-style=black, ${s}`];
    doc.modifiers.mode.default = 'dark-style=dim, light';
  });
  await send({ type: 'apply', text: dim }, 'applied');
  await send({ type: 'apply', text: dim }, 'applied');
  expect(col.modes.find((m: any) => m.modeId === first).name).toBe('unused: dark-style=black, light');
  expect(plan().parked).toEqual({ 'dark-style=black, light': first });
  const back = variant(doc => { doc.modifiers.mode.default = 'dark-style=dim, light'; });
  await send({ type: 'apply', text: back }, 'applied');
  expect(plan().contextIds['dark-style=black, light']).toBe(first);
  expect(plan().contextIds['dark-style=black, dark']).not.toBe(first);
});

test('round 4: a deleted variable is reported before a pull and on push; a raised mode limit keeps one collection a tuple', async () => {
  await load(2);
  await send({ type: 'apply', text: TEXT }, 'applied');
  const fill = f.variables.find(v => v.name === 'accent/fill');
  fill.remove();
  const checked = await send({ type: 'check', text: TEXT }, 'checked');
  expect(checked.unread.map((u: any) => u.why)).toContain('the variable is gone from this file');
  expect((await send({ type: 'collect' }, 'collected')).unread.length).toBeGreaterThan(0);
  f.raise(10);
  const names = f.collections.map(c => c.name);
  const applied = await send({ type: 'apply', text: TEXT }, 'applied');
  expect(applied.collections).toEqual(names);
  expect(f.collections.map(c => c.name)).toEqual(names);
});

test('round 4 (Grok): a variable of a token\'s name the plugin did not make is not read, and a pull refuses it; the long default parked by a survivor of its cut name', async () => {
  await load(10);
  await send({ type: 'apply', text: TEXT }, 'applied');
  const col = f.collections[0];
  f.variables.find(v => v.name === 'accent/fill').remove();
  f.figma.variables.createVariable('accent/fill', col, 'COLOR');
  const got = await send({ type: 'collect' }, 'collected');
  expect(got.unread.map((u: any) => u.why)).toContain('the variable is gone from this file');
  await expect(send({ type: 'apply', text: TEXT }, 'applied')).rejects.toThrow(/did not make/);
  await load(10);
  const axis = 'appearance-of-the-darker-scheme';
  const long = (drop: boolean) => variant(doc => {
    const contexts = doc.modifiers.mode.contexts;
    for (const c of Object.keys(contexts)) { contexts[c.replace('dark-style', axis)] = contexts[c]; delete contexts[c]; }
    doc.modifiers.mode.default = `${axis}=black, light`;
    if (drop) { delete contexts[`${axis}=black, light`]; doc.modifiers.mode.default = `${axis}=dim, light`; }
  });
  await send({ type: 'apply', text: long(false) }, 'applied');
  await send({ type: 'apply', text: long(true) }, 'applied');
  expect(f.collections[0].modes[0].name).toBe(`unused: ${axis}=black, light`.slice(0, 40));
});

test('round 5: a refusal that names a mode and a limit is still a refusal, not the mode limit; a parked default making room short gives new collections', async () => {
  await load(10);
  const doc = JSON.parse(TEXT);
  for (const c of Object.values<any>(doc.modifiers.mode.contexts)) c[0].mode = { limit: { $type: 'number', $value: 1 } };
  const text = JSON.stringify(doc);
  await send({ type: 'apply', text }, 'applied');
  const col = f.collections[0];
  f.variables.find(v => v.name === 'mode/limit').remove();
  f.figma.variables.createVariable('mode/limit', col, 'FLOAT');
  let undos = 0;
  const trigger = f.figma.triggerUndo;
  f.figma.triggerUndo = () => { undos++; trigger(); };
  await expect(send({ type: 'apply', text }, 'applied')).rejects.toThrow(/did not make/);
  expect([undos, f.collections.length, f.collections[0]]).toEqual([0, 1, col]);
  // Two modes a collection, one tuple in one collection; the tuple renamed: its default parked, the new pair needs three.
  await load(2);
  const one = (style: string) => variant(d => {
    const contexts = d.modifiers.mode.contexts;
    for (const c of Object.keys(contexts)) if (!c.startsWith('dark-style=black')) delete contexts[c];
    for (const s of ['light', 'dark']) { contexts[`dark-style=${style}, ${s}`] = contexts[`dark-style=black, ${s}`]; if (style !== 'black') delete contexts[`dark-style=black, ${s}`]; }
    d.modifiers.mode.default = `dark-style=${style}, light`;
  });
  await send({ type: 'apply', text: one('black') }, 'applied');
  const first = f.collections[0];
  const applied = await send({ type: 'apply', text: one('grey') }, 'applied');
  expect(applied.collections).toEqual(['Tamber']);
  expect(f.collections.length).toBe(1);
  expect(f.collections[0]).not.toBe(first);
  expect(f.collections[0].modes.map((m: any) => m.name)).toEqual(['dark-style=grey, light', 'dark-style=grey, dark']);
});

test('round 6: a refusal is never the mode limit by its text (a collection named so); a replaced collection holding a stranger\'s variable is refused, all undone', async () => {
  await load(10);
  await send({ type: 'apply', text: TEXT }, 'applied');
  const col = f.collections[0];
  col.name = 'Limited to 2 modes';
  f.variables.find(v => v.name === 'accent/fill').remove();
  f.figma.variables.createVariable('accent/fill', col, 'COLOR');
  let undos = 0;
  const trigger = f.figma.triggerUndo;
  f.figma.triggerUndo = () => { undos++; trigger(); };
  await expect(send({ type: 'apply', text: TEXT }, 'applied')).rejects.toThrow(/did not make/);
  expect([undos, f.collections.length, f.collections[0]]).toEqual([0, 1, col]);
  // The one-tuple rename that needs new collections, with a variable of the person's own in the old one.
  await load(2);
  const one = (style: string) => variant(d => {
    const contexts = d.modifiers.mode.contexts;
    for (const c of Object.keys(contexts)) if (!c.startsWith('dark-style=black')) delete contexts[c];
    for (const s of ['light', 'dark']) { contexts[`dark-style=${style}, ${s}`] = contexts[`dark-style=black, ${s}`]; if (style !== 'black') delete contexts[`dark-style=black, ${s}`]; }
    d.modifiers.mode.default = `dark-style=${style}, light`;
  });
  await send({ type: 'apply', text: one('black') }, 'applied');
  const first = f.collections[0];
  f.figma.variables.createVariable('local/gutter', first, 'FLOAT');
  await expect(send({ type: 'apply', text: one('grey') }, 'applied')).rejects.toThrow(/local\/gutter/);
  expect(f.collections).toEqual([first]);
  expect(f.variables.some(v => v.name === 'local/gutter')).toBe(true);
});

test('round 6 (Grok): new collections only when every context the old served is gone; a survivor keeps its mode and the pull is refused', async () => {
  await load(2);
  const ctx = (names: string[], def: string) => variant(d => {
    const contexts = d.modifiers.mode.contexts, all = { ...contexts };
    for (const c of Object.keys(contexts)) delete contexts[c];
    for (const n of names) contexts[n] = all[n];
    d.modifiers.mode.default = def;
  });
  await send({ type: 'apply', text: ctx(['dark-style=black, light', 'dark-style=black, dark'], 'dark-style=black, light') }, 'applied');
  await send({ type: 'apply', text: ctx(['dark-style=black, dark'], 'dark-style=black, dark') }, 'applied');
  const col = f.collections[0], dark = col.modes.find((m: any) => m.name === 'dark-style=black, dark').modeId;
  await expect(send({ type: 'apply', text: ctx(['dark-style=black, dark', 'dark-style=dim, light'], 'dark-style=black, dark') }, 'applied')).rejects.toThrow(/can't remove/);
  expect(f.collections).toEqual([col]);
  expect(col.modes.some((m: any) => m.modeId === dark)).toBe(true);
});

test('round 7: a mode the person added stays through a pull, and a collection holding one is never replaced; a first change that fails undoes nothing', async () => {
  await load(10);
  await send({ type: 'apply', text: TEXT }, 'applied');
  const col = f.collections[0];
  const mine = col.addMode('Local review');
  await send({ type: 'apply', text: TEXT }, 'applied');
  expect(col.modes.some((m: any) => m.modeId === mine)).toBe(true);
  // A layout change would replace the collection: refused, the person's mode named.
  f.raise(2);
  const six = variant(doc => {
    const contexts = doc.modifiers.mode.contexts;
    for (const scheme of ['light', 'dark']) contexts[`dark-style=grey, ${scheme}`] = structuredClone(contexts[`dark-style=dim, ${scheme}`]);
  });
  await expect(send({ type: 'apply', text: six }, 'applied')).rejects.toThrow(/Local review/);
  expect(f.collections).toEqual([col]);
  // The first change refused: nothing undone.
  await load(10);
  let undos = 0;
  const trigger = f.figma.triggerUndo;
  f.figma.triggerUndo = () => { undos++; trigger(); };
  f.figma.variables.createVariableCollection = () => { throw new Error('no collections today'); };
  await expect(send({ type: 'apply', text: TEXT }, 'applied')).rejects.toThrow('no collections today');
  expect(undos).toBe(0);
});

test('round 8: an unchanged pull changes nothing and undoes nothing; check lists the variables a pull removes or makes again; a person\'s mode in the way is named', async () => {
  await load(10);
  await send({ type: 'apply', text: TEXT }, 'applied');
  let undos = 0;
  const trigger = f.figma.triggerUndo;
  f.figma.triggerUndo = () => { undos++; trigger(); };
  const set = f.figma.root.setSharedPluginData;
  f.figma.root.setSharedPluginData = () => { throw new Error('read-only file'); };
  const values = JSON.stringify(f.variables.map(v => v.valuesByMode));
  // Nothing to write: the kept export is as it was, so no write is tried, and nothing is undone.
  await send({ type: 'apply', text: TEXT }, 'applied');
  expect(undos).toBe(0);
  f.figma.root.setSharedPluginData = set;
  expect(JSON.stringify(f.variables.map(v => v.valuesByMode))).toBe(values);
  const dropped = variant(doc => { for (const c of Object.values<any>(doc.modifiers.mode.contexts)) delete c[0].accent.fill; });
  const checked = await send({ type: 'check', text: dropped }, 'checked');
  expect(checked.drops).toEqual([{ token: 'accent.fill', variable: 'accent/fill', why: 'the design dropped it' }]);
  // Two modes a collection: one light context Tamber holds, and a mode of the person's own; the dark context can't come.
  await load(2);
  const only = (names: string[]) => variant(d => {
    const contexts = d.modifiers.mode.contexts, all = { ...contexts };
    for (const c of Object.keys(contexts)) delete contexts[c];
    for (const n of names) contexts[n] = all[n];
    d.modifiers.mode.default = names[0];
  });
  await send({ type: 'apply', text: only(['dark-style=black, light']) }, 'applied');
  f.collections[0].addMode('Local review');
  await expect(send({ type: 'apply', text: only(['dark-style=black, light', 'dark-style=black, dark']) }, 'applied')).rejects.toThrow(/Local review/);
});

test('round 8 (Grok): a default mode of the person\'s own is never renamed or taken for a context', async () => {
  await load(10);
  await send({ type: 'apply', text: TEXT }, 'applied');
  const col = f.collections[0];
  // The person's own mode made the default (first), as reordering does.
  const mine = col.addMode('Local review');
  col.modes.unshift(col.modes.splice(col.modes.findIndex((m: any) => m.modeId === mine), 1)[0]);
  const fewer = variant(doc => { delete doc.modifiers.mode.contexts['dark-style=dim, dark']; });
  await send({ type: 'apply', text: fewer }, 'applied');
  expect(col.modes[0]).toEqual({ modeId: mine, name: 'Local review' });
  expect(f.variables.every(v => v.valuesByMode[mine] === undefined)).toBe(true);
});

test('round 9: a Figma-only edit is listed before a pull of the unchanged export; identical values are not written, so a refused save undoes nothing', async () => {
  await load(10);
  await send({ type: 'apply', text: TEXT }, 'applied');
  const col = f.collections[0], fill = f.variables.find(v => v.name === 'accent/fill'), first = col.modes[0].modeId, was = fill.valuesByMode[first];
  fill.valuesByMode[first] = { r: 1, g: 0, b: 0, a: 1 };
  const checked = await send({ type: 'check', text: TEXT }, 'checked');
  expect(checked.conflicts.map((c: any) => [c.variable, c.figmaOnly])).toEqual([['accent/fill', true]]);
  fill.valuesByMode[first] = was;
  // A new export that changes only its metadata: every value as it is, so none is written; the save refused: no undo.
  let undos = 0, writes = 0;
  const trigger = f.figma.triggerUndo;
  f.figma.triggerUndo = () => { undos++; trigger(); };
  for (const v of f.variables) { const set = v.setValueForMode; v.setValueForMode = (...a: any[]) => { writes++; return set(...a); }; }
  const meta = variant(doc => { doc.$description = 'another export'; });
  f.figma.root.setSharedPluginData = () => { throw new Error('read-only file'); };
  await expect(send({ type: 'apply', text: meta }, 'applied')).rejects.toThrow('read-only file');
  expect([writes, undos]).toEqual([0, 0]);
});
