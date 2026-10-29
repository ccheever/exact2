// A restore that gives up, on the repository's scheduler: Excord's own `land` task (its gate, key and delay, read
// from app.contract) armed by host/web-js/schedule.js's `gated`, its `landJump` action and the list's `scrolled`
// handler (their bodies read from app.contract, run with the contract's rule that every statement reads the state
// the action started with), on a clock moved by hand. It holds the order the web (rt.js runs overdue timers before
// a scroll handler) and Linux and Android (the report after the paint's ack) give a reopen: the landing's timer
// fires before the remounted list's first report, with `endGap` still the channel left's.
import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const ROOT = resolve(import.meta.dir, '../../..');
const contract = readFileSync(resolve(import.meta.dir, '../app.contract'), 'utf8');
const lines = contract.split('\n');

// The contract's expressions as JS's: its operators, `max`/`round`, and a `match` on an option.
const js = (text: string) => text
  .replace(/match (\S+) \{ case some\((\w+)\) => \2, case none => ("[^"]*") \}/g, '($1 ?? $3)')
  .replace(/\band\b/g, '&&').replace(/\bor\b/g, '||').replace(/\bnot\b/g, '!')
  .replace(/\bmax\(/g, 'Math.max(').replace(/\bround\(/g, 'Math.round(');

// An action's body as a JS block: `let`s, assignments, `if`/`else` by indentation; a send or a host call is a call.
function body(head: string) {
  const start = lines.findIndex(l => l.startsWith(`  action ${head}`));
  const out: string[] = [], depth = [4];
  for (const line of lines.slice(start + 1)) {
    if (!line.startsWith('    ') && line.trim()) break;
    const t = line.trim();
    if (!t || t.startsWith('//')) continue;
    const indent = line.length - line.trimStart().length;
    while (indent < depth.at(-1)!) { depth.pop(); out.push('}'); }
    if (t.startsWith('if ')) { out.push(`if (${js(t.slice(3))}) {`); depth.push(indent + 2); }
    else if (t === 'else') { out.push('else {'); depth.push(indent + 2); }
    else if (t.startsWith('send ')) out.push(`${js(t.replace(/^send \w+ = /, ''))};`);
    else if (t.startsWith('scrollIntoView(')) out.push('scrollIntoView();');
    else out.push(`${js(t)};`);
  }
  while (depth.length > 1) { depth.pop(); out.push('}'); }
  return out.join('\n');
}

// Run one action: reads from the state it started with, its assignments landing when it ends (LLP 1089 D2).
function action(code: string, params: string[]) {
  const f = new Function('view', ...params, `with (view) { ${code} }`);
  return (s: Record<string, any>, ...args: any[]) => {
    const start = { ...s }, writes: Record<string, any> = {};
    const view = new Proxy({}, {
      has: (_, k) => typeof k === 'string' && k in start,
      get: (_, k) => (k === Symbol.unscopables ? undefined : start[k as string]),
      set: (_, k, v) => { writes[k as string] = v; return true; },
    });
    f(view, ...args);
    Object.assign(s, writes);
  };
}

const schedule = readFileSync(resolve(ROOT, 'host/web-js/schedule.js'), 'utf8')
  .replace(/^import .* from "\.\/rt\.js";\n/m, '').replaceAll('export function ', 'function ');
const landLine = lines.find(l => l.trim().startsWith('task land '))!;
const [, landGate, landKey] = landLine.match(/when (.*) key=(.*)$/)!;
const landMs = Number(lines[lines.indexOf(landLine) + 1].trim().match(/^after\((\d+), landJump\)$/)![1]);
const ackLine = lines.find(l => l.trim().startsWith('task ack '))!.match(/when (.*) key=/)![1];
const followLine = lines.find(l => l.trim().startsWith('task follow '))!.match(/when (.*) key=/)![1];

const KEYS = Array.from({ length: 148 }, (_, i) => `r${i}`);
// A reopen of #general at a saved place whose row is gone, the window around its anchor reaching the live end.
function reopen() {
  const s: Record<string, any> = {
    open: 'general', anchor: 'r20', target: 'gone', targetBlock: 'place', restoreOffset: 0, highlight: '', away: false,
    settling: false, scrolling: false, hoverKey: '', scrollTick: 0, topKey: 'elsewhere', topOffset: 0, leadHeld: false,
    endGap: 1823, historyStart: false, baseArrived: 0, followed: 5, reading: true, signedOutNow: false,
    current: true, keys: KEYS, rows: KEYS.map(key => ({ key, id: key })), lastKey: KEYS.at(-1),
    win: { hasAfter: false, hasBefore: true, pin: 'r20', firstId: 'r0', lastId: 'r147' },
    chan: { arrived: 6, unread: 1 }, page: { visibilityState: 'visible' },
    jumpFromAnchor: '', jumpFromKey: '', jumpFromOffset: 0,
  };
  const env = { keyOf: (rows: any[], id: string) => rows.find(r => r.id === id)?.key ?? '', pending: () => false,
    frame: () => ({ height: 72 }), markRead: () => {} };
  const landJump = action(body('landJump'), Object.keys(env).concat('scrollIntoView'));
  const report = action(body('scrolled(x: number, y: number, e: ScrollEvent)'), ['x', 'y', 'e', ...Object.keys(env)]);
  const clock = { now: 0, timers: [] as any[], agent: true };
  let Q: any, serial = 0;
  const { gated } = new Function('env', `with (env) { ${schedule}; return { gated }; }`)({ clock, Mutations: [], Tasks: [],
    journal: [], untracked: (f: () => any) => f(), useSchedule: (q: any) => { Q = q; }, order: () => serial++, Refusal: Error });
  const gateOf = (text: string) => new Function('s', 'pending', `with (s) { return ${js(text)}; }`);
  const landOn = gateOf(landGate), landBy = new Function('s', `with (s) { return ${landKey}; }`);
  const order: string[] = [];
  gated(landMs, () => { order.push('land'); landJump(s, ...Object.values(env), () => {}); Q.gates(); return true; }, true, false,
    () => landOn(s, env.pending), () => landBy(s), 'land');
  const advance = (to: number) => {
    for (;;) {
      const t = clock.timers.find(t => t.due <= to);
      if (!t) break;
      clock.now = t.due; clock.timers.splice(clock.timers.indexOf(t), 1); t.action();
    }
    clock.now = to;
  };
  // The landing's timer is overdue as the list's first report arrives: it fires first, then the report (at the end).
  advance(1);
  order.push('report');
  report(s, 0, 0, { firstVisibleKey: KEYS.at(-12), firstVisibleOffset: 0, scrollTop: 9000, scrollHeight: 9600, clientHeight: 600 },
    ...Object.values(env));
  Q.gates();
  return { s, order, ack: gateOf(ackLine)(s, env.pending), follow: gateOf(followLine)(s, env.pending) };
}

test('a reopen that gives up: the landing fires before the list\'s first report, which finds the reader at the end', () => {
  const { s, order, ack, follow } = reopen();
  expect([order, s.target, s.endGap, s.away]).toEqual([['land', 'report'], '', 0, false]);
  // The end report clears the anchor left over, so an arrival there is read and followed.
  expect([s.anchor, ack, follow]).toEqual(['', true, true]);
});

// One report at the live end, from a given state: what it leaves of the anchor.
function endReport(change: Record<string, any>, top = KEYS.at(-12)) {
  const s: Record<string, any> = {
    open: 'general', anchor: 'r20', target: '', away: false, settling: false, scrolling: true, hoverKey: '', scrollTick: 3,
    topKey: top, topOffset: 0, leadHeld: false, endGap: 0, historyStart: false, baseArrived: 0, followed: 5, reading: true,
    current: true, keys: KEYS, lastKey: KEYS.at(-1), win: { hasAfter: false, hasBefore: true, pin: 'r20', firstId: 'r0', lastId: 'r147' },
    chan: { arrived: 6, unread: 1 }, ...change,
  };
  const env = { frame: () => ({ height: 72 }), markRead: () => {} };
  action(body('scrolled(x: number, y: number, e: ScrollEvent)'), ['x', 'y', 'e', ...Object.keys(env)])(
    s, 0, 0, { firstVisibleKey: top, firstVisibleOffset: 0, scrollTop: 9000, scrollHeight: 9600, clientHeight: 600 }, ...Object.values(env));
  return s.anchor;
}

test('an end report leaves a held anchor alone', () => {
  // A placement or jump still owed (target set, which a placement's leadHeld follows), a send's return settling,
  // a window of older history (where F1's lead runs: away, or an anchor with hasAfter), a reader away.
  expect(endReport({ target: 'r30' })).toBe('r20');
  expect(endReport({ settling: true })).toBe('r20');
  expect(endReport({ win: { hasAfter: true, hasBefore: true, pin: 'r20', firstId: 'r0', lastId: 'r147' } }, 'r70')).toBe('r20');
  expect(endReport({ away: true })).toBe('r20');
  // (Rows away from both of the window's edges, so the report's own paging, below, leaves the anchor alone.)
  // Near the window's first row the report pages older, as before: that anchor is the report's own, set after the clear.
  expect(endReport({}, 'r5')).toBe('r0');
  // Otherwise, at the live end, a leftover anchor goes.
  expect(endReport({})).toBe('');
});
