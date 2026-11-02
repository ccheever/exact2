import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { formControlsSmoke } from './smoke-inspect.mjs';

test('the form-control smoke waits for a chooser that presents after the tap returns', async () => {
  const failures = [], pictures = [];
  await formControlsSmoke({ host: 'ios', check: (ok, why) => { if (!ok) failures.push(why); return ok; },
    open: async ({ touch }) => {
      const slots = {};
      let tapped = false, reads = 0;
      return {
        tap: async () => { tapped = true; return touch ? { delivery: 'platform' } : { delivery: 'unsupported', reason: 'use --touch platform' }; },
        type: async (target, value) => {
          if (value === 'unavailable') throw new Error('disabled');
          slots[target] = value;
        },
        state: async () => ({ slots }),
        screenshot: async () => {
          if (!tapped) return {};
          pictures.push(++reads);
          return reads < 3 ? {} : { presented: 'popover' };
        },
        close: async () => {},
      };
    } });
  assert.deepEqual(failures, []);
  assert.deepEqual(pictures, [1, 2, 3, 1, 2, 3]);
});
