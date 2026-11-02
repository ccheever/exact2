import { expect, test } from 'bun:test';
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

// A data module's `scheduler` on the JS target (LLP 1141.001 D10), ts-fetch.js as it is: with no
// browser `scheduler.yield` (Bun has none, nor has Safari), a yield resolves in a task of its own, after
// the microtasks queued before it; `postTask` refuses, naming the yield; and an overlay may not yield.
const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');

test('a yield without the browser scheduler resolves in a later task; postTask and an overlay refuse', async () => {
  expect(globalThis.scheduler).toBeUndefined();
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-scheduler-'));
  cpSync(resolve(ROOT, 'host/web-js/ts-fetch.js'), resolve(dir, 'ts-fetch.js'));
  writeFileSync(resolve(dir, 'admission.js'), `export { ownWithout, redirectOf, signalDeadline } from ${JSON.stringify(pathToFileURL(resolve(ROOT, 'host/web-js/admission.js')).href)};
export class FetchError extends Error {} export const overlaying = { on: false }; export const inOverlay = what => new Error(what + ' in an overlay');
export const fetchWith = () => Promise.reject(new Error('no fetch'));\n`);
  writeFileSync(resolve(dir, 'admission-data.js'), 'export const tsGrantSet = {};\n');
  try {
    const { scheduler } = await import(pathToFileURL(resolve(dir, 'ts-fetch.js')).href);
    const { overlaying } = await import(pathToFileURL(resolve(dir, 'admission.js')).href);
    const order = [];
    const yielded = scheduler.yield().then(() => order.push('yield'));
    let chained = Promise.resolve();
    for (let i = 0; i < 64; i++) chained = chained.then(() => order.push(i));
    await yielded;
    expect(order.at(-1)).toBe('yield');
    expect(() => scheduler.postTask(() => {})).toThrow('await scheduler.yield()');
    overlaying.on = true;
    await expect(scheduler.yield()).rejects.toThrow('scheduler.yield() in an overlay');
    overlaying.on = false;
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('sample history keeps only its requested tail across large clock jumps and hidden spans', async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-sample-bound-'));
  cpSync(resolve(ROOT, 'host/web-js/samples.js'), resolve(dir, 'samples.js'));
  writeFileSync(resolve(dir, 'rt.js'), 'export const clock={now:0};');
  try {
    const { samples } = await import(pathToFileURL(resolve(dir, 'samples.js')).href);
    const { clock } = await import(pathToFileURL(resolve(dir, 'rt.js')).href);
    const row = { key: 'motion.acceleration', sub: { samples: { frequency: 'frequency', bound: 'history' }, default: { x: 1 } } };
    for (const history of [0, 3]) {
      clock.now = 0; const source = samples(), args = { frequency: 60, history };
      source.answered(row, 'reader', source.substitute(row, args, 'reader'));
      clock.now = 3_600_000;
      let pushes = 0;
      const original = Array.prototype.push;
      Array.prototype.push = function (...args) { pushes += args.length; return original.apply(this, args); };
      let result;
      try { result = source.substitute(row, args, 'reader'); } finally { Array.prototype.push = original; }
      expect(pushes).toBeLessThanOrEqual(5);
      expect(result.samples).toHaveLength(history);
      expect(result.dropped).toBe(history ? 216000 - history : 0);
      expect(result.sequence).toBe(216000);
    }
    clock.now = 0; const source = samples(), args = { frequency: 10, history: 3 };
    source.answered(row, 'reader', source.substitute(row, args, 'reader'));
    clock.now = 2000; source.visibility(true);
    clock.now = 5000; source.visibility(false);
    clock.now = 10000;
    let result = source.substitute(row, args, 'reader');
    expect(result.samples.map(x => x.timestamp)).toEqual([9800, 9900, 10000]);
    expect([result.dropped, result.interrupted]).toEqual([67, true]);
    clock.now = 9900; source.visibility(true); clock.now = 10000;
    result = source.substitute(row, args, 'reader');
    expect(result.samples.map(x => x.timestamp)).toEqual([9600, 9700, 9800]);
    expect(result.dropped).toBe(65);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('live tool deadlines refresh time and arm a wake without unrelated commits; the agent clock stays explicit', async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-call-clock-')), old = globalThis.exact;
  cpSync(resolve(ROOT, 'host/web-js/calls.js'), resolve(dir, 'calls.js'));
  // Keep the production dispatcher: its callback result decides whether an advance continues.
  const runtime = readFileSync(resolve(ROOT, 'host/web-js/rt.js'), 'utf8');
  const advance = runtime.slice(runtime.indexOf('export function advance('), runtime.indexOf('let driving ='));
  expect(advance).toContain('function fire(');
  writeFileSync(resolve(dir, 'rt.js'), `export const clock={now:0,agent:false,timers:[]},wall={now:0},data={},After=[],Clocked=[],Mutations=[],Origin={now:()=>'app'},journal=[],say=x=>journal.push(x),order=()=>0,drive=()=>{};
const Link={},Poisoned=false;let Timing=false,Instant=null,start=0;
export const time=()=>{if(!clock.agent)clock.now=Math.max(clock.now,wall.now)};\n${advance}`);
  writeFileSync(resolve(dir, 'held.js'), 'export const Counting=new Set(),heldTickets=()=>[];');
  try {
    globalThis.exact = {};
    const { tools } = await import(pathToFileURL(resolve(dir, 'calls.js')).href);
    const { clock, wall, After, advance } = await import(pathToFileURL(resolve(dir, 'rt.js')).href);
    const { Counting } = await import(pathToFileURL(resolve(dir, 'held.js')).href);
    tools({ run: [() => () => Counting.add({}), {}, 'reads', [false, true, true], 'run'] });
    const calls = globalThis.exact.calls;
    const first = calls.call({ tool: 'run' }); await Promise.resolve();
    expect(clock.timers.map(t => t.due)).toEqual([10000]);
    wall.now = 10001; clock.timers.shift().action();
    expect(calls.called(first.call).outcome).toBe('timeout');
    let waking = true;
    globalThis.exact.extensions = { wake: () => waking ? {} : null };
    const queued = calls.call({ tool: 'run' }); await Promise.resolve();
    expect(clock.timers.map(t => t.due)).toEqual([15001]);
    wall.now = 15002; clock.timers.shift().action();
    expect(calls.called(queued.call)).toMatchObject({ outcome: 'refused', reason: 'busy' });
    waking = false; Counting.clear(); clock.agent = true;
    const driven = calls.call({ tool: 'run' }); await Promise.resolve();
    wall.now = 1000000;
    for (const f of After) f(); await Promise.resolve();
    expect(calls.called(driven.call).state).toBe('running');
    expect(clock.now).toBe(15002);
    let later = false;
    clock.timers.push({ due: 30000, once: true, data: true, action: () => { later = true; return true; } });
    expect(advance(35000, false)).toBe(false);
    expect(clock.now).toBe(35000);
    expect(later).toBe(true);
    expect(calls.called(driven.call).outcome).toBe('timeout');
  } finally { globalThis.exact = old; rmSync(dir, { recursive: true, force: true }); }
});
