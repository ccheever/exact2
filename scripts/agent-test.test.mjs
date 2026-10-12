// Authored expects over a tree (agent-test.mjs): a node the tree carries but the
// shipped app does not show — a header node the iOS navigation bar leaves out,
// which the agent's own chrome still paints — fails, and says why (LLP 1116 D2).
import { test, expect } from 'bun:test';
import { serverRequest, textExpectation, treeExpectation } from './agent-test.mjs';
import { axFindings } from './agent-ax.mjs';

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
  } finally { await server.stop(true); }
  await expect(serverRequest(base, { method: 'GET', path: '/items', body: null, as: null })).rejects.toThrow(/nothing answered/);
});
