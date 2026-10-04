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
for (const f of ['rt.js', 'roster.js', 'router.js', 'shape.js']) copyFileSync(webJs(f), resolve(dir, f));
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

// LLP 1088 D2: the roster's string entries are JavaScript's, made well formed, and `replaceAll` is bounded by the
// runner's MAX_STRING as it builds (a quadratic `$\`` stops there), throwing the runner's trap as a Refusal.
test('slice, replaceAll and toLowerCase are the web methods, well formed and bounded', async () => {
  const { x_slice, x_replaceAll, x_toLowerCase, Refusal } = await import(resolve(dir, 'rt.js'));
  expect([x_slice('calc', 0, -1), x_slice('hello', -3, Infinity), x_slice('a😀b', 1, 2), x_slice('hello', NaN, 2.9)]).toEqual(['cal', 'llo', '�', 'he']);
  for (const [s, f, w] of [['aXbXc', 'X', '-'], ['aaa', 'aa', 'b'], ['abc', '', '-'], ['😀', '', ''], ['😀😀', '', ''], ['abc', 'b', "[$&|$`|$'|$$|$1|$<n>|$]"], ['abc', '', '$`'], ['x.y', '.', '$$'], ['', '', ' ']])
    expect(x_replaceAll(s, f, w)).toBe(s.replaceAll(f, w).toWellFormed());
  expect(x_replaceAll('😀', '', '-')).toBe('-�-�-');
  expect([x_toLowerCase('ΟΣ'), x_toLowerCase('İ'), x_toLowerCase('ABC')]).toEqual(['ος', 'i̇', 'abc']);
  expect(() => x_replaceAll('x'.repeat(10000), '', "$`$'")).toThrow(Refusal);
});
