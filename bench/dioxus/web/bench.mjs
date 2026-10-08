// The heavy list on the web: Dioxus Web vs exact2's JS target in headed
// Chrome, the same harness for both. Run from an exact2 checkout (for
// playwright-core): bun bench/dioxus/web/bench.mjs <out.json> <app=root>...
//
// Per app and round: a fresh profile (cold HTTP cache), a 420×900 CSS-px
// window at DPR 2, then
//  - load: first rAF in which message 0's avatar is laid out (40 px), LCP,
//    bytes transferred, JS heap, DOM nodes, renderer RSS;
//  - scroll: Input.synthesizeScrollGesture (wheel) down the list at 3k, 6k,
//    12k px/s for 3 s each: rAF intervals, main-thread task/script/layout/
//    style ms from Performance.getMetrics, Chrome process CPU (all helpers),
//    and blank samples (each rAF, five points down the list's centre line
//    that hit the list itself or a spacer instead of a row);
//  - tap: five clicks on the first visible reaction chip, Event Timing
//    duration each, and whether the count went up by five.
import { chromium } from 'playwright-core';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync, existsSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, extname, resolve } from 'node:path';

const [out, ...specs] = process.argv.slice(2);
const apps = specs.map((s) => { const [name, root] = s.split('='); return { name, root: resolve(root) }; });
const rounds = Number(process.env.ROUNDS ?? 3);
const speeds = (process.env.SPEEDS ?? '3000,6000,12000').split(',').map(Number);
const CHROME = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const types = { '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.wasm': 'application/wasm', '.css': 'text/css', '.json': 'application/json', '.jpg': 'image/jpeg', '.png': 'image/png', '.plan': 'application/octet-stream' };

function serve(root) {
  return Bun.serve({ port: 0, fetch(req) {
    let path = decodeURIComponent(new URL(req.url).pathname);
    let file = join(root, path);
    if (!existsSync(file) || statSync(file).isDirectory()) file = join(root, 'index.html');
    return new Response(Bun.file(file), { headers: { 'content-type': types[extname(file)] ?? 'application/octet-stream', 'cache-control': 'no-store' } });
  } });
}

// Chrome's processes: every descendant of this process.
function chromeProcs() {
  const rows = execFileSync('ps', ['-A', '-o', 'pid=,ppid=,rss=,time=,ucomm='], { encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 }).trim().split('\n').map((l) => {
    const [pid, ppid, rss, time, ...comm] = l.trim().split(/\s+/);
    const [m, s] = time.split(':'); const parts = time.split(':').map(Number);
    const sec = parts.length === 3 ? parts[0] * 3600 + parts[1] * 60 + parts[2] : parts[0] * 60 + parts[1];
    return { pid: +pid, ppid: +ppid, rss: +rss, cpu: sec, comm: comm.join(' ') };
  });
  const mine = new Set([process.pid]); let grew = true;
  while (grew) { grew = false; for (const r of rows) if (!mine.has(r.pid) && mine.has(r.ppid)) { mine.add(r.pid); grew = true; } }
  return rows.filter((r) => r.pid !== process.pid && mine.has(r.pid));
}
const cpuOf = (procs) => procs.reduce((a, p) => a + p.cpu, 0);

const probe = () => {
  const w = window;
  w.__b = { frames: [], blank: 0, samples: 0, first: null, lcp: null, events: [], sampling: false };
  new PerformanceObserver((l) => { for (const e of l.getEntries()) w.__b.lcp = e.startTime; }).observe({ type: 'largest-contentful-paint', buffered: true });
  new PerformanceObserver((l) => { for (const e of l.getEntries()) if (e.name === 'click' || e.name === 'pointerup') w.__b.events.push({ name: e.name, duration: e.duration, start: e.startTime }); }).observe({ type: 'event', durationThreshold: 16, buffered: true });
  const list = () => {
    if (w.__list && w.__list.isConnected) return w.__list;
    let best = null;
    for (const el of document.querySelectorAll('*')) {
      const s = getComputedStyle(el);
      if ((s.overflowY === 'auto' || s.overflowY === 'scroll') && el.scrollHeight > el.clientHeight + 100 && (!best || el.scrollHeight > best.scrollHeight)) best = el;
    }
    return (w.__list = best);
  };
  w.__findList = list;
  const tick = (t) => {
    const b = w.__b;
    if (b.first === null) {
      const a = document.querySelector('img[src*="avatar-00"]');
      if (a && a.getBoundingClientRect().height >= 39) b.first = performance.now();
    }
    if (b.sampling) {
      b.frames.push(t);
      const el = list();
      if (el) {
        const r = el.getBoundingClientRect();
        for (let k = 1; k <= 5; k++) {
          const hit = document.elementFromPoint(r.left + r.width / 2, r.top + (r.height * k) / 6);
          b.samples++;
          if (!hit || hit === el || (hit.parentElement === el && hit.childElementCount === 0 && !hit.textContent)) b.blank++;
        }
      }
    }
    requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
};

async function metrics(cdp) {
  const { metrics } = await cdp.send('Performance.getMetrics');
  return Object.fromEntries(metrics.map((m) => [m.name, m.value]));
}

async function run(app, url, round) {
  const dir = mkdtempSync(join(tmpdir(), 'dxbench-'));
  const ctx = await chromium.launchPersistentContext(dir, {
    executablePath: CHROME, headless: false, viewport: { width: 420, height: 900 }, deviceScaleFactor: 2,
    args: ['--window-size=440,1000', '--window-position=40,40', '--disable-renderer-backgrounding', '--disable-background-timer-throttling', '--disable-backgrounding-occluded-windows', '--no-first-run', '--no-default-browser-check'],
  });
  const result = { app: app.name, round };
  try {
    const page = ctx.pages()[0] ?? (await ctx.newPage());
    await page.addInitScript(probe);
    const cdp = await ctx.newCDPSession(page);
    await cdp.send('Performance.enable', { timeDomain: 'timeTicks' });
    let bytes = 0; const files = {};
    cdp.on('Network.loadingFinished', (e) => { bytes += e.encodedDataLength; });
    cdp.on('Network.responseReceived', (e) => { const p = new URL(e.response.url).pathname; files[e.requestId] = p; });
    await cdp.send('Network.enable');
    const fileBytes = {};
    cdp.on('Network.loadingFinished', (e) => { const p = files[e.requestId]; if (p && !p.endsWith('.jpg')) fileBytes[p] = e.encodedDataLength; });
    const t0 = Date.now();
    await page.goto(url, { waitUntil: 'load', timeout: 60000 });
    await page.waitForFunction(() => window.__b.first !== null, null, { timeout: 60000 });
    await page.waitForTimeout(2500);
    const load = await page.evaluate(() => ({ first: window.__b.first, lcp: window.__b.lcp, nav: performance.getEntriesByType('navigation')[0]?.toJSON() }));
    const m0 = await metrics(cdp);
    const dom = await cdp.send('Memory.getDOMCounters').catch(() => ({}));
    const procs = chromeProcs();
    result.load = {
      firstRowMs: load.first, lcpMs: load.lcp, domContentLoadedMs: load.nav?.domContentLoadedEventEnd, loadEventMs: load.nav?.loadEventEnd,
      bytesNonImage: Object.values(fileBytes).reduce((a, b) => a + b, 0), bytesAll: bytes, files: fileBytes,
      jsHeapMB: m0.JSHeapUsedSize / 1e6, nodes: dom.nodes, rendererRssMB: Math.max(...procs.filter((p) => /Chrome H/.test(p.comm)).map((p) => p.rss)) / 1024,
      chromeRssMB: procs.reduce((a, p) => a + p.rss, 0) / 1024, wallMs: Date.now() - t0,
    };
    // Scroll.
    const box = await page.evaluate(() => { const r = window.__findList().getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 }; });
    result.scroll = [];
    for (const speed of speeds) {
      const top0 = await page.evaluate(() => { window.__b.frames = []; window.__b.blank = 0; window.__b.samples = 0; window.__b.sampling = true; return window.__findList().scrollTop; });
      const a = await metrics(cdp); const c0 = cpuOf(chromeProcs()); const w0 = performance.now();
      await cdp.send('Input.synthesizeScrollGesture', { x: box.x, y: box.y, yDistance: -Math.round(speed * 3), speed, gestureSourceType: 'mouse', repeatCount: 1 });
      const wall = (performance.now() - w0) / 1000;
      const b = await metrics(cdp); const c1 = cpuOf(chromeProcs());
      const s = await page.evaluate(() => { const b = window.__b; b.sampling = false; const top = window.__findList().scrollTop; return { frames: b.frames, blank: b.blank, samples: b.samples, top }; });
      const iv = s.frames.slice(1).map((t, i) => t - s.frames[i]);
      const sorted = [...iv].sort((x, y) => x - y);
      const vsync = sorted[Math.floor(sorted.length / 2)] ?? 0;
      result.scroll.push({
        speed, pxPerS: (s.top - top0) / wall, wallS: wall, frames: s.frames.length, fps: s.frames.length / wall, medianFrameMs: vsync,
        late: iv.filter((d) => d > vsync * 1.5).length, worstFrameMs: sorted[sorted.length - 1] ?? 0, p99FrameMs: sorted[Math.floor(sorted.length * 0.99)] ?? 0,
        blankPct: s.samples ? (100 * s.blank) / s.samples : 0, scrollTop: s.top,
        mainTaskMsPerS: (1000 * (b.TaskDuration - a.TaskDuration)) / wall, scriptMsPerS: (1000 * (b.ScriptDuration - a.ScriptDuration)) / wall,
        layoutMsPerS: (1000 * (b.LayoutDuration - a.LayoutDuration)) / wall, styleMsPerS: (1000 * (b.RecalcStyleDuration - a.RecalcStyleDuration)) / wall,
        chromeCpuMsPerS: (1000 * (c1 - c0)) / wall,
      });
      await page.waitForTimeout(700);
    }
    // Back to the top for the tap test.
    await page.evaluate(() => { window.__findList().scrollTop = 0; });
    await page.waitForTimeout(800);
    await page.evaluate(() => {
      const list = window.__findList(); const lr = list.getBoundingClientRect();
      const b = [...document.querySelectorAll('button[aria-label]')].find((b) => /^\S+ \d+$/.test(b.getAttribute('aria-label')));
      if (b) list.scrollTop += b.getBoundingClientRect().top - lr.top - 300;
    });
    await page.waitForTimeout(800);
    const chip = await page.evaluate(() => {
      const list = window.__findList(); const lr = list.getBoundingClientRect();
      for (const b of document.querySelectorAll('button[aria-label]')) {
        const r = b.getBoundingClientRect();
        if (/^\S+ \d+$/.test(b.getAttribute('aria-label')) && r.top > lr.top && r.bottom < lr.bottom) return { x: r.left + r.width / 2, y: r.top + r.height / 2, label: b.getAttribute('aria-label') };
      }
      return null;
    });
    if (chip) {
      await page.evaluate(() => { window.__b.events = []; });
      for (let i = 0; i < 5; i++) { await page.mouse.click(chip.x, chip.y); await page.waitForTimeout(300); }
      const after = await page.evaluate(({ x, y }) => document.elementFromPoint(x, y)?.closest('button')?.getAttribute('aria-label'), chip);
      const ev = await page.evaluate(() => window.__b.events);
      const before = Number(chip.label.split(' ')[1]); const now = Number((after ?? ' -1').split(' ')[1]);
      result.tap = { before: chip.label, after, counted: now - before, eventMs: ev.filter((e) => e.name === 'click').map((e) => e.duration) };
    }
    const m1 = await metrics(cdp);
    result.end = { jsHeapMB: m1.JSHeapUsedSize / 1e6, rendererRssMB: Math.max(...chromeProcs().filter((p) => /Chrome H/.test(p.comm)).map((p) => p.rss)) / 1024 };
    await page.screenshot({ path: out.replace(/\.json$/, `-${app.name}-${round}.png`) });
  } finally {
    await ctx.close();
    rmSync(dir, { recursive: true, force: true });
  }
  return result;
}

const servers = apps.map((a) => ({ ...a, server: serve(a.root) }));
const results = [];
for (let r = 1; r <= rounds; r++) {
  const order = r % 2 ? servers : [...servers].reverse();
  for (const a of order) {
    const res = await run(a, `http://127.0.0.1:${a.server.port}/`, r);
    results.push(res);
    console.log(JSON.stringify({ app: res.app, round: r, firstRowMs: res.load.firstRowMs?.toFixed(0), heap: res.load.jsHeapMB.toFixed(1), scroll: res.scroll.map((s) => `${s.pxPerS.toFixed(0)}:${s.fps.toFixed(0)}fps late${s.late} blank${s.blankPct.toFixed(1)}% main${s.mainTaskMsPerS.toFixed(0)}`).join(' '), tap: res.tap?.counted }));
    writeFileSync(out, JSON.stringify(results, null, 1));
  }
}
for (const s of servers) s.server.stop(true);
