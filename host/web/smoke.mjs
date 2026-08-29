#!/usr/bin/env node
// Serve dist/ and render it in headless Chrome: the first real-browser check
// and the first boot measurement. Prints the DOM's boot stamp and asserts the
// app's landmarks are present. Not a blocking check (it needs Chrome and a
// second or two); run it by hand after `node host/web/build.mjs`.
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { readFileSync, existsSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, extname } from 'node:path';

const dist = resolve(new URL('./dist', import.meta.url).pathname);
if (!existsSync(resolve(dist, 'app.wasm'))) { console.error('run node host/web/build.mjs first'); process.exit(2); }
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm' };
let gpuBeacon = null;
const server = createServer((req, res) => {
  if (req.url.startsWith('/__gpu')) { gpuBeacon = new URL(req.url, 'http://x').searchParams.get('ms'); res.writeHead(204); res.end(); return; }
  const pathname = new URL(req.url, 'http://x').pathname;
  const path = resolve(dist, '.' + (pathname === '/' ? '/index.html' : pathname));
  if (!path.startsWith(dist) || !existsSync(path)) { res.writeHead(404); res.end(); return; }
  res.writeHead(200, { 'content-type': types[extname(path)] ?? 'application/octet-stream' });
  res.end(readFileSync(path));
});
await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
const port = server.address().port;
const chrome = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const profile = mkdtempSync(resolve(tmpdir(), 'exact-web-smoke-'));
// With EXACT_SHOT=<png>, Chrome also renders real frames (so animation
// frames fire and the GPU module loads after the paint stamp) and writes a
// screenshot; WebGPU is enabled for it. Without it, --dump-dom alone fires
// no animation frame and the GPU path is reported, not asserted.
const shot = process.env.EXACT_SHOT;
const argsFor = (rendering) => ['--headless=new', ...(rendering ? ['--enable-unsafe-webgpu', `--screenshot=${shot}`, '--window-size=420,900', '--hide-scrollbars'] : ['--disable-gpu']), `--user-data-dir=${profile}`, '--no-sandbox', '--disable-extensions', '--disable-background-networking', '--disable-component-update', '--no-first-run', '--no-default-browser-check', `--timeout=${rendering ? 6000 : 10000}`, '--dump-dom', `http://127.0.0.1:${port}/${rendering ? '?smoke=1' : ''}`];
const args = argsFor(false);
// Chrome's helper processes keep the pipes open after the main process
// exits, so read until the main process exits and then kill the whole group.
// A fresh profile's first launch can spend the whole timeout before the page
// runs, so a dump with no boot stamp is retried once.
const render = (rendering = false) => new Promise((done, fail) => {
  const child = spawn(chrome, rendering ? argsFor(true) : args, { detached: true, stdio: ['ignore', 'pipe', 'pipe'] });
  let out = '';
  let err = '';
  child.stdout.on('data', (d) => { out += d; });
  child.stderr.on('data', (d) => { err += d; });
  // In screenshot mode Chrome's main process lingers after its work; the
  // DOM it printed is the result, so a timeout with a DOM is a success.
  const timer = setTimeout(() => { try { process.kill(-child.pid, 'SIGKILL'); } catch {} if (rendering || out.length > 0) done(out); else fail(new Error('chrome timed out; stderr:\n' + err)); }, rendering ? 12000 : 30000);
  child.on('exit', (code) => {
    clearTimeout(timer);
    try { process.kill(-child.pid, 'SIGKILL'); } catch {}
    if (code !== 0) fail(new Error(`chrome exited ${code}; stderr:\n` + err.split('\n').filter((l) => !/crashpad|updater|gcm|VERBOSE/i.test(l)).join('\n')));
    else done(out);
  });
});
let dom = await render().catch((e) => { server.close(); console.error(e.message); process.exit(1); });
if (!/data-boot-ms=/.test(dom)) dom = await render().catch((e) => { server.close(); console.error(e.message); process.exit(1); });
// A rendering run (real frames, WebGPU, a screenshot) when asked: judged by the
// GPU glue's beacon and the PNG, not by the DOM — Chrome's --screenshot mode
// is unreliable about printing one.
if (shot) await render(true).catch(() => {});
server.close();
rmSync(profile, { recursive: true, force: true });

const boot = /data-boot-ms="([\d.]+)"/.exec(dom)?.[1];
const paint = /data-paint-ms="([\d.]+)"/.exec(dom)?.[1];
const failures = [];
for (const id of ['caltrain-main', 'station-name', 'board-north', 'board-south', 'change-station']) {
  if (!dom.includes(`data-testid="${id}"`)) failures.push(`missing data-testid="${id}"`);
}
if (!/Mountain View/.test(dom)) failures.push('station name not rendered');
if (/data-error=/.test(dom)) failures.push('glue reported an error');
if (!boot) failures.push('no boot stamp (the first frame never reached the DOM)');
const countdowns = (dom.match(/data-testid="countdown-/g) ?? []).length;
// The GPU module (LLP 1009): a canvas on the page, the module loaded after
// the paint stamp, and the surface created (its drawable sized) — or the
// reason it was not.
const hasCanvas = /<canvas[^>]*data-testid="line-map"/.test(dom);
const gpuMs = gpuBeacon;
if (hasCanvas && shot && !gpuMs) failures.push('a canvas is on the page but the GPU module did not load in the rendering run (no beacon; WebGPU unavailable in this Chrome?)');
if (hasCanvas && shot && !existsSync(shot)) failures.push('no screenshot was written');
console.log(`gpu: canvas ${hasCanvas ? 'present' : 'absent'}${gpuMs ? `; module loaded ${gpuMs} ms after injection (after the first paint); screenshot ${shot}` : shot ? '' : ' (rendering not exercised: set EXACT_SHOT=<png> for a run with real frames and WebGPU)'}`);
console.log(`headless Chrome: script start → first frame in the DOM ${boot ?? '?'} ms${paint ? `, painted ${paint} ms` : ' (no paint stamp: headless dump has no compositor frame)'}; ${countdowns} countdowns; ${dom.length} bytes of DOM`);
if (failures.length) { for (const f of failures) console.error('  ' + f); process.exit(1); }
console.log('web smoke: ok');
