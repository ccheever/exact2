#!/usr/bin/env bun
// One bench run, one JSON line (game/bench/README.md): the same scene in this
// engine and in its twins, measured the same way. Diagnostic, never a check.
//
//   bun game/bench/run.mjs godot cubes 100000 multimesh [forward_plus|mobile]
//   bun game/bench/run.mjs three cubes 100000 instanced [webgl|webgpu]
//
// Every run presents to the display, so the numbers are frame pacing under vsync:
// the question a scene answers is the largest N that still holds the refresh rate.
import { spawn, spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, existsSync } from 'node:fs';
import { tmpdir, loadavg } from 'node:os';
import { resolve, join, extname } from 'node:path';

const here = resolve(new URL('.', import.meta.url).pathname);
const [engine, scene = 'cubes', n = '10000', mode, variant] = process.argv.slice(2);
const seconds = process.env.BENCH_SECONDS ?? '8';
const GODOT = process.env.GODOT ?? resolve(process.env.HOME, 'Library/Caches/exact2-game/godot/Godot.app/Contents/MacOS/Godot');
const CHROME = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';

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
  const root = resolve(here, '../twins/three');
  const types = { '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.json': 'application/json' };
  let chrome = null, profile = null;
  const server = Bun.serve({
    port: 0,
    async fetch(req) {
      const url = new URL(req.url);
      if (url.pathname === '/__bench') {
        const result = JSON.parse(await req.text());
        setTimeout(() => { chrome?.kill(); server.stop(true); rmSync(profile, { recursive: true, force: true }); finish(result); }, 50);
        return new Response('ok');
      }
      const file = Bun.file(join(root, decodeURIComponent(url.pathname)));
      if (!(await file.exists())) return new Response('not found', { status: 404 });
      return new Response(file, { headers: { 'content-type': types[extname(url.pathname)] ?? 'application/octet-stream' } });
    },
  });
  profile = mkdtempSync(join(tmpdir(), 'exact2-bench-chrome-'));
  const url = `http://localhost:${server.port}/bench.html?scene=${scene}&n=${n}&mode=${mode ?? 'instanced'}&renderer=${variant ?? 'webgl'}&seconds=${seconds}`;
  chrome = spawn(CHROME, [`--user-data-dir=${profile}`, '--no-first-run', '--no-default-browser-check', '--window-size=1400,900',
    '--disable-background-timer-throttling', '--disable-renderer-backgrounding', '--disable-backgrounding-occluded-windows', `--app=${url}`], { stdio: 'ignore' });
  setTimeout(() => { console.error('three: no result in time'); chrome?.kill(); process.exit(1); }, (Number(seconds) + 60) * 1000);
} else {
  console.error('usage: bun game/bench/run.mjs <godot|three> <scene> <n> [mode] [variant]');
  process.exit(2);
}
