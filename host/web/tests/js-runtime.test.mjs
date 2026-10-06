// The JS target over a baked plan (host/web-js/rt.js `res`): a build-time
// answer is the first frame. A source not ready yet (a Rust module loads after
// first paint) leaves it shown, not pending, and asks it at `ready`, as a native
// runner asks at data_ready (review B3); a settled answer (an `else` row's, a dev
// reload's) is not asked again (review B4). A key handler's preventDefault and
// stopPropagation act on its event even when a view transition defers the tree
// update (review C3). rt.js runs beside stand-ins for the modules it imports,
// with the real shape.js.
import { test, expect } from 'bun:test';
import { copyFileSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';

const dir = mkdtempSync(resolve(tmpdir(), 'exact-js-baked-'));
const webJs = name => resolve(new URL(`../../web-js/${name}`, import.meta.url).pathname);
for (const f of ['rt.js', 'roster.js', 'router.js', 'schedule.js', 'budget.js', 'shape.js', 'notify.js']) copyFileSync(webJs(f), resolve(dir, f));
copyFileSync(resolve(new URL('../notify-glue.js', import.meta.url).pathname), resolve(dir, 'notify-glue.js'));
for (const [file, names] of Object.entries({ 'navigation.js': ['renderMarkup', 'reportPlace', 'onSelection', 'textField', 'settleRadios', 'animationClocks', 'launchLocation'], 'pointer.js': ['pointer', 'record'], 'commands.js': ['commands'],
  'media.js': ['media', 'mediaProp', 'mediaOn', 'mediaPiece'], 'document.js': ['Docs', 'Head', 'head', 'markDocument', 'projectRoots'],
  'svg-transform.js': ['svgTransform'], 'dataset.js': ['ds'], 'hooks.js': ['hk'], 'perf.js': ['pf'], 'format.js': ['x_formatTime', 'x_formatDate', 'x_formatNumber', 'x_toFixed', 'x_formatDecimal'] }))
  writeFileSync(resolve(dir, file), names.map(n => `export const ${n} = () => {};`).join('\n') + (file === 'media.js' ? '\nexport const MEDIA_EVENTS = new Set();' : ''));
// A view transition that holds every tree update (shared.js's commit returns before its callback).
writeFileSync(resolve(dir, 'shared.js'), 'export const commit = (tail) => { globalThis.heldTail = tail; return true; };');
writeFileSync(resolve(dir, 'presence-glue.js'), 'globalThis.exact.presence = () => ({ before() {}, after() {}, exit() {} });');
// ts-data.js over a scripted store (the storage test below), written before
// anything is imported from here: the loader reads this directory once.
const stub = (file, text) => writeFileSync(resolve(dir, file), text);
stub('admission.js', 'export const createSecretFacade = () => ({ read: false }); export const hasGrant = () => true; export const setAppGrantSet = g => g;');
stub('admission-data.js', 'export const tsGrantSet = {};');
stub('ts-fetch.js', 'export const answering = { call: null };');
stub('names.js', 'export const sourceTypes = {};');
stub('storage-environment.js', "export const storageKey = () => 'k'; export const agentStorageRefusal = 'no store';");
// A write lands a task later; a read answers at once: unqueued, it would overtake.
stub('storage-fs.js', `const files = new Map(); export const createFileSystem = () => ({
  atomicWriteFile: (p, v) => new Promise((ok, no) => setTimeout(() => p.includes('absent/') ? no(Object.assign(new Error('filesystem: No such file'), { code: 'ENOENT' })) : ok(files.set(p, v)), 5)),
  writeFile: (p, v) => new Promise(ok => setTimeout(() => ok(files.set(p, v)), 1)),
  readFile: async p => files.get(p) ?? '' });`);
stub('app.mjs', `export const appId = 'test';
  export function answer(source, [op, value], store, storage) {
    if (op === 'save') { storage.fs.atomicWriteFile('app:/data/song', value).catch(() => {}); return 'saved ' + value; }
    if (op === 'read') return storage.fs.readFile('app:/data/song');
    if (op === 'bad') { storage.fs.atomicWriteFile('app:/data/absent/x', value).catch(() => {}); return 'saved'; }
    const codes = [];
    for (let i = 0; i < 258; i++) codes.push(storage.fs.writeFile('app:/data/flood', String(i)).then(() => 'ok', e => e.code));
    return codes[257];
  }`);
const tsData = readFileSync(webJs('ts-data.js'), 'utf8').replace("'__APP_TS__'", JSON.stringify(resolve(dir, 'app.mjs'))).replace('__AUTH_IMPORT__', '').replace('__AUTH_INSTALL__', '');
writeFileSync(resolve(dir, 'ts-data.js'), tsData);

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
// A mutation answered at once has landed in the sending commit: the read it
// `refreshes` is forced then, as a reply's landing forces it (runner
// commit.rs `landed_now`). A re-read there dropped an async read's promise and
// no reply came to ask again, so it kept its old value (the data6 lane).
test('a mutation answered at once forces the async read it refreshes', async () => {
  const { res, mut, M, data, commit, sig } = await import(resolve(dir, 'rt.js'));
  let count = 0;
  data.answer = (source) => source === 'inc' ? { v: ++count } : { promise: Promise.resolve(count) };
  let doc;
  commit(() => { doc = res('doc', 'doc', () => [], undefined, undefined, 'n', 0); doc(); });
  await new Promise(r => setTimeout(r, 0));
  expect(doc()).toBe(0);
  const op = mut('op', sig(null), [doc], null);
  commit(() => M(op, 'inc', []));
  await new Promise(r => setTimeout(r, 0));
  expect([doc(), doc.p()]).toEqual([1, false]);
});


// A queue with no `then` (LLP 1092 D3, D6): off the agent, the commit its reply lands in makes the waiting send due,
// and the wall clock's `drive()` asks it with nothing else to wake it; the send waited with its own arguments.
test('a queue with no then asks its waiting send by drive() alone, off the agent', async () => {
  const { mut, M, act, sig, data, queues, clock } = await import(resolve(dir, 'rt.js'));
  const asked = [], replies = [];
  data.answer = (source, args) => { asked.push(args[0]); return { promise: new Promise(r => replies.push(r)) }; };
  const slot = sig(null);
  const m = mut('rec', slot, [], null, 1);
  queues([[slot], [], []]);
  const send = act(op => M(m, 'save', [op]));
  send('p'); send('q');
  expect([asked, m.p(), m.wait.length, clock.agent]).toEqual([['p'], true, 1, false]);
  replies[0]('P');
  for (let i = 0; i < 50 && asked.length < 2; i++) await new Promise(r => setTimeout(r, 5));
  expect([asked, slot(), m.p(), m.wait.length]).toEqual([['p', 'q'], 'P', true, 0]);
  replies[1]('Q');
  for (let i = 0; i < 50 && m.p(); i++) await new Promise(r => setTimeout(r, 5));
  expect([slot(), m.p(), m.next]).toEqual(['Q', false, Infinity]);
});

// LLP 1092 D8 on the JS target: the gate step runs inside the commit's undo `try`, after settlement, so a key that is
// no key refuses the commit (`TaskKey`), its writes and the timers both as they were; a changed key re-arms from now.
test('a gate step that refuses rolls back the commit and its timers', async () => {
  const { sig, act, W, gated, clock, journal } = await import(resolve(dir, 'rt.js'));
  clock.agent = true;
  try {
    const k = sig(0, 'n');
    const tick = act(() => {});
    gated(1000, tick, 1, 0, () => true, () => (k() === 5 ? NaN : k()), 'r');
    const timer = () => clock.timers.find(t => t.name === 'r');
    const due = timer().due;
    act(() => W(k, 5))();
    expect(journal.at(-1)).toContain('TaskKey { task: "r" }');
    expect([k(), timer().due]).toEqual([0, due]);
    clock.now = 400;
    act(() => W(k, 1))();
    expect([k(), timer().due]).toEqual([1, 1400]);
  } finally { clock.agent = false; }
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

// LLP 1102 §3.1–§3.4: the cases of runner/src/stdlib.rs's `number_and_date_reads_are_javascript_s`, roster.js's
// against the oracle the runner names: `Number` for a numeral the grammar admits, `Math.round` and `Math.ceil`.
test('parseNumber, round, ceil and calendarDiff are the runner\'s', async () => {
  const { x_parseNumber, x_round, x_ceil, x_calendarDiff } = await import(resolve(dir, 'roster.js'));
  for (const [text, want] of [[' 12.5 ', 12.5], ['-3', -3], ['+.5', 0.5], ['5.', 5], ['5.e3', 5000], ['1E-2', 0.01], ['00012', 12],
    ['-0', -0], ['0e999999999999', 0], [' \t7\n', 7], ['﻿8', 8], ['9007199254740993', 9007199254740992],
    ['1.7976931348623157e308', Number.MAX_VALUE], ['2.4703282292062328e-324', 5e-324], ['1.7976931348623159e308', null],
    ['2.4703282292062327e-324', null], ['1e-400', null], ['1e999999999999', null], ['', null], ['.', null], ['+', null], ['1e', null],
    ['1e+', null], ['.e1', null], ['12px', null], ['0x1F', null], ['1_000', null], ['Infinity', null], ['NaN', null], ['1 2', null],
    ['1,5', null], ['\u00859', null], ['١', null], [`0.${'0'.repeat(65535)}1e655360`, null], [`0.${'0'.repeat(70000)}1e70300`, 1e299],
    [`-${'1'.repeat(65536)}e-65630`, -1.1111111111111112e-95], [`${'0'.repeat(70000)}e-1000`, 0], [`1${'0'.repeat(400)}`, null],
    [`1${'0'.repeat(65535)}e-655360`, null]])
    expect(Object.is(x_parseNumber(text), want)).toBe(true);
  for (const [x, want] of [[2.5, 3], [-2.5, -2], [-1.5, -1], [0.49999999999999994, 0], [-0.4, -0], [-0.5, -0], [-0, -0],
    [4503599627370495.5, 4503599627370496], [-4503599627370495.5, -4503599627370495], [Infinity, Infinity], [NaN, NaN]])
    expect(Object.is(x_round(x), want)).toBe(true);
  expect([x_ceil(-0.5), x_ceil(0.1)].map(v => Object.is(v, -0) ? '-0' : v)).toEqual(['-0', 1]);
  for (const [from, to, years, months] of [['1990-06-15', '2026-06-14', 35, 431], ['1990-06-15', '2026-06-15', 36, 432],
    ['2024-02-29', '2025-02-28', 0, 11], ['2024-02-29', '2025-03-01', 1, 12], ['2024-01-31', '2024-02-29', 0, 0],
    ['2024-01-31', '2024-03-01', 0, 1], ['2026-06-14', '1990-06-15', -35, -431], ['2024-03-01', '2024-01-31', 0, -1],
    ['2024-05-05', '2024-05-05', 0, 0], ['2024-02-29', '2024-01-31', 0, 0], ['0000-02-29', '9999-12-31', 9999, 119998], ['2025-02-29', '2026-01-01', null, null],
    ['2024-13-01', '2026-01-01', null, null], ['2024-1-01', '2026-01-01', null, null], ['2024-01-01', ' 2026-01-01', null, null]]) {
    expect(Object.is(x_calendarDiff(from, to, 'years'), years)).toBe(true);
    expect(Object.is(x_calendarDiff(from, to, 'months'), months)).toBe(true);
  }
});

// LLP 1102 §3.2 (decided (c)): format.js's `toFixed` is JavaScript's but "" for a non-finite number (D7), and its
// `formatDecimal` is runner/src/format.rs's exact count (runner/tests/it/format.rs has the same rows).
test('toFixed and formatDecimal are the runner\'s', async () => {
  const { x_toFixed, x_formatDecimal } = await import(webJs('format.js'));
  expect([[1.005, 2], [-0.001, 2], [-0, 2], [2.5, 0], [-2.5, 0], [1e21, 2], [NaN, 2], [Infinity, 0], [-Infinity, 100]].map(([x, d]) => x_toFixed(x, d)))
    .toEqual(['1.00', '-0.00', '0.00', '3', '-3', '1e+21', '', '', '']);
  expect([[1234, 2], [-5, 2], [7, 0], [-0, 2], [0, 0], [5, 20], [-1234567, 3], [9007199254740993, 2], [1e21, 0], [12.5, 2], [NaN, 2], [Infinity, 2], [5e-324, 2]]
    .map(([x, d]) => x_formatDecimal(x, d)))
    .toEqual(['12.34', '-0.05', '7', '0.00', '0', '0.00000000000000000005', '-1234.567', '90071992547409.92', '1000000000000000000000', '', '', '', '']);
  expect(x_formatDecimal(-Number.MAX_VALUE, 2)).toBe('-1797693134862315708145274237317043567980705675258449965989174768031572607800285387605895586327668781715404589535143824642343213268894641827684675467035375169860499105765512820762454900903893289440758685084551339423045832369032229481658085593321233482747978262041447231687381771809192998812504040261841248583.68');
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
// The JS target's storage (LLP 1097 D3, D8, D9): one queue for the module's
// operations, so a read issued after an unawaited write sees it, though the
// store would answer the read first; at most 256 wait behind the one in
// flight, the next refused `full` and journaled; each counts in flight until
// it lands, and a failure is journaled. ts-data.js over a scripted store.
test('storage keeps the order issued, a bound, a count, and a journal', async () => {
  const { data, inflight, journal } = await import(resolve(dir, 'rt.js'));
  const { install } = await import(resolve(dir, 'ts-data.js'));
  install(data);
  const before = inflight.n, ask = (op, value = '') => data.ts('work', [op, value], new Map());
  expect(ask('save', 'one').v).toBe('saved one');
  expect(ask('save', 'two').v).toBe('saved two');
  const read = ask('read');
  expect(inflight.n - before).toBe(3);
  expect(data.background()).toMatchObject({ queued: 2, inFlight: 1 });
  expect(await read.promise).toBe('two');
  expect(await ask('flood').promise).toBe('full');
  expect(journal.some(l => l.endsWith('storage refused: full (writeFile app:/data/flood)'))).toBe(true);
  const drained = async () => {
    for (let i = 0; i < 500 && (data.background().queued || data.background().inFlight); i++) await new Promise(r => setTimeout(r, 2));
    await new Promise(r => setTimeout(r, 10));
  };
  await drained(); // the flood's 257 run in order first
  ask('bad', 'x');
  await drained();
  expect(inflight.n).toBe(before);
  expect(data.background()).toMatchObject({ queued: 0, inFlight: 0, failed: 1, last: 'storage failed: atomicWriteFile app:/data/absent/x: ENOENT filesystem: No such file' });
  expect(journal.some(l => l.endsWith('storage failed: atomicWriteFile app:/data/absent/x: ENOENT filesystem: No such file'))).toBe(true);
});

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


test('a refused derive names itself and only the failed resources it reads', async () => {
  const { memo, res, data } = await import(resolve(dir, 'rt.js') + '?derive-refusal');
  data.answer = () => { throw new Error('ambient Date is refused'); };
  const failed = res('profile', 'loadProfile', () => [], undefined, undefined, 's', '');
  const unrelated = res('other', 'loadOther', () => [], undefined, undefined, 's', '');
  expect(unrelated).toThrow("loadOther");
  const intermediate = memo(() => failed());
  const derive = memo(() => intermediate(), 's', 'displayName');
  expect(derive).toThrow('derive "displayName": resource "profile" (source "loadProfile"): value does not conform to its type; source failed: ambient Date is refused');
  try { derive(); } catch (error) { expect(error.message).not.toContain('loadOther'); }
  expect(memo(() => 1, 's', 'plain')).toThrow('derive "plain": value does not conform to its type');
});


test('a refused derive traces only failed resources it read through retained values', async () => {
  const { memo, res, data } = await import(resolve(dir, 'rt.js') + '?retained-derive-refusal');
  data.answer = () => { throw new Error('storage unavailable'); };
  const unrelated = res('other', 'loadOther', () => [], 1, [], 'n', 0);
  unrelated();
  const count = res('count', 'loadCount', () => [], 0, [], 'n', 0);
  const intermediate = memo(() => count());
  const invalid = memo(() => 1 / intermediate(), 'n', 'inverse');
  expect(invalid).toThrow(/derive "inverse".*resource "count".*source "loadCount".*storage unavailable/);
  expect(invalid).not.toThrow(/loadOther/);
});


test('an input runs a due then before its own action', async () => {
  // Runner::dispatch_at moves the clock first, so a focus answer's `then` runs
  // at the start of the input that follows and reads the value it landed, not
  // the one this input writes (synthetic-then: wasm " a3 T3 T2 T12").
  const { mut, sig, W, commit, on, clock } = await import(resolve(dir, 'rt.js') + '?dispatch-at');
  clock.agent = true;
  const slot = sig(0);
  const m = mut('quick', slot, []);
  const seen = [];
  commit(() => W(slot, 2));
  m.then = () => seen.push('T' + slot());
  m.due = clock.now;
  const el = new EventTarget();
  on(el, 'input', () => { commit(() => W(slot, 12)); seen.push('E' + slot()); });
  el.dispatchEvent(new Event('input'));
  expect(seen).toEqual(['T2', 'E12']);
});

test('an async source failure remains named when its retained value breaks a derive', async () => {
  const { memo, res, data, commit, journal } = await import(resolve(dir, 'rt.js') + '?async-derive-refusal');
  let reject;
  data.answer = () => new Promise((_, fail) => { reject = fail; });
  const count = res('count', 'loadCount', () => [], 0, [], 'n', 0);
  const invalid = memo(() => count.f() ? 1 / count() : 0, 'n', 'inverse');
  expect(commit(() => invalid())).not.toBe(false);
  reject(new Error('database refused'));
  await new Promise(resolve => setTimeout(resolve, 0));
  expect(journal.some(line => /derive "inverse".*resource "count".*source "loadCount".*database refused/.test(line))).toBe(true);
});

// r27 t2: a submit runs in a task after Enter's default, so text typed at once (a driver, a scanner) reached the
// draft first and the action submitted it. The field's next key or edit now runs the pending submit first, before the
// edit applies, so a submit that clears the field keeps the arriving text.
test('a submit runs before the field\'s next key or edit applies; the edit lands after it', async () => {
  const { on } = await import(resolve(dir, 'rt.js'));
  const saved = globalThis.addEventListener;
  try {
    // The handler's own field, both paths; an Enter that bubbled from a textarea inside the handler's element.
    for (const [tag, origin, next] of [['input', 'input', 'beforeinput'], ['input', 'input', 'keydown'], ['main', 'textarea', 'keydown']]) {
      const win = [], field = [];
      globalThis.addEventListener = (type, f) => win.push([type, f]);
      const el = { localName: tag, value: 'Buy milk', addEventListener: (type, f, capture) => field.push([type, f, capture]),
        removeEventListener: (type, f) => { const i = field.findIndex(([t, g]) => t === type && g === f); if (i >= 0) field.splice(i, 1); } };
      const added = [];
      on(el, 'submit', () => { added.push(el.value); el.value = ''; }); // the action submits the draft and clears the bound field
      const enter = { key: 'Enter', isComposing: false, defaultPrevented: false, target: { localName: origin, isContentEditable: false } };
      for (const [type, f] of field.slice()) if (type === 'keydown') f(enter); // the field's own listener
      for (const [type, f] of win.splice(0)) if (type === 'keydown') f(enter); // the window's, last on the path
      // An Enter from a textarea edits (a line break), so only the next key flushes.
      expect(field.some(([type, , capture]) => type === 'beforeinput' && capture)).toBe(origin !== 'textarea');
      for (const [type, f, capture] of field.slice()) if (type === next && capture) f({}); // the next key or edit, before it applies
      el.value += 'B'; // the browser applies it
      await new Promise(r => setTimeout(r, 5));
      expect([added, el.value]).toEqual([['Buy milk'], 'B']); // once, with the submitted text, and the edit kept
      expect(field.filter(([, , capture]) => capture)).toEqual([]);
    }
  } finally { globalThis.addEventListener = saved; }
});
