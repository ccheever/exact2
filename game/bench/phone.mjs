#!/usr/bin/env bun
// The cubes scene on an Android phone over adb: Exact's native game (the Canvas reader with the game's GPU
// module), three.js in the system WebView (game/twins/three/android), and three.js in Chrome. Diagnostic,
// never a check. One JSON line per run, on stdout and appended to game/bench/results/phone.jsonl.
//
//   bun game/bench/phone.mjs exact 2000 5000 …     Exact at each N (an APK per N: N is baked into the plan)
//   bun game/bench/phone.mjs webview 2000 5000 …   three.js in the WebView shell (MODE=meshes: a Mesh per cube)
//   bun game/bench/phone.mjs chrome 2000 5000 …    three.js in Chrome
//
// Each run launches the app, waits WARMUP seconds, resets the app's gfxinfo and reads it SECONDS later: the
// frames its RenderThread drew, where the reader draws the GPU canvas and the WebView composites. Chrome draws
// outside gfxinfo, so its numbers are the page's own requestAnimationFrame intervals (the WebView's page reports
// them too). The display is held at HZ (default 60) for every run; `settings put system peak_refresh_rate` and
// `min_refresh_rate` are restored to the phone's default at the end. The phone must be awake and unlocked.
import { spawnSync } from 'node:child_process';
import { appendFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { resolve } from 'node:path';

const here = resolve(new URL('.', import.meta.url).pathname), root = resolve(here, '../..');
const sdk = process.env.ANDROID_HOME ?? resolve(homedir(), process.platform === 'darwin' ? 'Library/Android/sdk' : 'Android/Sdk');
const ADB = existsSync(resolve(sdk, 'platform-tools/adb')) ? resolve(sdk, 'platform-tools/adb') : 'adb';
const WARMUP = Number(process.env.WARMUP ?? 4), SECONDS = Number(process.env.SECONDS ?? 10), HZ = Number(process.env.HZ ?? 60);
const MODE = process.env.MODE ?? 'instanced', PORT = 8790;
const EXACT = 'com.exact.bench_cubes', TWIN = 'dev.exact.twin', CHROME = 'com.android.chrome';
const contract = resolve(here, 'cubes/app.contract'), twinDir = resolve(here, '../twins/three');
const out = resolve(here, 'results/phone.jsonl');

const adb = (...args) => {
  const r = spawnSync(ADB, args, { encoding: 'utf8' });
  if (r.status !== 0) throw new Error(`adb ${args.join(' ')}: ${(r.stderr || r.stdout).trim()}`);
  return r.stdout;
};
const shell = (cmd) => adb('shell', cmd);

function device() {
  const battery = Number(/temperature: (\d+)/.exec(shell('dumpsys battery'))?.[1]) / 10;
  const thermal = /Thermal Status: (\d)/.exec(shell('dumpsys thermalservice'))?.[1];
  return { battery_c: battery, thermal_status: thermal === undefined ? null : Number(thermal) };
}

function gfxinfo(pkg) {
  const text = shell(`dumpsys gfxinfo ${pkg}`), num = (re) => Number(re.exec(text)?.[1] ?? NaN);
  return {
    frames: num(/Total frames rendered: (\d+)/), janky: num(/Janky frames: (\d+)/),
    p50: num(/50th percentile: (\d+)ms/), p90: num(/90th percentile: (\d+)ms/),
    p95: num(/95th percentile: (\d+)ms/), p99: num(/99th percentile: (\d+)ms/),
    gpu_p50: num(/50th gpu percentile: (\d+)ms/), gpu_p95: num(/95th gpu percentile: (\d+)ms/),
  };
}

async function measure(pkg) {
  await Bun.sleep(WARMUP * 1000);
  // A run behind a dialog (Play Protect asks about each new install) measures nothing.
  const focus = /mCurrentFocus=\S+ \S+ (\S+)\//.exec(shell('dumpsys window'))?.[1];
  if (focus !== pkg) throw new Error(`${pkg} is not in front (${focus}): dismiss what covers it and run again`);
  shell(`dumpsys gfxinfo ${pkg} reset`);
  const start = Date.now();
  await Bun.sleep(SECONDS * 1000);
  const g = gfxinfo(pkg);
  return { ...g, fps: Math.round(g.frames / ((Date.now() - start) / 1000) * 10) / 10 };
}

function record(row) {
  const line = JSON.stringify({ at: new Date().toISOString(), hz: HZ, ...row });
  console.log(line);
  mkdirSync(resolve(here, 'results'), { recursive: true });
  appendFileSync(out, line + '\n');
}

// The twin's page, served to the phone through `adb reverse`; its report arrives at /__bench.
let report = null;
const server = Bun.serve({ port: PORT, async fetch(req) {
  const url = new URL(req.url);
  if (url.pathname === '/__bench') { report = JSON.parse(await req.text()); return new Response('ok'); }
  const file = Bun.file(resolve(twinDir, '.' + (url.pathname === '/' ? '/bench.html' : url.pathname)));
  return (await file.exists()) ? new Response(file, { headers: { 'cache-control': 'no-store' } }) : new Response('not found', { status: 404 });
} });

async function exact(n) {
  const source = readFileSync(contract, 'utf8');
  try {
    writeFileSync(contract, source.replace(/state n = \d+/, `state n = ${n}`));
    const build = spawnSync('bun', ['scripts/agent-android.mjs', 'build', 'bench-cubes', '--apk', '--release'],
      { cwd: root, encoding: 'utf8', env: { ...process.env, EXACT_APP_DIR: resolve(here, 'cubes') } });
    if (build.status !== 0) throw new Error(`build N=${n}:\n${(build.stdout + build.stderr).slice(-3000)}`);
  } finally { writeFileSync(contract, source); }
  adb('install', '-r', resolve(root, 'host/android/app/build/bench-cubes/outputs/apk/release/app-release.apk'));
  shell(`am force-stop ${EXACT}`);
  await Bun.sleep(2000);
  const before = device();
  shell(`am start -W -n ${EXACT}/dev.exact.app.MainActivity`);
  const m = await measure(EXACT);
  shell(`am force-stop ${EXACT}`);
  record({ engine: 'exact', n, ...m, ...before });
}

async function twin(n, where) {
  const pkg = where === 'chrome' ? CHROME : TWIN;
  const url = `http://localhost:${PORT}/bench.html?scene=cubes&n=${n}&mode=${MODE}&renderer=webgl&fit=screen&warmup=${WARMUP}&seconds=${SECONDS}`;
  report = null;
  shell(`am force-stop ${pkg}`);
  await Bun.sleep(2000);
  const before = device();
  if (where === 'chrome') shell(`am start -W -a android.intent.action.VIEW -d '${url}' ${CHROME}`);
  else shell(`am start -W -n ${TWIN}/.MainActivity --es url '${url}'`);
  const m = where === 'chrome' ? {} : await measure(TWIN);
  // The page reports after its own WARMUP + SECONDS, timed from its first frame.
  for (let i = 0; i < 60 && !report; i++) await Bun.sleep(500);
  shell(`am force-stop ${pkg}`);
  record({ engine: `three-${where}`, mode: MODE, n, ...m, page: report, ...before });
}

const [which, ...ns] = process.argv.slice(2);
if (!['exact', 'webview', 'chrome'].includes(which) || !ns.length || ns.some((n) => !/^[1-9]\d*$/.test(n))) {
  console.error('usage: bun game/bench/phone.mjs <exact|webview|chrome> <n> [<n> …]');
  process.exit(2);
}
adb('reverse', `tcp:${PORT}`, `tcp:${PORT}`);
shell(`settings put system min_refresh_rate ${HZ}.0; settings put system peak_refresh_rate ${HZ}.0`);
try {
  for (const n of ns.map(Number)) await (which === 'exact' ? exact(n) : twin(n, which));
} finally {
  shell('settings delete system min_refresh_rate; settings delete system peak_refresh_rate');
  server.stop(true);
}
