import { test, expect } from 'bun:test';
import { ScrollActivity, frameAtOrAfter, idleAllowed, cap } from './timer-waits.js';

const scroller = () => ({ scrollLeft: 0, scrollTop: 0, isConnected: true });

test('virtual frame boundaries include exact frames and correct adjacent binary64 values', () => {
  expect(frameAtOrAfter(Infinity)).toBe(Infinity);
  expect(frameAtOrAfter(NaN)).toBeNaN();
  for (const k of [1, 2, 3, 59, 60, 61, 1e6, 2 ** 32]) {
    const at = k * 1000 / 60;
    expect(frameAtOrAfter(at)).toBe(at);
    expect(frameAtOrAfter(at - at * Number.EPSILON / 2)).toBe(at);
    expect(frameAtOrAfter(at + at * Number.EPSILON)).toBeGreaterThan(at);
  }
});

test('caps are measured from nominal deadlines and preserve explicit endpoints', () => {
  expect(cap({ due: 250, ms: 250, wait: 1 })).toBe(350);
  expect(cap({ due: 250, ms: 250, wait: 2 })).toBe(350);
  expect(cap({ due: 250, ms: 250, wait: 3 })).toBe(Infinity);
  expect(cap({ due: 250, ms: 250, wait: 4 })).toBe(1250);
  expect(cap({ due: 5000, ms: 5000, wait: 4 })).toBe(10000);
  for (const max of [1, 4294967295]) {
    for (const wait of [3, 4]) { expect(cap({ due: 250, wait, max })).toBe(250 + max); }
  }
});

test('each idle term independently prevents admission and the horizon includes its endpoint', () => {
  expect(idleAllowed({}, 100)).toBe(true);
  for (const term of ['scroll', 'demand', 'applying', 'ordinary']) {
    expect(idleAllowed({ [term]: true }, 100)).toBe(false);
  }
  expect(idleAllowed({ next: 100 + 1000 / 60 }, 100)).toBe(false);
  expect(idleAllowed({ next: 100 + 1000 / 60 + 1e-12 }, 100)).toBe(true);
});

test('touch holds, movement and wheel quiet periods have independent lifetimes', () => {
  const activity = new ScrollActivity(), target = scroller();
  activity.hold('touch1', target); activity.hold('touch2', target);
  activity.moved(target, 0);
  activity.ended(target);
  expect(activity.sample(2000, true)).toBe(true);
  activity.lift('touch1'); expect(activity.sample(2000, true)).toBe(true);
  activity.lift('pointercancel'); expect(activity.sample(2000, true)).toBe(true);
  activity.lift('touch2'); expect(activity.sample(2000, true)).toBe(false);
  activity.moved(target, 3000); activity.hold('touch3', target); activity.lift('touch3');
  expect(activity.sample(3500, true)).toBe(true);
  expect(activity.sample(4000, true)).toBe(false);
  activity.wheelUntil = 5000;
  expect(activity.sample(4999, true)).toBe(true);
  expect(activity.sample(5000, true)).toBe(false);
});

test('quiet completion, removal and movement after a quiet pause are deterministic', () => {
  for (const supported of [false, true]) {
    const activity = new ScrollActivity(), target = scroller(), bound = supported ? 1000 : 100;
    activity.moved(target, 0);
    expect(activity.sample(bound - 1, supported)).toBe(true);
    expect(activity.sample(bound, supported)).toBe(false);
    target.scrollTop = 1; activity.moved(target, 1200);
    expect(activity.sample(1200, supported)).toBe(true);
    target.isConnected = false;
    expect(activity.sample(1201, supported)).toBe(false);
  }
});

test('observed positions reset quiet time and nested movement survives a child completion', () => {
  const activity = new ScrollActivity(), child = scroller(), parent = scroller();
  activity.moved(child, 0); activity.moved(parent, 0);
  parent.scrollTop = 1;
  expect(activity.sample(999, true)).toBe(true);
  activity.ended(child);
  expect(activity.sample(1998, true)).toBe(true);
  expect(activity.sample(1999, true)).toBe(false);
});
