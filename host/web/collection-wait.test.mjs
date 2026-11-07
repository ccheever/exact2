// LLP 1070.000 §2.6 in the web glue: what it tells the runner about the reader's scroll (flag bit 5) and about a
// gesture begun since the list's `scrollIntoView` (bit 6). Real DOM and events; the frame clock and the quiet
// timer are the test's, fired by hand.
import { beforeAll, afterAll, test as bunTest, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { cdpBrowser, chromium } from '../../scripts/agent-launch.mjs';
import { need } from '../../scripts/coverage.mjs';

const { unavailable } = chromium();
const test = need('chrome', unavailable, { test: bunTest });
let server, suite, evaluate, call;
beforeAll(async () => {
  if (unavailable) return;
  server = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch(req) {
    const path = new URL(req.url).pathname;
    return path === '/collection-glue.js'
      ? new Response(readFileSync(new URL('./collection-glue.js', import.meta.url)), { headers: { 'content-type': 'text/javascript' } })
      : new Response('<!doctype html><body><main id="root"></main>', { headers: { 'content-type': 'text/html' } });
  }});
  suite = await cdpBrowser();
  const { sessionId } = await suite.page();
  call = (method, params) => suite.cdp.send(method, params, sessionId);
  evaluate = async expression => {
    const result = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
    if (result.exceptionDetails) throw Error(result.exceptionDetails.exception?.description ?? result.exceptionDetails.text);
    return result.result.value;
  };
  await call('Page.navigate', { url: `http://127.0.0.1:${server.port}` });
  await evaluate(`import('/collection-glue.js').then(c => { globalThis.createController = c.collectionController; })`);
}, 60_000); // hang bounds: Chrome's launch and exit are awaited as events, and take longer than 5 s under load
afterAll(async () => {
  await suite?.close();
  server?.stop(true);
}, 60_000);

// A list that is its own port, two rows mounted. `report()` runs the frames owed and returns the last report's
// flags; `timers` holds the glue's timers with their due times, and `fire(ms)` moves the test's clock on by `ms` and
// runs the ones that come due, in order.
function fixture() {
  globalThis.wf?.controller.dispose();
  const root = document.getElementById('root');
  root.innerHTML = `<div data-view="1" style="height:180px;width:320px;overflow:auto"><div data-view="2" style="height:40px"></div><div data-view="4" style="height:60px"></div><div style="height:5000px"></div></div>`;
  const views = new Map([...root.querySelectorAll('[data-view]')].map(el => [+el.dataset.view, el]));
  const list = views.get(1), frames = [], timers = new Map(), seen = [];
  let timer = 0, clock = 0;
  const controller = createController({ root, views,
    requestFrame(fn) { frames.push(fn); return frames.length; }, cancelFrame() {},
    setTimer(fn, ms) { timers.set(++timer, { fn, at: clock + ms }); return timer; }, clearTimer(id) { timers.delete(id); }, now: () => clock,
    report(bytes) {
      const d = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength), flags = d.getUint32(92, true);
      seen.push({ top: d.getFloat64(24, true), scrolling: !!(flags & 32), gesture: !!(flags & 64), animating: !!(flags & 128) });
    } });
  const snapshot = (revision, extra = {}) => ({ view: 1, revision, scrollSequence: '0', totalExtent: 5100, count: 100,
    rows: [{ view: 2, root: 2, index: 0, start: 0, size: 40, epoch: '1' }, { view: 4, root: 4, index: 1, start: 40, size: 60, epoch: '1' }], correction: null, ...extra });
  const report = () => { for (let n = 0; n < 4 && frames.length; n++) frames.splice(0).forEach(fn => fn()); return seen.at(-1); };
  const fire = ms => {
    const until = clock + ms;
    for (let n = 0; ; n++) {
      const next = [...timers].filter(([, t]) => t.at <= until).sort((a, b) => a[1].at - b[1].at)[0];
      if (!next) break;
      if (n > 1000) throw Error('the timers never settle');
      clock = Math.max(clock, next[1].at); timers.delete(next[0]); next[1].fn();
    }
    clock = until;
  };
  // The reader's own scroll: the port moves and its event fires.
  const scroll = top => { list.scrollTop = top; list.dispatchEvent(new Event('scroll')); };
  // `n` fingers on the port afterwards: a touchstart adds finger n - 1, a touchend lifts finger n.
  const finger = (i, target = list) => new Touch({ identifier: i, target, clientX: 10, clientY: 10 });
  const touch = (type, n, target = list) => target.dispatchEvent(new TouchEvent(type, { bubbles: true,
    touches: Array.from({ length: n }, (_, i) => finger(i)), changedTouches: [finger(type === 'touchstart' ? n - 1 : n, target)] }));
  controller.commit([snapshot('1')]);
  report();
  // An input in a later task than the commit before it, as a reader's is: the page's clock counts in steps, and an
  // input stamped with the request's own time is not after it
  const later = () => new Promise(r => setTimeout(r, 2));
  globalThis.wf = { controller, list, seen, snapshot, report, fire, timers, scroll, touch, later };
  return wf;
}

// Two lists, 1 and 3, in one scrollport, `#port`, one row mounted in each. `seen` holds each list's last scrolling
// flag (bit 5), `gestures` its last gesture flag (bit 6), `animating` its last animating flag (bit 7); `timers` the
// glue's pending timers, run on the real clock; `run()` runs the frames owed.
function sharedPort() {
  globalThis.wf?.controller.dispose();
  const root = document.getElementById('root');
  root.innerHTML = '<div id="port" style="height:180px;width:320px;overflow:auto"><div data-view="1"><div data-view="2" style="height:40px"></div></div><div data-view="3"><div data-view="4" style="height:40px"></div></div><div style="height:5000px"></div></div>';
  const views = new Map([...root.querySelectorAll('[data-view]')].map(el => [+el.dataset.view, el]));
  const port = document.getElementById('port'), frames = [], seen = new Map(), gestures = new Map(), animating = new Map();
  const timers = new Set();
  const controller = createController({ root, views, requestFrame(fn) { frames.push(fn); return frames.length; }, cancelFrame() {},
    setTimer(fn, ms) { const id = setTimeout(() => { timers.delete(id); fn(); }, ms); timers.add(id); return id; },
    clearTimer(id) { clearTimeout(id); timers.delete(id); },
    report(bytes) {
      const d = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength), view = d.getUint32(4, true), flags = d.getUint32(92, true);
      seen.set(view, !!(flags & 32)); gestures.set(view, !!(flags & 64)); animating.set(view, !!(flags & 128));
    } });
  globalThis.wf = { controller };
  const snapshot = (view, row, revision, extra = {}) => ({ view, revision, scrollSequence: '0', totalExtent: 40, count: 1,
    rows: [{ view: row, root: row, index: 0, start: 0, size: 40, epoch: '1' }], correction: null, ...extra });
  const run = () => { for (let n = 0; n < 4 && frames.length; n++) frames.splice(0).forEach(fn => fn()); };
  controller.commit([snapshot(1, 2, '1'), snapshot(3, 4, '1')]); run();
  return { controller, port, seen, gestures, animating, timers, snapshot, run };
}

test('fresh rows are measured before paint while a scroll is still moving', async () => {
  const result = await evaluate(`(async () => {
    globalThis.wf?.controller.dispose();
    const root = document.getElementById('root');
    root.innerHTML = '<div data-view="1" style="height:180px;width:320px;overflow:auto"><div data-view="2" style="height:40px"></div><div data-view="4" style="height:60px"></div><div style="height:5000px"></div></div>';
    const views = new Map([...root.querySelectorAll('[data-view]')].map(el => [+el.dataset.view, el]));
    const port = views.get(1), frames = [], reports = [];
    let clock = 1000;
    const controller = createController({ root, views, now:() => clock,
      requestFrame(fn) { frames.push(fn); return frames.length; }, cancelFrame() {},
      report(bytes) { const d = new DataView(bytes.buffer); reports.push({velocity:d.getFloat64(80, true), size:d.getFloat64(112, true)}); }
    });
    globalThis.wf = {controller};
    const snapshot = (revision, epoch = '1') => ({view:1, revision, scrollSequence:'0', totalExtent:5100, count:100,
      rows:[{view:2, root:2, index:0, start:0, size:40, epoch}, {view:4, root:4, index:1, start:40, size:60, epoch:'1'}], correction:null});
    controller.commit([snapshot('1')]);
    frames.splice(0).forEach(fn => fn());
    await Promise.resolve();
    for (const top of [20, 40]) {
      clock += 16; port.scrollTop = top; port.dispatchEvent(new Event('scroll'));
      frames.splice(0).forEach(fn => fn());
    }
    const velocity = reports.at(-1).velocity, before = reports.length;
    views.get(2).style.height = '90px';
    controller.commit([snapshot('2', '2')]);
    await Promise.resolve(); await Promise.resolve();
    const added = reports.slice(before);
    controller.dispose(); delete globalThis.wf;
    return {velocity, added};
  })()`);
  expect(result.velocity).toBe(1250);
  expect(result.added.length).toBeGreaterThan(0);
  expect(result.added.length).toBeLessThanOrEqual(2);
  expect(result.added[0].size).toBe(90);
  expect(result.added[0].velocity).toBe(1250);
});

test('a finger held still on the port holds nothing: its drag ends after the quiet interval, a request lands under it, and the drag going on ends it', async () => {
  const result = await evaluate(`(() => { const f=(${fixture})();
    f.touch('touchstart',1); f.scroll(200); f.fire(300); const held=f.report().scrolling;
    f.controller.commit([f.snapshot('2',{seeking:true,request:1,correction:{scrollSequence:'0',offset:900}})]); const landed=f.list.scrollTop;
    f.report(); f.scroll(950); const dragged=f.report().gesture;
    return {held,landed,dragged}; })()`);
  expect(result).toEqual({ held: false, landed: 900, dragged: true });
});

// The agent's settle ends the reader's motion: a request that waits for it lands there, and the next frame's report
// says the same as the settle's.
test('the agent\'s settle ends the reader\'s motion, and the frame after it says so too', async () => {
  const result = await evaluate(`(() => { const f=(${fixture})();
    f.scroll(200); const before=f.report().scrolling;
    f.controller.settle(); const settled=f.seen.at(-1).scrolling;
    f.controller.commit([f.snapshot('2',{seeking:true,request:1,correction:{scrollSequence:'0',offset:900}})]);
    const after=f.report();
    return {before, settled, after:[after.scrolling, after.gesture], timers: f.timers.size}; })()`);
  expect(result).toEqual({ before: true, settled: false, after: [false, false], timers: 0 });
});

// LLP 1070.000 §2.6: only an instant write that moved the port owns the next `scrollend`. One to where the port already
// is makes none, so the reader's next `scrollend` stays the reader's.
test('a write to where the port already is owns no scrollend: the reader\'s next one ends the scroll', async () => {
  const result = await evaluate(`(() => { const f=(${fixture})();
    f.controller.commit([f.snapshot('2',{seeking:true,request:1,correction:{scrollSequence:'0',offset:0}})]);
    f.report(); f.scroll(200); const moving=f.report().scrolling;
    f.list.dispatchEvent(new Event('scrollend'));
    return {moving, ended: !f.report().scrolling}; })()`);
  expect(result).toEqual({ moving: true, ended: true });
});

test('where no scrollend comes, the scroll ends after the quiet interval since its last move', async () => {
  const result = await evaluate(`(() => { const f=(${fixture})();
    f.scroll(200); const moving=f.report().scrolling, armed=f.timers.size;
    f.fire(100); const early=f.report().scrolling, rearmed=f.timers.size;
    f.fire(200); const ended=f.report().scrolling;
    return {moving,armed,early,rearmed,ended,left:f.timers.size}; })()`);
  expect(result).toEqual({ moving: true, armed: 1, early: true, rearmed: 1, ended: false, left: 0 });
});

test('a gesture is reported against the request it began after, not the next one', async () => {
  const result = await evaluate(`(async () => { const f=(${fixture})();
    f.scroll(200); f.controller.commit([f.snapshot('2',{waiting:true,request:1})]); const before=f.report().gesture;
    await f.later(); f.touch('touchstart',1); f.list.dispatchEvent(new PointerEvent('pointerdown',{bubbles:true,pointerId:3,pointerType:'touch'}));
    f.scroll(220); const during=f.report().gesture;
    f.controller.commit([f.snapshot('3',{waiting:true,request:2})]); f.scroll(240); const next=f.report().gesture;
    return {before,during,next}; })()`);
  expect(result).toEqual({ before: false, during: true, next: false });
});

test('a wheel event is no gesture: its scroll is motion while a request waits, and ends one that has been admitted', async () => {
  const result = await evaluate(`(async () => { const f=(${fixture})();
    f.scroll(200); f.controller.commit([f.snapshot('2',{waiting:true,request:1})]); f.report(); await f.later();
    f.list.dispatchEvent(new WheelEvent('wheel',{deltaY:40,bubbles:true})); f.scroll(240); const waiting=f.report();
    f.controller.commit([f.snapshot('3',{seeking:true,request:2})]); f.report(); await f.later();
    f.list.dispatchEvent(new WheelEvent('wheel',{deltaY:40,bubbles:true})); const still=f.report().gesture;
    f.scroll(280); const moved=f.report().gesture;
    return {waiting:[waiting.gesture, waiting.scrolling], still, moved}; })()`);
  expect(result).toEqual({ waiting: [false, true], still: false, moved: true });
});

// WebKit's and Safari's order with a real trackpad: a glide that ends by itself sends its `scrollend`, then its last
// wheel events, which move nothing (the wheel probe, macOS 27.2). The `scrollend` ends the reader's scroll, and the
// glide's last events change nothing for the request admitted there.
test('a glide\'s last wheel events after its scrollend are no gesture against the request admitted there', async () => {
  const result = await evaluate(`(async () => { const f=(${fixture})();
    const wheel = dy => f.list.dispatchEvent(new WheelEvent('wheel',{deltaY:dy,bubbles:true}));
    wheel(40); f.scroll(240); wheel(1); f.scroll(241);
    f.controller.commit([f.snapshot('2',{waiting:true,request:1})]); f.report(); await f.later();
    f.list.dispatchEvent(new Event('scrollend')); const ended=!f.report().scrolling;
    f.controller.commit([f.snapshot('3',{seeking:true,request:1})]); f.report(); await f.later();
    for (const ms of [17, 33, 33, 34, 66]) { f.fire(ms); wheel(1); }
    const after=f.report();
    return {ended, after:[after.scrolling, after.gesture]}; })()`);
  expect(result).toEqual({ ended: true, after: [false, false] });
});

test('a request\'s landing that meets the reader\'s scroll still lands, and the scroll is still reported', async () => {
  const result = await evaluate(`(() => { const f=(${fixture})();
    f.scroll(300); f.report();
    f.controller.commit([f.snapshot('2',{seeking:true,request:1,correction:{scrollSequence:'0',offset:900}})]);
    return {landed:f.list.scrollTop,scrolling:f.report().scrolling}; })()`);
  expect(result).toEqual({ landed: 900, scrolling: true });
});

test('a smooth correction lands at once where the browser has no scrollend, the event that says where it ends', async () => {
  const result = await evaluate(`(() => { const d = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'onscrollend');
    delete HTMLElement.prototype.onscrollend;
    try { const f=(${fixture})();
      f.controller.commit([f.snapshot('2',{seeking:true,request:1,correction:{scrollSequence:'0',offset:800,smooth:true}})]);
      return { top: f.list.scrollTop, animating: f.report().animating }; }
    finally { Object.defineProperty(HTMLElement.prototype, 'onscrollend', d); } })()`);
  expect(result).toEqual({ top: 800, animating: false });
});

test('in one scrollport, the move that lands one list\'s smooth correction is no list\'s reader move', async () => {
  const result = await evaluate(`(async () => { const { controller, port, seen, gestures, animating, snapshot, run } = (${sharedPort})();
    // List 1's smooth request animates to 800; list 3's own request has been admitted
    controller.commit([snapshot(1, 2, '2', { seeking: true, request: 1, correction: { scrollSequence: '0', offset: 800, smooth: true } }),
      snapshot(3, 4, '2', { seeking: true, request: 1 })]); run();
    const during = animating.get(1);
    await new Promise(r => port.addEventListener('scrollend', r, { once: true })); run();
    return { during, animating: animating.get(1), scrolling: [seen.get(1), seen.get(3)], gesture: [gestures.get(1), gestures.get(3)],
      top: port.scrollTop }; })()`);
  expect(result).toEqual({ during: true, animating: false, scrolling: [false, false], gesture: [false, false], top: 800 });
});

// LLP 1050.000 §6: an authored jump reports its target before the port moves, so its rows are built first; also when
// it stops a smooth correction under way, whose stop would otherwise hold the list's report to the next frame.
test('a jump made while a smooth correction runs reports its target before the port moves', async () => {
  const result = await evaluate(`(() => { const f=(${fixture})();
    f.controller.commit([f.snapshot('2',{seeking:true,request:1,correction:{scrollSequence:'0',offset:800,smooth:true}})]);
    const n=f.seen.length; f.controller.jump(1, 1000); const first=f.seen.slice(n)[0];
    return { reported: first?.top, animating: first?.animating, top: f.list.scrollTop }; })()`);
  expect(result).toEqual({ reported: 1000, animating: false, top: 1000 });
});

// CSSOM View's "perform a scroll" aborts a smooth scroll under way on the box whatever the new scroll's behaviour: another
// list's smooth write in the scrollport ends a list's smooth correction, and its reports say so.
test('in one scrollport, another list\'s smooth write ends a smooth correction\'s animation', async () => {
  const result = await evaluate(`(() => { const { controller, animating, snapshot, run } = (${sharedPort})();
    controller.commit([snapshot(1, 2, '2', { seeking: true, request: 1, correction: { scrollSequence: '0', offset: 2000, smooth: true } }),
      snapshot(3, 4, '1')]); run();
    const before = animating.get(1);
    controller.commit([snapshot(1, 2, '2', { seeking: true, request: 1 }),
      snapshot(3, 4, '2', { seeking: true, request: 1, correction: { scrollSequence: '0', offset: 1000, smooth: true } })]); run();
    return { before, after: animating.get(1) }; })()`);
  expect(result).toEqual({ before: true, after: false });
});

// CSSOM View's "scroll an element" aborts a smooth scroll under way on the box: another list's instant write in the
// scrollport ends a list's smooth correction where the write put the port, and its reports say so.
test('in one scrollport, another list\'s instant write ends a smooth correction\'s animation where it put the port', async () => {
  const result = await evaluate(`(async () => { const { controller, port, animating, snapshot, run } = (${sharedPort})();
    controller.commit([snapshot(1, 2, '2', { seeking: true, request: 1, correction: { scrollSequence: '0', offset: 800, smooth: true } }),
      snapshot(3, 4, '2', { correction: { scrollSequence: '0', offset: 40, from: 0 } })]); run();
    await Promise.race([new Promise(r => port.addEventListener('scrollend', r, { once: true })), new Promise(r => setTimeout(r, 800))]);
    const frame = () => new Promise(r => requestAnimationFrame(() => setTimeout(r, 0)));
    for (let i = 0; i < 10; i++) { await frame(); run(); }
    controller.settle();
    return { animating: animating.get(1), top: port.scrollTop }; })()`);
  expect(result).toEqual({ animating: false, top: 40 });
}, 60_000);

test('a real touch does not count against a request its pointerdown handler makes, though its touchstart comes after', async () => {
  const at = await evaluate(`(() => { const f=(${fixture})();
    f.controller.commit([f.snapshot('2',{waiting:true,request:1})]); f.report();
    // The app's handler: a newer request in answer to the press
    document.addEventListener('pointerdown', () => f.controller.commit([f.snapshot('3',{waiting:true,request:2})]), { capture: true, once: true });
    const r = f.list.getBoundingClientRect(); return { x: r.left + 40, y: r.top + 20 }; })()`);
  await call('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [at] });
  await call('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
  expect(await evaluate(`wf.report().gesture`)).toBe(false);
});

test('the lists in one scrollport take one list\'s write as its own, at rest too', async () => {
  const result = await evaluate(`(() => { const { controller, seen, snapshot, run } = (${sharedPort})();
    const rest = [seen.get(1), seen.get(3)];
    // List 1's anchor correction moves the port the two lists share
    controller.commit([snapshot(1, 2, '2', {correction:{scrollSequence:'0',offset:40,from:0}}), snapshot(3, 4, '2')]); run();
    return { rest, written: [seen.get(1), seen.get(3)], top: document.getElementById('port').scrollTop }; })()`);
  expect(result).toEqual({ rest: [false, false], written: [false, false], top: 40 });
});

test('in one scrollport, a list\'s correction planned before another\'s request moved the port does not land', async () => {
  const result = await evaluate(`(() => { const { controller, snapshot } = (${sharedPort})();
    // List 1's request moves the port to 900; list 3's absolute correction was planned before it
    controller.commit([snapshot(1, 2, '2', { seeking: true, request: 1, correction: { scrollSequence: '0', offset: 900 } }),
      snapshot(3, 4, '2', { correction: { scrollSequence: '0', offset: 0 } })]);
    return document.getElementById('port').scrollTop; })()`);
  expect(result).toBe(900);
});

test('a request that ends or is replaced takes its smooth animation with it, stopped where it is; so does a list that goes', async () => {
  const run = end => evaluate(`(async () => { const f=(${fixture})();
    const frame = () => new Promise(r => requestAnimationFrame(() => setTimeout(r, 0)));
    f.controller.commit([f.snapshot('2',{seeking:true,request:1,correction:{scrollSequence:'0',offset:2000,smooth:true}})]);
    const running=f.report().animating;
    await frame(); await frame();
    f.controller.commit(${end});
    // Chrome runs the animation on its compositor: the page reads where it stopped a frame later.
    await frame(); const stopped=f.list.scrollTop;
    for (let i = 0; i < 20; i++) await frame();
    return {running,moved:f.list.scrollTop!==stopped,short:f.list.scrollTop<2000}; })()`);
  expect(await run("[f.snapshot('3')]")).toEqual({ running: true, moved: false, short: true });
  expect(await run("[f.snapshot('3',{waiting:true,request:2})]")).toEqual({ running: true, moved: false, short: true });
  expect(await run('[]')).toEqual({ running: true, moved: false, short: true });
}, 60_000);

test('where a stopped animation shows a frame later is not the reader\'s scroll; a move after that frame is', async () => {
  const result = await evaluate(`(() => { const f=(${fixture})();
    f.controller.commit([f.snapshot('2',{seeking:true,request:1,correction:{scrollSequence:'0',offset:2000,smooth:true}})]);
    const running=f.report().animating;
    // Replaced: the glue stops the animation, and the browser shows where it stopped a frame later
    f.controller.commit([f.snapshot('3',{waiting:true,request:2})]); f.scroll(f.list.scrollTop+30);
    const stopped=f.report();
    f.scroll(f.list.scrollTop+30); const reader=f.report().scrolling;
    return {running,stopped:[stopped.animating,stopped.scrolling],reader}; })()`);
  expect(result).toEqual({ running: true, stopped: [false, false], reader: true });
});

test('the lists in one scrollport share the reader\'s scroll: one list\'s own write owns the next scrollend for both', async () => {
  const result = await evaluate(`(() => { const { controller, port, seen, timers, snapshot, run } = (${sharedPort})();
    port.scrollTop = 300; port.dispatchEvent(new Event('scroll')); run();
    const moving = [seen.get(1), seen.get(3)];
    // List 1's anchor correction: the glue's own write, whose scrollend answers it, for list 3 too
    controller.commit([snapshot(1, 2, '2', {correction:{scrollSequence:'1',offset:20,from:0}}), snapshot(3, 4, '1')]);
    port.dispatchEvent(new Event('scrollend')); run();
    const atOwnEnd = [seen.get(1), seen.get(3)];
    port.scrollTop += 10; port.dispatchEvent(new Event('scroll')); run();
    const afterOwn = [seen.get(1), seen.get(3)];
    port.dispatchEvent(new Event('scrollend')); run();
    return { moving, atOwnEnd, afterOwn, ended: [seen.get(1), seen.get(3)], timers: timers.size }; })()`);
  expect(result).toEqual({ moving: [true, true], atOwnEnd: [true, true], afterOwn: [true, true], ended: [false, false], timers: 0 });
});

test('an input counts against the request under way when it began: one made before the request does not', async () => {
  const result = await evaluate(`(async () => { const f=(${fixture})();
    // An event's timeStamp is when it was made: this one is made before the request and dispatched after it
    const early=new PointerEvent('pointerdown',{bubbles:true,pointerType:'mouse',button:2});
    await f.later(); f.controller.commit([f.snapshot('2',{seeking:true,request:1})]); await f.later();
    f.list.dispatchEvent(early); const before=f.report().gesture;
    f.list.dispatchEvent(new PointerEvent('pointerdown',{bubbles:true,pointerType:'mouse',button:2})); const after=f.report().gesture;
    return {before,after}; })()`);
  expect(result).toEqual({ before: false, after: true });
});

test('a key is no gesture, in the list or beside it; what it scrolls is the reader\'s motion', async () => {
  const result = await evaluate(`(async () => { const f=(${fixture})();
    f.controller.commit([f.snapshot('2',{seeking:true,request:1})]); await f.later();
    f.list.dispatchEvent(new KeyboardEvent('keydown',{key:'PageDown',code:'PageDown',bubbles:true}));
    document.body.dispatchEvent(new KeyboardEvent('keydown',{key:'a',code:'KeyA',bubbles:true}));
    f.controller.commit([f.snapshot('3',{seeking:true,request:1})]); const pressed=f.report();
    f.scroll(262); const scrolled=f.report();
    return {gesture:pressed.gesture, scrolling:pressed.scrolling, moved:scrolled.scrolling}; })()`);
  expect(result).toEqual({ gesture: false, scrolling: false, moved: true });
});
test('in a port that holds more than its list, a press beside the list is not its gesture; a finger anywhere on the port is', async () => {
  const result = await evaluate(`(async () => {
    globalThis.wf?.controller.dispose();
    const root = document.getElementById('root');
    root.innerHTML = '<div id="port" style="height:180px;width:320px;overflow:auto"><header><button id="button">b</button></header>'
      + '<div data-view="1"><div data-view="2" style="height:40px"><button id="row">r</button></div><div style="height:5000px"></div></div></div>';
    const views = new Map([...root.querySelectorAll('[data-view]')].map(el => [+el.dataset.view, el]));
    const frames = [];
    let gesture = null, revision = 1, request = 0;
    const controller = createController({ root, views, requestFrame(fn) { frames.push(fn); return frames.length; }, cancelFrame() {},
      report(bytes) { gesture = !!(new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getUint32(92, true) & 64); } });
    globalThis.wf = { controller };
    const commit = () => { revision++; controller.commit([{ view: 1, revision: String(revision), scrollSequence: '0', totalExtent: 5040, count: 2,
      correction: null, waiting: true, request, rows: [{ view: 2, root: 2, index: 0, start: 0, size: 40, epoch: '1' }] }]);
      for (let n = 0; n < 4 && frames.length; n++) frames.splice(0).forEach(fn => fn()); };
    const later = () => new Promise(r => setTimeout(r, 2));
    // A new request, then the input in a later task, then a report once the input's dispatch is over
    const after = async input => { request++; commit(); await later(); input(); await later(); commit(); return gesture; };
    const at = id => id ? document.getElementById(id) : document.body;
    const press = (id, pointerType = 'mouse') => () => { for (const type of ['pointerdown', 'pointerup'])
      at(id).dispatchEvent(new PointerEvent(type, { bubbles: true, button: 0, pointerId: 1, pointerType })); };
    return { pressedBeside: await after(press('button')), pressedInList: await after(press('row')), pressedPort: await after(press('port')),
      touchedBeside: await after(press('button', 'touch')) }; })()`);
  expect(result).toEqual({ pressedBeside: false, pressedInList: true, pressedPort: true, touchedBeside: true });
});

test('a request\'s correction sent again after the reader moved the port does not land, and the request is the reader\'s', async () => {
  const result = await evaluate(`(async () => { const f=(${fixture})();
    f.controller.commit([f.snapshot('2',{seeking:true,request:1,correction:{scrollSequence:'0',offset:900}})]); f.report();
    const landed=f.list.scrollTop;
    // The reader's scroll with no gesture, as a key's is; then a data update sends the same correction again
    f.scroll(1100);
    f.controller.commit([f.snapshot('3',{seeking:true,request:1,correction:{scrollSequence:'0',offset:900}})]);
    const after=f.list.scrollTop, gesture=f.report().gesture;
    return {landed, after, gesture}; })()`);
  expect(result).toEqual({ landed: 900, after: 1100, gesture: true });
});

test('a move first found by a commit or a reorder drag\'s mapping is the reader\'s, whose scroll event then finds nothing changed', async () => {
  const result = await evaluate(`(() => {
    const found = read => { const f=(${fixture})();
      f.list.scrollTop = 300; read(f); const at=f.report().scrolling;
      f.list.dispatchEvent(new Event('scroll')); return [at, f.report().scrolling]; };
    return { commit: found(f => f.controller.commit([f.snapshot('2')])), mapping: found(f => f.controller.reorderMapping(1, 0)) }; })()`);
  expect(result).toEqual({ commit: [true, true], mapping: [true, true] });
});

test('a request\'s move is made though the port resized since the report it was planned on', async () => {
  const result = await evaluate(`(() => { const f=(${fixture})();
    f.list.scrollTop = 300; f.list.style.height = '200px';
    f.controller.commit([f.snapshot('2',{seeking:true,request:1,correction:{scrollSequence:'0',offset:900}})]);
    return {landed:f.list.scrollTop}; })()`);
  expect(result).toEqual({ landed: 900 });
});

test('a list whose animation was stopped reports from the next frame, where the browser stopped it', async () => {
  const result = await evaluate(`(async () => { const f=(${fixture})();
    const frame = () => new Promise(r => requestAnimationFrame(() => setTimeout(r, 0)));
    f.controller.commit([f.snapshot('2',{seeking:true,request:1,correction:{scrollSequence:'0',offset:2000,smooth:true}})]);
    f.report(); await frame(); await frame();
    const before=f.seen.length;
    f.controller.commit([f.snapshot('3',{waiting:true,request:2})]); f.controller.settle();
    const sameFrame=f.seen.length-before;
    await frame(); const next=f.report();
    return {sameFrame,animating:next.animating,reported:f.seen.length>before}; })()`);
  expect(result).toEqual({ sameFrame: 0, animating: false, reported: true });
}, 60_000);

// A list with no scrolling ancestor, two rows mounted: its port is the viewport, whose `scroll` and `scrollend` CSSOM
// View fires at the document. `run()` runs the frames owed and returns the last report; timers run on the real clock;
// `done()` puts the page back and waits for that scroll's end, which would otherwise reach the next test.
function pagePort() {
  globalThis.wf?.controller.dispose();
  const root = document.getElementById('root');
  root.innerHTML = '<div data-view="1"><div data-view="2" style="height:40px"></div><div data-view="4" style="height:60px"></div><div style="height:5000px"></div></div>';
  const views = new Map([...root.querySelectorAll('[data-view]')].map(el => [+el.dataset.view, el]));
  const page = document.scrollingElement, frames = [], seen = [];
  const controller = createController({ root, views, requestFrame(fn) { frames.push(fn); return frames.length; }, cancelFrame() {},
    setTimer: (fn, ms) => setTimeout(fn, ms), clearTimer: id => clearTimeout(id),
    report(bytes) {
      const d = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength), flags = d.getUint32(92, true);
      seen.push({ top: d.getFloat64(24, true), scrolling: !!(flags & 32), animating: !!(flags & 128) });
    } });
  globalThis.wf = { controller };
  const snapshot = (revision, extra = {}) => ({ view: 1, revision, scrollSequence: '0', totalExtent: 5100, count: 100,
    rows: [{ view: 2, root: 2, index: 0, start: 0, size: 40, epoch: '1' }, { view: 4, root: 4, index: 1, start: 40, size: 60, epoch: '1' }], correction: null, ...extra });
  const run = () => { for (let n = 0; n < 4 && frames.length; n++) frames.splice(0).forEach(fn => fn()); return seen.at(-1); };
  const ended = () => Promise.race([new Promise(r => document.addEventListener('scrollend', r, { once: true })), new Promise(r => setTimeout(r, 1500))]);
  const done = async () => { controller.dispose(); globalThis.wf = null; if (page.scrollTop) { page.scrollTop = 0; await ended(); } root.innerHTML = ''; };
  controller.commit([snapshot('1')]); run();
  return { controller, page, list: views.get(1), snapshot, run, ended, done };
}

test('a list whose port is the page hears the reader\'s scroll there, and its end', async () => {
  const result = await evaluate(`(async () => { const p=(${pagePort})();
    try { p.page.scrollTop = 600; await new Promise(r => document.addEventListener('scroll', r, { once: true }));
      const during = p.run(); await p.ended(); await new Promise(r => setTimeout(r, 350)); const after = p.run();
      return { during: [during?.top, during?.scrolling], after: after?.scrolling, at: 600 - p.list.offsetTop }; }
    finally { await p.done(); } })()`);
  expect(result.during).toEqual([result.at, true]);
  expect(result.after).toBe(false);
}, 60_000);

test('a smooth correction in a list whose port is the page lands at the page\'s scrollend', async () => {
  const result = await evaluate(`(async () => { const p=(${pagePort})();
    try { p.controller.commit([p.snapshot('2',{seeking:true,request:1,correction:{scrollSequence:'0',offset:2000,smooth:true}})]);
      const during = p.run().animating; await p.ended(); const after = p.run();
      return { during, animating: after.animating, top: [after.top, p.page.scrollTop - p.list.offsetTop] }; }
    finally { await p.done(); } })()`);
  expect(result).toEqual({ during: true, animating: false, top: [2000, 2000] });
}, 60_000);
