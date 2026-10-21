// `state <name>` (agent-inspect.mjs `stateOp`, LLP 1116.007 D3.9): a state, derive or resource (or a field of the record)
// reads by name, down a dotted path, as `expect state` does, even where a view carries the name; a plain view's is the record.
import { test, expect } from 'bun:test';
import { render, renderTrace, stateOp, tapRefusal, nextCommitTasks, settleBound } from './agent-inspect.mjs';

test('log rendering skips canvas error rows and preserves journal order', () => {
  const error = { canvas: 7, error: 'journal unavailable' };
  const row = lines => ({ canvas: 7, lines });
  expect(render('logs', { lines: ['host entry'], world: [error, row(['first']), error, row(['second']), { canvas: 8, lines: null }] }))
    .toBe('host entry\nworld first\nworld second');
  expect(render('logs', { lines: [], world: [error] })).toBe('(nothing new)');
});

test('a screenshot says what it shows that the app does not on lines of their own', () => {
  const text = render('screenshot', { screenshot: 'a.png', w: 402, h: 874, painted: [{ id: 9, testId: 'menu' }], paintedNote: 'the agent painted it' });
  expect(text).toBe('screenshot a.png · 402×874 · painted=[{"id":9,"testId":"menu"}]\n  NOTE: the agent painted it');
});

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

test('state world events uses the world seam and an optional type filter', async () => {
  const calls = [], s = {state: async (...args) => { calls.push(args); return {events:[]}; }};
  await stateOp(s, ['world','events','Hit']);
  expect(calls[0][0]).toBe('world');
  expect(calls[0][4]).toEqual({world:true, events:true, event_type:'Hit'});
  await stateOp(s, ['world','events']);
  expect(calls[1][4]).toEqual({world:true, events:true});
});

// A target covered by what may leave on a timer the frozen clock never reaches (euxy FRICTION 9): the refusal names the
// tasks still waiting and the step that runs them; with none waiting it is as it was.
test('a covered target\'s refusal names the tasks waiting on the clock', async () => {
  const covered = (tasks, clock = 0) => tapRefusal({ state: async () => ({ clock, tasks }), tree: async () => ({ nodes: [] }) }, 'go', new Error('tap #333: node #6 covers its middle'));
  expect((await covered({ boot: 1, splash: 900, charge: null })).message).toBe('tap #333: node #6 covers its middle; the agent\'s clock stands at 0 ms, and the app\'s tasks wait on it (`boot` at 1 ms, `splash` at 900 ms): if what covers it leaves on a timer, `clock +900` runs them');
  expect((await covered({ splash: 900 }, 1000)).message).toBe('tap #333: node #6 covers its middle');
  expect((await covered({})).message).toBe('tap #333: node #6 covers its middle');
});

// A task due a commit later (an `after(1, …)` an answer armed, euxy FRICTION 7) is what `clock settle` runs and
// `clock data` names; one due later, one spent and one past are not.
test('the tasks due a commit later are those within 1 ms of the clock', async () => {
  const session = { state: async () => ({ tasks: { importLink: 1001, later: 1002, spent: null, past: 999, now: 1000, adopt: 1000.5 } }) };
  expect(await nextCommitTasks(session, 1000)).toEqual([['adopt', 1000.5], ['importLink', 1001]]);
  expect(await nextCommitTasks(session, 2000)).toEqual([]);
});

// Past its 16 rounds, `clock settle` is not settled while a task is still due a commit later, and names it.
test('a settle that runs out of rounds says which tasks are still due', () => {
  const r = settleBound([['chain', 17]]);
  expect([r.settled, r.reason, r.tasks]).toEqual([false, 'tasks', { chain: 17 }]);
  expect(r.diagnostic).toContain('`chain` is due again at 17 ms');
});
