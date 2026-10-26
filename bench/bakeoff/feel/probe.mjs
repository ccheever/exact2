#!/usr/bin/env bun
// The scroll-feel probe: relaunches an iOS simulator app with the recorder
// (feel.m) inserted, reaches its list with optional steps, plays a fixed
// script of real touches on it through the touch runner
// (host/apple/touches.mjs), and writes trace.jsonl, gestures.json, feel.json
// and feel.txt to --out. The same for any app, whatever it was built with.
//   bun bench/bakeoff/feel/probe.mjs --udid U --app BUNDLE [--steps "tap x,y;wait 1;drag x,y,x,y[,v]"]
//       [--at x,y] [--out DIR] [--sample N] [--dump DIR]
// --sample N measures blank bands every Nth moving frame (0: never; 4); --dump DIR saves the first sampled
// images and any with a blank band.
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { openTouches } from '../../../host/apple/touches.mjs';
import { analyze, pickList, readTrace, report } from './analyze.mjs';

const HERE = dirname(new URL(import.meta.url).pathname);
const USAGE = 'usage: bun bench/bakeoff/feel/probe.mjs --udid <simulator udid> --app <bundle id> [--steps "tap x,y;wait 1;drag x1,y1,x2,y2[,v]"] [--at x,y] [--out DIR] [--sample N] [--dump DIR]';

const opts = {};
for (let i = 2; i < process.argv.length; i++) {
  const k = process.argv[i];
  if (!k.startsWith('--')) { console.error(USAGE); process.exit(2); }
  opts[k.slice(2)] = process.argv[++i];
}
if (!opts.udid || !opts.app) { console.error(USAGE); process.exit(2); }
const udid = opts.udid, app = opts.app;
const out = resolve(opts.out ?? resolve(HERE, '../../../target/feel', `${app}-${new Date().toISOString().replace(/[:.]/g, '-')}`));
mkdirSync(out, { recursive: true });
for (const f of ['feel.json', 'feel.txt', 'gestures.json']) rmSync(resolve(out, f), { force: true });
const tracePath = resolve(out, 'trace.jsonl');
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const say = (s) => console.error(`feel: ${s}`);

// The recorder, rebuilt when its source is newer.
const dylib = resolve(HERE, '../../../target/feel/feel-sim.dylib');
if (!existsSync(dylib) || statSync(dylib).mtimeMs < statSync(resolve(HERE, 'feel.m')).mtimeMs) {
  const b = spawnSync('sh', [resolve(HERE, 'build.sh')], { stdio: 'inherit' });
  if (b.status !== 0) throw new Error('build.sh failed');
}

const simctl = (args, env) => spawnSync('xcrun', ['simctl', ...args], { encoding: 'utf8', env: { ...process.env, ...env } });

/** The trace so far, parsed. */
function trace() {
  try { return readTrace(readFileSync(tracePath, 'utf8')); } catch { return readTrace(''); }
}
const mediaNow = (hello) => hello.media + (Date.now() - hello.wall) / 1000;

/** Until no scroll view has moved for `quiet` ms (the recorder flushes every 250 ms), or `cap` ms. */
async function rest(cap = 6000, quiet = 400) {
  const until = Date.now() + cap;
  await sleep(quiet);
  for (; Date.now() < until; await sleep(100)) {
    const t = trace();
    const last = t.frames.findLast((f) => f.mv);
    if (!last || mediaNow(t.hello) - last.t > (quiet + 250) / 1000) return true;
  }
  return false;
}

const lastFrame = () => trace().frames.at(-1);
/** Blank sampling on or off for the gestures that follow (feel.m reads the switch every 250 ms). */
async function sampling(on) {
  if (on) writeFileSync(`${tracePath}.sample`, ''); else rmSync(`${tracePath}.sample`, { force: true });
  await sleep(300);
}

const touches = await openTouches({ udid, appId: app });
const gestures = [];
let display;
try {
  simctl(['terminate', udid, app]);
  await sleep(500);
  const env = { SIMCTL_CHILD_DYLD_INSERT_LIBRARIES: dylib, SIMCTL_CHILD_FEEL_OUT: tracePath, ...(opts.sample != null ? { SIMCTL_CHILD_FEEL_SAMPLE: opts.sample } : {}), ...(opts.dump ? { SIMCTL_CHILD_FEEL_DUMP: resolve(opts.dump) } : {}) };
  if (opts.dump) mkdirSync(resolve(opts.dump), { recursive: true });
  writeFileSync(tracePath, '');
  const launched = simctl(['launch', udid, app], env);
  if (launched.status !== 0) throw new Error(`launch: ${launched.stderr.trim()}`);
  for (const until = Date.now() + 20000; !trace().hello; await sleep(100)) if (Date.now() > until) throw new Error('the recorder wrote nothing in 20 s: is the app built for the simulator, and the path writable?');
  const hello = trace().hello;
  // The display a gesture names: the runner's when it says (`screen`), else the app's screen.
  const screen = await touches.ask({ op: 'screen' });
  display = screen.displays?.[0] ?? [Math.min(...hello.screen), Math.max(...hello.screen)];
  await sleep(2500); // launch work: first frames, data, the accessibility runtime XCTest turns on

  const fail = (what, r) => { if (r.error) throw new Error(`${what}: ${r.error}`); return r; };
  const tap = async (name, [x, y]) => {
    const r = fail(name, await touches.ask({ op: 'tap', point: [x, y], screen: [x, y], display }));
    gestures.push({ name, kind: 'tap', at: [x, y], start: r.injected.start, end: r.injected.end });
  };
  const drag = async (name, [x1, y1], [x2, y2], velocity, { press = 0.05, hold = 0 } = {}) => {
    const r = fail(name, await touches.ask({ op: 'drag', point: [x1, y1], to: [x2, y2], press, hold, velocity, screen: [x1, y1], screenTo: [x2, y2], display }, 30000));
    gestures.push({ name, kind: 'drag', from: [x1, y1], to: [x2, y2], velocity, press, hold, start: r.injected.start, end: r.injected.end });
  };

  for (const step of (opts.steps ?? '').split(';').map((s) => s.trim()).filter(Boolean)) {
    const [op, arg = ''] = step.split(/\s+/, 2);
    const n = arg.split(',').map(Number);
    if (op === 'tap') await tap(`step tap ${arg}`, n);
    else if (op === 'wait') await sleep(n[0] * 1000);
    else if (op === 'drag') await drag(`step drag ${arg}`, [n[0], n[1]], [n[2], n[3]], n[4] || 1000);
    else throw new Error(`unknown step "${step}"`);
  }
  if (opts.steps) await rest(3000);
  await sleep(600); // the next scan
  const steps = gestures.length;

  const list = pickList(trace().scans.at(-1));
  if (!list) {
    writeFileSync(resolve(out, 'feel.json'), JSON.stringify({ app, verdict: 'no-scrollable-list', scan: trace().scans.at(-1) ?? null }, null, 1));
    writeFileSync(resolve(out, 'feel.txt'), `${app}: no-scrollable-list (no shown scroll view whose content is meaningfully taller than its bounds)\n`);
    say(`no-scrollable-list; ${out}`);
  } else {
    const [fx, fy, fw, fh] = list.frame, [it, , ib] = list.inset;
    const top = Math.max(fy + it, 0), bottom = Math.min(fy + fh - ib, hello.screen[1]);
    const at = opts.at ? opts.at.split(',').map(Number) : null;
    const x = Math.round(at?.[0] ?? fx + fw / 2);
    const span = bottom - top;
    const lo = Math.round(bottom - Math.min(100, span * 0.15)), hi = Math.round(top + Math.min(100, span * 0.15));
    const mid = Math.round(at?.[1] ?? (top + bottom) / 2);
    const flingLen = Math.round(Math.min(400, span * 0.5));
    say(`list #${list.id} ${list.cls} at x ${x}, y ${hi}…${lo}`);

    // Finger tracking: a slow drag, held still, lifted with no velocity.
    await drag('slow-drag', [x, lo], [x, hi], 250, { press: 0.1, hold: 0.5 });
    await rest();
    // Momentum: flings down the list (the finger moves up); blanks are measured in the 4,000 pt/s one.
    for (const [name, v] of [['fling-1500', 1500], ['fling-4000', 4000], ['fling-6000', 6000]]) {
      if (v === 4000) await sampling(true);
      await drag(name, [x, lo], [x, lo - flingLen], v);
      await rest();
      if (v === 4000) await sampling(false);
    }
    // Back to the top, measuring blanks, each fling 1.2 s after the last; the last meets the edge with momentum.
    await sampling(true);
    for (let i = 0; i < 8; i++) {
      const f = lastFrame();
      if (f && f.oy <= -f.it + 1) break;
      await drag(`fling-up-${i + 1}`, [x, hi], [x, hi + flingLen], 8000);
      await sleep(1200);
    }
    await rest(8000);
    await sampling(false);
    // Rubber band: pulled past the top, held, released.
    await drag('pull-top', [x, hi], [x, Math.min(hi + 240, bottom - 10)], 400, { press: 0.1, hold: 0.3 });
    await rest(4000);
    // Scroll to top from the status bar.
    await drag('fling-partway', [x, lo], [x, lo - flingLen], 3000);
    await rest();
    const safeTop = trace().scans.at(-1)?.safeTop || 47;
    await tap('status-bar', [Math.round(hello.screen[0] * 0.2), Math.round(safeTop / 2)]);
    await rest(4000);
    // A tap during deceleration stops the list and activates nothing.
    await drag('fling-for-tap', [x, lo], [x, lo - flingLen], 4000);
    await sleep(200);
    await tap('tap-decelerating', [x, mid]);
    await rest(4000);
    await sleep(600); // a pushed screen's scan
  }
  writeFileSync(resolve(out, 'gestures.json'), JSON.stringify({ app, udid, display, steps, gestures }, null, 1));
} finally {
  rmSync(`${tracePath}.sample`, { force: true });
  await touches.close().catch((e) => say(`closing the touch runner: ${e.message}`));
  simctl(['terminate', udid, app]);
}

if (existsSync(resolve(out, 'gestures.json')) && !existsSync(resolve(out, 'feel.json'))) {
  const result = analyze(trace(), JSON.parse(readFileSync(resolve(out, 'gestures.json'), 'utf8')));
  writeFileSync(resolve(out, 'feel.json'), JSON.stringify(result, null, 1));
  writeFileSync(resolve(out, 'feel.txt'), report(result));
  process.stdout.write(report(result));
  say(out);
}
