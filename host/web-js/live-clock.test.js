// The JS target's timers on the page's clock (the timers RFC, D2, D3): a repeat that missed beats fires once,
// observing when it runs, and its next beat is the first of its phase after its callback ends; on the agent's
// seekable clock every beat fires at its own time. Run alone, `bun test ./host/web-js/live-clock.test.js`:
// reorder.test.js mocks rt.js for its process.
import { test, expect, mock, beforeEach, afterAll } from "bun:test";

// The build copies navigation.js and prompt-glue.js in from host/web and writes names.js.
mock.module("./navigation.js", () => import("../web/navigation.js"));
mock.module("./prompt-glue.js", () => ({}));
mock.module("./alert-message.js", () => ({}));
mock.module("./menu-rows.js", () => ({}));
mock.module("./names.js", () => ({ sourceTypes: {} }));
const { clock, every, advance, armedAt, commit, Refusal, Mutations } = await import("./rt.js");
const { beatAfter } = await import("./beat.js");

// The page's clock, the test's own while it runs
let wall = 0;
const pageNow = globalThis.performance.now;
globalThis.performance.now = () => wall;
afterAll(() => { globalThis.performance.now = pageNow; });

/** A repeat every `ms` from now, whose callback records what it observed and may take `spend` ms. */
function ticker(ms, spend = 0) {
  const seen = [];
  clock.agent = true; // no page timer behind the test's back
  every(ms, () => { seen.push([clock.now, armedAt()]); wall += spend; return true; }, false, "ticker");
  clock.agent = false;
  return { seen, timer: clock.timers.at(-1) };
}

beforeEach(() => { clock.now = 0; clock.timers.length = 0; wall = 0; });

test("a refused commit and an invalid advance keep the live timer", () => {
  const { seen, timer } = ticker(1000);
  expect(commit(() => { throw new Refusal("invalid input"); })).toBe(false);
  advance(NaN, true);
  expect(clock.timers).toContain(timer);
  expect(timer.due).toBe(1000);
  wall = 1000;
  advance(1000, true);
  expect(seen).toEqual([[1000, 1000]]);
  expect(timer.due).toBe(2000);
});

test("missed beats fire once, observing when they run, and keep their phase", () => {
  const { seen, timer } = ticker(1000);
  wall = 3500;
  advance(3500, true);
  expect(seen).toEqual([[3500, 1000]]);
  expect(timer.due).toBe(4000);
});

test("a repeat is rescheduled after its callback ends", () => {
  const { seen, timer } = ticker(1000, 3500);
  wall = 1000;
  advance(1000, true);
  expect(seen.length).toBe(1);
  expect(clock.now).toBe(4500);
  expect(timer.due).toBe(5000);
});

test("the agent's clock fires every beat at its own time", () => {
  const { seen } = ticker(1000);
  advance(3500, false);
  expect(seen.map(([at]) => at)).toEqual([1000, 2000, 3000]);
});

test("the next beat is strictly after the time, whatever the rounding", () => {
  expect(beatAfter(74.19, 3, 884.1899999999999)).toBe(884.19);
  expect(beatAfter(1000, 1000, 5000)).toBe(6000);
  expect(beatAfter(1000, 1000, 10)).toBe(2000);
});

test("a hatch's afters keep their place behind the timers due first", async () => {
  globalThis.exact ??= {};
  const { containers } = await import("./hatches.js");
  containers().catch(() => {}); // publishes the hatch clock; the page module is not this test's
  const order = [];
  clock.agent = true; // no page timer behind the test's back
  every(2000, () => { order.push("timer"); return true; }, false, "ticker");
  globalThis.exact.hatches.after(1500, () => order.push("after 1500"));
  globalThis.exact.hatches.after(5000, () => order.push("after 5000"));
  clock.agent = false;
  // The page was busy: its clock reads 6500 by the time the advance to 3500 runs.
  wall = 6500;
  advance(3500, true);
  expect(order).toEqual(["after 1500", "timer"]);
});

test('waiting repeats seek once per virtual frame and keep their nominal phase', async () => {
  const { timerWaits } = await import('./timer-waits.js');
  clock.agent = true;
  timerWaits();
  const seen = [];
  every(1, () => { seen.push(clock.now); return true; }, false, 'waiting', 1);
  advance(100, false);
  expect(seen).toEqual([1000 / 60, 2000 / 60, 50, 4000 / 60, 5000 / 60, 100]);
  expect(clock.timers[0].due).toBe(101);
});


test('the first seek after takeover runs outstanding firings at the sampled floor', async () => {
  const { timerWaits } = await import('./timer-waits.js'); timerWaits();
  clock.agent = false; clock.agent = true;
  const seen = [];
  every(49, () => { seen.push(clock.now); return true; }, true, 'frame', 1);
  every(51, () => { seen.push(clock.now); return true; }, true, 'precise');
  clock.agent = false; wall = 100; clock.agent = true;
  advance(0, false);
  expect(seen).toEqual([100, 100]);
  advance(0, false); expect(seen.length).toBe(2);
});

function withAdmissionHost(run) {
  const names = ['document', 'window', 'requestAnimationFrame', 'cancelAnimationFrame', 'requestIdleCallback', 'setTimeout', 'clearTimeout'];
  const originals = names.map(name => Object.getOwnPropertyDescriptor(globalThis, name));
  let idle;
  Object.assign(globalThis, {
    document: { hidden: false, timeline: {}, documentElement: { append() {} },
      createElement() { return { style: {}, setAttribute() {} }; },
      addEventListener() {}, getAnimations() { return []; } },
    window: { onscrollend: null },
    requestAnimationFrame() { return 1; }, cancelAnimationFrame() {},
    requestIdleCallback(callback) { idle = callback; return 1; },
    setTimeout() { return 1; }, clearTimeout() {},
  });
  try { run(() => { const callback = idle; idle = null; expect(callback).toBeDefined(); callback(); }); }
  finally {
    Mutations.length = 0;
    for (let i = 0; i < names.length; i++) {
      if (originals[i]) { Object.defineProperty(globalThis, names[i], originals[i]); }
      else { delete globalThis[names[i]]; }
    }
  }
}

test('admission cancels a held instance when an earlier action changes its key', async () => {
  const { rt } = await import('./rt-link.js');
  const { timerWaits } = await import('./timer-waits.js');
  const { gated } = await import('./schedule.js');
  withAdmissionHost(() => {
    delete rt.timerWait; timerWaits(); clock.agent = true;
    let key = 0;
    const fired = [];
    gated(10, () => commit(() => { fired.push('first'); key++; }), true, false, () => true, null, 'first', 3);
    gated(10, () => commit(() => fired.push('second')), true, false, () => true, () => key, 'second', 3);
    clock.agent = false; wall = 10;
    rt.timerWait.check();
    expect(fired).toEqual(['first']);
    wall = 20; rt.timerWait.check();
    expect(fired).toEqual(['first', 'second']);
  });
});

test('ordinary refusals retain the cutoff and finish due work before admitting waits', async () => {
  const { rt } = await import('./rt-link.js');
  const { timerWaits } = await import('./timer-waits.js');
  withAdmissionHost(() => {
    delete rt.timerWait; timerWaits();
    for (const cancel of [false, true]) {
      clock.agent = true; clock.now = 0; clock.timers.length = 0; wall = 0;
      const fired = [];
      every(10, () => { wall = 20; return commit(() => { throw new Refusal('test refusal'); }); }, true, 'refused');
      let waiting;
      every(10, () => {
        fired.push('ordinary');
        if (cancel) { clock.timers.splice(clock.timers.indexOf(waiting), 1); }
        return true;
      }, true, 'ordinary');
      every(10, () => { fired.push('waiting'); return true; }, true, 'waiting', 3);
      waiting = clock.timers.at(-1);
      every(11, () => { fired.push('later'); return true; }, true, 'later');
      clock.agent = false; wall = 10;
      rt.timerWait.check();
      expect(fired).toEqual(cancel ? ['ordinary'] : ['ordinary', 'waiting']);
      advance(20, true);
      expect(fired.at(-1)).toBe('later');
    }
  });
});

test('idle keeps the frame horizon clear of both then and queue deadlines', async () => {
  const { rt } = await import('./rt-link.js');
  const { timerWaits } = await import('./timer-waits.js');
  withAdmissionHost(idle => {
    delete rt.timerWait; timerWaits();
    for (const field of ['due', 'next']) {
      clock.agent = true; clock.now = 0; clock.timers.length = 0; wall = 0;
      const fired = [];
      every(10, () => { fired.push('idle'); return true; }, true, 'idle', 4);
      const mutation = { due: Infinity, next: Infinity, [field]: 15 };
      Mutations.push(mutation);
      clock.agent = false; wall = 10;
      rt.timerWait.check(); idle();
      expect(fired).toEqual([]);
      rt.timerWait.check();
      mutation[field] = 10; idle();
      expect(fired).toEqual([]);
      mutation[field] = 10 + 1000 / 60;
      rt.timerWait.check(); idle();
      expect(fired).toEqual([]);
      mutation[field] = 27;
      rt.timerWait.check(); idle();
      expect(fired).toEqual(['idle']);
      Mutations.length = 0;
    }
  });
});

test('answer timers follow authored tasks at a tie and do not count as task firings', async () => {
  const { rt } = await import('./rt-link.js');
  const { timerWaits } = await import('./timer-waits.js');
  withAdmissionHost(idle => {
    delete rt.timerWait; timerWaits();
    for (const live of [false, true]) {
      clock.agent = true; clock.now = 0; clock.timers.length = 0; wall = 0;
      clock.timerFirings = Array(7).fill(0);
      const fired = [];
      clock.timers.push({ due: 10, once: true, data: true, action() { fired.push('answer'); return true; } });
      every(10, () => { fired.push('ordinary'); return true; }, true, 'ordinary');
      every(10, () => { fired.push('idle'); return true; }, true, 'idle', 4);
      clock.agent = !live; wall = 10;
      if (live) { rt.timerWait.check(); idle(); }
      else { advance(10, false); }
      expect(fired).toEqual(live ? ['ordinary', 'answer', 'idle'] : ['ordinary', 'idle', 'answer']);
      expect(clock.timerFirings).toEqual([1, 0, 0, 0, 1, 0, 0]);
    }
  });
});
