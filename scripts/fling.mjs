#!/usr/bin/env bun
// `bun scripts/fling.mjs ios <app> --target <scroller> [--to "<op>" …] [--flings N] [--distance <pt>] [--json]`
// (LLP 1144 D1): an app's list flung as a person flings it, and what that cost.
//
// The app runs on the agent's socket under platform timing and real touches, so
// the frames are the display's and each fling is UIKit's own tracking and
// deceleration. `--to` ops (`tap <target>`, `type <target> <text>`, `wait <ms>`)
// reach the screen first; none takes the clock. The frame sampler's segment is
// held across the flings, so a list built up front is sampled as it glides.
// What it prints is diagnosis: a simulator's frames are the Mac's, so no frame
// budget is judged here; `--fail-on late` makes late frames an exit status.
import { open } from './agent.mjs';
import { perfDelta } from './agent-inspect.mjs';
import { resolveApp } from './app.mjs';
import { driveServer } from './verify.mjs';

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
/** How long each fling's finger moves: short, so the lift carries the velocity. */
const FLING_MS = 60;

/** Flags and the `--to` ops, in order. */
export function parseArgs(argv) {
  const [host, app, ...rest] = argv;
  const out = { host, app, to: [], flings: 6, distance: 600, target: null, json: false, failOn: [], storage: null };
  for (let i = 0; i < rest.length; i++) {
    const a = rest[i], next = () => rest[++i];
    if (a === '--to') out.to.push(next());
    else if (a === '--target') out.target = next();
    else if (a === '--flings') out.flings = Number(next());
    else if (a === '--distance') out.distance = Number(next());
    else if (a === '--storage') out.storage = next();
    else if (a === '--fail-on') out.failOn = next().split(',');
    else if (a === '--json') out.json = true;
    else throw new Error(`fling: unknown argument ${a}`);
  }
  if (!['ios', 'macos'].includes(host) || !app || !out.target) throw new Error('usage: bun scripts/fling.mjs ios|macos <app> --target <scroller testId> [--to "<op>" …] [--flings N] [--distance <pt>] [--storage <name>] [--fail-on late] [--json]\n  macos: the flings are yours, on the trackpad; it holds the frames until you press Enter');
  if (!(out.flings > 0) || !(out.distance > 0)) throw new Error('fling: --flings and --distance are positive numbers');
  return out;
}

/** The frames between two reads of `perf frames`: what was presented, late and
 * missed in the window, its quantiles, and the late frames' phases summed. */
export function framesBetween(before, after) {
  const from = before.window?.to ?? -Infinity;
  const records = (after.records ?? []).filter((r) => r.t > from);
  const late = (after.late ?? []).filter((r) => r.t > from);
  const xs = records.map((r) => r.interval).sort((a, b) => a - b);
  const q = (f) => xs.length ? xs[Math.min(xs.length - 1, Math.floor(f * xs.length))] : null;
  const phases = new Map();
  let unattributed = 0;
  for (const l of late) {
    for (const p of l.phases ?? []) phases.set(p.phase, (phases.get(p.phase) ?? 0) + p.ms);
    unattributed += l.unattributed ?? 0;
  }
  const d = (k) => (after.lifetime?.[k] ?? 0) - (before.lifetime?.[k] ?? 0);
  const r2 = (x) => Math.round(x * 100) / 100;
  return {
    presented: d('presented'), late: d('late'), missed: d('missed'), overruns: d('overruns'),
    samples: records.length, period: after.period?.ms ?? null, p50: q(0.5), p95: q(0.95), p99: q(0.99), max: xs.at(-1) ?? null,
    phases: [...phases].map(([phase, ms]) => ({ phase, ms: r2(ms) })).sort((a, b) => b.ms - a.ms),
    unattributed: r2(unattributed),
    keptLate: late.length,
  };
}

framesBetween.records = (before, after) => (after.records ?? []).filter((r) => r.t > (before.window?.to ?? -Infinity));

/** What the tracked scroller did frame by frame, while momentum alone moved it
 * (`d`): steps far larger than their neighbours' (a jump an anchor or a bounce
 * does not explain) and frames where it stood still mid-glide (a stall).
 * Diagnostic: neither is judged until checked against known cases (LLP 1144 D1). */
export function motionOf(records) {
  const xs = records.filter((r) => Array.isArray(r.scroll));
  const jumps = [], stalls = [];
  for (let i = 1; i < xs.length; i++) {
    const [y, phase] = xs[i].scroll, [py, pphase] = xs[i - 1].scroll;
    if (phase !== 'd' || pphase !== 'd') continue;
    // Per period: a late frame's step covers the frames it missed, and is no jump.
    const per = (j) => Math.abs(xs[j].scroll[0] - xs[j - 1].scroll[0]) / Math.max(1, (xs[j].missed ?? 0) + 1);
    const step = per(i);
    const near = [i - 2, i - 1, i + 1, i + 2].filter((j) => j > 0 && j < xs.length && xs[j].scroll[1] === 'd' && xs[j - 1].scroll[1] === 'd')
      .map(per).sort((a, b) => a - b);
    const typical = near.length ? near[Math.floor(near.length / 2)] : null;
    if (step < 0.01 && typical > 1) stalls.push(xs[i].t);
    else if (typical != null && step > 8 && step > typical * 2.5) jumps.push({ t: xs[i].t, step: Math.round(step), typical: Math.round(typical) });
  }
  return { frames: xs.length, gliding: xs.filter((r) => r.scroll[1] === 'd').length, jumps, stalls };
}

/** The window's work: rows created and handler dispatches and commits, largest first. */
export function workBetween(a, b) {
  const delta = perfDelta(a, b);
  const sites = (delta.sites ?? []).filter((s) => s.created > 0).sort((x, y) => y.created - x.created).slice(0, 8);
  const handlers = (delta.handlers ?? []).filter((h) => h.dispatched > 0).sort((x, y) => y.dispatched - x.dispatched).slice(0, 8);
  const where = (x) => x.source?.status === 'compatible' ? `${x.source.file.split('/').pop()}:${x.source.line} (${x.source.component})` : `site ${x.site}`;
  return {
    created: (delta.sites ?? []).reduce((n, s) => n + (s.created ?? 0), 0),
    sites: sites.map((s) => ({ at: where(s), created: s.created })),
    handlers: handlers.map((h) => ({ at: where(h), event: h.event, dispatched: h.dispatched, committed: h.committed })),
  };
}

export function report({ frames, motion, work, images, flings, distance }) {
  const f = (x) => x == null ? '—' : `${x} ms`;
  const lines = [
    `${flings ? `${flings} flings of ${distance} pt` : 'a person\'s flings'} · ${frames.presented} frames presented · ${frames.late} late (${frames.missed} missed, ${frames.overruns} overruns) · p50 ${f(frames.p50)} p95 ${f(frames.p95)} p99 ${f(frames.p99)} max ${f(frames.max)}${frames.period ? ` · period ${frames.period} ms` : ''}`,
  ];
  if (frames.phases.length) lines.push(`  late frames spent: ${[...frames.phases.map((p) => `${p.phase} ${p.ms}`), `unattributed ${frames.unattributed}`].join(' · ')} ms${frames.keptLate < frames.late ? ` (the last ${frames.keptLate} late frames)` : ''}`);
  if (motion?.frames) lines.push(`  motion (diagnostic): ${motion.gliding} gliding frames · ${motion.jumps.length} unexplained steps${motion.jumps.length ? ` (${motion.jumps.slice(0, 3).map((j) => `t=${j.t} ${j.step} pt where ${j.typical} was typical`).join('; ')})` : ''} · ${motion.stalls.length} stalls mid-glide`);
  lines.push(`  rows created: ${work.created}${work.sites.length ? ` (${work.sites.map((s) => `${s.at} ${s.created}`).join('; ')})` : ''}`);
  for (const h of work.handlers) lines.push(`  ${h.event} at ${h.at}: ${h.dispatched} dispatched, ${h.committed} changed the tree`);
  if (images) {
    const waits = Object.entries(images.waitingOn ?? {}).sort((a, b) => b[1] - a[1]).map(([stage, n]) => `${stage} ${n}`).join(', ');
    lines.push(`  images shown blank since launch: ${images.late}${images.late ? ` · blank p50 ${f(images.blankMs?.p50)} p95 ${f(images.blankMs?.p95)} · lead p50 ${f(images.leadMs?.p50)} · waiting on ${waits}` : ''}`);
  }
  lines.push('  a simulator\'s frames are the Mac\'s: compare runs and commits here, and judge a frame budget on a device');
  return lines.join('\n');
}

/** The tracked scroller's offset now: the newest frame record's (the held segment tracks it). */
async function offsetNow(s) {
  await sleep(150);
  const r = await s.op({ op: 'perf', frames: true, records: true, late: 0 });
  return (r.records ?? []).findLast((x) => Array.isArray(x.scroll))?.scroll[0] ?? null;
}

async function toScreen(s, op) {
  // A target of several words is quoted, as the driver's are: `tap "Sky off"`.
  const [verb, target, ...text] = op.trim().match(/"[^"]*"|\S+/g).map((w) => w.replace(/^"|"$/g, ''));
  if (verb === 'wait') return sleep(Number(target));
  if (verb === 'tap') return s.tap(target, {});
  if (verb === 'type') return s.type(target, text.join(' '));
  throw new Error(`fling: --to takes tap, type and wait, not ${verb}`);
}

export async function main(argv) {
  const args = parseArgs(argv);
  // The app's own server, if it has one (app.json `server`), as a drive starts it.
  const served = await driveServer(resolveApp(args.app).dir, args.storage ?? undefined);
  let s;
  // macOS: AppKit ignores momentum an app posts (LLP 1142 §12), so the flings are a person's on the trackpad.
  try { s = await open({ host: args.host, app: args.app, timing: 'platform', ...(args.host === 'ios' ? { touch: 'platform' } : {}), storage: args.storage ?? undefined }); }
  catch (e) { await served?.stop(); throw e; }
  try {
    for (const op of args.to) await toScreen(s, op);
    await sleep(1000); // the screen's own loading, on the wall's time
    // A real touch begins and ends inside the screen, so each fling stays inside
    // the scroller's box: 70% of its height, from near one edge to near the other.
    const box = (await s.layout()).nodes.find((n) => n.testId === args.target);
    if (!box) throw new Error(`fling: ${args.target} has no box on screen`);
    const before = await s.op({ op: 'perf', frames: true, hold: true, track: box.id, late: 0 });
    if (before.virtual) throw new Error('fling: the clock was taken, so frames are virtual; an op in --to took it');
    const workBefore = await s.perf();
    const travel = Math.min(args.distance, box.h * 0.7), moved = [];
    // Toward the end first.
    let flip = 0;
    if (args.host === 'macos') {
      process.stderr.write(`fling ${args.target} on the trackpad as often as you like, then press Enter\n`);
      await new Promise((r) => process.stdin.once('data', r));
      args.flings = 0;
    }
    const before0 = await offsetNow(s), records = new Map();
    // The sampler keeps 600 frames: each fling's are read as it ends, so a long run loses none.
    const collect = async () => { for (const r of (await s.op({ op: 'perf', frames: true, records: true, late: 0 })).records ?? []) if (r.t > (before.window?.to ?? -Infinity)) records.set(r.t, r); };
    for (let i = 0; i < args.flings; i++) {
      const down = (i + flip) % 2 === 1;
      await s.tap(args.target, { drag: { dx: 0, dy: down ? travel : -travel, from: [box.w / 2, down ? box.h * 0.15 : box.h * 0.85], over: FLING_MS } });
      // A real touch's reply comes before UIKit's deceleration: what moved is read from the offset the frames recorded.
      const now = await offsetNow(s);
      // A list already at its end (a chat): the first fling goes the other way, once.
      if (i === 0 && !flip && now === before0) { flip = 1; i--; continue; }
      moved.push(now);
      await sleep(1500); // UIKit's deceleration, then rest
      await collect();
    }
    if (moved.length && moved.every((m) => m === before0)) throw new Error(`fling: ${args.target} did not scroll: it is at the end of its range, or its content fits it`);
    const after = await s.op({ op: 'perf', frames: true, hold: false, records: true, late: 100 });
    const workAfter = await s.perf();
    // Images whose box showed before their pixels (LLP 1145 D2), from the app's start: the screen's own loading too.
    const images = (await s.op({ op: 'state' }))?.raster?.lateImages ?? null;
    for (const r of framesBetween.records(before, after)) records.set(r.t, r);
    const all = [...records.values()].sort((a, b) => a.t - b.t);
    const result = { frames: framesBetween(before, { ...after, records: all }), motion: motionOf(all), work: workBetween(workBefore, workAfter), images, flings: args.flings, distance: Math.round(travel) };
    console.log(args.json ? JSON.stringify(result) : report(result));
    return args.failOn.includes('late') && result.frames.late > 0 ? 1 : 0;
  } finally { await s.close(); await served?.stop(); }
}

if (import.meta.main) process.exit(await main(process.argv.slice(2)).catch((e) => { console.error(e.message); return 2; }));
