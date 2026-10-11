import { test, expect, mock } from 'bun:test';

const clock = { now: 0, timers: [] };
mock.module('./rt.js', () => ({ clock, journal: [], commit: action => action(), every(ms, action) { clock.timers.push({ due: clock.now + ms, action }); } }));
const { install } = await import('./reorder.js');

test('grouped hold starts at the packet release time after an idle page', () => {
  const binding = { handle: 1, list: 2, wrapper: 3, root: 4, rowEpoch: 5 };
  const geometry = { list: 2, revision: 1, scrollSequence: 1, scrollTop: 0, portWidth: 100, portHeight: 100, rowWidth: 100, totalExtent: 20 };
  const list = { $reorderdrop() {}, dataset: { reordergroup: 'cards' }, id: 'cards', isConnected: true };
  const grip = { $reorderList: list, isConnected: true, closest: () => null, getClientRects: () => [1] };
  class Collection {}
  const c = new Collection();
  Object.assign(c, {
    view: 2, el: list, reorderGeometry: () => geometry, reorderBinding: () => binding,
    beginPreview(binding, token) { this.preview = { binding, token, pinOwned: true, terminal: false, height: 20 }; return true; },
    previewItem: () => 's:a', setHidden() {}, holdDrop: () => ['a', null], placeOf: () => [null, null],
  });
  list.$list = c;
  const timers = clock.timers.length, previous = clock.now;
  clock.now = 0;
  try {
    const reorder = install({ internals: { Collection, Lists: new Map([[2, c]]), Views: new Map([[1, grip]]), settled() {}, controller() {} } }, {}, []);
    const token = reorder.beginGroup(binding, geometry, true).token;
    expect(reorder.dropGroup(token, geometry, NaN)).toBe(false);
    expect(clock.now).toBe(0);
    expect(reorder.dropGroup(token, geometry, 3000)).toBe(true);
    expect(reorder.session().deadline).toBe(4000);
    expect(clock.timers.at(-1).due).toBe(4000);
  } finally {
    clock.timers.splice(timers);
    clock.now = previous;
  }
});
