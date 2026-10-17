import { test, expect } from 'bun:test';
import { transformDrags } from './transform.js';

test('passed-over transform targets are journaled once until no longer valid', () => {
  const views = new Map(), lines = [], bindings = new Map();
  const element = id => {
    const el = { id, valid: true, $tgeom() {}, $trelease() {} };
    views.set(id, el);
    return el;
  };
  const [clip, first, second, a, b, c, d] = [1, 2, 3, 4, 5, 6, 7].map(element);
  const oldFrame = globalThis.requestAnimationFrame;
  globalThis.requestAnimationFrame = fn => fn();
  try {
    const drag = transformDrags({
      w: {}, views, viewId: el => el.id, now: () => 0, inflight: { n: 0 },
      eligible: el => !!el?.valid, say: line => lines.push(line),
      api: { attachTransformDrag() {}, transformBinding: b => bindings.set(b.id, b.target) },
    });
    drag.attach(a, first, clip);
    drag.attach(b, first, clip);
    drag.attach(c, second, clip);
    drag.attach(d, second, clip);
    const report = 'transformDragFor: target view #3 passed over for owner view #2; one owner (first valid, never stolen; LLP 1071)';
    drag.reconcile();
    expect(lines).toEqual([report]);
    expect([...bindings.values()]).toEqual([2, 2, null, null]);
    drag.reconcile();
    expect(lines).toHaveLength(1);
    second.valid = false;
    drag.reconcile();
    expect(lines).toHaveLength(1);
    second.valid = true;
    drag.reconcile();
    expect(lines).toEqual([report, report]);
    drag.gone(c.id);
    drag.reconcile();
    expect(lines).toHaveLength(2);
    drag.gone(d.id);
    drag.reconcile();
    drag.attach(c, second, clip);
    drag.reconcile();
    expect(lines).toEqual([report, report, report]);
    first.valid = false;
    drag.reconcile();
    expect(bindings.get(c.id)).toBe(3);
    expect(lines).toHaveLength(3);
    first.valid = true;
    drag.reconcile();
    expect(bindings.get(a.id)).toBeNull();
    expect(lines.at(-1)).toBe('transformDragFor: target view #2 passed over for owner view #3; one owner (first valid, never stolen; LLP 1071)');
  } finally {
    globalThis.requestAnimationFrame = oldFrame;
  }
});
