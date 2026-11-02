// A heavy-list page load to its first row (message 0's avatar laid out), fresh profile, no profiler:
// bun loadtime.mjs <root>. Prints {first, heap}; run it a few times and take the median.
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


  await page.goto(`http://127.0.0.1:${server.port}/`);
  await page.waitForFunction(() => window.__first, null, { timeout: 30000 });
  const first = await page.evaluate(() => window.__first);
  const heap = (await cdp.send('Runtime.getHeapUsage')).usedSize / 1e6;
  console.log(JSON.stringify({ first: Math.round(first), heap: +heap.toFixed(1) }));
} finally { await ctx.close(); rmSync(dir, { recursive: true, force: true }); server.stop(true); }
