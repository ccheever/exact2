#!/usr/bin/env bun
// Record the motion parity fixture from a real browser and check the engine
// against it. The cases and the check are Rust (host/web/src/parity.rs);
// this script serves parity.html + cases.json, renders it in headless Chrome,
// writes host/web/tests/fixtures/browser-motion.txt, and runs the check.
// Not a blocking check (it needs Chrome); the fixture it writes is held by
// `cargo test -p exact-web` (tests/parity.rs) with no browser at all.
import { spawn, spawnSync } from 'node:child_process';
import { createServer } from 'node:http';
import { readFileSync, writeFileSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';

const here = resolve(new URL('.', import.meta.url).pathname);
const root = resolve(here, '../..');
const run = (args) => spawnSync('cargo', ['run', '-q', '--release', '-p', 'exact-web', '--bin', 'parity', '--', ...args], { cwd: root, encoding: 'utf8' });
const cases = run(['cases']);
if (cases.status !== 0) { console.error(cases.stderr); process.exit(1); }
const html = readFileSync(resolve(here, 'parity.html'));
const server = createServer((req, res) => {
  if (req.url.startsWith('/cases.json')) { res.writeHead(200, { 'content-type': 'application/json' }); res.end(cases.stdout); return; }
  res.writeHead(200, { 'content-type': 'text/html' }); res.end(html);
});
await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
const port = server.address().port;
const chrome = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const render = () => new Promise((done) => {
  const profile = mkdtempSync(resolve(tmpdir(), 'exact-parity-'));
  const child = spawn(chrome, ['--headless=new', '--disable-gpu', `--user-data-dir=${profile}`, '--no-sandbox', '--disable-extensions', '--disable-background-networking', '--no-first-run', '--no-default-browser-check', '--timeout=8000', '--dump-dom', `http://127.0.0.1:${port}/`], { detached: true, stdio: ['ignore', 'pipe', 'ignore'] });
  let dom = '';
  child.stdout.on('data', (d) => { dom += d; });
  const timer = setTimeout(() => { try { process.kill(-child.pid, 'SIGKILL'); } catch {} }, 20000);
  child.on('exit', () => { clearTimeout(timer); try { process.kill(-child.pid, 'SIGKILL'); } catch {} rmSync(profile, { recursive: true, force: true }); done(dom); });
});
let dom = await render();
if (!/data-done="1"/.test(dom)) dom = await render();
server.close();
const text = /<pre id="out"[^>]*>([\s\S]*?)<\/pre>/.exec(dom)?.[1];
if (!text || /^error /m.test(text)) { console.error('the page did not finish:\n' + (text ?? dom.slice(0, 500))); process.exit(1); }
const lines = text.trim().split('\n').map((l) => l.replace(/&amp;/g, '&').replace(/&lt;/g, '<').replace(/&gt;/g, '>'));
const browser = lines.find((l) => l.startsWith('browser '))?.slice(8) ?? 'unknown browser';
const header = run(['header', `${new Date().toISOString().slice(0, 10)}, ${browser}`]).stdout;
const fixture = resolve(here, 'tests/fixtures/browser-motion.txt');
writeFileSync(fixture, header + lines.filter((l) => l.startsWith('sample ')).join('\n') + '\n');
console.log(`recorded ${lines.filter((l) => l.startsWith('sample ')).length} samples from ${browser} → host/web/tests/fixtures/browser-motion.txt`);
const check = run(['check', fixture]);
process.stdout.write(check.stdout);
process.exit(check.status ?? 1);
