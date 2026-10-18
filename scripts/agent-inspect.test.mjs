// `state <name>` (agent-inspect.mjs `stateOp`, LLP 1116.007 D3.9): a state, derive or resource (or a field of the record)
// reads by name, down a dotted path, as `expect state` does, even where a view carries the name; a plain view's is the record.
import { test, expect } from 'bun:test';
import { renderTrace, stateOp } from './agent-inspect.mjs';

const record = { slots: { liked: true, draft: '' }, derives: { left: 2 }, resources: { groceries: [{ title: 'Eggs', done: false }, { title: 'Milk', done: true }], weather: { temp: 21 } }, mediaSession: { owner: null }, transport: { state: 'stopped' }, clock: 0 };
// A session whose views are `like` and `transport` (testIds); asking `state` of anything else is refused as a missing view, as
// the driver's is. A view's answer is the whole record, as every host's is for a view that is not a world.
const session = () => {
  const asked = [];
  return { asked, state: async (target) => {
    asked.push(target);
    if (target != null && target !== 'like' && target !== 'transport') throw new Error(`no view matches ${target} by testId, label or text; targets here: like, transport`);
    return record;
  } };
};

test('a view named like a section of the record reads that section, once', async () => {
  const s = session();
  expect(await stateOp(s, ['transport'])).toEqual({ state: 'stopped' }); // a sequencer's `transport` bar: 142 KB was the record
  expect(s.asked).toEqual(['transport']);
  expect(await stateOp(session(), ['transport.state'])).toBe('stopped');
});

test('a world\'s view keeps its own answer: it has no slots', async () => {
  const world = { tick: 3, entities: [{ name: 'transport' }] };
  expect(await stateOp({ state: async () => world }, ['transport'])).toBe(world);
});

test('a view keeps `state <testId>`; a state, derive, resource or field of the record reads by name and dotted path', async () => {
  expect(await stateOp(session(), [])).toBe(record);
  expect(await stateOp(session(), ['like'])).toBe(record); // a view: the host's own answer
  expect(await stateOp(session(), ['liked'])).toBe(true);
  expect(await stateOp(session(), ['left'])).toBe(2);
  expect(await stateOp(session(), ['weather'])).toEqual({ temp: 21 });
  expect(await stateOp(session(), ['weather.temp'])).toBe(21);
  expect(await stateOp(session(), ['groceries.1.done'])).toBe(true);
  expect(await stateOp(session(), ['mediaSession'])).toEqual({ owner: null });
  const s = session();
  await stateOp(s, ['like']);
  expect(s.asked).toEqual(['like']); // a view is read once, not looked up as a name
});

test('a name that is nothing is refused with the view\'s refusal, and what the state holds; a missing field says what is there', async () => {
  await expect(stateOp(session(), ['nope'])).rejects.toThrow(/^no view matches nope .*; nor is it a state, derive, resource or field of `state`: groceries, weather, left, liked, draft/);
  await expect(stateOp(session(), ['groceries.0.price'])).rejects.toThrow('state groceries.0 has no field "price" (its fields: title, done)');
  await expect(stateOp(session(), ['liked.x'])).rejects.toThrow('state liked has no field "x" (it is true)');
  await expect(stateOp(session(), ['groceries.7'])).rejects.toThrow('state groceries has no field "7"');
});

test('a release build\'s first trace says its frames were not measured yet, and keeps its sites', () => {
  const frames = { unmeasured: 'a release build measures frames from the first agent request or trace; this is the first',
    period: { ms: null, source: 'target' }, lifetime: { presented: 0, late: 0, missed: 0, overruns: 0, segments: 0 }, window: {}, late: [], records: [] };
  const text = renderTrace({ identity: { host: 'ios', app: 'euxy' }, frames, journal: { lines: [] }, perf: { seq: 1, clock: 0, incarnation: 1, sites: [] } });
  expect(text).toContain(`frames not measured: ${frames.unmeasured}`);
  expect(text).not.toContain('presented 0');
});
