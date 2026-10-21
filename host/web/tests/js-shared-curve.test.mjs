// The JS target's shared-element curve (host/web-js/shared.js `curve`) reads the
// `--exact-layout-transition` row the web host writes (css.rs
// `layout_transition_css`): a spring there is the private `spring(k, c, m)`
// (LLP 1081 D5 keeps it), whose commas must not split it, lowered to `linear()`.
import { test, expect } from 'bun:test';
import { curve } from '../../web-js/shared.js';

const row = text => ({ style: { getPropertyValue: name => (name === '--exact-layout-transition' ? text : '') } });

test('a spring row lowers to a linear() curve over its settle time', () => {
  const c = curve(row('0 100 spring(300, 30, 1)'));
  expect(c.delay).toBe(100);
  expect(c.timing).toMatch(/^linear\(0, .*, 1\)$/);
  expect(c.duration).toBeGreaterThan(0);
});

test('an eased row keeps its duration, delay and timing function', () => {
  expect(curve(row('300 50 cubic-bezier(0.4, 0, 0.2, 1)'))).toEqual({ duration: 300, delay: 50, timing: 'cubic-bezier(0.4, 0, 0.2, 1)' });
  expect(curve(row('none'))).toBe(null);
});
