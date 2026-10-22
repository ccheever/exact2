#!/usr/bin/env bun
// LLP 1133.003 §9's throughput gate, driven: the stream fixture on one host,
// fed by `serve.mjs`, measured by `perf streams` and the host's frames.
//
//   bun apps/stream-fixture/gate.mjs <web|macos> [--seconds 60] [--baseline <n>]
//
// `--baseline <n>` runs only the run with no stream, n times, to measure
// the frame numbers' noise.
//
// For each run it starts the timing, opens the streams, waits, and reads:
// every stream's message, commit and decode counts and rates, receipt to
// decode p95 and p99, the data executor's busy share, what the server sent,
// and the main thread's frame times beside a run with no stream. Pass, as
// ruled (§16 question 8): three streams for the whole run with no overflow
// and no frame dropped by the path; receipt to decode p95 under 8 ms and
// p99 under 16 ms; busy under 25% of one core; frame time unchanged within
// the noise of the run with no stream. It writes `target/stream-fixture/
// gate-<host>.json` and prints a table.
import { spawn } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { open } from '../../scripts/agent.mjs';

const args = process.argv.slice(2);
const host = args[0];
if (!['web', 'macos'].includes(host)) throw new Error('usage: gate.mjs <web|macos> [--seconds 60]');
const seconds = Number(args[args.indexOf('--seconds') + 1]) || 60;
const baseline = args.includes('--baseline') ? Number(args[args.indexOf('--baseline') + 1]) || 3 : 0;
const ROOT = resolve(import.meta.dirname, '../..'), SERVER = 'http://127.0.0.1:4340';
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function server() {
  const up = () => fetch(`${SERVER}/stats`).then((r) => r.ok, () => false);
  if (await up()) return null;
  const child = spawn(process.execPath, [resolve(import.meta.dirname, 'serve.mjs')], { stdio: 'inherit' });
  for (let i = 0; i < 600 && !(await up()); i++) await sleep(100);
  return child;
}

// The main thread's frames over the run: on the web, requestAnimationFrame's
// intervals in the page; on macOS, the frame sampler's window (`perf frames`).
const SAMPLER = `(() => { const f = globalThis.__gateFrames = []; let last = performance.now();
  const tick = (t) => { f.push(t - last); last = t; if (globalThis.__gateFrames === f) requestAnimationFrame(tick); };
  requestAnimationFrame(tick); return true; })()`;
const READ = `(() => { const f = globalThis.__gateFrames ?? []; globalThis.__gateFrames = null; return JSON.stringify(f.slice(1)); })()`;
function percentiles(xs) {
  const s = [...xs].sort((a, b) => a - b), at = (p) => (s.length ? +s[Math.max(0, Math.ceil(s.length * p) - 1)].toFixed(2) : null);
  return { n: s.length, p50: at(0.5), p95: at(0.95), p99: at(0.99), max: at(1), presented: s.length, late: s.filter((x) => x > 1000 / 60 * 1.5).length,
    missed: s.reduce((n, x) => n + Math.max(0, Math.round(x / (1000 / 60)) - 1), 0) };
}
async function frames(s, during) {
  if (host === 'web') {
    await s.carrier.evaluate(SAMPLER);
    await during();
    return percentiles(JSON.parse(await s.carrier.evaluate(READ)));
  }
  const before = await s.op({ op: 'perf', frames: true });
  await during();
  const after = await s.op({ op: 'perf', frames: true });
  const w = after.window ?? {};
  const delta = (k) => (after.lifetime?.[k] ?? 0) - (before.lifetime?.[k] ?? 0);
  // `late` is a frame that missed its vsync or whose main-thread turn ran past
  // its target (LLP 1079's amendment of 2026-10-05); `missed` counts vsyncs lost.
  return { n: w.samples, p50: w.p50, p95: w.p95, p99: w.p99, max: w.max,
    presented: delta('presented'), late: delta('late'), missed: delta('missed'), overruns: delta('overruns') };
}

const RUNS = [
  { name: 'no stream', tap: null, streams: 0 },
  { name: 'one iOS (HTTP bytes)', tap: 'ios', streams: 1 },
  { name: 'one iOS, 1 KiB writes', tap: 'ios-split', streams: 1 },
  { name: 'one Android (WebSocket)', tap: 'android', streams: 1 },
  { name: 'three iOS', tap: 'three-ios', streams: 3 },
  { name: 'two iOS and one Android', tap: 'duo', streams: 3 },
];

// On macOS a tap without the driver's landing, which would take the clock
// from the wall (`clock land`) and with it the host's frame sampler; the
// web's page delivers only the browser's input, and its frames are the page's.
const press = async (s, testId) => host === 'web' ? s.tap(testId) : s.op({ op: 'tap', id: (await s.find(testId)).id });

// The app process's CPU time in seconds (macOS: every thread of it, the
// executor's readers too), from `ps`'s `[[dd-]hh:]mm:ss.cc`.
let pid = null;
const cpu = () => {
  if (!pid) return null;
  const text = Bun.spawnSync(['ps', '-o', 'cputime=', '-p', String(pid)]).stdout.toString().trim();
  const [days, rest] = text.includes('-') ? text.split('-') : ['0', text];
  return rest.split(':').reduce((n, part) => n * 60 + Number(part), 0) + Number(days) * 86400;
};

if (baseline) RUNS.splice(0, RUNS.length, ...Array.from({ length: baseline }, (_, i) => ({ name: `no stream ${i + 1}`, tap: null, streams: 0 })));

const child = await server();
const s = await open({ host, app: 'stream-fixture', onProcess: (p) => { pid ??= host === 'macos' ? p.pid : null; }, ...(host === 'macos' ? { timing: 'platform' } : {}) });
const results = [];
try {
  for (const run of RUNS) {
    await press(s, 'stop');
    await sleep(500);
    await fetch(`${SERVER}/reset`);
    await s.op({ op: 'perf', streams: true, start: true });
    if (run.tap) await press(s, run.tap);
    const cpu0 = cpu(), wall0 = performance.now();
    const frame = await frames(s, () => sleep(seconds * 1000));
    const cores = cpu0 == null ? null : +((cpu() - cpu0) / ((performance.now() - wall0) / 1000)).toFixed(3);
    const timing = (await s.op({ op: 'perf', streams: true })).streamTiming;
    const sent = await (await fetch(`${SERVER}/stats`)).json();
    const state = await s.state();
    const failed = ['a', 'b', 'c'].map((k) => state.resources?.[k]?.failed).filter(Boolean);
    const streams = timing.streams.filter((x) => x.messages > 0);
    const decodes = streams.reduce((n, x) => n + x.decodes, 0), framesSent = sent.reduce((n, c) => n + c.frames, 0);
    const worst = (k, p) => Math.max(0, ...streams.map((x) => x[k][p]));
    results.push({ ...run, seconds: timing.seconds, frame, cores, failed, framesSent, decodes,
      dropped: Math.max(0, framesSent - decodes - 2 * run.streams), refused: streams.reduce((n, x) => n + (x.refused ?? 0), 0),
      busy: +streams.reduce((n, x) => n + x.busy, 0).toFixed(4), toDecode: { p95: worst('toDecode', 'p95'), p99: worst('toDecode', 'p99'), max: worst('toDecode', 'max') },
      toCommit: { p95: worst('toCommit', 'p95'), p99: worst('toCommit', 'p99') }, perStream: streams });
    const r = results.at(-1);
    console.log(`${host} · ${run.name}: ${r.framesSent} sent, ${r.decodes} decoded, ${r.dropped} dropped, ${r.refused} refused${failed.length ? `, failed: ${failed.join('; ')}` : ''} · receipt→decode p95 ${r.toDecode.p95} p99 ${r.toDecode.p99} max ${r.toDecode.max} ms · busy ${(r.busy * 100).toFixed(2)}%${cores != null ? ` · process ${(cores * 100).toFixed(1)}% of a core` : ''} · frames p50 ${frame.p50} p95 ${frame.p95} p99 ${frame.p99} ms; of ${frame.presented} presented ${frame.missed} vsyncs missed, ${frame.late} late${frame.overruns != null ? ` (${frame.overruns} overruns)` : ''}`);
  }
  await press(s, 'stop');
} finally {
  await s.close?.();
  child?.kill();
}

const out = resolve(ROOT, `target/stream-fixture/gate-${host}${baseline ? '-baseline' : ''}.json`);
if (baseline) {
  writeFileSync(out, JSON.stringify({ host, seconds, results }, null, 2));
  console.log(out);
  process.exit(0);
}
const base = results[0].frame, three = results.filter((r) => r.streams === 3);
if (three.length !== 2) throw new Error(`gate: ${three.length} three-stream runs, not 2`);
const pass = {
  noOverflowOrDrop: three.every((r) => !r.failed.length && !r.refused && !r.dropped),
  decodeP95: three.every((r) => r.toDecode.p95 < 8),
  decodeP99: three.every((r) => r.toDecode.p99 < 16),
  busy: three.every((r) => r.busy < 0.25),
  // Within the noise: the p95 frame interval moves by under a millisecond, and
  // missed vsyncs by under 1% of the frames presented.
  // A run that presented nothing (the app left the foreground, and the
  // macOS sampler with it) measured no frame time, and fails.
  frames: base.presented > 0 && three.every((r) => r.frame.presented > 0 && r.frame.p95 - base.p95 < 1 && (r.frame.missed / r.frame.presented) - (base.missed / base.presented) < 0.01),
};
writeFileSync(out, JSON.stringify({ host, seconds, pass, results }, null, 2));
console.log(`${host}: ${Object.entries(pass).map(([k, v]) => `${k} ${v ? 'pass' : 'FAIL'}`).join(' · ')}\n${out}`);
process.exit(Object.values(pass).every(Boolean) ? 0 : 1);
