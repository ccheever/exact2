// CPU profile of a heavy-list page (bench/heavy-list/SPEC.md) at rest and while it scrolls, in headed Chrome:
//   bun bench/web-shootout/scrollprof.mjs <dist root> <out prefix> [px/s=6000] [seconds=3] [query]
// Prints Performance.getMetrics deltas (task, script, style, layout ms), the DOM node delta, the count of the DOM
// calls the runtime made (rect reads, getComputedStyle, createElement, attribute and inline style writes, …) and
// main-thread self time by function (minified names: url:line:col finds the site in app.js) and by script; writes
// <out prefix>-rest.cpuprofile and <out prefix>-scroll.cpuprofile. LLP 1152.000 §2.1 is a run of it on the bounded
// feed (bench/dioxus/web/build.sh); LLP 1071.001 is the RFC its numbers led to. Run from an exact2 checkout (for
// playwright-core); CHROME names the browser.
import { chromium } from 'playwright-core';
import { mkdtempSync, rmSync, writeFileSync, existsSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, extname, resolve } from 'node:path';
const [rootArg, out, speedArg, secondsArg, query] = process.argv.slice(2);
const root = resolve(rootArg), speed = Number(speedArg ?? 6000), seconds = Number(secondsArg ?? 3);
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.css': 'text/css', '.json': 'application/json', '.jpg': 'image/jpeg' };
const server = Bun.serve({ port: 0, fetch(req) { let f = join(root, decodeURIComponent(new URL(req.url).pathname)); if (!existsSync(f) || statSync(f).isDirectory()) f = join(root, 'index.html'); return new Response(Bun.file(f), { headers: { 'content-type': types[extname(f)] ?? 'application/octet-stream', 'cache-control': 'no-store' } }); } });
const dir = mkdtempSync(join(tmpdir(), 'scrollprof-'));
const ctx = await chromium.launchPersistentContext(dir, { executablePath: process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: false, viewport: { width: 420, height: 900 }, deviceScaleFactor: 2, args: ['--window-position=40,40', '--no-first-run'] });
const metricsOf = async (cdp) => Object.fromEntries((await cdp.send('Performance.getMetrics')).metrics.map(m => [m.name, m.value]));
const summarize = (profile, label) => {
  const byId = new Map(profile.nodes.map((n) => [n.id, n]));
  const fn = new Map(), script = new Map(); let total = 0;
  for (let i = 0; i < profile.samples.length; i++) {
    const f = byId.get(profile.samples[i]).callFrame, dt = profile.timeDeltas[i];
    if (f.functionName === '(idle)') continue;
    total += dt;
    const url = f.url.split('/').pop() || '(native)';
    const k = `${f.functionName || '(anon)'} ${url}:${f.lineNumber}:${f.columnNumber}`;
    fn.set(k, (fn.get(k) ?? 0) + dt); script.set(url, (script.get(url) ?? 0) + dt);
  }
  const top = (m, n) => [...m].sort((a, b) => b[1] - a[1]).slice(0, n).map(([k, us]) => `${(us / 1000).toFixed(1).padStart(7)} ms  ${k}`).join('\n');
  console.log(`\n== ${label}: ${(total / 1000).toFixed(0)} ms sampled (non-idle)\nby script:\n${top(script, 6)}\nby function (self):\n${top(fn, 30)}`);
};
try {
  const page = ctx.pages()[0];
  await page.addInitScript(() => { const tick = () => { const a = document.querySelector('img[src*="avatar-00"]'); if (a && a.getBoundingClientRect().height >= 39) { window.__first = performance.now(); return; } requestAnimationFrame(tick); }; requestAnimationFrame(tick); });
  const cdp = await ctx.newCDPSession(page);
  await cdp.send('Performance.enable');
  await cdp.send('Profiler.enable'); await cdp.send('Profiler.setSamplingInterval', { interval: 100 });
  await page.goto(`http://127.0.0.1:${server.port}/${query ?? ''}`);
  await page.waitForFunction(() => window.__first, null, { timeout: 30000 });
  await page.waitForTimeout(1500);
  const list = await page.evaluateHandle(() => { let best = null; for (const el of document.querySelectorAll('*')) { const s = getComputedStyle(el); if ((s.overflowY === 'auto' || s.overflowY === 'scroll') && el.scrollHeight > el.clientHeight + 100 && (!best || el.scrollHeight > best.scrollHeight)) best = el; } return best; });
  const box = await list.evaluate(el => { const r = el.getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 }; });
  const nodes = () => page.evaluate(() => document.querySelectorAll('*').length);
  await page.evaluate(() => {
    const C = window.__c = {}; const wrap = (proto, name, key = name) => { const f = proto[name]; proto[name] = function (...a) { C[key] = (C[key] ?? 0) + 1; return f.apply(this, a); }; };
    wrap(Element.prototype, 'getBoundingClientRect'); wrap(Element.prototype, 'getClientRects'); wrap(Element.prototype, 'setAttribute'); wrap(Element.prototype, 'getAttribute'); wrap(Element.prototype, 'getAttributeNames');
    wrap(Document.prototype, 'createElement'); wrap(Document.prototype, 'querySelectorAll', 'doc.querySelectorAll'); wrap(Element.prototype, 'querySelectorAll', 'el.querySelectorAll'); wrap(Element.prototype, 'querySelector', 'el.querySelector');
    const gcs = window.getComputedStyle; window.getComputedStyle = function (...a) { C.getComputedStyle = (C.getComputedStyle ?? 0) + 1; return gcs.apply(this, a); };
    wrap(CSSStyleDeclaration.prototype, 'setProperty'); wrap(CSSStyleDeclaration.prototype, 'removeProperty'); wrap(Node.prototype, 'cloneNode');
    let scrolls = 0, frames = 0; addEventListener('scroll', () => scrolls++, { capture: true, passive: true }); const raf = () => { frames++; requestAnimationFrame(raf); }; requestAnimationFrame(raf);
    Object.defineProperty(C, 'scrollEvents', { get: () => scrolls, enumerable: true }); Object.defineProperty(C, 'rafFrames', { get: () => frames, enumerable: true });
  });
  // Phase 1: at rest (live mode's cost when the page is idle)
  let m0 = await metricsOf(cdp), n0 = await nodes();
  await cdp.send('Profiler.start');
  await page.waitForTimeout(seconds * 1000);
  const rest = (await cdp.send('Profiler.stop')).profile;
  let m1 = await metricsOf(cdp), n1 = await nodes();
  writeFileSync(`${out}-rest.cpuprofile`, JSON.stringify(rest));
  const d = (a, b, k) => ((b[k] - a[k]) * 1000).toFixed(0);
  console.log(`rest ${seconds}s: task ${d(m0, m1, 'TaskDuration')} ms, script ${d(m0, m1, 'ScriptDuration')}, style ${d(m0, m1, 'RecalcStyleDuration')}, layout ${d(m0, m1, 'LayoutDuration')}, nodes ${n0} -> ${n1}`);
  summarize(rest, `rest ${seconds}s`);
  const counts = () => page.evaluate(() => JSON.parse(JSON.stringify(window.__c)));
  // Phase 2: scrolling
  const c0 = await counts();
  m0 = await metricsOf(cdp); n0 = await nodes();
  const top0 = await list.evaluate(el => el.scrollTop);
  await cdp.send('Profiler.start');
  const t0 = Date.now();
  await cdp.send('Input.synthesizeScrollGesture', { x: box.x, y: box.y, yDistance: -speed * seconds, speed, gestureSourceType: 'mouse' });
  const wall = (Date.now() - t0) / 1000;
  const scroll = (await cdp.send('Profiler.stop')).profile;
  m1 = await metricsOf(cdp); n1 = await nodes();
  const top1 = await list.evaluate(el => el.scrollTop);
  writeFileSync(`${out}-scroll.cpuprofile`, JSON.stringify(scroll));
  console.log(`\nscroll ${speed} px/s for ${seconds}s (wall ${wall.toFixed(1)}s, scrollTop ${top0} -> ${top1}): task ${d(m0, m1, 'TaskDuration')} ms, script ${d(m0, m1, 'ScriptDuration')}, style ${d(m0, m1, 'RecalcStyleDuration')}, layout ${d(m0, m1, 'LayoutDuration')}, nodes ${n0} -> ${n1}`);
  const c1 = await counts(); console.log('DOM API calls during the scroll: ' + Object.keys(c1).map(k => `${k} ${c1[k] - (c0[k] ?? 0)}`).join(', '));
  summarize(scroll, `scroll ${speed} px/s`);
} finally { await ctx.close(); rmSync(dir, { recursive: true, force: true }); server.stop(true); }
