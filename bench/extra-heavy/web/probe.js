// xheavy web probe — the heavy-list probe's scenarios in a page (the web pair). server.py injects
// it as the first <script> of each build's index.html. URL params: scenario (fling | ladder | rest | jump |
// coldstart), sample (N: blank check every Nth frame; jump/coldstart check every frame), out (result file
// name), delay (s after load, default 8; coldstart ignores it). It finds the feed (the element with the largest
// scrollHeight > 20,000 and clientHeight >= 300), drives its scrollTop from requestAnimationFrame, and POSTs one
// JSON in the iOS probe's shape to /__result. CPU and RSS come from the server (/__mark), which sums `ps` over
// the Chrome process tree the runner launched, split by Chrome process type.
(() => {
  const q = new URLSearchParams(location.search);
  const scenario = q.get('scenario');
  if (!scenario) return;
  const sample = Number(q.get('sample') || 0);
  const out = q.get('out') || 'result.json';
  const delay = Number(q.get('delay') || 8);
  const now = () => performance.now();

  // Long animation frames (Chrome 123+) and long tasks, accumulated; per segment by difference.
  const acc = { loafMs: 0, loafBlockingMs: 0, loafs: 0, longTaskMs: 0 };
  try {
    new PerformanceObserver((l) => { for (const e of l.getEntries()) { acc.loafMs += e.duration; acc.loafBlockingMs += e.blockingDuration || 0; acc.loafs++; } })
      .observe({ type: 'long-animation-frame', buffered: true });
  } catch {}
  try { new PerformanceObserver((l) => { for (const e of l.getEntries()) acc.longTaskMs += e.duration; }).observe({ type: 'longtask', buffered: true }); } catch {}

  const errors = [];
  const consoleLines = [];
  for (const k of ['log', 'warn', 'error']) { const f = console[k].bind(console); console[k] = (...a) => { if (consoleLines.length < 40) consoleLines.push(k + ': ' + a.map(String).join(' ').slice(0, 400)); f(...a); }; }
  addEventListener('error', (e) => { if (errors.length < 4) errors.push(String(e.error?.stack || e.message).slice(0, 1500)); });
  addEventListener('unhandledrejection', (e) => { if (errors.length < 4) errors.push('rejection: ' + String(e.reason?.stack || e.reason).slice(0, 1500)); });
  const heap = () => (performance.memory ? performance.memory.usedJSHeapSize : 0);
  const mark = (tag) => fetch('/__mark?tag=' + encodeURIComponent(tag), { method: 'POST' }).then((r) => r.json()).catch(() => null);

  function findScroll() {
    let best = null;
    for (const el of document.querySelectorAll('*')) {
      if (el.clientHeight < 300 || el.scrollHeight <= 20000) continue;
      const oy = getComputedStyle(el).overflowY;
      if (oy !== 'auto' && oy !== 'scroll' && el !== document.scrollingElement) continue;
      if (!best || el.scrollHeight > best.scrollHeight) best = el;
    }
    return best;
  }

  // Blank bands: sample elementFromPoint down the content column's centre (x = the feed's centre, every 10 px);
  // a point that hits the feed itself, a content container (> 3,000 px tall: taller than any row), or an ancestor
  // of the feed shows no row. Runs of
  // such points >= 60 px are blank, as the iOS probe's uniform bands are. This forces a style/layout flush, so
  // it runs only when sampling. It sees missing rows, not rows whose pixels are not painted yet.
  function blankPoints(s) {
    const r = s.getBoundingClientRect();
    const x = r.left + r.width / 2, top = Math.max(r.top, 0), bottom = Math.min(r.bottom, innerHeight);
    let blank = 0, run = 0;
    for (let y = top + 1; y <= bottom; y += 10) {
      const hit = y < bottom ? document.elementFromPoint(x, y) : s;
      // The feed's own structure (the scroller, its content containers) spans the whole content, taller than
      // any row; a hit on it, or on an ancestor of the feed, is a place no row covers.
      const empty = y < bottom && (!hit || hit === s || hit.contains(s) || (s.contains(hit) && hit.getBoundingClientRect().height > 3000));
      if (empty) run += 10;
      else { if (run >= 60) blank += run; run = 0; }
    }
    return { blank };
  }

  const res = {
    scenario, sample, platform: 'web', bundle: location.port === '8811' ? 'exact2-web' : 'expo-web', userAgent: navigator.userAgent,
    crossOriginIsolated: self.crossOriginIsolated, live: false,
    dts: [], expected: [], busyFrameMs: [], blank: [], jumps: [], segments: [], segStats: [], setCost: [],
  };
  let scroll = null, period = 1 / 60;
  const segs = [];

  function buildSegments() {
    if (scenario === 'rest') { segs.push({ v: 0, dir: 1, rest: 1, dur: 10 }); return; }
    segs.push({ v: 3000, dir: 1, warm: 1 }, { v: 3000, dir: -1, warm: 1 });
    const speeds = scenario === 'ladder' ? [3000, 6000, 12000, 24000, 48000, 96000] : [1000, 3000, 6000, 12000, 24000];
    for (const v of speeds) segs.push({ v, dir: 1 }, { v, dir: -1 });
  }

  const snap = async (tag) => ({ t: now(), heap: heap(), ...acc, ps: await mark(tag) });
  const psDelta = (a, b, sec) => {
    if (!a?.ps || !b?.ps) return {};
    const d = {};
    for (const k of Object.keys(b.ps.cpu)) d[k + 'CpuMsPerSec'] = ((b.ps.cpu[k] - (a.ps.cpu[k] || 0)) * 1000) / sec;
    return d;
  };
  function delta(a, b, frames) {
    const sec = (b.t - a.t) / 1000;
    const d = { sec, frames, loafMs: b.loafMs - a.loafMs, loafBlockingMs: b.loafBlockingMs - a.loafBlockingMs, longTaskMs: b.longTaskMs - a.longTaskMs,
      busyMsPerFrame: null, ...psDelta(a, b, sec) };
    // cpuMsPerSec: the whole Chrome tree (renderer + GPU + browser + utility); mainCpuMsPerSec: the renderer
    // process (its main thread is not separable from its compositor and raster threads with ps).
    d.cpuMsPerSec = d.totalCpuMsPerSec ?? null; d.mainCpuMsPerSec = d.rendererCpuMsPerSec ?? null;
    return d;
  }

  let peakHeap = 0, peakRss = 0, rssEnd = 0;
  // The page's footprint analog: RSS of the renderer process(es) plus the GPU process (one tab, so the GPU
  // process works for this page alone). The browser and utility processes are Chrome's own and are left out
  // (still in rssByType). RSS counts shared pages in each process, so it overstates, equally for both apps.
  const trackRss = (s) => { if (s?.ps) { const v = s.ps.rss.renderer + s.ps.rss.gpu; peakRss = Math.max(peakRss, v); rssEnd = v; } };

  async function finish(error) {
    const end = await snap('end'); trackRss(end);
    res.run = delta(res._runSnap || end, end, res._ticks || 0);
    peakHeap = Math.max(peakHeap, heap());
    let uasm = null;
    if (self.crossOriginIsolated && performance.measureUserAgentSpecificMemory) {
      try { uasm = (await Promise.race([performance.measureUserAgentSpecificMemory(), new Promise((r) => setTimeout(() => r(null), 20000))]))?.bytes ?? null; } catch (e) { uasm = 'error: ' + e.message; }
    }
    // mem: renderer + GPU process RSS (ps, bytes) as peak/end (the probe's footprint columns), sampled at each
    // segment boundary; the JS heap and the browser's own per-page measurement (uaMemory) beside them.
    try { const a = navigator.gpu && await navigator.gpu.requestAdapter(); res.webgpu = a ? (a.info ? { vendor: a.info.vendor, architecture: a.info.architecture } : true) : (navigator.gpu ? 'no adapter' : 'no navigator.gpu'); } catch (e) { res.webgpu = 'error ' + e.message; }
    res.mem = { peak: peakRss, end: rssEnd, jsHeapPeak: peakHeap, jsHeapEnd: heap(), uaMemory: uasm, rssByType: end?.ps?.rss ?? null };
    if (error) res.error = error;
    if (errors.length) res.pageErrors = errors;
    if (consoleLines.length) res.console = consoleLines;
    res.maxFps = Math.round(1 / period);
    res.contentHeight = scroll ? scroll.scrollHeight : 0;
    res.viewport = scroll ? [scroll.clientWidth, scroll.clientHeight] : null;
    delete res._runSnap; delete res._ticks;
    await fetch('/__result?name=' + encodeURIComponent(out), { method: 'POST', body: JSON.stringify(res) });
    document.title = 'probe done';
  }

  // coldstart: from the navigation's start (performance.timeOrigin) to the first frame where the feed exists
  // and no blank band shows. Chrome's own process start is not in it.
  if (scenario === 'coldstart') {
    res.timeline = [];
    const tick = () => {
      const t = now();
      if (t > 30000) { finish('coldstart: no blank-free list frame within 30 s'); return; }
      scroll = scroll || findScroll();
      const b = scroll ? blankPoints(scroll) : { blank: -1 };
      res.timeline.push([t, !!scroll, b.blank]);
      if (scroll && b.blank === 0) { res.coldstart = { ms: t, frames: res.timeline.length, source: 'navigationStart' }; res._ticks = res.timeline.length; finish(); return; }
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
    return;
  }

  async function start() {
    for (let i = 0; i < 200 && !(scroll = findScroll()); i++) await new Promise((r) => setTimeout(r, 100));
    if (!scroll) {
      const tall = [...document.querySelectorAll('*')].map((e) => [e.tagName + '.' + (e.className?.baseVal ?? e.className) + '#' + e.id, e.scrollHeight, e.clientHeight, getComputedStyle(e).overflowY])
        .sort((a, b) => b[1] - a[1]).slice(0, 8);
      res.diag = { tall, text: (document.body?.innerText || '').slice(0, 300), size: [innerWidth, innerHeight], nodes: document.querySelectorAll('*').length };
      finish('no scroll container with scrollHeight > 20000 found'); return;
    }
    // The display's frame interval: the median rAF delta over one idle second.
    const ds = []; let last = 0;
    await new Promise((resolve) => { const f = (t) => { if (last) ds.push(t - last); last = t; if (ds.length < 60) requestAnimationFrame(f); else resolve(); }; requestAnimationFrame(f); });
    ds.sort((a, b) => a - b); period = ds[ds.length >> 1] / 1000;
    res.periodMs = period * 1000;
    buildSegments();
    res.segments = segs;
    const maxY = () => scroll.scrollHeight - scroll.clientHeight;
    if (scenario !== 'rest' && scenario !== 'jump') scroll.scrollTop = maxY() / 3;
    await new Promise((r) => setTimeout(r, 1000));
    res._runSnap = await snap('run'); trackRss(res._runSnap);
    let lastT = 0, seg = -1, segStart = 0, segSnap = null, segFrames = 0, travel = 0, frame = 0, ticks = 0, closing = false;
    let jumpIndex = -1, jumpAt = 0, cleanRun = 0, lastCleanAt = 0;
    const loop = async (t) => {
      ticks++; res._ticks = ticks;
      if (lastT) { res.dts.push((t - lastT) / 1000); res.expected.push(period); res.busyFrameMs.push(null); }
      const dt = lastT ? (t - lastT) / 1000 : 0; lastT = t;
      peakHeap = Math.max(peakHeap, heap());
      if (scenario === 'jump') {
        if (jumpIndex < 0 || cleanRun >= 3 || t - jumpAt > 3000) {
          if (jumpIndex >= 0) res.jumps.push({ target: scroll.scrollTop, ms: cleanRun >= 3 ? lastCleanAt - jumpAt : null });
          jumpIndex++;
          if (jumpIndex >= 10) { finish(); return; }
          scroll.scrollTop = ((0.137 + jumpIndex * 0.618) % 1) * maxY();
          jumpAt = now(); cleanRun = 0;
          requestAnimationFrame(loop); return;
        }
        const b = blankPoints(scroll).blank; res.blank.push(b);
        if (b === 0) { if (cleanRun === 0) lastCleanAt = now(); cleanRun++; } else cleanRun = 0;
        requestAnimationFrame(loop); return;
      }
      const cur = segs[seg];
      const dur = cur ? (cur.dur || 2) : 0;
      if (seg < 0 || (t - segStart) / 1000 >= dur) {
        if (seg >= 0) { const s1 = await snap('seg' + seg); trackRss(s1); res.segStats.push({ ...delta(segSnap, s1, segFrames), travelPt: travel }); }
        seg++;
        if (seg >= segs.length) { finish(); return; }
        segStart = t; segSnap = await snap('seg' + seg + 'start'); segFrames = 0; travel = 0; lastT = 0;
        res.dts.push(-1); res.expected.push(-1); res.busyFrameMs.push(-1);
        requestAnimationFrame(loop); return;
      }
      const s = segs[seg];
      segFrames++; frame++;
      if (!s.rest) {
        const y0 = scroll.scrollTop, c0 = now();
        scroll.scrollTop = Math.max(0, Math.min(maxY(), y0 + s.dir * s.v * dt));
        res.setCost.push(now() - c0);
        travel += Math.abs(scroll.scrollTop - y0);
      }
      if (sample > 0) res.blank.push(frame % sample === 0 ? blankPoints(scroll).blank : -1);
      requestAnimationFrame(loop);
    };
    requestAnimationFrame(loop);
  }
  addEventListener('load', () => setTimeout(start, delay * 1000));
})();
