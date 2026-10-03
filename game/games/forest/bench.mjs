#!/usr/bin/env bun
// Live frame cost of the forest in Chrome WebGPU, one JSON line per scene.
// Diagnostic, never a check. Build first (`bun game/games/forest/proof.mjs web
// --screenshot-only` leaves game/games/forest/dist), then:
//
//   bun game/games/forest/bench.mjs trees-1k trees-20k trees-100k
//   bun game/games/forest/bench.mjs trees-20k,lite trees-5k,wolves-4096 trees-2k,torches-64
//
// A scene is the title buttons to press before Play. The page runs on the live
// clock, holds W for two seconds (the camera walks into the trees), then samples
// state.world.perf for BENCH_SECONDS (default 6). Headless Chrome does real GPU
// work but has no display: the numbers are sanity-only, as game/bench labels them.
import { spawn, spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir, loadavg } from 'node:os';
import { join, resolve, extname } from 'node:path';
import { cdp, sleep, stop, track } from '../../bench/exact.mjs';

const dist = resolve(import.meta.dir, 'dist');
const CHROME = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const seconds = Number(process.env.BENCH_SECONDS ?? 6);
const headless = process.env.BENCH_HEADLESS !== '0';
const types = {'.html':'text/html', '.js':'text/javascript', '.json':'application/json', '.wasm':'application/wasm'};
const server = Bun.serve({port: 0, async fetch(req) {
  const path = resolve(dist, '.' + decodeURIComponent(new URL(req.url).pathname.replace(/\/$/, '/index.html')));
  if (!path.startsWith(dist + '/')) return new Response('forbidden', {status: 403});
  const file = Bun.file(path);
  if (!(await file.exists())) return new Response('not found', {status: 404});
  return new Response(file, {headers: {'content-type': types[extname(path)] ?? 'application/octet-stream',
    'Cross-Origin-Opener-Policy': 'same-origin', 'Cross-Origin-Embedder-Policy': 'require-corp'}});
}});

const state = `(reset=false)=>{const el=document.querySelector('[data-testid="world"]');const id=Number(el?.dataset.view);return globalThis.exact?.gpu?.agent(id,{op:'state',perf:true,...(reset?{perf_reset:true}:{})});}`;
const percentile = (ring, q) => ring?.[q] ?? null;

async function scene(spec) {
  const profile = mkdtempSync(join(tmpdir(), 'forest-bench-chrome-'));
  let chrome, client;
  try {
    chrome = track(spawn(CHROME, [`--user-data-dir=${profile}`, '--no-first-run', '--no-default-browser-check',
      '--window-size=1280,720', '--remote-debugging-port=0', ...(headless ? ['--headless=new'] : []),
      '--enable-unsafe-webgpu', '--enable-precise-memory-info', '--disable-background-timer-throttling',
      '--disable-renderer-backgrounding', '--disable-backgrounding-occluded-windows',
      `http://localhost:${server.port}/`], {stdio: 'ignore'}));
    const deadline = Date.now() + 60000;
    while (!client && Date.now() < deadline) {
      try {
        const port = (await Bun.file(join(profile, 'DevToolsActivePort')).text()).split('\n')[0];
        const page = (await (await fetch(`http://localhost:${port}/json/list`)).json()).find(p => p.type === 'page');
        if (page) client = await cdp(page.webSocketDebuggerUrl);
      } catch {}
      if (!client) await sleep(100);
    }
    if (!client) throw new Error('no CDP page');
    await client.call('Emulation.setDeviceMetricsOverride', {width: 1280, height: 720, deviceScaleFactor: 2, mobile: false});
    const click = async id => {
      let box;
      for (let i = 0; i < 200 && !box; i++) {
        box = await client.eval(`(()=>{const e=document.querySelector('[data-testid="${id}"]');if(!e)return null;const r=e.getBoundingClientRect();return r.width?[r.x+r.width/2,r.y+r.height/2]:null})()`).catch(() => null);
        if (!box) await sleep(100);
      }
      if (!box) throw new Error(`no ${id}`);
      for (const type of ['mouseMoved', 'mousePressed', 'mouseReleased'])
        await client.call('Input.dispatchMouseEvent', {type, x: box[0], y: box[1], button: 'left', clickCount: 1});
      await sleep(150);
    };
    // A press before the page takes input is lost: repeat until the title's summary changes.
    const choice = () => client.eval(`document.querySelector('[data-testid="choice"]')?.textContent ?? ''`);
    while (!(await choice().catch(() => '')) && Date.now() < deadline) await sleep(100);
    for (const id of spec.split(',')) {
      const before = await choice();
      // A default choice (trees-2k, wolves-8) changes nothing; three presses then move on.
      for (let i = 0; i < 3 && (await choice()) === before; i++) { await click(id); await sleep(400); }
    }
    const chosen = await choice();
    const pressed = Date.now();
    await click('play');
    let s;
    while (Date.now() < deadline + 120000) {
      s = await client.eval(`(${state})()`).catch(() => null);
      if (s?.world?.perf?.frameMs?.count > 0) break;
      await sleep(100);
    }
    const ready = Date.now() - pressed;
    if (!s?.world?.perf) throw new Error(`world never drew: ${JSON.stringify(s)?.slice(0, 400)}`);
    const key = async (type) => client.call('Input.dispatchKeyEvent', {type, key: 'w', code: 'KeyW', windowsVirtualKeyCode: 87}).catch(() => null);
    await key('keyDown'); await sleep(2000); await key('keyUp');
    await sleep(500);
    await client.eval(`(${state})(true)`).catch(() => null);
    await sleep(seconds * 1000);
    // A frame that takes longer than CDP's 30 s answer window is itself the result.
    for (let i = 0; i < 4; i++) { s = await client.eval(`(${state})()`).catch(error => ({error})); if (!s.error) break; }
    if (s.error) throw s.error;
    const memory = await client.eval(`(async()=>{try{return (await performance.measureUserAgentSpecificMemory()).bytes}catch(e){return performance.memory?.usedJSHeapSize??null}})()`);
    const p = s.world.perf, ms = r => r && {mean: +r.mean?.toFixed(3), p50: percentile(r, 'p50'), p95: percentile(r, 'p95'), max: +r.max?.toFixed(2)};
    return {scene: spec, chosen, entities: s.world.entities, ready_ms: ready, frames: p.frameMs.count,
      fps: +(p.frameMs.count / seconds).toFixed(1), frame_ms: ms(p.frameMs), tick_ms: ms(p.tickMs), feed_ms: ms(p.feedMs),
      encode_ms: ms(p.encodeMs), ticks_per_frame: ms(p.ticksPerFrame), draws: p.draws, instances: p.instances, triangles: p.triangles,
      culled: p.culled ?? null, gpu_ms: s.world.gpuMs ?? null, memory_bytes: memory, headless, load1: +loadavg()[0].toFixed(1)};
  } finally {
    client?.close();
    await stop(chrome);
    const left = spawnSync('ps', ['-axo', 'pid=,command='], {encoding: 'utf8'}).stdout.split('\n').filter(l => l.includes(profile));
    if (!left.length) rmSync(profile, {recursive: true, force: true});
  }
}

try {
  for (const spec of process.argv.slice(2)) {
    try { console.log(JSON.stringify(await scene(spec))); }
    catch (error) { console.log(JSON.stringify({scene: spec, error: String(error.message ?? error).slice(0, 500)})); }
  }
} finally { server.stop(true); }
