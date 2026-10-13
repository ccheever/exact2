// Authored expects over a tree (agent-test.mjs): a node the tree carries but the
// shipped app does not show — a header node the iOS navigation bar leaves out,
// which the agent's own chrome still paints — fails, and says why (LLP 1116 D2).
import { test, expect } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { disabledExpectation, refuseDisabled, runTests, serverRequest, textExpectation, treeExpectation } from './agent-test.mjs';
import { axFindings } from './agent-ax.mjs';
import { withEpoch } from './agent-launch.mjs';
import { serveBuildTree } from '../host/web/serve.mjs';

test('a presented popover belongs to its AppKit modal for outside-modal findings', () => {
  const elements = [
    { i: 0, parent: null, id: null, via: 'none', role: 'scrollarea', name: '' },
    { i: 1, parent: 0, id: 10, via: 'self', role: 'dialog', name: 'Setup' },
    { i: 2, parent: 1, id: 11, via: 'self', role: 'button', name: 'Options' },
    { i: 3, parent: 0, id: 12, via: 'self', role: 'group', name: 'Options' },
    { i: 4, parent: 3, id: 13, via: 'self', role: 'textbox', name: 'Filter' },
  ];
  const modal = { present: true, by: 'aria-modal', element: 1, presented: [3] };
  expect(axFindings({ ax: { source: 'appkit', modal, elements } }).filter(f => f.kind === 'outside-modal')).toEqual([]);
  expect(axFindings({ ax: { source: 'appkit', modal: { ...modal, presented: [] }, elements } }).filter(f => f.kind === 'outside-modal').map(f => f.id)).toEqual([12, 13]);
});

const why = 'the iOS navigation bar does not show this node; in the app it is not visible';
const nodes = [
  { id: 3, depth: 0, type: 'View', props: {} },
  { id: 4, depth: 1, type: 'Text', props: { text: 'Shopping List' } },
  { id: 5, depth: 1, type: 'Text', props: { text: '2 items left', testId: 'count' } },
  { id: 6, depth: 1, type: 'Text', props: { text: 'Shared', testId: 'tagline' }, unshown: why },
];

test('a text the bar shows passes; one it leaves out fails naming why', () => {
  expect(textExpectation(nodes, 'count', '2 items left')).toBeNull();
  expect(textExpectation(nodes, 'count', '1 item left').unshown).toBeUndefined();
  const f = textExpectation(nodes, 'tagline', 'Shared');
  expect(f.unshown).toBe(true);
  expect(f.message).toBe(`text of "tagline" is "Shared", but ${why}`);
  // A wrong text is the ordinary failure, which a clock step may still settle.
  expect(textExpectation(nodes, 'tagline', 'Other').unshown).toBeUndefined();
});

test('present means shown in the app; missing is the tree\'s', () => {
  expect(treeExpectation(nodes, 'count', true)).toBeNull();
  expect(treeExpectation(nodes, 'tagline', true)).toEqual({ message: `testId "tagline" is in the tree, but ${why}`, unshown: true });
  expect(treeExpectation(nodes, 'gone', true).message).toBe('expected testId "gone" present, it was absent');
  expect(treeExpectation(nodes, 'gone', false)).toBeNull();
  expect(treeExpectation(nodes, 'tagline', false).message).toBe('expected testId "tagline" absent, it was present');
});

// LLP 1116.010 D3 (chat-rooms and password-generator, round 12): a disabled control is asserted, and pressing one fails the step.
test('expect disabled and enabled read the prop every host\'s tree carries; a press on a disabled control is refused, a hover is not', async () => {
  const tree = [{ id: 1, props: { testId: 'send', disabled: true } }, { id: 2, props: { testId: 'arm' } }, { id: 3, props: { testId: 'native', disabled: 'true' } }, { id: 4, props: { testId: 'update', accessibilityDisabled: true } }];
  expect(disabledExpectation(tree, 'send', true)).toBeNull();
  expect(disabledExpectation(tree, 'native', true)).toBeNull();
  // `aria-disabled` reads as disabled for the expectation…
  expect(disabledExpectation(tree, 'update', true)).toBeNull();
  expect(disabledExpectation(tree, 'update', false).message).toBe('expected "update" enabled, it was disabled');
  expect(disabledExpectation(tree, 'arm', false)).toBeNull();
  expect(disabledExpectation(tree, 'send', false).message).toBe('expected "send" enabled, it was disabled');
  expect(disabledExpectation(tree, 'arm', true).message).toBe('expected "arm" disabled, it was enabled');
  expect(disabledExpectation(tree, 'gone', true).message).toBe('expected "gone" disabled, no view carries that testId');
  const s = { op: async (req) => ({ nodes: tree.filter((n) => n.id === req.target) }) };
  await expect(refuseDisabled(s, { id: 1 }, 'send', {})).rejects.toThrow('send is disabled: a press does nothing');
  await expect(refuseDisabled(s, { id: 3 }, 'native', { dblclick: true })).rejects.toThrow('native is disabled');
  await refuseDisabled(s, { id: 2 }, 'arm', {});
  // …but its press still dispatches, so the tap is not refused.
  await refuseDisabled(s, { id: 4 }, 'update', {});
  await refuseDisabled(s, { id: 1 }, 'send', { hover: true });
  await refuseDisabled(s, { id: 1, entity: 'unit' }, 'send', {});
});

test('an active screen\'s node is read before a covered one\'s', () => {
  const covered = [{ id: 9, depth: 0, type: 'Text', props: { text: 'x', testId: 'count' }, inactive: true, unshown: why }, ...nodes];
  expect(textExpectation(covered, 'count', '2 items left')).toBeNull();
});

// LLP 1116.006 D8: a test's `request` line is the driver calling the app's server, the persona's name its bearer token.
test('a request line sends the method, the JSON body and the persona\'s token under the server URL; a refusal says its status and body', async () => {
  const seen = [];
  const server = Bun.serve({ port: 0, hostname: '127.0.0.1', async fetch(req) {
    const url = new URL(req.url);
    seen.push([req.method, url.pathname, req.headers.get('authorization'), req.headers.get('content-type'), await req.text()]);
    return url.pathname === '/refused' ? Response.json({ error: 'Sign in again.' }, { status: 401 }) : url.pathname === '/gone' ? new Response(null, { status: 404 }) : Response.json({ ok: true }, { status: 201 });
  } });
  const base = `http://127.0.0.1:${server.port}`;
  try {
    await serverRequest(base, { method: 'POST', path: '/polls/1/votes', body: '{"option":2}', as: 'bob' });
    await serverRequest(base, { method: 'GET', path: `${base}/health`, body: null, as: null });
    expect(seen).toEqual([['POST', '/polls/1/votes', 'Bearer bob', 'application/json', '{"option":2}'], ['GET', '/health', null, null, '']]);
    await expect(serverRequest(base, { method: 'PATCH', path: '/refused', body: '{}', as: 'mallory' })).rejects.toThrow(`request PATCH ${base}/refused as mallory: the server answered 401 {"error":"Sign in again."}`);
    await expect(serverRequest(base, { method: 'DELETE', path: '/gone', body: null, as: 'bob' })).rejects.toThrow(`request DELETE ${base}/gone as bob: the server answered 404`);
    await expect(serverRequest(null, { method: 'GET', path: '/items', body: null, as: null })).rejects.toThrow('app.ts calls no local server');
    // LLP 1116.010 D3 (polls, round 12): `expect <status>` passes on that status and fails on any other, a success included.
    await serverRequest(base, { method: 'DELETE', path: '/gone', body: null, as: 'bob', expect: 404 });
    await serverRequest(base, { method: 'PATCH', path: '/refused', body: '{}', as: 'mallory', expect: 401 });
    await expect(serverRequest(base, { method: 'DELETE', path: '/gone', body: null, as: 'bob', expect: 409 })).rejects.toThrow(`request DELETE ${base}/gone as bob: the server answered 404, expected 409`);
    await expect(serverRequest(base, { method: 'POST', path: '/items', body: '{}', as: 'bob', expect: 409 })).rejects.toThrow('the server answered 201 {"ok":true}, expected 409');
  } finally { await server.stop(true); }
  await expect(serverRequest(base, { method: 'GET', path: '/items', body: null, as: null })).rejects.toThrow(/nothing answered/);
});

// LLP 1116.009 D1: a `reload` relaunches at the virtual time the test had reached (the date it launched at plus every
// `clock +N`), not at its launch date: a phone restarted at 9:05 does not wake at 9:00 (habit-tracker, rounds 10 and 11).
test('the page address a reload loads takes the date it relaunches at, and keeps the rest', () => {
  const href = 'http://127.0.0.1:9/detail/3?agent=1&seed=1&epoch=1773489600000&timeZone=UTC&q=a';
  expect(new URL(withEpoch(href, 1773576000000)).searchParams.get('epoch')).toBe('1773576000000');
  expect(new URL(withEpoch(href, 1773576000000)).searchParams.get('q')).toBe('a');
  expect(new URL(withEpoch('http://127.0.0.1:9/', 5)).searchParams.get('epoch')).toBe('5');
  expect(withEpoch(href, undefined)).toBe(href);
});

test('a reload keeps the clock the test had reached, again and again, and the run ends with it', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-reload-clock-')), contract = join(dir, 'app.contract'), plan = join(dir, 'app.plan'), dist = join(dir, 'dist'), file = join(dir, 'app.test.contract');
  writeFileSync(contract, `shape Time
  epochAtZero: number
  utcOffset: number

component Day
  state now = 0
  resource time = exactTime() as shape Time
  derive day = formatDate(time.epochAtZero + now, time.utcOffset, "iso")
  task tick mount
    every(60000, step)
  action step
    now = performanceNow()
  view
    column testId="root"
      text day testId="day"
`);
  writeFileSync(file, `test "a relaunch is on the day the test reached"
  epoch "2026-03-14T12:00:00Z"
  expect text "day" == "2026-03-14"
  clock +86400000
  clock data
  expect text "day" == "2026-03-15"
  reload
  expect text "day" == "2026-03-15"
  clock +86400000
  clock data
  expect text "day" == "2026-03-16"
  reload
  expect text "day" == "2026-03-16"
`);
  const root = new URL('..', import.meta.url).pathname;
  const compile = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'build', contract, '-o', plan], { cwd: root, encoding: 'utf8' });
  expect(compile.status, compile.stderr).toBe(0);
  const build = spawnSync(process.execPath, ['host/web-js/build.mjs', 'caltrain', '--plan', plan, '--out', dist, '--render', 'none'], { cwd: root, encoding: 'utf8' });
  expect(build.status, build.stderr).toBe(0);
  const server = createServer((request, response) => serveBuildTree(dist, request, response));
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const ends = [];
  try {
    const r = await runTests({ host: 'web', file, app: 'caltrain', url: `http://127.0.0.1:${server.address().port}/`, size: [420, 900], atEnd: async (s, t) => { ends.push([t.launch.epoch, t.reached]); return {}; } });
    expect(r.results.map((x) => x.failures)).toEqual([[]]);
    expect(r.passed).toBe(1);
    // verify's relaunch starts where the test left off too: the test names the date it launched at and the date it had reached.
    expect(ends).toEqual([[Date.UTC(2026, 2, 14, 12), Date.UTC(2026, 2, 16, 12)]]);
  } finally {
    server.close();
    rmSync(dir, { recursive: true, force: true });
  }
}, 180_000);
