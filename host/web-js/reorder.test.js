import { test, expect, mock } from 'bun:test';
import { arrangeController } from '../web/motion-glue.js';

test('idle Arrange reconciliation and reset tolerate a collection controller that has not loaded', () => {
  let controller = null;
  const collections = new Proxy({}, { get: (_, key) => controller?.[key] });
  const arrange = arrangeController({ views: new Map(), collections });
  expect(() => arrange.commit()).not.toThrow();
  expect(() => arrange.reset()).not.toThrow();
  controller = { reporting: () => false };
  expect(() => arrange.commit()).not.toThrow();
  expect(() => arrange.reset()).not.toThrow();
});

const clock = { now: 0, timers: [] };
mock.module('./rt.js', () => ({ clock, journal: [], commit(action, what, finished) { const result = action(); finished?.(true); return result; }, every(ms, action) { clock.timers.push({ due: clock.now + ms, action }); } }));
mock.module('./navigation.js', () => import('../web/navigation.js'));
const { install } = await import('./reorder.js');

test('grouped hold starts at the packet release time after an idle page', () => {
  const binding = { handle: 1, list: 2, wrapper: 3, root: 4, rowEpoch: 5 };
  const geometry = { list: 2, revision: 1, scrollSequence: 1, scrollTop: 0, portWidth: 100, portHeight: 100, rowWidth: 100, totalExtent: 20 };
  const list = { hasAttribute: () => false, $reorderdrop() {}, dataset: { reordergroup: 'cards' }, id: 'cards', isConnected: true };
  const grip = { hasAttribute: () => false, parentElement: list, dataset: { reorderfor: 'cards' }, isConnected: true, closest: () => null, getClientRects: () => [1] };
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

test('a pending drop with unchanged order does not scan keys', () => {
  class Collection {}
  install({ internals: { Collection, Lists: new Map(), Views: new Map() } }, {}, []);
  const c = new Collection(), order = Array.from({length:10_000}, (_, i) => `s:${i}`);
  let scans = 0;
  order.indexOf = function (key) { scans++; return Array.prototype.indexOf.call(this, key); };
  c.index = { order, pos:new Map(order.map((key, i) => [key, i])) };
  c.preview = {source:'s:9990', before:'s:9991', dropped:true, holding:true};
  const ordinary = {}, drop = {source:'s:9990', before:'s:9991', anchor:{order}};
  expect(c.committedAnchor(ordinary, drop)).toBe(ordinary);
  expect(scans).toBe(0);
});
