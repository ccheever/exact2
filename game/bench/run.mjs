#!/usr/bin/env bun
// One bench run, one JSON line (game/bench/README.md): the same scene in this
// engine and in its twins, measured the same way. Diagnostic, never a check.
//
//   bun game/bench/run.mjs godot cubes 100000 multimesh [forward_plus|mobile]
//   bun game/bench/run.mjs three cubes 100000 instanced [webgl|webgpu]
//   bun game/bench/run.mjs godot-web cubes 100000 multimesh
//   bun game/bench/run.mjs sweep godot cubes multimesh     the largest N holding the refresh rate
//   bun game/bench/run.mjs exact-web cubes 100000 field    Exact's culling scene
//
// Every run presents to the display, so the numbers are frame pacing under vsync:
// the question a scene answers is the largest N that still holds the refresh rate.
import { spawn, spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, existsSync } from 'node:fs';
import { tmpdir, loadavg } from 'node:os';
import { resolve, join, extname } from 'node:path';
import { build, dist, browserWorld, macWorld, summarize, displayReady, stop, track, focus } from './exact.mjs';

const here = resolve(new URL('.', import.meta.url).pathname);
if (process.argv[2] === 'compare') {await (await import('./compare.mjs')).compare();process.exit(0);}
if (process.argv[2] === 'sweep') await sweep(process.argv.slice(3));
const [engine, scene = 'cubes', n = process.env.BENCH_N ?? '10000', mode, variant] = process.argv.slice(2);
const seconds = process.env.BENCH_SECONDS ?? '8';
const headless = process.env.BENCH_HEADLESS === '1';
if (!(Number(seconds)>0 && Number(seconds)<=60) || !/^[1-9][0-9]*$/.test(n)) throw new Error('N must be positive and BENCH_SECONDS must be in (0, 60]');
const GODOT = process.env.GODOT ?? resolve(process.env.HOME, 'Library/Caches/exact2-game/godot/Godot.app/Contents/MacOS/Godot');
const CHROME = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';

// The largest N that holds the refresh rate (BENCH_HZ, default 120): at least 96% of
// its frames per second, and a p95 frame under one and a half intervals — a browser's
// rAF jitters by a couple of milliseconds with nothing on screen, and a dropped frame
// is a whole interval. Doubling until it breaks, then bisecting to 10%.
async function sweep([engine, scene = 'cubes', mode, variant]) {
  if(process.env.BENCH_HEADLESS === '1') throw new Error('Headless runs cannot be swept');
  displayReady();
  const hz = Number(process.env.BENCH_HZ ?? 120), budget = 1000 / hz * 1.5;
  const run = (n) => {
    const r = spawnSync(process.execPath, [import.meta.path, engine, scene, String(n), ...(mode ? [mode] : []), ...(variant ? [variant] : [])], { encoding: 'utf8', env: { ...process.env, BENCH_SECONDS: process.env.BENCH_SECONDS ?? '5' } });
    if(r.status!==0) {console.error(r.stdout,r.stderr);throw new Error('sweep run failed');}
    const line = (r.stdout ?? '').trim().split('\n').pop();
    let out; try { out = JSON.parse(line); } catch { console.error(r.stdout, r.stderr); process.exit(1); }
    if (out.measurement === 'sanity-only') throw new Error('Headless runs cannot be swept');
    const holds = out.fps_avg >= hz * 0.96 && out.ms_p95 <= budget;
    console.log(JSON.stringify({ ...out, holds }));
    return holds;
  };
  let lo = 0, hi = Number(process.env.BENCH_START ?? 10000);
  while (run(hi)) { lo = hi; hi *= 2; if (hi > 8_000_000) break; }
  while (hi - lo > lo * 0.1 && hi - lo > 500) { const mid = Math.round((lo + hi) / 2 / 500) * 500; if (run(mid)) lo = mid; else hi = mid; }
  console.log(JSON.stringify({ engine, scene, mode: mode ?? null, variant: variant ?? null, holds: lo, breaks: hi, budget_ms: Math.round(budget * 100) / 100, load1: Math.round(loadavg()[0] * 10) / 10 }));
  process.exit(0);
}

// Serve `root`, open `page` in a throwaway Chrome, and finish with what the page
// posts to /__bench. Only the Chrome this function launched is killed.
async function browse(root, page, map = r => r, exact = false) {
  if (!headless) displayReady();
  const types = { '.html':'text/html', '.js':'text/javascript', '.mjs':'text/javascript', '.json':'application/json', '.wasm':'application/wasm', '.pck':'application/octet-stream', '.png':'image/png' };
  const profile = mkdtempSync(join(tmpdir(), 'exact2-bench-chrome-'));
  let chrome, timer, result, fail;
  const posted = new Promise((ok,no)=>{result=ok;fail=no;});
  // Exact reads over CDP instead; avoid an unhandled rejection from unused POST.
  posted.catch(()=>{});
  const server = Bun.serve({port:0, async fetch(req) {
    const url = new URL(req.url);
    if (url.pathname === '/__bench') {result(JSON.parse(await req.text()));return new Response('ok');}
    const path=resolve(root,'.'+decodeURIComponent(url.pathname === '/' ? '/index.html' : url.pathname));
    if(!path.startsWith(resolve(root)+'/')) return new Response('forbidden',{status:403});
    const file=Bun.file(path);
    if(!(await file.exists())) return new Response('not found',{status:404});
    return new Response(file,{headers:{'content-type':types[extname(path)]??'application/octet-stream',
      'Cross-Origin-Opener-Policy':'same-origin','Cross-Origin-Embedder-Policy':'require-corp'}});
  }});
  const interrupted=()=>{fail(new Error('Benchmark interrupted'));void stop(chrome);};
  process.once('SIGINT',interrupted);process.once('SIGTERM',interrupted);
  try {
    chrome=track(spawn(CHROME,[`--user-data-dir=${profile}`,'--no-first-run','--no-default-browser-check','--window-size=1400,900',
      '--remote-debugging-port=0',...(headless?['--headless=new']:[]),
      '--enable-unsafe-webgpu','--disable-background-timer-throttling','--disable-renderer-backgrounding','--disable-backgrounding-occluded-windows',
      `--app=http://localhost:${server.port}/${page}`],{stdio:'ignore'}));
    console.error(`LAUNCH Chrome pid=${chrome.pid} profile=${profile}`);
    chrome.once('error',fail);chrome.once('exit',()=>fail(new Error('Chrome exited before result')));
    const timeout=new Promise((_,no)=>{timer=setTimeout(()=>no(new Error(`${engine}: no result in time`)),(Number(seconds)+100)*1000);});
    const output=await Promise.race([exact?browserWorld(chrome,profile,Number(seconds)):posted,timeout]);
    focus(chrome.pid);
    return map(output);
  } finally {
    clearTimeout(timer);await stop(chrome);server.stop(true);
    process.removeListener('SIGINT',interrupted);process.removeListener('SIGTERM',interrupted);
    // The recorded Chrome must have exited before its profile can be discarded.
    const ps=spawnSync('ps',['-axo','pid=,command='],{encoding:'utf8'});
    const remaining=ps.stdout.split('\n').filter(l=>l.includes(profile));
    if(remaining.length) throw new Error('Chrome profile processes still present: '+remaining.join('\n'));
    rmSync(profile,{recursive:true,force:true});
  }
}

function finish(result) {
  console.log(JSON.stringify({ ...result, ...(headless?{headless:true,measurement:'sanity-only'}:{}), load1: Math.round(loadavg()[0] * 10) / 10 }));
  process.exit(0);
}

if (engine === 'exact-web' || engine === 'exact-macos') {
  if(scene !== 'cubes') throw new Error('Exact currently implements cubes only');
  // `field`: the culling scene, an eye-level camera inside n pillars with shadows.
  const field = mode === 'field';
  const paths=await build(engine === 'exact-web' ? 'web' : 'macos', Number(n), field);
  if(process.env.BENCH_BUILD_ONLY === '1') {console.log(JSON.stringify({engine,built:true,n:Number(n)}));process.exit(0);}
  const measured=engine === 'exact-web' ? await browse(dist,'index.html',r=>r,true) : await macWorld(paths,Number(seconds));
  finish(summarize(measured,engine,n,field));
} else if (engine === 'godot') {
  if(!headless) displayReady();
  if (!existsSync(GODOT)) { console.error(`no Godot at ${GODOT} (set GODOT)`); process.exit(2); }
  const args = ['--path', resolve(here, '../twins/godot/bench'), ...(variant ? ['--rendering-method', variant] : []),
    '--', `--scene=${scene}`, `--n=${n}`, `--mode=${mode ?? 'multimesh'}`, `--seconds=${seconds}`];
  const r = spawnSync(GODOT, args, { encoding: 'utf8' });
  const line = (r.stdout ?? '').split('\n').find((l) => l.startsWith('BENCH '));
  if (!line) { console.error(r.stdout, r.stderr); process.exit(1); }
  finish({ ...JSON.parse(line.slice(6)), ...(variant ? { renderer: variant } : {}) });
} else if (engine === 'three') {
  finish(await browse(resolve(here, '../twins/three'), `bench.html?scene=${scene}&n=${n}&mode=${mode ?? 'instanced'}&renderer=${variant ?? 'webgl'}&seconds=${seconds}`));
} else if (engine === 'godot-web') {
  // Godot's web export (its Compatibility renderer over WebGL2), built fresh.
  const out = mkdtempSync(join(tmpdir(), 'exact2-bench-godot-web-'));
  const r = spawnSync(GODOT, ['--headless', '--path', resolve(here, '../twins/godot/bench'), '--export-release', 'Web', join(out, 'index.html')], { encoding: 'utf8' });
  if (!existsSync(join(out, 'index.wasm'))) { console.error(r.stdout, r.stderr); process.exit(1); }
  try { finish(await browse(out, `index.html?scene=${scene}&n=${n}&mode=${mode ?? 'multimesh'}&seconds=${seconds}`, (result) => { rmSync(out, { recursive: true, force: true }); return { ...result, engine: 'godot-web' }; })); } finally {rmSync(out,{recursive:true,force:true});}
} else {
  console.error('usage: bun game/bench/run.mjs <exact-web|exact-macos|godot|godot-web|three> <scene> <n> [mode] [variant]');
  process.exit(2);
}
