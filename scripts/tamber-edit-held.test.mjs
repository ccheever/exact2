// `edit-held` (tamber/1.17; scripts/tamber-edit-held.mjs): one value of a held proposal changed, on the real server
// (`contract tamber serve`): a value and a `when` branch's, `items` kept and the review renumbered, the old one replaced
// in one step (`proposal-dropped {reason: 'edited', replacedBy, editedBy}`), the original client's still, a 3,000-op
// held import edited with no ops sent, every refusal, another client's needing `write`, an edit racing an accept, and
// a failed plan putting the old one back.
import { afterAll, beforeAll, expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { developmentGate } from './install-page.mjs';
import { edited } from './tamber-edit-held.mjs';
import { needs } from './tamber-pairing.mjs';
import { tamberServer } from './tamber-server.mjs';

const DESIGN = 'tamber t\n\naxes\n  density = normal | compact\n\ntheme\n  ink = #000000\n    when density=compact: #111111\n  edge = #101010\n';

let dir, server, tamber, origin, headers;
beforeAll(async () => {
  dir = mkdtempSync(join(tmpdir(), 'exact-edit-held-'));
  writeFileSync(join(dir, 'app.contract'), 'component App\n  view\n    text "a"\n');
  writeFileSync(join(dir, 'app.tamber'), DESIGN);
  const git = (...args) => spawnSync('git', ['-c', 'user.email=t@example.test', '-c', 'user.name=t', ...args], { cwd: dir });
  git('init', '-q'); git('add', '.'); git('commit', '-q', '-m', 'design');
  server = createServer();
  await new Promise(ok => server.listen(0, '127.0.0.1', ok));
  const port = server.address().port;
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  tamber = tamberServer({ appDir: dir, log: () => {}, settle: 250 });
  server.on('request', async (req, res) => {
    if (!(await tamber.handle(req, res, new URL(req.url, 'http://x'), gate.check(req)))) { res.writeHead(404); res.end(); }
  });
  origin = `http://127.0.0.1:${port}`;
  const { token } = await (await fetch(`${origin}/__tamber/token`)).json();
  headers = { 'content-type': 'application/json', 'x-exact-tamber-token': token };
  await tamber.ready;
}, 600_000);
afterAll(async () => {
  await tamber?.close();
  server?.close();
  if (dir) rmSync(dir, { recursive: true, force: true });
});

const post = async request => {
  const res = await fetch(`${origin}/__tamber`, { method: 'POST', headers, body: JSON.stringify(request) });
  return { status: res.status, ...(await res.json()) };
};
/** The change stream's events as they come. */
async function subscribe() {
  const res = await fetch(`${origin}/__tamber/subscribe`, { method: 'POST', headers });
  const reader = res.body.getReader(), decoder = new TextDecoder(), events = [];
  let buf = '';
  (async () => { for (;;) { const { value, done } = await reader.read().catch(() => ({ done: true })); if (done) return; buf += decoder.decode(value, { stream: true }); const parts = buf.split('\n\n'); buf = parts.pop(); for (const part of parts) if (part.startsWith('data: ')) events.push(JSON.parse(part.slice(6))); } })();
  const until = async (pass, ms = 5000) => { const end = Date.now() + ms; while (!pass(events)) { if (Date.now() > end) throw new Error(`no such event in ${JSON.stringify(events)}`); await Bun.sleep(20); } };
  return { events, until, close: () => reader.cancel() };
}
const heldList = async () => (await (await fetch(`${origin}/__tamber/events`, { method: 'POST', headers, body: JSON.stringify({ after: 0, wait: false, client: 'tool' }) })).json()).proposals;

test('a value and a when branch edited: held again in its place, the original client\'s, the review kept and renumbered', async () => {
  const stream = await subscribe();
  const review = { importer: 'dtcg', tokens: 2, offers: [{ role: 'AccentColor', token: 'edge', items: [1] }] };
  const held = await post({ method: 'propose', hold: true, client: 'agent:codex', ops: [
    { op: 'set-token', name: 'ink', value: '#010101', whens: [{ when: [{ axis: 'density', value: 'compact' }], value: '#020202' }] },
    { op: 'set-token', name: 'edge', value: '#030303' },
  ], review });
  expect(held.status).toBe(200);
  const old = held.ok.proposal;
  // The when branch, by the tool's person: no ops sent.
  const edit = await post({ method: 'edit-held', client: 'design-tool:Ada', proposal: old, index: 0, when: 1, value: ' #0a0a0a ' });
  expect(edit.status).toBe(200);
  expect(edit.ok).toMatchObject({ recomputed: true, replaces: old, client: 'agent:codex', replaces: old });
  expect(edit.ok.ops.map(o => o.op)).toEqual([
    { op: 'set-token', name: 'ink', value: '#010101', whens: [{ when: [{ axis: 'density', value: 'compact' }], value: '#0a0a0a' }] },
    { op: 'set-token', name: 'edge', value: '#030303' },
  ]);
  expect(edit.ok.review).toMatchObject({ importer: 'dtcg', kept: true, from: old, tokens: 2, offers: [{ items: [1] }] });
  const list = await heldList();
  expect(list.map(p => p.proposal)).toEqual([edit.ok.proposal]);
  // Both names wherever it is: its owner, the agent, and its last editor, the person (the answer, the list, the events).
  expect([edit.ok.client, edit.ok.editedBy]).toEqual(['agent:codex', 'design-tool:Ada']);
  expect([list[0].client, list[0].editedBy]).toEqual(['agent:codex', 'design-tool:Ada']);
  await stream.until(e => e.some(x => x.event === 'proposal' && x.proposal === edit.ok.proposal) && e.some(x => x.event === 'proposal-dropped' && x.proposal === old));
  expect(stream.events.find(x => x.event === 'proposal' && x.proposal === edit.ok.proposal)).toMatchObject({ client: 'agent:codex', editedBy: 'design-tool:Ada', replaces: old });
  expect(stream.events.find(x => x.event === 'proposal-dropped' && x.proposal === old)).toMatchObject({ reason: 'edited', replacedBy: edit.ok.proposal, editedBy: 'design-tool:Ada' });
  stream.close();
  // Its own value, keeping only it: the offer that named the other op goes with it.
  const again = await post({ method: 'edit-held', client: 'design-tool:Ada', proposal: edit.ok.proposal, index: 0, value: '#0b0b0b', items: [0] });
  expect(again.status).toBe(200);
  expect(again.ok.ops.map(o => o.op.value)).toEqual(['#0b0b0b']);
  expect(again.ok.review).toMatchObject({ tokens: 1, offers: [{ items: [] }], from: edit.ok.proposal });
  await post({ method: 'drop', client: 'agent:codex', proposal: again.ok.proposal });
}, 60_000);

test('every refusal: no proposal, an index, a when, a value, items without the edit', async () => {
  const held = await post({ method: 'propose', hold: true, client: 'agent', ops: [{ op: 'set-token', name: 'edge', value: '#040404' }, { op: 'remove-token', name: 'ink' }] });
  const id = held.ok.proposal;
  const tries = [
    [{ proposal: 'prop-none-1', index: 0, value: '#050505' }, 404, 'no held proposal'],
    [{ proposal: id, index: 2, value: '#050505' }, 400, '`index`'],
    [{ proposal: id, index: 1, value: '#050505' }, 400, 'only these carry a value'],
    [{ proposal: id, index: 0, when: 1, value: '#050505' }, 400, '`when`'],
    [{ proposal: id, index: 0, value: '  ' }, 400, '`value`'],
    [{ proposal: id, index: 0, value: '#050505', items: [1] }, 400, 'keeps the operation it edits'],
    [{ proposal: id, index: 0, value: '#050505', items: [7] }, 400, '`items`'],
  ];
  for (const [request, status, says] of tries) {
    const r = await post({ method: 'edit-held', client: 'agent', ...request });
    expect([JSON.stringify(request), r.status]).toEqual([JSON.stringify(request), status]);
    expect(r.error.message).toContain(says);
  }
  // Each left it held as it was.
  expect((await heldList()).map(p => p.proposal)).toContain(id);
  await post({ method: 'drop', client: 'agent', proposal: id });
}, 60_000);

test('a 3,000-op held import has one value edited with no ops sent', async () => {
  const ops = Array.from({ length: 3000 }, (_, i) => ({ op: 'add-token', name: `big.t${i}`, value: '#0c0c0c' }));
  const held = await post({ method: 'propose', hold: true, client: 'agent:figma', ops, review: { importer: 'dtcg', tokens: 3000 } });
  expect(held.status).toBe(200);
  const request = { method: 'edit-held', client: 'design-tool:Ada', proposal: held.ok.proposal, index: 1234, value: '#0d0d0d' };
  expect(JSON.stringify(request).length).toBeLessThan(200);
  const edit = await post(request);
  expect(edit.status).toBe(200);
  expect(edit.ok.ops).toHaveLength(3000);
  expect(edit.ok.ops[1234].op.value).toBe('#0d0d0d');
  expect(edit.ok.ops[1233].op.value).toBe('#0c0c0c');
  await post({ method: 'drop', client: 'agent:figma', proposal: edit.ok.proposal });
}, 120_000);

test('an edit racing an accept: one of them has it, the other finds none; a plan that fails puts it back as it was', async () => {
  const held = await post({ method: 'propose', hold: true, client: 'agent', ops: [{ op: 'set-token', name: 'edge', value: '#060606' }] });
  const id = held.ok.proposal;
  const [accepted, edit] = await Promise.all([post({ method: 'accept', client: 'person', proposal: id }), post({ method: 'edit-held', client: 'person', proposal: id, index: 0, value: '#070707' })]);
  expect([accepted.status, edit.status].sort()).toEqual([200, 404]);
  // A value the design can't take: the plan fails, and the proposal is held again, unchanged.
  spawnSync('git', ['checkout', '--', 'app.tamber'], { cwd: dir });
  const kept = await post({ method: 'propose', hold: true, client: 'agent', ops: [{ op: 'set-token', name: 'edge', value: '#080808' }] });
  const bad = await post({ method: 'edit-held', client: 'person', proposal: kept.ok.proposal, index: 0, value: 'not a colour (' });
  expect(bad.status).toBeGreaterThanOrEqual(400);
  const list = await heldList();
  const back = list.find(p => p.proposal === kept.ok.proposal);
  expect(back?.ops.map(o => o.op.value)).toEqual(['#080808']);
  await post({ method: 'drop', client: 'agent', proposal: kept.ok.proposal });
}, 60_000);

test('the scope: one\'s own at propose, another client\'s at write, as drop', () => {
  const owner = () => 'agent:codex';
  expect(needs({ method: 'edit-held', client: 'agent:codex', proposal: 'p' }, owner)).toBe('propose');
  expect(needs({ method: 'edit-held', client: 'remote:Ada#1', proposal: 'p' }, owner)).toBe('write');
  expect(needs({ method: 'edit-held', client: 'x', proposal: 'gone' }, () => null)).toBe('propose');
  // The edit itself, pure: a re-import's server mark is not carried (an edit is the value given).
  const p = { ops: [{ op: 'add-token', name: 'a', value: '#000' }], shown: { proposal: 'prop-e-1', review: { importer: 'css', dials: [1], pins: [2] } } };
  const made = edited({ index: 0, value: '#fff' }, p);
  expect(made.review).toEqual({ importer: 'css', kept: true, from: 'prop-e-1', offers: [] });
  expect(p.ops[0].value).toBe('#000');
});
