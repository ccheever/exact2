// CPU profile of reaction taps on a heavy-list page: bun tapprof.mjs <root> <out-prefix>
import { chromium } from 'playwright-core';
import { mkdtempSync, rmSync, writeFileSync, existsSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, extname, resolve } from 'node:path';
const [root, out] = process.argv.slice(2).map((p, i) => i === 0 ? resolve(p) : p);
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.css': 'text/css', '.json': 'application/json', '.jpg': 'image/jpeg' };
const server = Bun.serve({ port: 0, fetch(req) { let f = join(root, decodeURIComponent(new URL(req.url).pathname)); if (!existsSync(f) || statSync(f).isDirectory()) f = join(root, 'index.html'); return new Response(Bun.file(f), { headers: { 'content-type': types[extname(f)] ?? 'application/octet-stream' } }); } });
const dir = mkdtempSync(join(tmpdir(), 'dxprof-'));
const ctx = await chromium.launchPersistentContext(dir, { executablePath: process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: false, viewport: { width: 420, height: 900 }, deviceScaleFactor: 2, args: ['--window-position=40,40', '--no-first-run'] });
try {
  const page = ctx.pages()[0];
  await page.goto(`http://127.0.0.1:${server.port}/`);
  await page.waitForTimeout(3000);
  // Scroll the first reaction chip into view.
  const chip = await page.evaluate(async () => {
    const lists = [...document.querySelectorAll('*')].filter((e) => { const s = getComputedStyle(e); return (s.overflowY === 'auto' || s.overflowY === 'scroll') && e.scrollHeight > e.clientHeight + 100; });
    const list = lists.sort((a, b) => b.scrollHeight - a.scrollHeight)[0];
    const b = [...document.querySelectorAll('button[aria-label]')].find((b) => /^\S+ \d+$/.test(b.getAttribute('aria-label')));
    list.scrollTop += b.getBoundingClientRect().top - list.getBoundingClientRect().top - 300;
    await new Promise((r) => setTimeout(r, 500));
    const r = b.getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 };
  });
  await page.mouse.click(chip.x, chip.y); // the first tap (module load) is not profiled
  await page.waitForTimeout(1500);
  const cdp = await ctx.newCDPSession(page);
  await cdp.send('Profiler.enable'); await cdp.send('Profiler.setSamplingInterval', { interval: 50 });
  await cdp.send('Profiler.start');
  for (let i = 0; i < 5; i++) { await page.mouse.click(chip.x, chip.y); await page.waitForTimeout(400); }
  const { profile } = await cdp.send('Profiler.stop');
  writeFileSync(`${out}.cpuprofile`, JSON.stringify(profile));
  // Self time by function and by script.
  const dt = new Map(); const byId = new Map(profile.nodes.map((n) => [n.id, n]));
  for (let i = 0; i < profile.samples.length; i++) { const id = profile.samples[i]; dt.set(id, (dt.get(id) ?? 0) + (profile.timeDeltas[i] ?? 0)); }
  const fn = new Map(), script = new Map();
  for (const [id, us] of dt) { const n = byId.get(id); const f = n.callFrame; if (f.functionName === '(idle)' || f.functionName === '(program)') continue; const url = f.url.split('/').pop() || '(native)'; const k = `${f.functionName || '(anon)'} ${url}:${f.lineNumber}:${f.columnNumber}`; fn.set(k, (fn.get(k) ?? 0) + us); script.set(url, (script.get(url) ?? 0) + us); }
  const top = (m, n) => [...m].sort((a, b) => b[1] - a[1]).slice(0, n).map(([k, us]) => `${(us / 5000).toFixed(1).padStart(6)} ms/tap  ${k}`).join('\n');
  console.log('by script (self, per tap):\n' + top(script, 8) + '\n\nby function:\n' + top(fn, 25));
} finally { await ctx.close(); rmSync(dir, { recursive: true, force: true }); server.stop(true); }
