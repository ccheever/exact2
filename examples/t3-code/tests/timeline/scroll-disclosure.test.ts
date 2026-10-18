import { expect, test } from 'bun:test';
import { contractComponent } from '../support/contract-action-fixture';

const root = contractComponent('app.contract', 'T3Code');
for (const op of ['fold', 'group', 'attempt', 'tree-all', 'tree-dir']) test(`${op} preserves end following, and holds the reader's position away from the end`, () => {
  for (const [atEnd, owner, held] of [[true, 'thread', false], [false, 'thread', true], [true, 'old-thread', true]] as const) {
    const state = { foldHold: false, rightPanelAt: '' };
    const calls: unknown[][] = [];
    root.call('chatLocal', state, { restAtEnd: atEnd, restOwner: owner, composerOwner: 'thread', command: (...args: unknown[]) => calls.push(args) }, [op, 'item', '']);
    expect(state.foldHold).toBe(held);
    expect(calls).toEqual([[`chatlocal:${op}`, 'item', '', 0]]);
  }
});

const column = contractComponent('app-main.contract', 'ChatColumn');
function wheel() {
  const state = { wheelTop: 0, wheelJump: -1, wheelPending: false };
  const calls: unknown[][] = [];
  const apply = (deltaY: number, deltaMode = 0, ctrlKey = false) => column.call('minimapWheel', state, {
    min: Math.min, max: Math.max, jumpRequest: 1, scrolledTop: 500, scrolledMax: 2000, scrolledPort: 600,
    listTop: state.wheelJump === 1 ? state.wheelTop : 1000001,
    citeEnd: () => calls.push(['citeEnd']), restGesture: (toward: boolean) => calls.push(['restGesture', toward]),
    preventDefault: () => calls.push(['preventDefault']),
  }, [{ deltaY, deltaMode, ctrlKey }]);
  return { state, calls, apply };
}

test('minimap wheels use pixels, sixteen-pixel lines and viewport pages, clamped to the transcript', () => {
  for (const [delta, mode, top] of [[-120, 0, 380], [-3, 1, 452], [1, 2, 1100], [-5, 2, 0], [5, 2, 2000]]) {
    const w = wheel(); w.apply(delta!, mode!);
    expect(w.state).toEqual({ wheelTop: top, wheelJump: 1, wheelPending: true });
    expect(w.calls).toEqual([['preventDefault'], ['citeEnd'], ['restGesture', delta! > 0]]);
  }
  const pending = wheel(); pending.apply(30); pending.apply(40);
  expect(pending.state.wheelTop).toBe(570);
});

test('horizontal-only wheels and pinch gestures retain their default behavior', () => {
  for (const [delta, ctrl] of [[0, false], [100, true]] as const) {
    const w = wheel(); w.apply(delta, 0, ctrl);
    expect(w.calls).toEqual([]);
    expect(w.state).toEqual({ wheelTop: 0, wheelJump: -1, wheelPending: false });
  }
});
