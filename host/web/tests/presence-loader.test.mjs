import { test, expect } from 'bun:test';
import { presenceLoader } from '../navigation.js';

test('a failed presence load releases held and later batches once, in order', async () => {
  let fail, calls = 0;
  const applied = [], journal = [];
  const load = () => { calls++; return new Promise((_, reject) => { fail = reject; }); };
  const apply = batch => { if (!presence.hold(batch)) applied.push(batch); };
  const presence = presenceLoader(load, {}, apply, line => journal.push(line));
  const exit = { ops: [{ op: 'exit', id: 1, css: 'leave 200ms' }, { op: 'destroy', id: 1 }] };
  const update = { ops: [{ op: 'style', id: 2, css: '--exact-layout-transition:200 0 linear' }] };
  apply(exit);
  apply(update);
  expect(applied).toEqual([]);
  fail(new Error('offline'));
  await new Promise(resolve => setTimeout(resolve, 0));
  apply(exit);
  apply(update);
  expect(applied).toEqual([exit, update, exit, update]);
  expect(calls).toBe(1);
  expect(presence.live).toBeNull();
  expect(journal).toHaveLength(1);
  expect(journal[0]).toContain('offline');
});

test('a loaded presence module releases the original batch order', async () => {
  let ready;
  const applied = [], live = {};
  const apply = batch => { if (!presence.hold(batch)) applied.push(batch); };
  const presence = presenceLoader(() => new Promise(resolve => { ready = resolve; }), {}, apply);
  const batches = [{ ops: [{ op: 'exit', id: 1 }] }, { ops: [{ op: 'destroy', id: 1 }] }, { ops: [] }];
  for (const batch of batches) apply(batch);
  ready(() => live);
  await new Promise(resolve => setTimeout(resolve, 0));
  expect(applied).toEqual(batches);
  expect(presence.live).toBe(live);
});
