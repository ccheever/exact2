// Scroll progress by content, not scrollTop: each rAF, the message at the
// list's top edge (data-testid="row-<id>"). A bounded window shifts scrollTop
// back when rows above it leave, so rows passed is the honest progress.
// bun progress.mjs <root> <speed px/s> <seconds> [shot.png]   (LOG=1 echoes the page's UPDATE/RESTORE
// console lines, DUMP=1 the frames around the first backward step)
import { chromium } from 'playwright-core';
import { mkdtempSync, rmSync, existsSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, extname, resolve } from 'node:path';
const [root, speed, seconds, shot] = process.argv.slice(2);
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.css': 'text/css', '.json': 'application/json', '.jpg': 'image/jpeg' };
const server = Bun.serve({ port: 0, fetch(req) { let f = join(resolve(root), decodeURIComponent(new URL(req.url).pathname)); if (!existsSync(f) || statSync(f).isDirectory()) f = join(resolve(root), 'index.html'); return new Response(Bun.file(f), { headers: { 'content-type': types[extname(f)] ?? 'application/octet-stream' } }); } });
const dir = mkdtempSync(join(tmpdir(), 'dxprog-'));
const ctx = await chromium.launchPersistentContext(dir, { executablePath: process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: false, viewport: { width: 420, height: 900 }, deviceScaleFactor: 2, args: ['--window-position=40,40', '--no-first-run'] });
try {
  const page = ctx.pages()[0];
  if (process.env.LOG) page.on('console', (m) => { const t = m.text(); if (/^(UPDATE|RESTORE)/.test(t)) console.log(t); });
  await page.goto(`http://127.0.0.1:${server.port}/`);
  await page.waitForTimeout(3000);
  const box = await page.evaluate(() => {
    const list = [...document.querySelectorAll('*')].filter((e) => { const s = getComputedStyle(e); return (s.overflowY === 'auto' || s.overflowY === 'scroll') && e.scrollHeight > e.clientHeight + 100; }).sort((a, b) => b.scrollHeight - a.scrollHeight)[0];
    window.__list = list; window.__tops = []; window.__run = true;
    const r = list.getBoundingClientRect();
    const tick = () => {
      if (!window.__run) return;
      const hit = document.elementFromPoint(r.left + r.width / 2, r.top + 4)?.closest('[data-testid^="row-"]');
      window.__tops.push(hit ? hit.getAttribute('data-testid').slice(4) : null); (window.__st ??= []).push([Math.round(list.scrollTop), Math.round(list.scrollHeight), list.querySelectorAll('[data-testid^="row-"]').length]);
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
    return { x: r.left + r.width / 2, y: r.top + r.height / 2 };
  });
  const cdp = await ctx.newCDPSession(page);
  const t0 = performance.now();
  await cdp.send('Input.synthesizeScrollGesture', { x: box.x, y: box.y, yDistance: -Math.round(Number(speed) * Number(seconds)), speed: Number(speed), gestureSourceType: 'mouse' });
  const wall = (performance.now() - t0) / 1000;
  const tops = await page.evaluate(() => { window.__run = false; return window.__tops; });
  const st = await page.evaluate(() => window.__st);
  if (process.env.DUMP) { let i0 = -1, best = -1; for (let i = 0; i < tops.length; i++) { const v = tops[i] ? Number(tops[i].slice(1)) : null; if (v === null) continue; if (v < best) { i0 = i; break; } best = v; } for (let i = Math.max(0, i0 - 6); i < Math.min(tops.length, i0 + 8); i++) console.log(i, tops[i], st[i]); }
  const idx = tops.map((t) => (t && t.startsWith('m') ? Number(t.slice(1)) : null));
  let back = 0, worstBack = 0, gaps = 0;
  for (let i = 1; i < idx.length; i++) {
    if (idx[i] === null) { gaps++; continue; }
    const prev = idx.slice(0, i).reverse().find((v) => v !== null);
    if (prev !== undefined && idx[i] < prev) { back++; worstBack = Math.max(worstBack, prev - idx[i]); }
  }
  const first = idx.find((v) => v !== null), last = [...idx].reverse().find((v) => v !== null);
  console.log(JSON.stringify({ speed: Number(speed), wallS: +wall.toFixed(2), frames: idx.length, firstRow: first, lastRow: last, rowsPerS: +((last - first) / wall).toFixed(1), backwardFrames: back, worstBackRows: worstBack, framesWithNoRowAtTop: gaps }));
  if (shot) await page.screenshot({ path: shot });
} finally { await ctx.close(); rmSync(dir, { recursive: true, force: true }); server.stop(true); }
