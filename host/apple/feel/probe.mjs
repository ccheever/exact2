#!/usr/bin/env bun
// The scroll-feel probe: relaunches an iOS simulator app with the recorder
// (feel.m) inserted, reaches its list with optional steps, plays a fixed
// script of real touches on it through the touch runner
// (host/apple/touches.mjs), and writes trace.jsonl, gestures.json, feel.json
// and feel.txt to --out. The same for any app, whatever it was built with.
// `verify ios` plays the short script (`play`) in a drive of its own.
//   bun host/apple/feel/probe.mjs --udid U --app BUNDLE [--steps "tap x,y;wait 1;drag x,y,x,y[,v]"]
//       [--at x,y] [--out DIR] [--sample N] [--dump DIR] [--short]
// --sample N measures blank bands every Nth moving frame (0: never; 4); --dump DIR saves the first sampled
// images and any with a blank band; --short plays verify's script.
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { openTouches } from '../touches.mjs';
import { analyze, pickList, readTrace, report } from './analyze.mjs';

const HERE = dirname(new URL(import.meta.url).pathname);
const ROOT = resolve(HERE, '../../..');
const USAGE = 'usage: bun host/apple/feel/probe.mjs --udid <simulator udid> --app <bundle id> [--steps "tap x,y;wait 1;drag x1,y1,x2,y2[,v]"] [--at x,y] [--out DIR] [--sample N] [--dump DIR] [--short]';
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** The recorder for the simulator, built when missing or older than feel.m. */
export function recorder() {
  const dylib = resolve(ROOT, 'target/feel/recorder-sim.dylib'), source = resolve(HERE, 'feel.m');
  if (existsSync(dylib) && statSync(dylib).mtimeMs >= statSync(source).mtimeMs) return dylib;
  mkdirSync(dirname(dylib), { recursive: true });
  const b = spawnSync('xcrun', ['-sdk', 'iphonesimulator', 'clang', '-dynamiclib', '-fobjc-arc', '-O2', '-Wall', '-target', 'arm64-apple-ios17.0-simulator',
    '-framework', 'UIKit', '-framework', 'QuartzCore', '-framework', 'CoreGraphics', '-framework', 'Foundation',
    source, '-o', dylib, '-install_name', '@rpath/feel.dylib'], { encoding: 'utf8' });
  if (b.status !== 0) throw new Error(`building the recorder failed: ${(b.stderr || b.stdout).trim().split('\n').slice(-3).join(' ')}`);
  return dylib;
}

/** The environment that inserts the recorder into an app, writing to `tracePath`; simctl passes each as SIMCTL_CHILD_<name>. */
export const recorderEnv = (tracePath, { sample, dump } = {}) => ({ DYLD_INSERT_LIBRARIES: recorder(), FEEL_OUT: tracePath,
  ...(sample != null ? { FEEL_SAMPLE: String(sample) } : {}), ...(dump ? { FEEL_DUMP: resolve(dump) } : {}) });

/** The trace so far, parsed. */
export function traceOf(tracePath) {
  try { return readTrace(readFileSync(tracePath, 'utf8')); } catch { return readTrace(''); }
}

/** Plays the gesture script on the largest list the app shows (after `steps`), recording into `tracePath` (the
 * recorder must already run in the app). `script`: 'full', the calibrated one (README.md), or 'short', verify's.
 * `{ display, steps, gestures, list }`; `list` is null when no shown scroll view qualifies (`pickList`). */
export async function play({ touches, tracePath, steps = '', at = null, script = 'full', say = () => {} }) {
  const trace = () => traceOf(tracePath);
  for (const until = Date.now() + 20000; !trace().hello; await sleep(100)) if (Date.now() > until) throw new Error('the recorder wrote nothing in 20 s: is the app built for the simulator, and the path writable?');
  const hello = trace().hello;
  const mediaNow = () => hello.media + (Date.now() - hello.wall) / 1000;
  /** Until no scroll view has moved for `quiet` ms (the recorder flushes every 250 ms), or `cap` ms. */
  const rest = async (cap = 6000, quiet = 400) => {
    const until = Date.now() + cap;
    await sleep(quiet);
    for (; Date.now() < until; await sleep(100)) {
      const last = trace().frames.findLast((f) => f.mv);
      if (!last || mediaNow() - last.t > (quiet + 250) / 1000) return true;
    }
    return false;
  };
  const lastFrame = () => trace().frames.at(-1);
  /** Blank sampling on or off for the gestures that follow (feel.m reads the switch every 250 ms). */
  const sampling = async (on) => {
    if (on) writeFileSync(`${tracePath}.sample`, ''); else rmSync(`${tracePath}.sample`, { force: true });
    await sleep(300);
  };
  // The display a gesture names: the runner's when it says (`screen`), else the app's screen.
  const screen = await touches.ask({ op: 'screen' });
  const display = screen.displays?.[0] ?? [Math.min(...hello.screen), Math.max(...hello.screen)];
  const gestures = [];
  const fail = (what, r) => { if (r.error) throw new Error(`${what}: ${r.error}`); return r; };
  const tap = async (name, [x, y]) => {
    const r = fail(name, await touches.ask({ op: 'tap', point: [x, y], screen: [x, y], display }));
    gestures.push({ name, kind: 'tap', at: [x, y], start: r.injected.start, end: r.injected.end });
  };
  const drag = async (name, [x1, y1], [x2, y2], velocity, { press = 0.05, hold = 0 } = {}) => {
    const r = fail(name, await touches.ask({ op: 'drag', point: [x1, y1], to: [x2, y2], press, hold, velocity, screen: [x1, y1], screenTo: [x2, y2], display }, 30000));
    gestures.push({ name, kind: 'drag', from: [x1, y1], to: [x2, y2], velocity, press, hold, start: r.injected.start, end: r.injected.end });
  };
  try {
    for (const step of steps.split(';').map((s) => s.trim()).filter(Boolean)) {
      const [op, arg = ''] = step.split(/\s+/, 2);
      const n = arg.split(',').map(Number);
      if (op === 'tap') await tap(`step tap ${arg}`, n);
      else if (op === 'wait') await sleep(n[0] * 1000);
      else if (op === 'drag') await drag(`step drag ${arg}`, [n[0], n[1]], [n[2], n[3]], n[4] || 1000);
      else throw new Error(`unknown step "${step}"`);
    }
    if (steps) await rest(3000);
    await sleep(600); // the next scan
    const stepCount = gestures.length;
    const list = pickList(trace().scans.at(-1));
    if (!list) return { display, steps: stepCount, gestures, list: null, scan: trace().scans.at(-1) ?? null };
    const [fx, fy, fw, fh] = list.frame, [it, , ib] = list.inset;
    const top = Math.max(fy + it, 0), bottom = Math.min(fy + fh - ib, hello.screen[1]);
    const x = Math.round(at?.[0] ?? fx + fw / 2);
    const span = bottom - top;
    const lo = Math.round(bottom - Math.min(100, span * 0.15)), hi = Math.round(top + Math.min(100, span * 0.15));
    const mid = Math.round(at?.[1] ?? (top + bottom) / 2);
    const flingLen = Math.round(Math.min(400, span * 0.5));
    // A list that opens at its end (a chat) has no room to move down it: the slow drag and the first flings go up
    // it instead, the finger moving down.
    const atEnd = list.off[1] > -it + 50 && list.off[1] >= list.cs[1] - fh + ib - 1;
    const onward = (name, v, opts) => atEnd ? drag(name, [x, hi], [x, hi + flingLen], v, opts) : drag(name, [x, lo], [x, lo - flingLen], v, opts);
    if (atEnd) say('the list opens at its end: the first gestures go up it');
    say(`list #${list.id} ${list.cls} at x ${x}, y ${hi}…${lo}`);
    const statusBar = async () => {
      // The right of the bar: its left may hold the back-to-app breadcrumb ("◀ Settings"), which leaves the app.
      const safeTop = trace().scans.at(-1)?.safeTop || 47;
      await tap('status-bar', [Math.round(hello.screen[0] * 0.8), Math.round(safeTop / 2)]);
      await rest(4000);
    };
    const pull = async () => {
      const f = lastFrame();
      if (f && f.oy > -f.it + 1) return say('the list is not at its top: no pull'); // a long list the flings did not bring back
      await drag('pull-top', [x, hi], [x, Math.min(hi + 240, bottom - 10)], 400, { press: 0.1, hold: 0.3 });
      await rest(4000);
    };
    const tapWhileMoving = async () => {
      await drag('fling-for-tap', [x, lo], [x, lo - flingLen], 4000);
      await sleep(200);
      await tap('tap-decelerating', [x, mid]);
      await rest(4000);
      await sleep(600); // a pushed screen's scan
    };

    // Finger tracking: a slow drag, held still, lifted with no velocity.
    await drag('slow-drag', atEnd ? [x, hi] : [x, lo], atEnd ? [x, lo] : [x, hi], 250, { press: 0.1, hold: 0.5 });
    await rest();
    if (script === 'short') {
      // Momentum and the lift: the fastest fling unsampled; blanks in the 4,000 pt/s one. The list is far from the top
      // now, so the status bar's tap is measured, which leaves it at the top for the pull.
      for (const [name, v] of [['fling-6000', 6000], ['fling-4000', 4000]]) {
        if (v === 4000) await sampling(true);
        await onward(name, v);
        await rest();
        if (v === 4000) await sampling(false);
      }
      await statusBar();
      await pull();
      await tapWhileMoving();
    } else {
      // Momentum: flings down the list (the finger moves up); blanks are measured in the 4,000 pt/s one.
      for (const [name, v] of [['fling-1500', 1500], ['fling-4000', 4000], ['fling-6000', 6000]]) {
        if (v === 4000) await sampling(true);
        await onward(name, v);
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
      await pull();
      // Scroll to top from the status bar.
      await drag('fling-partway', [x, lo], [x, lo - flingLen], 3000);
      await rest();
      await statusBar();
      // A tap during deceleration stops the list and activates nothing.
      await tapWhileMoving();
    }
    return { display, steps: stepCount, gestures, list };
  } finally {
    rmSync(`${tracePath}.sample`, { force: true });
  }
}

/** Writes feel.json and feel.txt to `out` for what `play` returned; the result (`analyze`'s, or a no-scrollable-list one). */
export function finish(out, app, played, udid) {
  const tracePath = resolve(out, 'trace.jsonl');
  writeFileSync(resolve(out, 'gestures.json'), JSON.stringify({ app, udid, display: played.display, steps: played.steps, gestures: played.gestures }, null, 1));
  const result = played.list
    ? analyze(traceOf(tracePath), { app, steps: played.steps, gestures: played.gestures })
    : { app, verdict: 'no-scrollable-list', scan: played.scan ?? null };
  writeFileSync(resolve(out, 'feel.json'), JSON.stringify(result, null, 1));
  writeFileSync(resolve(out, 'feel.txt'), played.list ? report(result) : `${app}: no-scrollable-list (no shown scroll view whose content is meaningfully taller than its bounds)\n`);
  return result;
}

if (import.meta.main) {
  const opts = {};
  for (let i = 2; i < process.argv.length; i++) {
    const k = process.argv[i];
    if (!k.startsWith('--')) { console.error(USAGE); process.exit(2); }
    if (k === '--short') opts.short = true; else opts[k.slice(2)] = process.argv[++i];
  }
  if (!opts.udid || !opts.app) { console.error(USAGE); process.exit(2); }
  const { udid, app } = opts;
  const say = (s) => console.error(`feel: ${s}`);
  const out = resolve(opts.out ?? resolve(ROOT, 'target/feel', `${app}-${new Date().toISOString().replace(/[:.]/g, '-')}`));
  mkdirSync(out, { recursive: true });
  for (const f of ['feel.json', 'feel.txt', 'gestures.json']) rmSync(resolve(out, f), { force: true });
  const tracePath = resolve(out, 'trace.jsonl');
  const simctl = (args, env) => spawnSync('xcrun', ['simctl', ...args], { encoding: 'utf8', env: { ...process.env, ...env } });
  const env = Object.fromEntries(Object.entries(recorderEnv(tracePath, opts)).map(([k, v]) => [`SIMCTL_CHILD_${k}`, v]));
  if (opts.dump) mkdirSync(resolve(opts.dump), { recursive: true });
  const touches = await openTouches({ udid, appId: app });
  let played = null;
  try {
    simctl(['terminate', udid, app]);
    await sleep(500);
    writeFileSync(tracePath, '');
    const launched = simctl(['launch', udid, app], env);
    if (launched.status !== 0) throw new Error(`launch: ${launched.stderr.trim()}`);
    for (const until = Date.now() + 20000; !traceOf(tracePath).hello; await sleep(100)) if (Date.now() > until) throw new Error('the recorder wrote nothing in 20 s: is the app built for the simulator, and the path writable?');
    await sleep(2500); // launch work: first frames, data, the accessibility runtime XCTest turns on
    played = await play({ touches, tracePath, steps: opts.steps ?? '', at: opts.at ? opts.at.split(',').map(Number) : null, script: opts.short ? 'short' : 'full', say });
  } finally {
    await touches.close().catch((e) => say(`closing the touch runner: ${e.message}`));
    simctl(['terminate', udid, app]);
  }
  const result = finish(out, app, played, udid);
  if (result.verdict === 'probed') process.stdout.write(report(result));
  say(result.verdict === 'probed' ? out : `no-scrollable-list; ${out}`);
}
