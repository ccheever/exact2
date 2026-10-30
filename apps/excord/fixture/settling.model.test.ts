// A send's settling on a host with no scrollend, on the repository's scheduler: Excord's own `scrollQuiet` task (its
// gate and key, read from app.contract) armed by host/web-js/schedule.js's `gated` (the JS target's gated tasks,
// LLP 1092 D7), its `scrollSettled` action's body (read from app.contract) run when it fires, and the clock moved by
// hand. It reaches what no end-to-end run does: a landing that stays pending past the quiet's 200 ms (in the real app
// the echo's own window answers "latest" one clock step after the send), cleared later with no scroll report.
import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const ROOT = resolve(import.meta.dir, '../../..');
const contract = readFileSync(resolve(import.meta.dir, '../app.contract'), 'utf8');

// The contract's operators as JS's; the rest of these expressions (names, ==, >, template keys) reads the same.
const js = (text: string) => text.replace(/\band\b/g, '&&').replace(/\bor\b/g, '||').replace(/\bnot\b/g, '!');
const quietLine = contract.split('\n').find(l => l.trim().startsWith('task scrollQuiet '))!;
const [, gateText, keyText] = quietLine.match(/when (.*) key=(.*)$/)!;
const nextLine = contract.split('\n')[contract.split('\n').indexOf(quietLine) + 1].trim();
const quietMs = Number(nextLine.match(/^after\((\d+), scrollSettled\)$/)![1]);

// `scrollSettled`'s body as a JS block: its assignments, `if`/`else` by indentation, its one scrollIntoView.
function body(action: string) {
  const text = contract.split(`  action ${action}\n`)[1].split('\n  action ')[0].split('\n  // ')[0];
  const out: string[] = [], depth = [4];
  for (const line of text.split('\n').filter(l => l.trim())) {
    const indent = line.length - line.trimStart().length, t = line.trim();
    while (indent < depth.at(-1)!) { depth.pop(); out.push('}'); }
    if (t.startsWith('if ')) { out.push(`if (${js(t.slice(3))}) {`); depth.push(indent + 2); }
    else if (t === 'else') { out.push('else {'); depth.push(indent + 2); }
    else if (t.startsWith('scrollIntoView(')) out.push('scrollIntoView();');
    else out.push(`${js(t)};`);
  }
  while (depth.length > 1) { depth.pop(); out.push('}'); }
  return out.join('\n');
}

// schedule.js on stand-ins for the few rt.js pieces `gated` and its gate step read: the clock and its timers.
const schedule = readFileSync(resolve(ROOT, 'host/web-js/schedule.js'), 'utf8')
  .replace(/^import .* from "\.\/rt\.js";\n/m, '').replaceAll('export function ', 'function ');

type State = Record<string, any>;
function run(initial: State, events: [number, State][], key = keyText) {
  const s: State = { scrolling: false, settling: true, endSeen: false, scrollTick: 1, endGap: 120, target: '',
    pointerKey: 'row', hoverKey: '', lastKey: 'last', ...initial };
  const gate = new Function('s', `with (s) { return ${js(gateText)}; }`) as (s: State) => boolean;
  const keyOf = new Function('s', `with (s) { return ${key}; }`) as (s: State) => string;
  const settle = new Function('s', 'scrollIntoView', `with (s) { ${body('scrollSettled')} }`);
  const clock = { now: 0, timers: [] as any[], agent: true };
  let Q: any, serial = 0, fired = 0, corrections = 0;
  const env = { clock, Mutations: [], Tasks: [], journal: [], untracked: (f: () => any) => f(), useSchedule: (q: any) => { Q = q; },
    order: () => serial++, Refusal: Error };
  const { gated } = new Function('env', `with (env) { ${schedule}; return { gated }; }`)(env);
  gated(quietMs, () => { fired++; settle(s, () => { corrections++; }); Q.gates(); return true; }, true, false,
    () => gate(s), () => keyOf(s), 'scrollQuiet');
  const advance = (to: number) => {
    for (;;) {
      const t = clock.timers.find(t => t.due <= to);
      if (!t) break;
      clock.now = t.due; clock.timers.splice(clock.timers.indexOf(t), 1); t.action();
      if (fired > 50) throw new Error('the quiet keeps firing');
    }
    clock.now = to;
  };
  // Each event is a commit: the state it leaves, then the gate step over it.
  for (const [at, change] of events) { advance(at); Object.assign(s, change); Q.gates(); }
  advance(5000);
  return { settling: s.settling, endGap: s.endGap, fired, corrections, pending: clock.timers.length };
}

test('the key is the contract\'s, and takes the landing', () => {
  expect([quietMs, keyText]).toEqual([200, '`${scrollTick}/${endGap}/${target}`']);
});

test('a send\'s landing pending past the quiet, then cleared with no scroll report, still settles', () => {
  // The send: settling, its landing (target "end") pending; the quiet fires at 200 ms, the landing clears at 300 ms.
  const now = run({ target: 'end', endGap: 0 }, [[300, { target: '' }]]);
  expect(now.settling).toBe(false);
  // Before the landing was in the key, the spent quiet never came again: settling stranded.
  const before = run({ target: 'end', endGap: 0 }, [[300, { target: '' }]], '`${scrollTick}/${endGap}`');
  expect([before.settling, before.fired, before.pending]).toEqual([true, 1, 0]);
});

test('a correction that brings no report asks the end once, then settles at the next quiet', () => {
  const r = run({ endGap: 120 }, []);
  expect([r.settling, r.corrections, r.fired]).toEqual([false, 1, 2]);
});

test('where the host has scrollend, the quiet never settles a send, however long it holds', () => {
  const r = run({ endSeen: true, scrolling: true, endGap: 0 }, [[600, { scrollTick: 2 }]]);
  expect([r.settling, r.fired]).toEqual([true, 0]);
});

test('outside settling the quiet runs once after a scroll, as before', () => {
  const r = run({ settling: false, scrolling: true }, []);
  expect([r.fired, r.settling]).toEqual([1, false]);
});
