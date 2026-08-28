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
const server = createServer((req, res) => {
  const path = resolve(dist, '.' + (req.url === '/' ? '/index.html' : req.url.split('?')[0]));
  if (!path.startsWith(dist) || !existsSync(path)) { res.writeHead(404); res.end(); return; }
  res.writeHead(200, { 'content-type': types[extname(path)] ?? 'application/octet-stream' });
  res.end(readFileSync(path));
});
await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
const port = server.address().port;
const chrome = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const profile = mkdtempSync(resolve(tmpdir(), 'exact-web-smoke-'));
const args = ['--headless=new', '--disable-gpu', `--user-data-dir=${profile}`, '--no-sandbox', '--disable-extensions', '--disable-background-networking', '--disable-component-update', '--no-first-run', '--no-default-browser-check', '--timeout=10000', '--dump-dom', `http://127.0.0.1:${port}/`];
// Chrome's helper processes keep the pipes open after the main process
// exits, so read until the main process exits and then kill the whole group.
// A fresh profile's first launch can spend the whole timeout before the page
// runs, so a dump with no boot stamp is retried once.
const render = () => new Promise((done, fail) => {
  const child = spawn(chrome, args, { detached: true, stdio: ['ignore', 'pipe', 'pipe'] });
  let out = '';
  let err = '';
  child.stdout.on('data', (d) => { out += d; });
  child.stderr.on('data', (d) => { err += d; });
  const timer = setTimeout(() => { try { process.kill(-child.pid, 'SIGKILL'); } catch {} fail(new Error('chrome timed out; stderr:\n' + err)); }, 30000);
  child.on('exit', (code) => {
    clearTimeout(timer);
    try { process.kill(-child.pid, 'SIGKILL'); } catch {}
    if (code !== 0) fail(new Error(`chrome exited ${code}; stderr:\n` + err.split('\n').filter((l) => !/crashpad|updater|gcm|VERBOSE/i.test(l)).join('\n')));
    else done(out);
  });
});
let dom = await render().catch((e) => { server.close(); console.error(e.message); process.exit(1); });
if (!/data-boot-ms=/.test(dom)) dom = await render().catch((e) => { server.close(); console.error(e.message); process.exit(1); });
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
console.log(`headless Chrome: script start → first frame in the DOM ${boot ?? '?'} ms${paint ? `, painted ${paint} ms` : ' (no paint stamp: headless dump has no compositor frame)'}; ${countdowns} countdowns; ${dom.length} bytes of DOM`);
if (failures.length) { for (const f of failures) console.error('  ' + f); process.exit(1); }
console.log('web smoke: ok');
