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
for (const f of ['rt.js', 'roster.js', 'router.js', 'budget.js', 'shape.js', 'notify.js']) copyFileSync(webJs(f), resolve(dir, f));
copyFileSync(resolve(new URL('../notify-glue.js', import.meta.url).pathname), resolve(dir, 'notify-glue.js'));
for (const [file, names] of Object.entries({ 'navigation.js': ['renderMarkup', 'reportPlace', 'onSelection', 'animationClocks', 'launchLocation'], 'pointer.js': ['pointer', 'record'], 'commands.js': ['commands'],
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

// An answer that keeps coming (LLP 1016.000): each message settles the
// resource and keeps its ticket; it is pending, and in flight for `clock
// settle`, only until its first message; new arguments let the ticket go, and
// the stream closes once that commit stands; its end lets the ticket go.
test('a stream answer settles per message, keeps its ticket, and closes when let go', async () => {
  const { res, data, commit, sig, W, inflight, journal } = await import(resolve(dir, 'rt.js'));
  const opened = [];
  data.answer = (source, args) => ({ stream: (deliver, controller) => new Promise(end => opened.push({ args, deliver, end, controller })) });
  const wave = sig(1);
  let feed;
  commit(() => { feed = res('feed', 'feed', () => [wave()], undefined, undefined, 'n', 0); feed(); });
  expect([opened.length, feed.p(), inflight.n]).toEqual([1, true, 1]);
  opened[0].deliver({ v: 7, coalesced: 0 });
  expect([feed(), feed.p(), inflight.n]).toEqual([7, false, 0]);
  opened[0].deliver({ v: 9, coalesced: 2 });
  const t = feed.r.ticket;
  expect([feed(), t.messages, t.coalesced, t.closed]).toEqual([9, 2, 2, undefined]);
  commit(() => W(wave, 2));
  expect([t.closed, opened[0].controller.signal.aborted, opened.length, opened[1].args]).toEqual([true, true, 2, [2]]);
  expect(journal.some(l => l.endsWith(`close stream ${t.id}: its ticket was let go`))).toBe(true);
  opened[0].deliver({ v: 100, coalesced: 0 }); // a closed stream's late message is nobody's
  expect([feed(), feed.p()]).toEqual([9, true]);
  opened[1].end({ v: -1 });
  await new Promise(r => setTimeout(r, 0));
  expect([feed(), feed.p(), feed.r.ticket, inflight.n]).toEqual([-1, false, null, 0]);
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
// runner's MAX_STRING as it builds (a quadratic `$\`` stops there), throwing the runner's trap at the call's pc (budget.js).
test('slice, replaceAll and toLowerCase are the web methods, well formed and bounded', async () => {
  const { x_slice: slice, x_replaceAll: replaceAll, x_toLowerCase: toLowerCase } = await import(resolve(dir, 'budget.js'));
  const x_slice = (s, a, b) => slice(s, a, b, 0, 4);
  const x_replaceAll = (s, f, w) => replaceAll(s, f, w, 4), x_toLowerCase = s => toLowerCase(s, 4);
  expect([x_slice('calc', 0, -1), x_slice('hello', -3, Infinity), x_slice('a😀b', 1, 2), x_slice('hello', NaN, 2.9)]).toEqual(['cal', 'llo', '�', 'he']);
  for (const [s, f, w] of [['aXbXc', 'X', '-'], ['aaa', 'aa', 'b'], ['abc', '', '-'], ['😀', '', ''], ['😀😀', '', ''], ['abc', 'b', "[$&|$`|$'|$$|$1|$<n>|$]"], ['abc', '', '$`'], ['x.y', '.', '$$'], ['', '', ' ']])
    expect(x_replaceAll(s, f, w)).toBe(s.replaceAll(f, w).toWellFormed());
  expect(x_replaceAll('😀', '', '-')).toBe('-�-�-');
  expect([x_toLowerCase('ΟΣ'), x_toLowerCase('İ'), x_toLowerCase('ABC')]).toEqual(['ος', 'i̇', 'abc']);
  expect(() => x_replaceAll('x'.repeat(10000), '', "$`$'")).toThrow('Trap(StringTooLong { pc: 4 })');
  // The bound at a smaller limit, as the runner's own tests take it (strings.rs `replace_all(…, max)`): a result
  // that ends on a completed surrogate pair past it traps too (review b5-a 1: "xxx" → three emoji is 12 bytes).
  expect(() => replaceAll('xxx', 'x', '😀', 9, 8)).toThrow('Trap(StringTooLong { pc: 9 })');
  expect(replaceAll('xx', 'x', '😀', 9, 8)).toBe('😀😀');
  expect(() => replaceAll('😀😀', '', '', 9, 7)).toThrow('Trap(StringTooLong { pc: 9 })');
  expect(replaceAll('😀😀', '', '', 9, 8)).toBe('😀😀');
});

// LLP 1088 §9.1: `concat`, and `slice` and `includes` over a list, are the web's array methods (`includes` by
// SameValueZero), on the caller's budget: each takes its `$s`, traps where the runner's `list_call` does — before it
// builds — and leaves its own steps in `ST` (one an item kept, or scanned up to the match); text takes none.
test('concat, slice and includes over a list are the web methods, on the caller\'s list steps', async () => {
  const B = await import(resolve(dir, 'budget.js'));
  const xs = [1, NaN, -0, 'a', true];
  expect([B.x_concat([1], [2, 3], 0, 1), B.ST]).toEqual([[1, 2, 3], 3]);
  expect([B.x_slice(xs, 1, -1, 0, 1), B.ST]).toEqual([[NaN, -0, 'a'], 3]);
  expect([B.x_slice(xs, -2, Number.MAX_VALUE, 0, 1), B.x_slice(xs, NaN, 1.9, 0, 1), B.x_slice(xs, 3, 1, 0, 1)]).toEqual([['a', true], [1], []]);
  expect([B.x_includes(xs, NaN, 0, 1), B.ST, B.x_includes(xs, 0, 0, 1), B.ST, B.x_includes(xs, 'b', 0, 1), B.ST]).toEqual([true, 2, true, 3, false, 5]);
  expect([B.x_includes('abc', 'b', 9, 1), B.ST, B.x_slice('abc', 1, Number.MAX_VALUE, 9, 1), B.ST]).toEqual([true, 0, 'bc', 0]);
  expect(() => B.x_concat([1, 2], [3], 65534, 7)).toThrow('Trap(IterationLimit { pc: 7 })');
  expect(B.x_concat([1], [2], 65534, 7)).toEqual([1, 2]);
  expect(() => B.x_includes([1, 2, 3], 3, 65534, 8)).toThrow('Trap(IterationLimit { pc: 8 })');
  expect(B.x_includes([1, 2, 3], 2, 65534, 8)).toBe(true);
  const big = 'a'.repeat(2 ** 25);
  expect(() => B.x_concat([big], [big, 'a'], 0, 9)).toThrow('Trap(ValueTooLarge { pc: 9 })');
});

// `indexOf` over a list is the web's IsStrictlyEqual (NaN is never found), a step an item scanned; over text, UTF-16
// positions and no step. `split` takes a step a piece; an empty separator splits into code units, each well formed.
test('indexOf and split are the web methods, on the caller\'s list steps', async () => {
  const B = await import(resolve(dir, 'budget.js'));
  const xs = [1, NaN, -0, 'a', true];
  expect([B.x_indexOf(xs, NaN, 0, 1), B.ST, B.x_indexOf(xs, 0, 0, 1), B.ST, B.x_indexOf(xs, 'a', 0, 1), B.ST]).toEqual([-1, 5, 2, 3, 3, 4]);
  expect([B.x_indexOf('a😀b', 'b', 9, 1), B.ST, B.x_indexOf('abc', '', 9, 1), B.x_indexOf('abc', 'z', 9, 1)]).toEqual([3, 0, 0, -1]);
  expect([B.x_split('a,b,,c', ',', 0, 1), B.ST]).toEqual([['a', 'b', '', 'c'], 4]);
  expect([B.x_split('a😀', '', 0, 1), B.ST, B.x_split('', '', 0, 1), B.x_split('', ',', 0, 1)]).toEqual([['a', '\uFFFD', '\uFFFD'], 3, [], ['']]);
  expect(() => B.x_indexOf([1, 2, 3], 3, 65534, 8)).toThrow('Trap(IterationLimit { pc: 8 })');
  expect(B.x_indexOf([1, 2, 3], 2, 65534, 8)).toBe(1);
  expect(() => B.x_split('a,b,c', ',', 65534, 6)).toThrow('Trap(IterationLimit { pc: 6 })');
  expect(() => B.x_split('abc', '', 65534, 6)).toThrow('Trap(IterationLimit { pc: 6 })');
  expect(B.x_split('a,b', ',', 65534, 6)).toEqual(['a', 'b']);
});

// Local notifications on the JS target (notify.js, linked by use, over the
// web host's notify-glue.js): the runner's rule (runner/src/notify.rs)
// refuses without `device.notifications`, lists under the agent (a tag
// replacing its older one), and otherwise asks permission once and posts
// through the Notification API, now or at `showTrigger` while the page is
// open; a tag's `closeNotification` takes away a shown or a waiting one.
test('notifications: refused without the grant, listed under the agent, else posted by the Notification API', async () => {
  const { Hosts, clock, data, journal } = await import(resolve(dir, 'rt.js'));
  await import(resolve(dir, 'notify.js'));
  const notices = globalThis.exact.notices, posted = [], closed = [], last = () => journal.at(-1).replace(/^t=\S+ /, '');
  clock.agent = true; data.grants = '';
  Hosts.showNotification('Stretch', null, 'stretch', null);
  expect(last()).toBe('showNotification: refused: the grants name no device.notifications');
  data.grants = 'net.fetch https://a.example\ndevice.notifications purpose.notifications\n';
  Hosts.showNotification('Stretch', 'Now', 'stretch', null);
  Hosts.showNotification('Stretch', 'Later', 'stretch', 5);
  Hosts.showNotification('Alert', null, null, null);
  expect(notices).toEqual([{ title: 'Stretch', body: 'Later', tag: 'stretch', showTrigger: 5 }, { title: 'Alert', body: null, tag: null, showTrigger: null }]);
  Hosts.closeNotification('stretch');
  expect(notices.map(n => n.title)).toEqual(['Alert']);
  globalThis.Notification = class { static permission = 'default'; static requestPermission() { Notification.permission = 'granted'; return Promise.resolve('granted'); }
    constructor(title, options) { this.title = title; this.options = options; posted.push(this); } close() { closed.push(this.title); } };
  try {
    clock.agent = false;
    await Hosts.showNotification('Price alert', 'BTC crossed 100k', 'btc', null);
    expect([posted[0].title, posted[0].options, last()]).toEqual(['Price alert', { body: 'BTC crossed 100k', tag: 'btc' }, 'showNotification: shown']);
    await Hosts.showNotification('Stretch', null, 'stretch', Date.now() + 60_000);
    expect([posted.length, last()]).toEqual([1, 'showNotification: scheduled while this page is open']);
    await Hosts.closeNotification('stretch'); await Hosts.closeNotification('btc');
    expect(closed).toEqual(['Price alert']);
    await Hosts.showNotification('Soon', null, null, Date.now() + 20);
    await new Promise(r => setTimeout(r, 60));
    expect([posted.at(-1).title, last()]).toEqual(['Soon', 'showNotification: shown']);
  } finally { delete globalThis.Notification; }
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
