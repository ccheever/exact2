// B5's runtime pieces without a browser (lower.js `Pc`'s arm rules; the readers that find a lowered node's rows:
// inspect-dev.js `written`, flow.js `classHeightOf`), over a stand-in sheet that holds rules as Chrome's CSSOM does.
import { expect, mock, test } from 'bun:test';

const journal = [], effects = [];
let Adopting = false;
mock.module('./rt.js', () => ({
  adopting: () => Adopting, effect: f => { effects.push(f); f(); }, say: l => journal.push(l),
  // As rt.js's: a number takes its unit, a blank declares nothing.
  cssText: (prop, unit, v) => v == null ? null : typeof v === 'number' ? v + unit : String(v).trim() ? String(v) : null,
  normal: (prop, t) => t === 'bogus' ? '' : t,
  css: (e, prop, unit, v) => { if (v == null) e.style.removeProperty(prop); else e.style.setProperty(prop, typeof v === 'number' ? v + unit : v); },
}));
mock.module('./textflow-glue.js', () => ({ createTextFlow: () => ({}), createFlowRequest: async () => ({}) }));

/** A declaration block, as CSSStyleDeclaration keeps one. */
const block = () => { const m = new Map(); return { setProperty: (p, v) => m.set(p, v), removeProperty: p => m.delete(p), getPropertyValue: p => m.get(p) ?? '', get height() { return m.get('height') ?? ''; }, [Symbol.iterator]: () => m.keys(), map: m }; };
/** A sheet whose `insertRule` parses only the arms' nested form. */
const sheet = () => { const cssRules = []; return { cssRules, insertRule(text, at) { const name = /& (\.[\w-]+)/.exec(text)[1]; cssRules.splice(at, 0, { cssRules: [{ selectorText: `& ${name}`, style: block() }] }); return at; }, deleteRule(at) { cssRules.splice(at, 1); } }; };
const element = (cls = '') => { let c = cls; return { style: block(), getAttribute: () => c || null, get classList() { return c.split(/\s+/).filter(Boolean); }, set className(v) { c = v; }, get className() { return c; } }; };
function page(staticSheet = { cssRules: [] }) {
  const nodes = new Map(), head = { append: n => nodes.set(n.id, n) };
  globalThis.document = {
    head, getElementById: id => nodes.get(id) ?? null,
    createElement: () => ({ id: '', sheet: sheet() }),
    get styleSheets() { return [staticSheet, ...[...nodes.values()].map(n => n.sheet)]; },
  };
  return nodes;
}
const recipe = (S, $b, $e) => { S($e, 'color', '', () => $b[0]); S($e, 'font-weight', '', () => $b[1]); };

test('an arm rule is built once a site an arm, from the recipe, and the class written after it', async () => {
  const nodes = page();
  const { Pc } = await import('./lower.js');
  const e = element('c3'), values = [['#000', 600], ['#00f', 400]];
  let k = 0;
  Pc(e, () => k, 'c3 ', 'xaone', values, ['color', 'font-weight'], recipe);
  const arms = nodes.get('exact-arms');
  expect(e.className).toBe('c3 xaone-0');
  expect([...arms.sheet.cssRules[0].cssRules[0].style.map]).toEqual([['color', '#000'], ['font-weight', '600']]);
  k = 1; effects.at(-1)();
  expect(e.className).toBe('c3 xaone-1');
  // A second instance at the same site builds nothing.
  const f = element('c3');
  Pc(f, () => 1, 'c3 ', 'xaone', values, ['color', 'font-weight'], recipe);
  expect(arms.sheet.cssRules.length).toBe(2);
  expect(f.$lowered).toEqual(['color', 'font-weight']);
});

test('a refused value falls to unset, as css does, and is said once', async () => {
  page();
  const { Pc } = await import('./lower.js?refused');
  Pc(element(), () => 0, '', 'xatwo', [['bogus', 400]], ['color', 'font-weight'], recipe);
  const st = document.getElementById('exact-arms').sheet.cssRules[0].cssRules[0].style;
  expect(st.getPropertyValue('color')).toBe('unset');
  expect(journal).toEqual(['unset color: "bogus" is not a value it takes']);
});

test('a recipe that throws caches nothing; a name that stood for other content throws on a test page', async () => {
  page();
  const { Pc } = await import('./lower.js?throws');
  const bad = () => { throw new Error('no'); };
  expect(() => Pc(element(), () => 0, '', 'xathree', [['#000', 400]], ['color'], bad)).toThrow('no');
  expect(document.getElementById('exact-arms').$built.has('xathree-0')).toBe(false);
  globalThis.__exactArmsCheck = true;
  try {
    Pc(element(), () => 0, '', 'xafour', [['#000', 400]], ['color', 'font-weight'], recipe);
    expect(() => Pc(element(), () => 0, '', 'xafour', [['#111', 400]], ['color', 'font-weight'], recipe)).toThrow('exact-arms: xafour-0 stands for other declarations');
  } finally { delete globalThis.__exactArmsCheck; }
});

test('a kept sheet of another host is started over', async () => {
  const nodes = page();
  const kept = { id: 'exact-arms', sheet: sheet(), $built: new Map([['xaold-0', true]]), $host: 'another host' };
  kept.sheet.insertRule('#exact-root#exact-root#exact-root{& .xaold-0{}}', 0);
  nodes.set('exact-arms', kept);
  const { Pc } = await import('./lower.js?host');
  Pc(element(), () => 0, '', 'xanew', [['#000', 400]], ['color', 'font-weight'], recipe);
  expect(document.getElementById('exact-arms')).toBe(kept);
  expect([...kept.$built.keys()]).toEqual(['xanew-0']);
  expect(kept.sheet.cssRules.map(r => r.cssRules[0].selectorText)).toEqual(['& .xanew-0']);
});

/** lower.js as the build writes it, stamped from `files`, beside this test (so its `./rt.js` is the mocked one). */
async function stamped(tag, files) {
  const { withRuntimeStamp } = await import('./module.mjs');
  const { readFileSync, writeFileSync } = await import('node:fs');
  const path = new URL(`./lower.stamped-${tag}.test-tmp.js`, import.meta.url);
  writeFileSync(path, withRuntimeStamp(readFileSync(new URL('./lower.js', import.meta.url), 'utf8'), files));
  return { path, module: await import(path.href) };
}
const RUNTIME_FILES = [{ path: 'rt.js', text: 'export const cssText = v => v;' }, { path: 'grid.js', text: 'export const gridValue = v => v;' }, { path: 'lower.js', text: 'function rule() {}' }];

test('a kept sheet of the same stamped host keeps its rules and adds the new one', async () => {
  const { unlinkSync } = await import('node:fs');
  const nodes = page();
  const one = await stamped('same1', RUNTIME_FILES), two = await stamped('same2', RUNTIME_FILES);
  try {
    one.module.Pc(element(), () => 0, '', 'xafirst', [['#000', 400]], ['color', 'font-weight'], recipe);
    const kept = nodes.get('exact-arms'), host = kept.$host;
    expect(host).toMatch(/^[0-9a-f]{24}$/);
    // A next generation of the same runtime (a module reload under a kept document): the old rule kept, the new added.
    two.module.Pc(element(), () => 0, '', 'xasecond', [['#111', 400]], ['color', 'font-weight'], recipe);
    expect(document.getElementById('exact-arms')).toBe(kept);
    expect(kept.$host).toBe(host);
    expect([...kept.$built.keys()]).toEqual(['xafirst-0', 'xasecond-0']);
  } finally { unlinkSync(one.path); unlinkSync(two.path); }
});

test('a helper-only change under an unchanged recipe rebuilds the declarations', async () => {
  const { unlinkSync } = await import('node:fs');
  const nodes = page();
  const before = await stamped('helper1', RUNTIME_FILES), after = await stamped('helper2', [RUNTIME_FILES[0], { path: 'grid.js', text: 'export const gridValue = v => String(v);' }, RUNTIME_FILES[2]]);
  try {
    before.module.Pc(element(), () => 0, '', 'xasame', [['#000', 400]], ['color', 'font-weight'], recipe);
    const kept = nodes.get('exact-arms');
    kept.sheet.cssRules[0].cssRules[0].style.setProperty('color', 'stale');
    // The same name and recipe, another runtime: the sheet starts over and the rule is built again from the recipe.
    after.module.Pc(element(), () => 0, '', 'xasame', [['#000', 400]], ['color', 'font-weight'], recipe);
    expect(kept.sheet.cssRules.length).toBe(1);
    expect(kept.sheet.cssRules[0].cssRules[0].style.getPropertyValue('color')).toBe('#000');
  } finally { unlinkSync(before.path); unlinkSync(after.path); }
});

test('an unbuilt runtime never reuses a kept sheet', async () => {
  const nodes = page();
  const { Pc } = await import('./lower.js?unbuilt1');
  Pc(element(), () => 0, '', 'xaone', [['#000', 400]], ['color', 'font-weight'], recipe);
  const kept = nodes.get('exact-arms');
  const { Pc: next } = await import('./lower.js?unbuilt2');
  next(element(), () => 0, '', 'xatwo', [['#000', 400]], ['color', 'font-weight'], recipe);
  expect([...kept.$built.keys()]).toEqual(['xatwo-0']);
});

test('the build stamps the arms sheet with a digest of the runtime, lower.js\'s own source included', async () => {
  const { withRuntimeStamp } = await import('./module.mjs');
  const text = 'const RUNTIME = "exact-runtime-unstamped";\nconst unstamped = RUNTIME !== "exact-runtime-unstamped";';
  const stamp = files => /RUNTIME = "([0-9a-f]+)"/.exec(withRuntimeStamp(text, files))?.[1];
  const one = stamp(RUNTIME_FILES);
  expect(one).toMatch(/^[0-9a-f]{24}$/);
  expect(stamp(RUNTIME_FILES)).toBe(one);
  // A helper changed, or lower.js alone (its rule builder, selector or unset path): another host.
  expect(stamp([RUNTIME_FILES[0], { path: 'grid.js', text: 'export const gridValue = v => String(v);' }, RUNTIME_FILES[2]])).not.toBe(one);
  expect(stamp([RUNTIME_FILES[0], RUNTIME_FILES[1], { path: 'lower.js', text: 'function rule() { /* unset */ }' }])).not.toBe(one);
  // Only the definition is stamped: the comparison still reads the unstamped token.
  expect(withRuntimeStamp(text, RUNTIME_FILES)).toContain('RUNTIME !== "exact-runtime-unstamped"');
});

test('adoption sets the class before it removes the renderer\'s inline rows', async () => {
  page();
  const { Pc } = await import('./lower.js?adopt');
  const e = element('c3'), order = [];
  e.style.setProperty('color', '#000');
  const remove = e.style.removeProperty;
  e.style.removeProperty = p => { order.push([p, e.className]); return remove(p); };
  Adopting = true;
  try { Pc(e, () => 0, 'c3 ', 'xafive', [['#000', 400]], ['color', 'font-weight'], recipe); } finally { Adopting = false; }
  expect(order).toEqual([['color', 'c3 xafive-0'], ['font-weight', 'c3 xafive-0']]);
  expect(e.style.getPropertyValue('color')).toBe('');
});

test('a render writes the rows inline as S does, the last write of a property winning', async () => {
  page();
  const { Pc } = await import('./lower.js?render');
  globalThis.__exactRender = true;
  try {
    const e = element('c3'), twice = (S, $b, $e) => { S($e, 'color', '', () => '#111'); S($e, 'color', '', () => $b[0]); S($e, 'font-weight', '', () => $b[1]); };
    Pc(e, () => 0, 'c3 ', 'xasix', [['#000', null]], ['color', 'color', 'font-weight'], twice);
    expect([...e.style.map]).toEqual([['color', '#000']]);
    expect(e.className).toBe('c3');
  } finally { delete globalThis.__exactRender; }
});

test('the readers find a lowered node\'s rows: inspect-dev `written` and flow `classHeightOf`', async () => {
  const statics = { cssRules: [{ selectorText: '#exact-root#exact-root', cssRules: [{ selectorText: '& .c3', style: block() }] }] };
  statics.cssRules[0].cssRules[0].style.setProperty('height', '40px');
  page(statics);
  const { Pc } = await import('./lower.js?readers');
  const e = element('c3');
  Pc(e, () => 1, 'c3 ', 'xaseven', [['#000', 400], ['#00f', 600]], ['color', 'font-weight'], recipe);
  const { written } = await import('./inspect-dev.js');
  expect([written(e, 'color'), written(e, 'font-weight'), written(e, 'margin-top')]).toEqual(['#00f', '600', '']);
  // The inline row stands over the arm, as it does on the page.
  e.style.setProperty('color', 'red');
  expect(written(e, 'color')).toBe('red');
  const { classHeightOf } = await import('./flow.js');
  expect(e.className).toBe('c3 xaseven-1');
  expect(classHeightOf(e, new Map())).toBe('40px');
});
