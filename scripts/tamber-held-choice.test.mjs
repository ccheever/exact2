// A held proposal's choice (tamber/1.18; scripts/tamber-held-choice.mjs), on the real server (`contract tamber
// serve`): `choose` recorded on the proposal and announced, every refusal, `accept` taking the choice when it names no
// `items` (a recompute when part, the whole when all), an explicit `items` winning, an accept that saw an older choice
// refused naming who chose since while one's own choice never stales one's own accept, `edit-held` carrying the
// choice and refused alike when it names an older one, and a recompute's proposal having none.
import { afterAll, beforeAll, expect, test } from 'bun:test';
import { EventEmitter } from 'node:events';
import { PassThrough } from 'node:stream';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { developmentGate } from './install-page.mjs';
import { accepted, carried } from './tamber-held-choice.mjs';
import { needs } from './tamber-pairing.mjs';
import { tamberServer } from './tamber-server.mjs';

const DESIGN = 'tamber t\n\naxes\n  density = normal | compact\n\ntheme\n  ink = #000000\n    when density=compact: #111111\n  edge = #101010\n';

let dir, server, tamber, origin, headers;
beforeAll(async () => {
  dir = mkdtempSync(join(tmpdir(), 'exact-held-choice-'));
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

const hold = async (client = 'agent:codex', ops = [{ op: 'set-token', name: 'ink', value: '#010101' }, { op: 'set-token', name: 'edge', value: '#020202' }, { op: 'add-token', name: 'glow', value: '#030303' }]) =>
  (await post({ method: 'propose', hold: true, client, ops })).ok.proposal;

test('choose records the choice on the proposal, announces it, and moves the held list; every refusal', async () => {
  const stream = await subscribe();
  const id = await hold();
  const before = (await (await fetch(`${origin}/__tamber/events`, { method: 'POST', headers, body: JSON.stringify({ after: 0, wait: false, client: 'tool' }) })).json()).held;
  const chose = await post({ method: 'choose', client: 'design-tool:Ada', proposal: id, items: [2, 0, 2] });
  expect([chose.status, chose.ok]).toEqual([200, { proposal: id, choice: [0, 2], choiceBy: 'design-tool:Ada', choiceAt: 1 }]);
  const list = await (await fetch(`${origin}/__tamber/events`, { method: 'POST', headers, body: JSON.stringify({ after: 0, wait: false, client: 'tool' }) })).json();
  expect(list.held).toBeGreaterThan(before);
  expect(list.proposals.find(p => p.proposal === id)).toMatchObject({ choice: [0, 2], choiceBy: 'design-tool:Ada', choiceAt: 1, client: 'agent:codex' });
  await stream.until(e => e.some(x => x.event === 'proposal' && x.proposal === id && x.choiceAt === 1));
  stream.close();
  // It records only: nothing written, the proposal still held, its operations as they were.
  expect(readFileSync(join(dir, 'app.tamber'), 'utf8')).toBe(DESIGN);
  expect(list.proposals.find(p => p.proposal === id).ops).toHaveLength(3);
  for (const items of [undefined, 'all', [3], [-1], [0.5], [0, 1, 2, 0, 1, 2, 0]]) {
    const r = await post({ method: 'choose', client: 'design-tool:Ada', proposal: id, items });
    expect([JSON.stringify(items), r.status]).toEqual([JSON.stringify(items), 400]);
  }
  expect((await post({ method: 'choose', client: 'design-tool:Ada', proposal: 'prop-never', items: [] })).status).toBe(404);
  expect((await post({ method: 'choose', proposal: id, items: [] })).status).toBe(400);
  // A replaced one is stale, naming what replaced it and who.
  const edit = await post({ method: 'edit-held', client: 'design-tool:Bo', proposal: id, index: 0, value: '#0a0a0a' });
  const old = await post({ method: 'choose', client: 'design-tool:Ada', proposal: id, items: [0] });
  expect([old.status, old.error.stale, old.error.replacedBy, old.error.by]).toEqual([409, true, edit.ok.proposal, 'design-tool:Bo']);
  await post({ method: 'drop', client: 'agent:codex', proposal: edit.ok.proposal });
}, 60_000);

test('accept with no items takes the choice: a recompute when part, the whole when all; an explicit items wins; nothing chosen is refused', async () => {
  const part = await hold();
  await post({ method: 'choose', client: 'design-tool:Ada', proposal: part, items: [0, 2] });
  const re = await post({ method: 'accept', client: 'design-tool:Ada', proposal: part });
  expect([re.status, re.ok.recomputed, re.ok.replaces]).toEqual([200, true, part]);
  expect(re.ok.ops.map(o => o.op.name)).toEqual(['ink', 'glow']);
  // (all of a recompute is chosen: it carries no choice)
  expect(re.ok.choice).toBeUndefined();
  await post({ method: 'drop', client: 'agent:codex', proposal: re.ok.proposal });
  // An explicit items wins over the choice.
  const both = await hold();
  await post({ method: 'choose', client: 'design-tool:Ada', proposal: both, items: [0] });
  const named = await post({ method: 'accept', client: 'design-tool:Ada', proposal: both, items: [1] });
  expect(named.ok.ops.map(o => o.op.name)).toEqual(['edge']);
  await post({ method: 'drop', client: 'agent:codex', proposal: named.ok.proposal });
  // Nothing chosen: refused, still held.
  const none = await hold();
  await post({ method: 'choose', client: 'design-tool:Ada', proposal: none, items: [] });
  const empty = await post({ method: 'accept', client: 'design-tool:Ada', proposal: none });
  expect(empty.status).toBe(400);
  expect((await heldList()).some(p => p.proposal === none)).toBe(true);
  // All chosen: the whole is written.
  await post({ method: 'choose', client: 'design-tool:Ada', proposal: none, items: [0, 1, 2] });
  const all = await post({ method: 'accept', client: 'design-tool:Ada', proposal: none });
  expect([all.status, all.ok.recomputed]).toEqual([200, undefined]);
  expect(readFileSync(join(dir, 'app.tamber'), 'utf8')).toContain('glow = #030303');
  writeFileSync(join(dir, 'app.tamber'), DESIGN);
}, 60_000);

test('an accept that saw an older choice is refused naming who chose since; one\'s own choice never stales one\'s own accept', async () => {
  const id = await hold();
  const mine = await post({ method: 'choose', client: 'design-tool:Ada', proposal: id, items: [0, 1] });
  const theirs = await post({ method: 'choose', client: 'design-tool:Bo', proposal: id, items: [0] });
  expect([mine.ok.choiceAt, theirs.ok.choiceAt]).toEqual([1, 2]);
  const stale = await post({ method: 'accept', client: 'design-tool:Ada', proposal: id, items: [0, 1], choiceAt: mine.ok.choiceAt });
  expect([stale.status, stale.error.stale, stale.error.choice, stale.error.choiceBy, stale.error.choiceAt]).toEqual([409, true, [0], 'design-tool:Bo', 2]);
  expect((await heldList()).some(p => p.proposal === id)).toBe(true);
  expect((await post({ method: 'accept', client: 'design-tool:Ada', proposal: id, choiceAt: 'x' })).status).toBe(400);
  // Ada chooses again, takes the choiceAt it answers, and accepts with it.
  const again = await post({ method: 'choose', client: 'design-tool:Ada', proposal: id, items: [1] });
  const ok = await post({ method: 'accept', client: 'design-tool:Ada', proposal: id, items: [1], choiceAt: again.ok.choiceAt });
  expect([ok.status, ok.ok.recomputed]).toEqual([200, true]);
  await post({ method: 'drop', client: 'agent:codex', proposal: ok.ok.proposal });
  // An accept naming no choiceAt is not checked: it takes the choice as it is, whoever made it.
  const free = await hold();
  await post({ method: 'choose', client: 'design-tool:Ada', proposal: free, items: [0, 1] });
  await post({ method: 'choose', client: 'design-tool:Bo', proposal: free, items: [2] });
  const unchecked = await post({ method: 'accept', client: 'design-tool:Ada', proposal: free });
  expect([unchecked.status, unchecked.ok.recomputed]).toEqual([200, true]);
  expect(unchecked.ok.ops.map(o => o.op.name)).toEqual(['glow']);
  await post({ method: 'drop', client: 'agent:codex', proposal: unchecked.ok.proposal });
}, 60_000);

test('edit-held carries the choice: all of it with no items, the kept ones renumbered with items; one that saw an older choice is refused', async () => {
  const id = await hold();
  const seen = await post({ method: 'choose', client: 'design-tool:Bo', proposal: id, items: [1] });
  await post({ method: 'choose', client: 'design-tool:Ada', proposal: id, items: [0, 2] });
  // Bo saw its own choice, Ada chose since: the edit is refused naming her choice, and the proposal is left held.
  const stale = await post({ method: 'edit-held', client: 'design-tool:Bo', proposal: id, index: 1, value: '#0a0a0a', choiceAt: seen.ok.choiceAt });
  expect([stale.status, stale.error.stale, stale.error.choice, stale.error.choiceBy, stale.error.choiceAt]).toEqual([409, true, [0, 2], 'design-tool:Ada', 2]);
  expect((await heldList()).find(p => p.proposal === id)).toMatchObject({ choice: [0, 2], choiceAt: 2 });
  expect((await post({ method: 'edit-held', client: 'design-tool:Bo', proposal: id, index: 1, value: '#0a0a0a', choiceAt: -1 })).status).toBe(400);
  const whole = await post({ method: 'edit-held', client: 'design-tool:Bo', proposal: id, index: 1, value: '#0a0a0a', choiceAt: 2 });
  expect(whole.ok).toMatchObject({ choice: [0, 2], choiceBy: 'design-tool:Ada', choiceAt: 1, editedBy: 'design-tool:Bo' });
  expect((await heldList()).find(p => p.proposal === whole.ok.proposal)).toMatchObject({ choice: [0, 2] });
  const part = await post({ method: 'edit-held', client: 'design-tool:Bo', proposal: whole.ok.proposal, index: 2, value: '#0b0b0b', items: [1, 2] });
  expect(part.ok.choice).toEqual([1]);
  // With none recorded, none is carried.
  const plain = await post({ method: 'edit-held', client: 'design-tool:Bo', proposal: await hold(), index: 0, value: '#0c0c0c' });
  expect(plain.ok.choice).toBeUndefined();
  for (const p of [part.ok.proposal, plain.ok.proposal]) await post({ method: 'drop', client: 'agent:codex', proposal: p });
}, 60_000);

test('the choice, pure: accept\'s defaults and stale rule, the carry, and the scope', () => {
  const p = { ops: [1, 2, 3], shown: { choice: [0, 2], choiceBy: 'a', choiceAt: 4 } };
  expect(accepted({ proposal: 'x' }, p)).toEqual({ items: [0, 2] });
  expect(accepted({ proposal: 'x', items: [1] }, p)).toEqual({ items: [1] });
  expect(accepted({ proposal: 'x', choiceAt: 4 }, p)).toEqual({ items: [0, 2] });
  expect(accepted({ proposal: 'x', choiceAt: 3 }, p).status).toBe(409);
  expect(accepted({ proposal: 'x' }, { ops: [1, 2], shown: { choice: [0, 1] } })).toEqual({ items: undefined });
  expect(accepted({ proposal: 'x' }, { ops: [1, 2], shown: {} })).toEqual({ items: undefined });
  expect(accepted({ proposal: 'x', choiceAt: 0 }, { ops: [1, 2], shown: {} })).toEqual({ items: undefined });
  expect(carried({ choice: [0, 2, 3], choiceBy: 'a' }, [0, 3])).toEqual({ choice: [0, 1], choiceBy: 'a', choiceAt: 1 });
  expect(carried({}, [0])).toBeUndefined();
  const owner = () => 'agent:codex';
  expect(needs({ method: 'choose', client: 'agent:codex', proposal: 'p' }, owner)).toBe('propose');
  expect(needs({ method: 'choose', client: 'remote:Ada#1', proposal: 'p' }, owner)).toBe('write');
  expect(needs({ method: 'choose', client: 'x', proposal: 'gone' }, () => null)).toBe('propose');
});
