// CPU profile of a heavy-list page load to its first row: bun loadprof.mjs <root> <out-prefix>
// Prints main-thread self time by category (V8 parse/compile is attributed to
// "(program)"/"(parse)" frames; the probe also reads Performance metrics).
import { chromium } from 'playwright-core';
import { mkdtempSync, rmSync, writeFileSync, existsSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, extname, resolve } from 'node:path';
const [root, out] = process.argv.slice(2).map((p, i) => i === 0 ? resolve(p) : p);
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.css': 'text/css', '.json': 'application/json', '.jpg': 'image/jpeg' };
const server = Bun.serve({ port: 0, fetch(req) { let f = join(root, decodeURIComponent(new URL(req.url).pathname)); if (!existsSync(f) || statSync(f).isDirectory()) f = join(root, 'index.html'); return new Response(Bun.file(f), { headers: { 'content-type': types[extname(f)] ?? 'application/octet-stream', 'cache-control': 'no-store' } }); } });
const dir = mkdtempSync(join(tmpdir(), 'dxload-'));
const ctx = await chromium.launchPersistentContext(dir, { executablePath: process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: false, viewport: { width: 420, height: 900 }, deviceScaleFactor: 2, args: ['--window-position=40,40', '--no-first-run'] });
try {
  const page = ctx.pages()[0];
  await page.addInitScript(() => { const tick = () => { const a = document.querySelector('img[src*="avatar-00"]'); if (a && a.getBoundingClientRect().height >= 39) { window.__first = performance.now(); return; } requestAnimationFrame(tick); }; requestAnimationFrame(tick); });
  const cdp = await ctx.newCDPSession(page);
  await cdp.send('Profiler.enable'); await cdp.send('Profiler.setSamplingInterval', { interval: 50 });
  await cdp.send('Profiler.start');
  await page.goto(`http://127.0.0.1:${server.port}/`);
  await page.waitForFunction(() => window.__first, null, { timeout: 30000 });
  const first = await page.evaluate(() => window.__first);
  const { profile } = await cdp.send('Profiler.stop');
  writeFileSync(`${out}.cpuprofile`, JSON.stringify(profile));
  const byId = new Map(profile.nodes.map((n) => [n.id, n]));
  const parent = new Map(); for (const n of profile.nodes) for (const c of n.children ?? []) parent.set(c, n.id);
  // Only samples before the first row.
  let t = profile.startTime; const cut = profile.startTime + first * 1000 + 50000;
  const fn = new Map(), script = new Map();
  for (let i = 0; i < profile.samples.length; i++) {
    t += profile.timeDeltas[i]; if (t > cut) break;
    const f = byId.get(profile.samples[i]).callFrame;
    if (f.functionName === '(idle)') continue;
    const url = f.url.split('/').pop() || '(native)';
    const k = `${f.functionName || '(anon)'} ${url}:${f.lineNumber}`;
    fn.set(k, (fn.get(k) ?? 0) + profile.timeDeltas[i]); script.set(url, (script.get(url) ?? 0) + profile.timeDeltas[i]);
  }
  const top = (m, n) => [...m].sort((a, b) => b[1] - a[1]).slice(0, n).map(([k, us]) => `${(us / 1000).toFixed(1).padStart(7)} ms  ${k}`).join('\n');
  console.log(`first row at ${first.toFixed(0)} ms\nby script:\n${top(script, 6)}\nby function:\n${top(fn, 14)}`);
} finally { await ctx.close(); rmSync(dir, { recursive: true, force: true }); server.stop(true); }
