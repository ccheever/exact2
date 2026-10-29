import { afterEach, expect, mock, test } from 'bun:test';
import { createTimerScheduler } from '../web/timer-glue.js';
const rt = {};
mock.module('./rt-link.js', () => ({ rt }));
const { timerWaits } = await import('./timer-waits.js');
const globals = ['document', 'window', 'IntersectionObserver', 'requestAnimationFrame', 'cancelAnimationFrame', 'requestIdleCallback', 'setTimeout', 'clearTimeout'];
const original = Object.fromEntries(globals.map(name => [name, Object.getOwnPropertyDescriptor(globalThis, name)]));
afterEach(() => {
  for (const name of globals) {
    if (original[name]) { Object.defineProperty(globalThis, name, original[name]); }
    else { delete globalThis[name]; }
  }
});
function fixture() {
  let now = 0, serial = 0, intersect;
  const frames = new Map(), timeouts = new Map(), idles = new Map(), events = new Map(), fired = [];
  const scroller = { scrollLeft: 0, scrollTop: 0, isConnected: true };
  const document = {
    hidden: false, documentElement: { append() {} }, scrollingElement: scroller, timeline: {}, animations: [],
    addEventListener(type, f) { events.set(type, f); }, getAnimations() { return this.animations; },
    createElement() { return { style: {}, setAttribute() {}, remove() {} }; },
  };
  Object.assign(globalThis, {
    document, window: { onscrollend: null },
    IntersectionObserver: class { constructor(f) { intersect = f; } observe() {} disconnect() {} },
    requestAnimationFrame(f) { frames.set(++serial, f); return serial; }, cancelAnimationFrame(id) { frames.delete(id); },
    requestIdleCallback(f) { idles.set(++serial, f); return serial; },
    setTimeout(f, delay) { timeouts.set(++serial, {f, due: now + delay}); return serial; }, clearTimeout(id) { timeouts.delete(id); },
  });
  for (const key of Object.keys(rt)) { delete rt[key]; }
  Object.assign(rt, { clock: { now: 0, agent: false, timers: [] }, wall: () => now, nextMutation: () => Infinity, busy: () => false,
    advance(to) { rt.clock.now = to; }, drive() {}, fireTimer(t) { fired.push(t.name); t.action?.(); } });
  timerWaits();
  const run = queue => { const pending = [...queue.values()]; queue.clear(); for (const f of pending) { (typeof f === 'function' ? f : f.f)(); } };
  return {
    fired, frames, timeouts, idles, document, scroller,
    now() { return now; },
    at(time) { now = time; rt.clock.now = time; },
    task(wait, action) { const t = {name: 'task' + rt.clock.timers.length, i: rt.clock.timers.length, due: 10, ms: 10, once: true, wait, action}; rt.clock.timers.push(t); return t; },
    check() { rt.timerWait.check(); }, frame() { run(frames); }, idle() { run(idles); },
    timeout() {
      const due = [...timeouts.values()].sort((a,b) => a.due - b.due)[0];
      if (due) {
        now = due.due;
        const ready = [...timeouts].filter(([, task]) => task.due <= now);
        for (const [id] of ready) { timeouts.delete(id); }
        for (const [, task] of ready) { task.f(); }
      }
    },
    event(type, data = {}) { events.get(type)?.({ target: scroller, ...data }); },
    intersect(value) { intersect([{ isIntersecting: value }]); },
  };
}
test('a frame wait requests a callback and a stale callback cannot admit after takeover', () => {
  const f = fixture(); f.task(1); f.at(10); f.check();
  expect(f.fired).toEqual([]); expect(f.frames.size).toBe(1);
  const old = [...f.frames.values()][0];
  rt.clock.agent = true; rt.clock.agent = false; old();
  expect(f.fired).toEqual([]);
  f.check(); f.frame(); expect(f.fired).toEqual(['task0']);
});
test('update demand is sampled again when demand ends before its requested callback', () => {
  const f = fixture(); f.task(2); rt.clock.timers.push({frame:true, due:100}); f.at(10); f.check();
  expect(f.fired).toEqual([]);
  rt.clock.timers.pop(); f.check(); expect(f.fired).toEqual(['task0']);
});
test('each admitted batch resamples demand before selecting another held firing', () => {
  const f = fixture();
  f.task(3, () => rt.clock.timers.push({ frame: true, due: 30 })); f.task(2);
  f.at(10); f.check();
  expect(f.fired).toEqual(['task0']);
  f.frame(); expect(f.fired).toEqual(['task0', 'task1']);
});
test('touch holds survive pointer cancellation, old scroll completion and the first finger lifting', () => {
  const f = fixture(); f.task(3);
  f.event('touchstart', { changedTouches: [{identifier:1}, {identifier:2}] });
  f.event('pointercancel', { pointerType:'touch', pointerId:1 });
  f.event('scrollend'); f.at(10); f.check();
  expect(f.fired).toEqual([]);
  f.event('touchend', { changedTouches:[{identifier:1}] }); f.frame(); expect(f.fired).toEqual([]);
  f.event('touchend', { changedTouches:[{identifier:2}] }); f.frame(); expect(f.fired).toEqual(['task0']);
});
test('movement survives a lift and a delivered offscreen report releases the wait', () => {
  const f = fixture(); f.task(3); f.event('scroll'); f.at(10); f.check();
  f.intersect(true); f.frame(); expect(f.fired).toEqual([]);
  f.intersect(false); expect(f.fired).toEqual(['task0']);
});
test('idle notices a paused animation at its probe', () => {
  const f = fixture();
  const animation = { timeline:f.document.timeline, playState:'running', playbackRate:1, finished:new Promise(() => {}) };
  f.document.animations = [animation]; f.task(4); f.at(10); f.check(); f.idle();
  expect(f.fired).toEqual([]);
  animation.playState = 'paused'; f.timeout(); f.idle(); expect(f.fired).toEqual(['task0']);
});


test('a timeout cannot declare a visible scroll complete after a blocked frame callback', () => {
  const f = fixture(); f.task(3); f.event('scroll'); f.at(1200); f.check();
  expect(f.fired).toEqual([]);
  f.frame(); expect(f.fired).toEqual(['task0']);
});
test('an idle callback resamples frame demand after the previous action', () => {
  const f = fixture();
  f.task(4, () => rt.clock.timers.push({frame:true, due:30})); f.task(4); f.at(10); f.check(); f.idle();
  expect(f.fired).toEqual(['task0']);
  f.check(); f.idle(); expect(f.fired).toEqual(['task0']);
  rt.clock.timers = rt.clock.timers.filter(t => !t.frame); f.check(); f.idle();
  expect(f.fired).toEqual(['task0','task1']);
});

test('a queued idle callback rechecks timer suspension and poison', () => {
  for (const blocked of ['stopped', 'poisoned']) {
    const f = fixture(); f.task(4); f.at(10); f.check();
    expect(f.idles.size).toBe(1);
    if (blocked === 'stopped') { rt.clock.stopped = true; }
    else { rt.poisoned = () => true; }
    f.idle();
    expect(f.fired).toEqual([]);
    if (blocked === 'stopped') {
      rt.clock.stopped = false; f.check(); f.idle();
      expect(f.fired).toEqual(['task0']);
    }
  }
});

test('the wasm scheduler requests a frame when its nominal check first reports held work', () => {
  const f = fixture(), firings = [];
  let waiting = [10, -1, -1, -1];
  const scheduler = createTimerScheduler({ now: f.now, advance() {}, release(kinds, cutoff) {
    if (waiting[0] >= 0 && waiting[0] <= cutoff) { waiting = [-2, -1, -1, -1]; }
    if ((kinds & 2) && waiting[0] === -2) { waiting = [-1, -1, -1, -1]; firings.push(cutoff); }
    scheduler.update(firings.length ? null : 110, false, waiting);
    return {};
  } });
  scheduler.update(110, false, waiting);
  f.timeout();
  expect(f.now()).toBe(10);
  expect(firings).toEqual([]);
  expect(f.frames.size).toBe(1);
  f.frame();
  expect(firings).toEqual([10]);
  scheduler.dispose();
});

test('the wasm scheduler resamples idle demand and discards callbacks after reset', () => {
  const f = fixture(), firings = [];
  let count = 2, waiting = [-1, -1, -1, 10], demand = false;
  const scheduler = createTimerScheduler({ now: f.now, advance() {}, present() {}, release(kinds, cutoff) {
    if ((kinds & 16) && count) { count--; firings.push(cutoff); demand = true; }
    waiting = [-1, -1, -1, count ? -2 : -1];
    scheduler.update(1010, demand, waiting);
    return {};
  } });
  scheduler.update(1010, false, waiting);
  f.timeout(); f.idle();
  expect(firings).toEqual([10]);
  f.timeout(); f.idle();
  expect(firings).toEqual([10]);
  demand = false; scheduler.update(1010, false, waiting); f.timeout();
  const stale = [...f.idles.values()][0];
  expect(stale).toBeDefined();
  scheduler.reset(); stale();
  expect(firings).toEqual([10]);
  scheduler.dispose();
});
