#!/usr/bin/env bun
// Messages flung with no agent: the development build launched as a person
// launches it (no `EXACT_AGENT`, so the host's own `SessionClockTimer` and
// display link run the runner's timers on the wall clock), real touches from
// the XCTest touch runner at the points `agent-probe.mjs` recorded, and a
// trace by `SIGUSR1` before the flings and after each one (frames, `perf` by
// plan site). Under the agent the driver owns the runner's clock, so a timer
// never fires on its own there; this is the run in which `poll` does.
//   bun experiments/timer-wait-points/compiled/fling-free.mjs --sim <udid> --aims <agent-probe.json> --out <dir> [--flings 6] [--fifty 3] [--distance 600] [--positions]
import { spawnSync } from 'node:child_process';
import { mkdirSync, readFileSync, renameSync } from 'node:fs';
import { resolve } from 'node:path';
import { openTouches } from '../../../host/apple/touches.mjs';
import { appleArtifacts } from '../../../host/apple/build.mjs';
import { pidTrace } from '../../../scripts/agent-inspect.mjs';
import { resolveApp } from '../../../scripts/app.mjs';

const argv = process.argv.slice(2);
const flag = (name, fallback) => (argv.includes(name) ? argv[argv.indexOf(name) + 1] : fallback);
const udid = flag('--sim'), aimsFile = flag('--aims'), out = flag('--out');
// The transcript's messages on screen, from the runner's accessibility snapshot after each fling has settled: the
// evidence the fling moved the list, since no agent reads its offset here (a simulator screenshot of a Duo is black)
const positions = argv.includes('--positions');
const flings = Number(flag('--flings', 6)), fifty = Number(flag('--fifty', 3)), distance = Number(flag('--distance', 600));
if (!udid || !aimsFile || !out) throw new Error('usage: fling-free.mjs --sim <udid> --aims <agent-probe.json> --out <dir> [--flings N] [--fifty N] [--distance pt]');
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const { aims } = JSON.parse(readFileSync(aimsFile, 'utf8'));
const app = resolveApp('messages');
mkdirSync(out, { recursive: true });

const log = [];
const note = (what, extra = {}) => { const e = { t: Date.now(), what, ...extra }; log.push(e); console.error(JSON.stringify(e)); };

function launch() {
  const capture = appleArtifacts(app, { destination: 'ios-simulator' }).capture;
  const r = spawnSync('xcrun', ['simctl', 'launch', '--terminate-running-process', udid, app.id], {
    encoding: 'utf8', env: { ...process.env, SIMCTL_CHILD_EXACT_ASSETS: capture },
  });
  if (r.status !== 0) throw new Error(`simctl launch: ${r.stderr || r.stdout}`);
  const pid = Number(r.stdout.trim().split(':').pop());
  if (!Number.isSafeInteger(pid)) throw new Error(`simctl launch printed no pid: ${r.stdout}`);
  return pid;
}

/** A tap at a recorded aim: the app's scene point, and the fixed screen point on a multi-display device. */
const tapRequest = (a) => ({ op: 'tap', point: a.point, ...(a.fixed ? { screen: a.fixed, display: a.display } : {}) });

/** One fling from a recorded aim, built as `realTap` builds a drag: 60 ms of travel, no press or hold. */
function flingRequest(a, dy) {
  const over = 60, dx = 0;
  const request = { op: 'drag', point: a.point, to: [a.point[0] + dx, a.point[1] + dy], press: 0, hold: 0, velocity: Math.hypot(dx, dy) / (over / 1000) };
  if (a.fixed && a.fixedAxes) {
    const [[rx, ry], [ux, uy]] = a.fixedAxes;
    Object.assign(request, { screen: a.fixed, screenTo: [a.fixed[0] + dx * rx + dy * ux, a.fixed[1] + dx * ry + dy * uy], display: a.display });
  }
  return request;
}

async function where(label) {
  const r = await touches.ask({ op: 'tree', max: 5000 });
  if (r.error) throw new Error(`tree: ${r.error}`);
  const shown = r.elements.filter((e) => /^Message \d+/.test(e.label ?? '') && e.frame[1] + e.frame[3] > 44 && e.frame[1] < 576)
    .map((e) => ({ label: e.label.split(',')[0], y: Math.round(e.frame[1]) }));
  note('positions', { label, first: shown[0] ?? null, last: shown.at(-1) ?? null, count: shown.length });
}

async function trace(pid, name) {
  const file = await pidTrace(pid, () => out);
  const named = resolve(out, `${name}.json`);
  renameSync(file, named);
  note('trace', { name, file: named });
  return named;
}

const pid = launch();
note('launched', { pid });
const touches = await openTouches({ udid, appId: app.id, foldable: true });
try {
  await sleep(3000);
  const ask = async (req) => {
    const r = await touches.ask(req);
    if (r.error) throw new Error(`${req.op}: ${r.error}`);
    return r;
  };
  await ask(tapRequest(aims['conversation-demo'].aim));
  note('tapped conversation-demo');
  await sleep(1500);
  for (let i = 0; i < fifty; i++) {
    await ask(tapRequest(aims.add.aim));
    await sleep(800);
    await ask(tapRequest(aims['add-fifty'].aim));
    note('added fifty', { i });
    await sleep(1200);
  }
  await sleep(2000);
  await trace(pid, 'trace-0-before');
  if (positions) await where('before');
  const travel = Math.min(distance, aims['transcript-top'].box.h * 0.7);
  for (let i = 0; i < flings; i++) {
    // Toward older messages first: a chat opens at its end.
    const down = i % 2 === 0;
    const a = aims[down ? 'transcript-top' : 'transcript-bottom'].aim;
    note('fling', { i, dy: down ? travel : -travel });
    await ask(flingRequest(a, down ? travel : -travel));
    note('lifted', { i });
    await sleep(1500);
    await trace(pid, `trace-${i + 1}-after-fling-${i + 1}`);
    // The screen at rest, which shows the fling moved the transcript (no agent reads its offset here)
    if (positions) await where(`after fling ${i + 1}`);
  }
} finally {
  await touches.close();
  await Bun.write(resolve(out, 'fling-free-log.json'), JSON.stringify({ udid, pid, flings, fifty, distance, log }, null, 2));
  spawnSync('xcrun', ['simctl', 'terminate', udid, app.id]);
}
