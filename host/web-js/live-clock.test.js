// The JS target's timers on the page's clock (the timers RFC, D2, D3): a repeat that missed beats fires once,
// observing when it runs, and its next beat is the first of its phase after its callback ends; on the agent's
// seekable clock every beat fires at its own time. Run alone, `bun test ./host/web-js/live-clock.test.js`:
// reorder.test.js mocks rt.js for its process.
import { test, expect, mock, beforeEach, afterAll } from "bun:test";

// The build copies navigation.js and prompt-glue.js in from host/web and writes names.js.
mock.module("./navigation.js", () => import("../web/navigation.js"));
mock.module("./prompt-glue.js", () => ({}));
mock.module("./names.js", () => ({ sourceTypes: {} }));
const { clock, every, advance, armedAt } = await import("./rt.js");
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
