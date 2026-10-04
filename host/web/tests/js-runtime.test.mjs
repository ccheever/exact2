// The JS target over a baked plan (host/web-js/rt.js `res`): a build-time
// answer is the first frame. A source not ready yet (a Rust module loads after
// first paint) leaves it shown, not pending, and asks it at `ready`, as a native
// runner asks at data_ready (review B3); a settled answer (an `else` row's, a dev
// reload's) is not asked again (review B4). A key handler's preventDefault and
// stopPropagation act on its event even when a view transition defers the tree
// update (review C3). rt.js runs beside stand-ins for the modules it imports,
// with the real shape.js.
import { test, expect } from 'bun:test';
import { copyFileSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';

const dir = mkdtempSync(resolve(tmpdir(), 'exact-js-baked-'));
const webJs = name => resolve(new URL(`../../web-js/${name}`, import.meta.url).pathname);
for (const f of ['rt.js', 'roster.js', 'router.js', 'budget.js', 'shape.js']) copyFileSync(webJs(f), resolve(dir, f));
for (const [file, names] of Object.entries({ 'navigation.js': ['renderMarkup', 'reportPlace', 'animationClocks', 'launchLocation'], 'pointer.js': ['pointer'], 'commands.js': ['commands'],
  'media.js': ['media', 'mediaProp', 'mediaOn', 'mediaPiece'], 'document.js': ['Docs', 'Head', 'head', 'markDocument', 'projectRoots'],
  'svg-transform.js': ['svgTransform'], 'dataset.js': ['ds'], 'hooks.js': ['hk'], 'perf.js': ['pf'], 'format.js': ['x_formatTime', 'x_formatDate', 'x_formatNumber'] }))
  writeFileSync(resolve(dir, file), names.map(n => `export const ${n} = () => {};`).join('\n') + (file === 'media.js' ? '\nexport const MEDIA_EVENTS = new Set();' : ''));
// A view transition that holds every tree update (shared.js's commit returns before its callback).
writeFileSync(resolve(dir, 'shared.js'), 'export const commit = (tail) => { globalThis.heldTail = tail; return true; };');
writeFileSync(resolve(dir, 'presence-glue.js'), 'globalThis.exact.presence = () => ({ before() {}, after() {}, exit() {} });');

test('a baked answer shows until the source is ready, then is asked; a settled one is not', async () => {
  const { res, data } = await import(resolve(dir, 'rt.js'));
  const asked = [];
  data.answer = (source) => { asked.push(source); return null; }; // not ready: no value, no request
  const baked = res('stamp', 'stamp', () => [], 0, [], 'n', 0);
  const kept = res('preview', 'preview', () => [], 5, [], 'n', 0, true);
  expect([baked(), baked.p(), kept(), kept.p()]).toEqual([0, false, 5, false]);
  expect(asked).toEqual(['stamp']); // the settled row is never asked
  expect(data.q.length).toBe(1);
  data.answer = (source) => { asked.push(source); return { v: source === 'stamp' ? 42 : 6 }; };
  for (const f of data.q.splice(0)) f();
  expect([baked(), kept()]).toEqual([42, 5]);
  expect(asked).toEqual(['stamp', 'stamp']);
});

test('a key handler stops and prevents its event while a view transition holds the tree update', async () => {
  globalThis.requestAnimationFrame = f => setTimeout(f, 0);
  globalThis.document = { getElementById: () => ({}) };
  try {
    const { on, act, C, pr, pieces } = await import(resolve(dir, 'rt.js'));
    pr({}); await pieces();
    const listeners = [];
    const el = { addEventListener: (type, f) => listeners.push([type, f]) };
    on(el, 'key', act(() => { C('preventDefault', []); C('stopPropagation', []); }));
    const ev = { key: 'Enter', defaultPrevented: false, preventDefault() { this.defaultPrevented = true; } };
    for (const [type, f] of listeners) if (type === 'keydown') f(ev);
    expect(typeof globalThis.heldTail).toBe('function'); // the tree update waits for the transition
    expect([ev.defaultPrevented, ev.$stopped]).toEqual([true, true]);
  } finally { delete globalThis.document; delete globalThis.requestAnimationFrame; }
});

// LLP 1090: a string past MAX_STRING comes only from a data source, which the
// JS target's Rust seam caps at 16 MiB a message, so conformance cannot carry
// one (host/web-js/conformance/budget.contract); the runtime's checks are run
// here, against the runner's texts (runner/src/runner/commit.rs, stdlib.rs).
test('a string past MAX_STRING joins to itself alone and is counted in UTF-8 bytes', async () => {
  const { x_join, K, cc, utf8, Trap } = await import(resolve(dir, 'budget.js'));
  const long = 'a'.repeat(2 ** 26 + 1);
  expect(x_join([long], ',', 3)).toBe(long); // stdlib::join's one string, unchecked
  expect(() => x_join([long, ''], ',', 3)).toThrow('Trap(StringTooLong { pc: 3 })');
  expect(() => K([long], 5)).toThrow(Trap);
  expect(() => K([long], 5)).toThrow('Trap(ValueTooLarge { pc: 5 })');
  expect([utf8('é'), utf8('€'), utf8('😀'), utf8('\ud800'), utf8('a\udc00b')]).toEqual([2, 3, 4, 3, 5]);
  // 2^25 units of "é" are exactly 2^26 bytes; one more is past.
  expect(cc('é'.repeat(2 ** 24), 'é'.repeat(2 ** 24), 7).length).toBe(2 ** 25);
  expect(() => cc('é'.repeat(2 ** 24), 'é'.repeat(2 ** 24 + 1), 7)).toThrow('Trap(StringTooLong { pc: 7 })');
  // A remembered part counted as 3 bytes a unit is recounted exactly (D3):
  // 10 MB three times is 30 MB, seven times 70 MB.
  const xs = K(Array.from({ length: 10000 }, () => 'a'.repeat(1000)), 1);
  expect(K([xs, xs, xs], 2)).toHaveLength(3);
  expect(() => K([xs, xs, xs, xs, xs, xs, xs], 2)).toThrow('Trap(ValueTooLarge { pc: 2 })');
});

test('an argument or a write past MAX_STRING is refused by name, and a trapping argument is a trap, poisoned or not', async () => {
  const { act, sig, W, effect, journal } = await import(resolve(dir, 'rt.js'));
  const { Trap } = await import(resolve(dir, 'budget.js'));
  const last = () => journal.at(-1).replace(/^t=\S+ /, '');
  const t = sig('', 's', 't');
  const put = act(v => W(t, v), ['s'], 0, ['v']);
  const row = act(($r, v) => W(t, v), ['s'], 1, ['v']);
  const long = 'a'.repeat(2 ** 26 + 1), euro = '€'.repeat(22369622); // 67,108,866 bytes in fewer than 2^26 units
  put(long);
  expect(last()).toBe('refused action: StringTooLong { name: "v" }');
  put.t(() => [euro])();
  expect(last()).toBe('refused action: StringTooLong { name: "v" }');
  row.t(() => [{}, long])(); // a row action's `$r` is not a parameter
  expect(last()).toBe('refused action: StringTooLong { name: "v" }');
  act(() => W(t, euro))();
  expect(last()).toBe('refused action: StringTooLong { name: "t" }');
  expect(t()).toBe('');
  put.t(() => { throw new Trap('IterationLimit', 17); })();
  expect(last()).toBe('refused action: Trap(IterationLimit { pc: 17 })');
  // A trap while the tree updates poisons, as the runner's InstanceError; an argument still traps first.
  const n = sig(0, 'n');
  effect(() => { if (n() > 0) throw new Trap('IterationLimit', 9); });
  act(() => W(n, 1))();
  globalThis.heldTail?.(); // a view transition (shared.js, above) holds the tree update
  expect(last()).toBe('poisoned: Instance(Trap(IterationLimit { pc: 9 }))');
  put.t(() => { throw new Trap('IterationLimit', 17); })();
  expect(last()).toBe('refused action: Trap(IterationLimit { pc: 17 })');
  put.t(() => ['x'])();
  expect(last()).toBe('refused action: the runner is poisoned; reload');
});
