// The sidebar's row-action sweep (upstream 1826fb55cc) starts fresh each time, as the
// reference's does: Sidebar.tsx `startActionSweep` (T3 Code 1e2ecbd975, MIT, see LICENSE-T3) makes a new closure per
// press (`targetKey = null`, `sweptKeys = []`, the pressed row and action) and each move resolves the armed rows from
// the pointer's current position (`sweepTo(sidebarThreadKeyAtY(list, y))`).
//
// This runs ThreadSection's own `sweep` and `cancelSweep` actions from sidebar.contract with Contract's action
// semantics (contract-action-fixture.ts: a read sees the value from before the action, a `let` is visible after its
// line, the last write to a slot wins). Main's `analyze-read-after-assign` (LLP 1116.003 D6) found the old body reading
// what its start branch had just reset: a sweep after an earlier one kept that sweep's offset, origin, verb, armed rows
// and cancel to its release, and every move armed the rows of the move before.
import { describe, expect, test } from 'bun:test';
import { contractComponent } from '../support/contract-action-fixture';

const threadSection = contractComponent('sidebar.contract', 'ThreadSection');

// Four settleable rows 28 pt tall, 30 pt apart, in a 200-pt list at the top; each row's action button is 20 pt tall,
// 4 pt below the row's top, so its centre (where a sweep starts) is the row's top + 14.
const rows = ['a', 'b', 'c', 'd'].map(id => ({ id, canSettle: true, canUnsettle: false, canWake: false, hoverKey: `|${id}|` }));
const frames: Record<string, { y: number; height: number }> = { 'thread-list': { y: 0, height: 200 } };
rows.forEach(({ id }, i) => { frames[`row-${id}`] = { y: 30 * i, height: 28 }; frames[`sweep-${id}`] = { y: 30 * i + 4, height: 20 }; });

function section() {
  const state = threadSection.state();
  const data = { sidebar: { sweepEpoch: 0 } };
  // The window's hover: its id and count, and each call of the window's sidebarHover
  // (`hoverCard`; since main #478 the sweep calls it, #468's called the root's half).
  const unhovered: [string, boolean][] = [];
  const hover = { hoverId: '|a|', hoverSeq: 0, hoverCard: (id: string, over: boolean) => { unhovered.push([id, over]); } };
  const scope = {
    min: Math.min, max: Math.max,
    frame: (id: string) => { const f = frames[id]; if (!f) throw new Error(`no frame ${id}`); return f; },
    rows, data, scrollY: 0, ...hover,
  };
  const call = (name: string, args: unknown[]) => threadSection.call(name, state, scope, args);
  return {
    state,
    unhovered,
    move: (id: string, dy: number, verb = 'settle', dx = 0) => call('sweep', [id, verb, dx, dy]),
    cancel: () => call('cancelSweep', []),
    // The release is a `sweep|…` drop; the client bumps sweepEpoch when it lands, which ends the sweep.
    release: () => { data.sidebar.sweepEpoch++; },
    get at() { return [state.sweepDy, state.sweepY, state.sweepAt, state.sweepKeys]; },
  };
}

describe('a sidebar row-action sweep starts fresh (sidebar.contract ThreadSection.sweep)', () => {
  test('each move arms the rows between the pressed row and the pointer as it is now', () => {
    const s = section();
    s.move('a', 2);
    expect(s.at).toEqual([2, 14, 16, '']); // under the 6-pt distance: still the button's click
    s.move('a', 8);
    expect(s.at).toEqual([10, 14, 24, '|a|']);
    s.move('a', 10);
    s.move('a', 10);
    expect(s.at).toEqual([30, 14, 44, '|a|b|']);
  });

  test('a sweep on another row after a release takes nothing from the last one', () => {
    const s = section();
    for (let i = 0; i < 3; i++) s.move('a', 10);
    s.release();
    s.move('c', 3);
    // The old body gave [33, 74, 44, '|a|b|']: the last sweep's offset and point, and its rows armed at once.
    expect(s.at).toEqual([3, 74, 77, '']);
    expect([s.state.sweepId, s.state.sweepVerb, s.state.sweepTop]).toEqual(['c', 'settle', 60]);
    s.move('c', 3);
    expect(s.at).toEqual([6, 74, 80, '|c|']);
  });

  test('a sweep on the same row after a release, and one after an Escape, start from the press', () => {
    const s = section();
    s.move('b', 10, 'unsettle');
    s.move('b', 10, 'unsettle');
    s.release();
    s.move('b', 10);
    expect([s.state.sweepVerb, ...s.at]).toEqual(['settle', 10, 44, 54, '|b|']);
    s.cancel();
    expect([s.state.sweepCancelled, s.state.sweepKeys]).toEqual([true, '']);
    s.move('b', 10);
    expect(s.state.sweepKeys).toBe(''); // a cancelled sweep stays cancelled to its release
    s.release();
    s.move('c', -10);
    expect([s.state.sweepCancelled, ...s.at]).toEqual([false, -10, 74, 64, '|c|']);
    s.move('c', -25);
    expect(s.at).toEqual([-35, 74, 39, '|b|c|']);
  });

  test('each sweep\'s first arming closes the window\'s hover once, a fresh sweep after a release included', () => {
    // SidebarPointerSensor's onStart, as the list takes `**:pointer-events-none`.
    const s = section();
    s.move('a', 4);
    expect(s.unhovered).toEqual([]); // under the 6-pt distance: still the button's click
    s.move('a', 4);
    s.move('a', 10);
    expect(s.unhovered).toEqual([['|a|', false]]);
    s.release();
    s.move('a', 10); // the same row's next sweep arms at once from its own press
    s.move('a', 10);
    expect(s.unhovered).toEqual([['|a|', false], ['|a|', false]]);
    s.cancel();
    s.move('a', 10);
    expect(s.unhovered.length).toBe(2); // a cancelled sweep arms nothing more
  });
});
