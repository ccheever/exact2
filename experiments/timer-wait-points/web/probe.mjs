// Measures the browser behavior LLP 1147's web host rests on, in Chromium, Firefox and WebKit.
// Each probe returns plain data; `bun experiments/timer-wait-points/web/probe.mjs [engine …] [--only probe,…]`
// prints a summary and writes `results/<engine>.json`.
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import process from 'node:process';
import { suiteBrowser } from '../../../scripts/agent-launch.mjs';

const PROBES = {
  support,
  scrollThenFrame,
  removedScroller,
  animations,
  idleDuringAnimation,
  offscreenIframe,
  scrolledDocument,
  heldDrag,
  blockedMainThread,
  stillBeforeEnd,
};

async function main() {
  const args = process.argv.slice(2);
  const onlyAt = args.indexOf('--only');
  const only = onlyAt >= 0 ? args[onlyAt + 1].split(',') : Object.keys(PROBES);
  const engines = args.filter((a, i) => !a.startsWith('--') && i !== onlyAt + 1);
  const servers = serve();
  const out = join(import.meta.dirname, 'results');
  mkdirSync(out, { recursive: true });
  for (const engine of engines.length ? engines : ['chromium', 'firefox', 'webkit']) {
    const suite = await suiteBrowser(engine);
    const file = join(out, `${engine}.json`);
    // Merge into the engine's earlier results so a `--only` run keeps the other probes'; each probe keeps the browser
    // version and date it ran with
    const earlier = existsSync(file) ? JSON.parse(readFileSync(file, 'utf8')) : null;
    const version = suite.browser.version(), date = new Date().toISOString();
    const ran = earlier?.ran ?? Object.fromEntries(Object.keys(earlier?.probes ?? {}).map((name) => [name, { version: earlier.version, date: earlier.date }]));
    const result = { engine, version, date, ran, probes: { ...earlier?.probes } };
    for (const name of only) {
      const context = await suite.newContext({ hasTouch: name === 'heldDrag' || name === 'stillBeforeEnd', viewport: { width: 800, height: 600 } });
      const page = await context.newPage();
      try {
        result.probes[name] = await PROBES[name]({ engine, page, context, servers });
      } catch (e) {
        result.probes[name] = { error: String(e.message ?? e).split('\n')[0] };
      } finally {
        await context.close();
      }
      result.ran[name] = { version, date };
      console.log(engine, name, JSON.stringify(result.probes[name]));
    }
    writeFileSync(file, `${JSON.stringify(result, null, 2)}\n`);
    await suite.close();
  }
  servers.stop();
  process.exit(0);
}

/** Whether the APIs the web host would use exist. */
async function support({ page }) {
  await page.setContent('<p>support</p>');
  return page.evaluate(() => ({
    requestIdleCallback: typeof requestIdleCallback === 'function',
    scrollend: 'onscrollend' in window,
    getAnimations: typeof document.getAnimations === 'function',
    scrollTimeline: typeof ScrollTimeline === 'function',
  }));
}

/**
 * Within one rendering update, whether `scroll` is dispatched before animation-frame callbacks, as HTML's "update the
 * rendering" runs the scroll steps before them. A smooth scroll is recorded frame by frame.
 */
async function scrollThenFrame({ page }) {
  await page.setContent(scrollerPage());
  return page.evaluate(() => new Promise((resolve) => {
    const s = document.getElementById('s'), log = [];
    s.addEventListener('scroll', () => log.push(['scroll', performance.now()]));
    s.addEventListener('scrollend', () => log.push(['scrollend', performance.now()]));
    const frame = (t) => { log.push(['frame', performance.now(), t]); if (log.length < 400) requestAnimationFrame(frame); };
    requestAnimationFrame(frame);
    s.scrollTo({ top: 3000, behavior: 'smooth' });
    setTimeout(() => {
      // A scroll dispatched in a rendering update's scroll steps comes just before that update's frame callbacks.
      const gaps = [];
      for (let i = 0; i < log.length; i++) {
        if (log[i][0] !== 'scroll') continue;
        const next = log.find((e, j) => j > i && e[0] === 'frame');
        if (next) gaps.push(next[1] - log[i][1]);
      }
      const s = gaps.sort((x, y) => x - y);
      resolve({ scrolls: gaps.length, scrollToNextFrameMs: s.length ? { min: s[0], p50: s[s.length >> 1], max: s.at(-1) } : null, scrollEnded: log.some((e) => e[0] === 'scrollend') });
    }, 1500);
  }));
}

/** Whether `scrollend` comes for a scroller removed while it scrolls (the design drops it from the moving set). */
async function removedScroller({ page }) {
  await page.setContent(scrollerPage());
  return page.evaluate(() => new Promise((resolve) => {
    const s = document.getElementById('s'), log = [];
    document.addEventListener('scroll', (e) => log.push(['scroll', e.target.id]), true);
    document.addEventListener('scrollend', (e) => log.push(['scrollend', e.target.id ?? 'document']), true);
    s.addEventListener('scroll', () => { if (s.isConnected) s.remove(); }, { once: true });
    s.scrollTo({ top: 3000, behavior: 'smooth' });
    setTimeout(() => resolve({ events: log, connected: s.isConnected }), 1500);
  }));
}

/**
 * What `getAnimations()` reports for running, rate-0 and scroll-driven animations, and how an animation's end is
 * announced: CSS `animationend`/`animationcancel` reach a capturing document listener, an `Element.animate()` one's
 * `finish`/`cancel` stay on its `Animation`, and `finished` settles for both (rejecting on cancel).
 */
async function animations({ page }) {
  await page.setContent(`<style>
    @keyframes slide { from { left: 0 } to { left: 100px } }
    #css { position: absolute; animation: slide 300ms linear }
    #still { position: absolute; animation: slide 10s linear }
    #scroller { height: 100px; overflow: scroll } #scroller > div { height: 2000px }
    #driven { animation: slide linear; animation-timeline: scroll(nearest) }
  </style><div id=css>a</div><div id=still>b</div><div id=scroller><div><div id=driven>c</div></div></div><div id=waapi>d</div>`);
  return page.evaluate(async () => {
    const docEvents = [];
    for (const type of ['animationend', 'animationcancel', 'transitionend', 'transitioncancel']) {
      document.addEventListener(type, (e) => docEvents.push([type, e.target.id]), true);
    }
    const still = document.getElementById('still').getAnimations()[0];
    still.updatePlaybackRate(0);
    await still.ready;
    const driven = document.getElementById('driven').getAnimations()[0];
    const waapi = document.getElementById('waapi').animate([{ opacity: 0 }, { opacity: 1 }], 200);
    const waapiEvents = [];
    waapi.addEventListener('finish', () => waapiEvents.push('finish'));
    const cancelled = document.getElementById('waapi').animate([{ opacity: 0 }, { opacity: 1 }], 5000);
    cancelled.addEventListener('cancel', () => waapiEvents.push('cancel'));
    const cancelledSettled = Promise.allSettled([cancelled.finished]);
    cancelled.cancel();
    const css = document.getElementById('css').getAnimations()[0];
    const [cssSettled, waapiSettled] = await Promise.all([Promise.allSettled([css.finished]), Promise.allSettled([waapi.finished])]);
    await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
    const timeline = (a) => a?.timeline?.constructor?.name ?? null;
    return {
      rateZero: { playState: still.playState, playbackRate: still.playbackRate, timeline: timeline(still) },
      scrollDriven: driven ? { playState: driven.playState, timeline: timeline(driven) } : 'unsupported',
      settled: { css: cssSettled[0].status, waapi: waapiSettled[0].status, cancelled: (await cancelledSettled)[0].status },
      documentEvents: docEvents,
      animationEvents: waapiEvents,
      runningAfter: document.getAnimations().filter((a) => a.playState === 'running').map((a) => a.constructor.name),
    };
  });
}

/** Whether `requestIdleCallback` runs between frames while a main-thread animation runs, and with what budget. */
async function idleDuringAnimation({ page }) {
  await page.setContent('<div id=box style="position:absolute;width:10px;height:10px;background:red"></div>');
  return page.evaluate(() => new Promise((resolve) => {
    if (typeof requestIdleCallback !== 'function') { resolve('unsupported'); return; }
    const box = document.getElementById('box'), budgets = [];
    let frames = 0;
    const start = performance.now();
    const frame = (t) => {
      frames++;
      box.style.left = `${(t - start) / 10}px`;
      if (performance.now() - start < 1000) requestAnimationFrame(frame);
      else resolve({ frames, idleCallbacks: budgets.length, budgetsMs: quantiles(budgets) });
    };
    const idle = (d) => { budgets.push(d.timeRemaining()); if (performance.now() - start < 1000) requestIdleCallback(idle); };
    const quantiles = (xs) => { const s = [...xs].sort((a, b) => a - b); return s.length ? { min: s[0], p50: s[s.length >> 1], max: s.at(-1) } : null; };
    requestAnimationFrame(frame);
    requestIdleCallback(idle);
  }));
}

/**
 * A cross-origin iframe scrolled out of view: how often its animation frames and 100 ms timeouts run, and whether an
 * IntersectionObserver in it says so. On screen first, then off screen, each for two seconds.
 */
async function offscreenIframe({ page, servers }) {
  await page.goto(`${servers.parent}/parent?child=${encodeURIComponent(`${servers.child}/child`)}`);
  const frame = await waitForChildFrame(page);
  const measure = () => frame.evaluate(() => window.measure(2000));
  const onscreen = await measure();
  // The parent scrolled 100 px: the frame's top 100 px are clipped, its other 200 px still on screen
  await page.evaluate(() => window.scrollTo(0, 100));
  const partly = await measure();
  await page.evaluate(() => window.scrollTo(0, 5000));
  const offscreen = await measure();
  await page.evaluate(() => window.scrollTo(0, 0));
  const back = await measure();
  // The child's own document scrolled past its first screen while the frame stays on screen
  await frame.evaluate(() => window.scrollTo(0, 2000));
  const scrolledInside = await measure();
  return { onscreen, partly, offscreen, back, scrolledInside };
}

/**
 * A touch drag that holds still mid-gesture: `scrollend` should wait for the finger to lift, so 100 ms without `scroll`
 * is not an end while a touch is down. Chromium only (CDP touch input).
 */
async function heldDrag({ engine, page }) {
  if (engine !== 'chromium') return 'chromium only';
  await page.setContent(scrollerPage());
  await page.evaluate(() => {
    window.log = [];
    const s = document.getElementById('s');
    for (const type of ['scroll', 'scrollend', 'touchstart', 'touchend']) {
      s.addEventListener(type, () => window.log.push([type, performance.now()]), { passive: true });
    }
  });
  const cdp = await page.context().newCDPSession(page);
  const touch = (type, y) => cdp.send('Input.dispatchTouchEvent', { type, touchPoints: type === 'touchEnd' ? [] : [{ x: 200, y }] });
  await touch('touchStart', 500);
  for (let y = 490; y >= 300; y -= 10) { await touch('touchMove', y); await page.waitForTimeout(16); }
  const heldAt = await page.evaluate(() => performance.now());
  await page.waitForTimeout(600);
  const liftAt = await page.evaluate(() => performance.now());
  await touch('touchEnd');
  await page.waitForTimeout(800);
  const log = await page.evaluate(() => window.log);
  const ends = log.filter((e) => e[0] === 'scrollend').map((e) => e[1]);
  return {
    scrolls: log.filter((e) => e[0] === 'scroll').length,
    lastScrollBeforeHoldMs: Math.max(...log.filter((e) => e[0] === 'scroll' && e[1] <= liftAt).map((e) => e[1])) - heldAt,
    holdMs: liftAt - heldAt,
    scrollendWhileHeld: ends.filter((t) => t < liftAt).length,
    scrollendAfterLiftMs: ends.filter((t) => t >= liftAt).map((t) => t - liftAt),
  };
}

/**
 * A main thread blocked for 1.2 s while a scroll runs: whether the scroll moved meanwhile (the compositor scrolled on),
 * and how late an animation frame asked for before the block comes. Programmatic smooth scroll and a wheel fling.
 */
async function blockedMainThread({ page }) {
  await page.setContent(scrollerPage());
  const run = (kind) => page.evaluate((kind) => new Promise((resolve) => {
    const s = document.getElementById('s');
    s.scrollTop = 0;
    requestAnimationFrame(() => requestAnimationFrame(() => {
      const r = { kind };
      s.addEventListener('scroll', () => {
        r.topAtBlock = s.scrollTop;
        const asked = performance.now();
        requestAnimationFrame(() => {
          r.frameLateMs = performance.now() - asked;
          r.topAtFirstFrame = s.scrollTop;
        });
        const until = performance.now() + 1200;
        while (performance.now() < until) { /* block */ }
        r.blockEnd = performance.now();
      }, { once: true });
      s.addEventListener('scrollend', () => { r.scrollendAfterBlockMs = performance.now() - r.blockEnd; r.finalTop = s.scrollTop; resolve(r); }, { once: true });
      if (kind === 'smooth') s.scrollTo({ top: 3000, behavior: 'smooth' });
      else window.wheelReady = true;
      setTimeout(() => resolve({ ...r, timedOut: true }), 5000);
    }));
  }), kind);
  const smooth = await run('smooth');
  const wheelResult = run('wheel');
  await page.waitForFunction(() => window.wheelReady);
  await page.mouse.move(200, 300);
  for (let i = 0; i < 5; i++) await page.mouse.wheel(0, 400);
  return { smooth, wheel: await wheelResult };
}

function scrollerPage() {
  return `<style>body{margin:0}#s{height:600px;overflow:scroll}#s>div{height:20000px;background:linear-gradient(#fff,#888)}</style>
    <div id=s><div></div></div>`;
}

/**
 * Whether a scroll that is still moving ever keeps one position for 100 ms of animation frames, which the design's
 * backstop would take for its end, and how a smooth scroll that an instant one to the same position replaces ends.
 * Each frame records the scroller's position from `requestAnimationFrame`; the longest still run counts frames after
 * the scroll's first `scroll` (and, for a touch, after the finger lifts) and before its `scrollend`.
 */
async function stillBeforeEnd({ engine, page }) {
  await page.setContent(scrollerPage());
  await page.evaluate(() => {
    const s = document.getElementById('s');
    window.watch = () => {
      const log = { frames: [], scrolls: [], ends: [] };
      s.addEventListener('scroll', () => log.scrolls.push(performance.now()));
      s.addEventListener('scrollend', () => log.ends.push(performance.now()));
      const frame = () => { log.frames.push([performance.now(), s.scrollTop]); if (log.frames.length < 600) requestAnimationFrame(frame); };
      requestAnimationFrame(frame);
      return log;
    };
    // Resets the scroller and waits for that jump's own `scroll` and `scrollend` to pass
    window.reset = () => new Promise((resolve) => { s.scrollTop = 0; setTimeout(resolve, 300); });
    // A still run that a later change ends was a pause mid-scroll; the run after the last change is the scroll at rest
    window.still = (log, from) => {
      const end = log.ends.find((t) => t >= from) ?? Infinity;
      const frames = log.frames.filter(([t]) => t >= from && t <= end);
      let midScroll = 0, since = null, lastChange = null;
      for (let i = 1; i < frames.length; i++) {
        if (frames[i][1] === frames[i - 1][1]) { since ??= frames[i - 1][0]; } else { if (since != null) { midScroll = Math.max(midScroll, frames[i - 1][0] - since); } since = null; lastChange = frames[i][0]; }
      }
      const round = (ms) => Math.round(ms * 10) / 10;
      return { midScrollStillMs: round(midScroll), lastChangeToScrollendMs: lastChange != null && end !== Infinity ? round(end - lastChange) : null, ended: end !== Infinity, frames: frames.length };
    };
  });
  const runs = {};
  for (let i = 0; i < 10; i++) {
    runs[`smooth${i}`] = await page.evaluate(async () => {
      await window.reset();
      const s = document.getElementById('s'), log = window.watch(), from = performance.now();
      s.scrollTo({ top: 3000, behavior: 'smooth' });
      await new Promise((resolve) => setTimeout(resolve, 2500));
      // The scroller joins the moving set at its first `scroll`; frames before it are the scroll starting
      return window.still(log, log.scrolls.find((t) => t >= from) ?? from);
    });
  }
  for (let i = 0; i < 3; i++) {
    await page.evaluate(async () => { await window.reset(); window.current = window.watch(); });
    await page.mouse.move(200, 300);
    for (let k = 0; k < 6; k++) { await page.mouse.wheel(0, 120); await page.waitForTimeout(16); }
    // The wheel holds its scroller until 100 ms after its last event, so the backstop counts from there
    const lastWheel = await page.evaluate(() => performance.now());
    await page.waitForTimeout(2000);
    runs[`wheel${i}`] = await page.evaluate((last) => ({ ...window.still(window.current, last + 100), lastWheelToScrollendMs: Math.round(((window.current.ends.find((t) => t >= last) ?? NaN) - last) * 10) / 10 }), lastWheel);
  }
  if (engine === 'chromium') {
    const cdp = await page.context().newCDPSession(page);
    const touch = (type, y) => cdp.send('Input.dispatchTouchEvent', { type, touchPoints: type === 'touchEnd' ? [] : [{ x: 200, y }] });
    for (let i = 0; i < 3; i++) {
      await page.evaluate(async () => { await window.reset(); window.current = window.watch(); });
      await touch('touchStart', 550);
      for (let y = 520; y >= 250; y -= 45) { await touch('touchMove', y); await page.waitForTimeout(8); }
      const liftAt = await page.evaluate(() => performance.now());
      await touch('touchEnd');
      await page.waitForTimeout(3000);
      runs[`fling${i}`] = await page.evaluate((from) => window.still(window.current, from), liftAt);
    }
  }
  await page.evaluate(() => window.reset());
  const replaced = await page.evaluate(() => new Promise((resolve) => {
    const s = document.getElementById('s');
    const log = window.watch();
    s.addEventListener('scroll', () => {
      const at = s.scrollTop, replacedAt = performance.now();
      s.scrollTo({ top: at, behavior: 'instant' });
      setTimeout(() => {
        const after = log.frames.filter(([t]) => t >= replacedAt);
        const moved = after.some(([, y]) => y !== at);
        const stillFor = after.length ? after.at(-1)[0] - after[0][0] : 0;
        resolve({ at, moved, scrollends: log.ends.filter((t) => t >= replacedAt).length, stillFramesMs: Math.round(stillFor) });
      }, 1500);
    }, { once: true });
    s.scrollTo({ top: 3000, behavior: 'smooth' });
  }));
  return { ...summarize(runs), runs, replaced };
}

function summarize(runs) {
  const values = Object.values(runs);
  return { maxMidScrollStillMs: Math.max(...values.map((r) => r.midScrollStillMs)), maxLastChangeToScrollendMs: Math.max(...values.map((r) => r.lastChangeToScrollendMs ?? 0)) };
}

/**
 * A top-level page laid out as Exact's `scroll document` (`html` at 100% height, the body growing with its content),
 * scrolled past its first screen: what an `IntersectionObserver` reports for the root element, whose box then lies
 * above the viewport, and for a 1 px viewport-fixed element, while the page still renders.
 */
async function scrolledDocument({ page }) {
  await page.setContent(`<style>html, body { margin: 0; height: 100%; } body { height: auto; min-height: 100%; }</style>
    <div id="pin" style="position:fixed;left:0;top:0;width:1px;height:1px"></div>
    <div id="cover" style="position:fixed;inset:0;pointer-events:none"></div><div style="height:5000px"></div>`);
  const read = () => page.evaluate(() => new Promise((resolve) => {
    const seen = {};
    const observe = (name, target) => new IntersectionObserver((es, o) => { seen[name] = es.at(-1).isIntersecting; o.disconnect(); }).observe(target);
    observe('root', document.documentElement);
    observe('pin', document.getElementById('pin'));
    observe('cover', document.getElementById('cover'));
    let frames = 0; const count = () => { frames++; if (frames < 30) requestAnimationFrame(count); };
    requestAnimationFrame(count);
    setTimeout(() => resolve({ scrollY: window.scrollY, rootIntersecting: seen.root, pinIntersecting: seen.pin, coverIntersecting: seen.cover, frames, visibility: document.visibilityState }), 700);
  }));
  const top = await read();
  await page.evaluate(() => window.scrollTo(0, 2000));
  const scrolled = await read();
  return { top, scrolled };
}

async function waitForChildFrame(page) {
  for (let i = 0; i < 100; i++) {
    const frame = page.frames().find((f) => f.url().endsWith('/child'));
    if (frame && await frame.evaluate(() => typeof window.measure === 'function').catch(() => false)) return frame;
    await page.waitForTimeout(50);
  }
  throw new Error('the child frame did not load');
}

/** Two origins on loopback: `127.0.0.1` and `localhost` differ, so the child frame is cross-origin. */
function serve() {
  const html = (body) => new Response(body, { headers: { 'content-type': 'text/html' } });
  const child = Bun.serve({ port: 0, hostname: 'localhost', fetch: () => html(`<style>html, body { margin: 0; height: 100%; } body { height: auto; min-height: 100%; }</style>
    <div id="pin" style="position:fixed;left:0;top:0;width:1px;height:1px"></div>
    <div id="cover" style="position:fixed;inset:0;pointer-events:none"></div><div style="height:5000px"></div><script>
    let visible = null, pinVisible = null, coverVisible = null;
    new IntersectionObserver((es) => { visible = es.at(-1).isIntersecting; }).observe(document.documentElement);
    new IntersectionObserver((es) => { pinVisible = es.at(-1).isIntersecting; }).observe(document.getElementById('pin'));
    new IntersectionObserver((es) => { coverVisible = es.at(-1).isIntersecting; }).observe(document.getElementById('cover'));
    window.measure = (ms) => new Promise((resolve) => {
      const start = performance.now(); let frames = 0; const delays = [];
      const frame = () => { frames++; if (performance.now() - start < ms) requestAnimationFrame(frame); };
      requestAnimationFrame(frame);
      let asked = performance.now();
      const tick = () => { delays.push(performance.now() - asked); asked = performance.now(); if (performance.now() - start < ms) setTimeout(tick, 100); };
      setTimeout(tick, 100);
      setTimeout(() => {
        const s = [...delays].sort((a, b) => a - b);
        resolve({ frames, timeouts: delays.length, timeoutDelayMs: { p50: s[s.length >> 1], max: s.at(-1) }, intersecting: visible, pinIntersecting: pinVisible, coverIntersecting: coverVisible, visibility: document.visibilityState });
      }, ms + 50);
    });
  </script><body style="margin:0;background:#cfc">child</body>`) });
  const parent = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch: (req) => html(`<body style="margin:0">
    <iframe src="${new URL(req.url).searchParams.get('child')}" style="width:400px;height:300px;border:0"></iframe>
    <div style="height:20000px"></div></body>`) });
  return { parent: `http://127.0.0.1:${parent.port}`, child: `http://localhost:${child.port}`, stop: () => { parent.stop(); child.stop(); } };
}

if (import.meta.main) {
  await main();
}
