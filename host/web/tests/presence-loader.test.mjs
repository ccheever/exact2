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
  const batches = [{ ops: [{ op: 'exit', id: 1 }] }, presence.resize({ ops: [{ op: 'destroy', id: 1 }] }), { ops: [] }];
  for (const batch of batches) apply(batch);
  ready(() => live);
  await new Promise(resolve => setTimeout(resolve, 0));
  expect(applied).toEqual(batches);
  expect(presence.live).toBe(live);
  expect(applied[1].presenceSnap).toBe(true);
});

// A plan that names a presence row loads the module after first paint (`preload`, glue.js after boot), before any batch
// carries the row: a view whose bound layout transition declares nothing at boot then moves from where it was when it
// first gains one. The agent waits for the scheduled load and the load under way (`pending`), and for none once it lands
// or fails; a failed module is not fetched again by a later boot's preload.
test('preload schedules the one load after paint, which pending names until it lands', async () => {
  const raf = globalThis.requestAnimationFrame;
  globalThis.requestAnimationFrame = f => setTimeout(f, 0);
  try {
    let ready, calls = 0;
    const live = {};
    const presence = presenceLoader(() => { calls++; return new Promise(resolve => { ready = resolve; }); }, {}, () => {});
    expect(presence.pending()).toBeNull();
    presence.preload();
    presence.preload();
    expect(calls).toBe(0); // not before paint
    const waiting = presence.pending();
    expect(waiting).toBeInstanceOf(Promise);
    await new Promise(r => setTimeout(r, 5));
    expect(calls).toBe(1);
    expect(presence.hold({ ops: [{ op: 'create', id: 1, css: '--exact-layout-transition:200 0 linear' }] })).toBe(false);
    ready(() => live);
    await waiting;
    expect([presence.live, presence.pending(), calls]).toEqual([live, null, 1]);
    // A failed one stays failed.
    let fail; calls = 0;
    const failed = presenceLoader(() => { calls++; return new Promise((_, reject) => { fail = reject; }); }, {}, () => {}, () => {});
    failed.preload();
    await new Promise(r => setTimeout(r, 5));
    fail(new Error('offline'));
    await new Promise(r => setTimeout(r, 0));
    failed.preload();
    await new Promise(r => setTimeout(r, 5));
    expect([failed.pending(), calls]).toEqual([null, 1]);
  } finally { globalThis.requestAnimationFrame = raf; }
});

// Only the two presence rows, or an exit, fetch the module: another custom property (press feedback, a tint) does not.
test('a batch with another custom property fetches nothing', () => {
  let calls = 0;
  const presence = presenceLoader(() => { calls++; return new Promise(() => {}); }, {}, () => {});
  expect(presence.hold({ ops: [{ op: 'style', id: 1, css: '--exact-press: 0.96; --exact-tint: red' }] })).toBe(false);
  expect(calls).toBe(0);
  presence.hold({ ops: [{ op: 'style', id: 1, css: '--exact-exit-animation: leave 200ms' }] });
  expect(calls).toBe(1);
});
