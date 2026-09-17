#!/usr/bin/env bun
// One bench run, one JSON line (game/bench/README.md): the same scene in this
// engine and in its twins, measured the same way. Diagnostic, never a check.
//
//   bun game/bench/run.mjs godot cubes 100000 multimesh [forward_plus|mobile]
//   bun game/bench/run.mjs three cubes 100000 instanced [webgl|webgpu]
//   bun game/bench/run.mjs godot-web cubes 100000 multimesh
//   bun game/bench/run.mjs sweep godot cubes multimesh     the largest N holding the refresh rate
//
// Every run presents to the display, so the numbers are frame pacing under vsync:
// the question a scene answers is the largest N that still holds the refresh rate.
import { spawn, spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, existsSync } from 'node:fs';
import { tmpdir, loadavg } from 'node:os';
import { resolve, join, extname } from 'node:path';

const here = resolve(new URL('.', import.meta.url).pathname);
if (process.argv[2] === 'sweep') await sweep(process.argv.slice(3));
const [engine, scene = 'cubes', n = '10000', mode, variant] = process.argv.slice(2);
const seconds = process.env.BENCH_SECONDS ?? '8';
const GODOT = process.env.GODOT ?? resolve(process.env.HOME, 'Library/Caches/exact2-game/godot/Godot.app/Contents/MacOS/Godot');
const CHROME = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';

// The largest N whose p95 frame stays within 15% of the refresh interval (BENCH_HZ,
// default 120): doubling until it breaks, then bisecting to 10%. One line per run.
async function sweep([engine, scene = 'cubes', mode, variant]) {
  const budget = 1000 / Number(process.env.BENCH_HZ ?? 120) * 1.15;
  const run = (n) => {
    const r = spawnSync(process.execPath, [import.meta.path, engine, scene, String(n), ...(mode ? [mode] : []), ...(variant ? [variant] : [])], { encoding: 'utf8', env: { ...process.env, BENCH_SECONDS: process.env.BENCH_SECONDS ?? '5' } });
    const line = (r.stdout ?? '').trim().split('\n').pop();
    let out; try { out = JSON.parse(line); } catch { console.error(r.stdout, r.stderr); process.exit(1); }
    console.log(JSON.stringify({ n, fps: out.fps_avg, p95: out.ms_p95, script: out.script_ms_avg ?? null, holds: out.ms_p95 <= budget }));
    return out.ms_p95 <= budget;
  };
  let lo = 0, hi = Number(process.env.BENCH_START ?? 10000);
  while (run(hi)) { lo = hi; hi *= 2; if (hi > 8_000_000) break; }
  while (hi - lo > lo * 0.1 && hi - lo > 500) { const mid = Math.round((lo + hi) / 2 / 500) * 500; if (run(mid)) lo = mid; else hi = mid; }
  console.log(JSON.stringify({ engine, scene, mode: mode ?? null, variant: variant ?? null, holds: lo, breaks: hi, budget_ms: Math.round(budget * 100) / 100, load1: Math.round(loadavg()[0] * 10) / 10 }));
  process.exit(0);
}

// Serve `root`, open `page` in a throwaway Chrome, and finish with what the page
// posts to /__bench. Only the Chrome this function launched is killed.
function browse(root, page, map = (r) => r) {
  const types = { '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.json': 'application/json', '.wasm': 'application/wasm', '.pck': 'application/octet-stream', '.png': 'image/png' };
  let chrome = null;
  const profile = mkdtempSync(join(tmpdir(), 'exact2-bench-chrome-'));
  const server = Bun.serve({
    port: 0,
    async fetch(req) {
      const url = new URL(req.url);
      if (url.pathname === '/__bench') {
        const result = JSON.parse(await req.text());
        setTimeout(() => { chrome?.kill(); server.stop(true); rmSync(profile, { recursive: true, force: true }); finish(map(result)); }, 50);
        return new Response('ok');
      }
      const file = Bun.file(join(root, decodeURIComponent(url.pathname)));
      if (!(await file.exists())) return new Response('not found', { status: 404 });
      return new Response(file, { headers: { 'content-type': types[extname(url.pathname)] ?? 'application/octet-stream' } });
    },
  });
  chrome = spawn(CHROME, [`--user-data-dir=${profile}`, '--no-first-run', '--no-default-browser-check', '--window-size=1400,900',
    '--disable-background-timer-throttling', '--disable-renderer-backgrounding', '--disable-backgrounding-occluded-windows', `--app=http://localhost:${server.port}/${page}`], { stdio: 'ignore' });
  setTimeout(() => { console.error(`${engine}: no result in time`); chrome?.kill(); process.exit(1); }, (Number(seconds) + 90) * 1000);
}

function finish(result) {
  console.log(JSON.stringify({ ...result, load1: Math.round(loadavg()[0] * 10) / 10 }));
  process.exit(0);
}

if (engine === 'godot') {
  if (!existsSync(GODOT)) { console.error(`no Godot at ${GODOT} (set GODOT)`); process.exit(2); }
  const args = ['--path', resolve(here, '../twins/godot/bench'), ...(variant ? ['--rendering-method', variant] : []),
    '--', `--scene=${scene}`, `--n=${n}`, `--mode=${mode ?? 'multimesh'}`, `--seconds=${seconds}`];
  const r = spawnSync(GODOT, args, { encoding: 'utf8' });
  const line = (r.stdout ?? '').split('\n').find((l) => l.startsWith('BENCH '));
  if (!line) { console.error(r.stdout, r.stderr); process.exit(1); }
  finish({ ...JSON.parse(line.slice(6)), ...(variant ? { renderer: variant } : {}) });
} else if (engine === 'three') {
  browse(resolve(here, '../twins/three'), `bench.html?scene=${scene}&n=${n}&mode=${mode ?? 'instanced'}&renderer=${variant ?? 'webgl'}&seconds=${seconds}`);
} else if (engine === 'godot-web') {
  // Godot's web export (its Compatibility renderer over WebGL2), built fresh.
  const out = mkdtempSync(join(tmpdir(), 'exact2-bench-godot-web-'));
  const r = spawnSync(GODOT, ['--headless', '--path', resolve(here, '../twins/godot/bench'), '--export-release', 'Web', join(out, 'index.html')], { encoding: 'utf8' });
  if (!existsSync(join(out, 'index.wasm'))) { console.error(r.stdout, r.stderr); process.exit(1); }
  browse(out, `index.html?scene=${scene}&n=${n}&mode=${mode ?? 'multimesh'}&seconds=${seconds}`, (result) => { rmSync(out, { recursive: true, force: true }); return { ...result, engine: 'godot-web' }; });
} else {
  console.error('usage: bun game/bench/run.mjs <godot|godot-web|three> <scene> <n> [mode] [variant]');
  process.exit(2);
}
