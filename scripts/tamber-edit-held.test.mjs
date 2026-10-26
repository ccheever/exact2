// `edit-held` (tamber/1.17; scripts/tamber-edit-held.mjs): one value of a held proposal changed, on the real server
// (`contract tamber serve`): a value and a `when` branch's, `items` kept and the review renumbered, the old one replaced
// in one step (`proposal-dropped {reason: 'edited', replacedBy, editedBy}`), the original client's still, a 3,000-op
// held import edited with no ops sent, every refusal, another client's needing `write`, an edit racing an accept, and
// a failed plan putting the old one back.
import { afterAll, beforeAll, expect, test } from 'bun:test';
import { EventEmitter } from 'node:events';
import { PassThrough } from 'node:stream';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { developmentGate } from './install-page.mjs';
import { edited, editHeld } from './tamber-edit-held.mjs';
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
    { op: 'add-token', name: 'glow', value: '#010101', whens: [{ when: [{ axis: 'density', value: 'compact' }], value: '#020202' }] },
    { op: 'set-token', name: 'edge', value: '#030303' },
  ], review });
  expect(held.status).toBe(200);
  const old = held.ok.proposal;
  // The when branch, by the tool's person: no ops sent.
  const edit = await post({ method: 'edit-held', client: 'design-tool:Ada', proposal: old, index: 0, when: 1, value: ' #0a0a0a ' });
  expect(edit.status).toBe(200);
  expect(edit.ok).toMatchObject({ recomputed: true, replaces: old, client: 'agent:codex', replaces: old });
  expect(edit.ok.ops.map(o => o.op)).toEqual([
    { op: 'add-token', name: 'glow', value: '#010101', whens: [{ when: [{ axis: 'density', value: 'compact' }], value: '#0a0a0a' }] },
    { op: 'set-token', name: 'edge', value: '#030303' },
  ]);
  // The plan reads the branch: its diff carries the edited value.
  expect(edit.ok.diff).toContain('#0a0a0a');
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
    [{ proposal: id, index: 0, when: 1, value: '#050505' }, 400, "an `add-token`'s"],
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

/** A Tailwind theme of 3,000 colours, `--color-<stem>-t<i>`, all `value`. */
const bigTheme = (stem, value) => `@theme {\n${Array.from({ length: 3000 }, (_, i) => `  --color-${stem}-t${i}: ${value};`).join('\n')}\n}\n`;

test('a 3,000-op held import has one value edited with no ops sent', async () => {
  // Planned and held by the server (a client may send at most 1024 operations, tamber-ops-size).
  const held = await post({ method: 'import', kind: 'tailwind', client: 'agent:figma', source: bigTheme('big', '#0c0c0c') });
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

test('an edit racing an accept: the edit holds it while it plans, so the accept finds none; a plan that fails leaves it held as it was', async () => {
  const stream = await subscribe();
  // A big proposal, so its edit's plan takes a while: the accept, the drop and a second edit come while it plans.
  const id = (await post({ method: 'import', kind: 'tailwind', client: 'agent', source: bigTheme('race', '#060606') })).ok.proposal;
  const editing = post({ method: 'edit-held', client: 'person', proposal: id, index: 0, value: '#070707' });
  await Bun.sleep(30);
  const [accepted, dropped, second] = await Promise.all([post({ method: 'accept', client: 'person', proposal: id }), post({ method: 'drop', client: 'agent', proposal: id }), post({ method: 'edit-held', client: 'person', proposal: id, index: 1, value: '#080808' })]);
  expect([accepted.status, dropped.status, second.status]).toEqual([404, 404, 404]);
  const edit = await editing;
  expect(edit.status).toBe(200);
  // The survivor: the edit, held; nothing written.
  expect(readFileSync(join(dir, 'app.tamber'), 'utf8')).not.toContain('race-t0');
  const list = await heldList();
  expect(list.map(p => p.proposal)).toEqual(expect.arrayContaining([edit.ok.proposal]));
  expect(list.map(p => p.proposal)).not.toContain(id);
  expect(list.find(p => p.proposal === edit.ok.proposal).ops[0].op.value).toBe('#070707');
  await post({ method: 'drop', client: 'agent', proposal: edit.ok.proposal });
  // A value the design can't take: the plan fails, and the proposal is held as it was: the same record, no event.
  const kept = await post({ method: 'propose', hold: true, client: 'agent', ops: [{ op: 'set-token', name: 'edge', value: '#080808' }], review: { importer: 'css', tokens: 1 } });
  await stream.until(e => e.some(x => x.event === 'proposal' && x.proposal === kept.ok.proposal));
  const before = (await heldList()).find(p => p.proposal === kept.ok.proposal);
  const seen = stream.events.length;
  const bad = await post({ method: 'edit-held', client: 'person', proposal: kept.ok.proposal, index: 0, value: 'not a colour (' });
  expect(bad.status).toBeGreaterThanOrEqual(400);
  expect((await heldList()).find(p => p.proposal === kept.ok.proposal)).toEqual(before);
  await Bun.sleep(100);
  expect(stream.events.slice(seen).filter(e => e.proposal === kept.ok.proposal || e.replaces === kept.ok.proposal)).toEqual([]);
  await post({ method: 'drop', client: 'agent', proposal: kept.ok.proposal });
  stream.close();
}, 120_000);

test('a held revert is not edited; an edit holds at its requester\'s scope', async () => {
  // A revert is planned from its commit: refused before anything is taken.
  const p = { ops: [{ op: 'set-token', name: 'a', value: '#000' }], guards: {}, shown: { proposal: 'prop-r-1', review: { revert: { commit: 'c' } } } };
  const proposals = new Map([['prop-r-1', p]]);
  let held = null;
  const ctx = { expire() {}, proposals, hold: async (...args) => { held = args; return { ok: { proposal: 'prop-r-2' } }; }, announce() {} };
  const revert = await editHeld({ client: 'x', proposal: 'prop-r-1', index: 0, value: '#fff' }, 'write', ctx);
  expect([revert.status, revert.error.message]).toEqual([400, 'a held revert is planned from its commit: drop it and revert again to change it']);
  expect(held).toBe(null);
  // Not a revert: held at the requester's scope (a `propose` client's edit never evicts another's at the cap), its
  // owner kept, the editor named, and it stays held (busy) while planned.
  delete p.shown.review;
  let busy = null;
  ctx.hold = async (...args) => { busy = proposals.get('prop-r-1').busy; held = args; return { ok: { proposal: 'prop-r-2' } }; };
  p.client = 'agent';
  expect((await editHeld({ client: 'remote:Ada#1', proposal: 'prop-r-1', index: 0, value: '#fff' }, 'propose', ctx)).ok).toMatchObject({ replaces: 'prop-r-1', proposal: 'prop-r-2' });
  expect([held[0], held[3], held[6], held[9], busy, p.busy]).toEqual(['agent', 'prop-r-1', 'propose', 'remote:Ada#1', true, false]);
});

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

/** A stand-in for `contract tamber serve` whose plan of an op valued `#barrier` waits for `release()`, and of one
 * valued `#refused` planned again (after its first plan) waits for `refuse()` and then is refused. */
function barrierServe() {
  let release, gate = new Promise(ok => { release = ok; }), refuseNow, later = new Promise(ok => { refuseNow = ok; });
  const child = Object.assign(new EventEmitter(), { stdin: new PassThrough(), stdout: new PassThrough(), stderr: new PassThrough(), exitCode: null, signalCode: null });
  child.kill = () => { child.exitCode = 0; child.emit('exit', 0); };
  let buf = '', seen = 0;
  child.stdin.on('data', chunk => {
    buf += chunk; const lines = buf.split('\n'); buf = lines.pop();
    for (const line of lines) {
      const r = JSON.parse(line), answer = () => child.stdout.write(`${JSON.stringify({ id: r.id, ok: { files: {}, diff: '', changed: [], class: 'live' } })}\n`);
      const refuse = () => child.stdout.write(`${JSON.stringify({ id: r.id, error: { message: 'refused at the gate' } })}\n`);
      const refused = (r.ops ?? []).some(o => o?.value === '#refused') && seen++ > 0;
      if ((r.ops ?? []).some(o => o?.value === '#barrier')) gate.then(answer); else if (refused) later.then(refuse); else answer();
    }
  });
  return { child, release: () => release(), refuse: () => refuseNow() };
}

test('while an edit plans, its proposal is held and counted but no one else\'s: an accept, a drop or an edit finds none, and a hold at the cap evicts another', async () => {
  const serve = barrierServe();
  const wire = tamberServer({ appDir: '/nowhere', spawnServe: () => serve.child, log: () => {} });
  const call = async (body, path = '/__tamber') => {
    let text = '', status = 0;
    const req = Object.assign(new PassThrough(), { method: 'POST', headers: { 'x-exact-tamber-token': wire.token }, socket: { remoteAddress: '127.0.0.1' } });
    queueMicrotask(() => req.end(JSON.stringify(body)));
    await wire.handle(req, Object.assign(new EventEmitter(), { writeHead(s) { status = s; }, setHeader() {}, write(t) { text += t; }, end(t) { text += t ?? ''; } }), new URL(`http://127.0.0.1${path}`), { allowed: true, local: true });
    return { status, ...JSON.parse(text) };
  };
  try {
    // 64 held, the first the one edited (the oldest, the first a cap would take).
    const p = (await call({ method: 'propose', hold: true, client: 'agent', ops: [{ op: 'set-token', name: 'a', value: '#000000' }] })).ok.proposal;
    for (let i = 1; i < 64; i++) await call({ method: 'propose', hold: true, client: `other-${i}`, ops: [{ op: 'set-token', name: 'a', value: '#000001' }] });
    const editing = call({ method: 'edit-held', client: 'person', proposal: p, index: 0, value: '#barrier' });
    await Bun.sleep(20);
    const [accepted, dropped, second] = await Promise.all([call({ method: 'accept', client: 'person', proposal: p }), call({ method: 'drop', client: 'agent', proposal: p }), call({ method: 'edit-held', client: 'person', proposal: p, index: 0, value: '#000002' })]);
    expect([accepted.status, dropped.status, second.status]).toEqual([404, 404, 404]);
    // A 65th hold meanwhile: the cap takes the oldest one not being edited, never the one planning.
    const extra = await call({ method: 'propose', hold: true, client: 'late', ops: [{ op: 'set-token', name: 'a', value: '#000003' }] });
    expect(extra.status).toBe(200);
    const listed = (await call({ after: 0, wait: false, client: 'tool' }, '/__tamber/events')).proposals.map(q => q.proposal);
    expect(listed).toContain(p);
    serve.release();
    const edit = await editing;
    expect(edit.status).toBe(200);
    const after = (await call({ after: 0, wait: false, client: 'tool' }, '/__tamber/events')).proposals;
    expect(after.map(q => q.proposal)).not.toContain(p);
    expect(after.find(q => q.proposal === edit.ok.proposal)).toMatchObject({ client: 'agent', editedBy: 'person', replaces: p });
    expect(after.length).toBe(64);
  } finally { await wire.close(); }
}, 60_000);

test('with every held proposal being edited, a new hold is refused, never held and dropped at once', async () => {
  const serve = barrierServe();
  const wire = tamberServer({ appDir: '/nowhere', spawnServe: () => serve.child, log: () => {} });
  const call = async (body, path = '/__tamber') => {
    let text = '', status = 0;
    const req = Object.assign(new PassThrough(), { method: 'POST', headers: { 'x-exact-tamber-token': wire.token }, socket: { remoteAddress: '127.0.0.1' } });
    queueMicrotask(() => req.end(JSON.stringify(body)));
    await wire.handle(req, Object.assign(new EventEmitter(), { writeHead(s) { status = s; }, setHeader() {}, write(t) { text += t; }, end(t) { text += t ?? ''; } }), new URL(`http://127.0.0.1${path}`), { allowed: true, local: true });
    return { status, ...JSON.parse(text) };
  };
  try {
    const ids = [];
    for (let i = 0; i < 64; i++) ids.push((await call({ method: 'propose', hold: true, client: `c-${i}`, ops: [{ op: 'set-token', name: 'a', value: '#000001' }] })).ok.proposal);
    const edits = ids.map(id => call({ method: 'edit-held', client: 'person', proposal: id, index: 0, value: '#barrier' }));
    await Bun.sleep(30);
    const at = await call({ after: 0, wait: false, client: 'tool' }, '/__tamber/events');
    const refused = await call({ method: 'propose', hold: true, client: 'late', ops: [{ op: 'set-token', name: 'a', value: '#000002' }] });
    expect([refused.status, refused.error?.message]).toEqual([409, '64 proposals are held and each is being edited: try again shortly']);
    // Never held: no event since, the same 64 listed.
    const since = await call({ after: at.seq, epoch: at.epoch, wait: false, client: 'tool' }, '/__tamber/events');
    expect([since.seq, since.events]).toEqual([at.seq, []]);
    expect(since.proposals.map(q => q.proposal).sort()).toEqual([...ids].sort());
    serve.release();
    expect((await Promise.all(edits)).map(e => e.status)).toEqual(ids.map(() => 200));
    expect((await call({ after: 0, wait: false, client: 'tool' }, '/__tamber/events')).proposals).toHaveLength(64);
  } finally { await wire.close(); }
}, 60_000);

test('an accept that cannot hold again, its slot taken and every other held proposal being edited, is dropped, never held past 64', async () => {
  const serve = barrierServe();
  const wire = tamberServer({ appDir: '/nowhere', spawnServe: () => serve.child, log: () => {} });
  const call = async (body, path = '/__tamber') => {
    let text = '', status = 0;
    const req = Object.assign(new PassThrough(), { method: 'POST', headers: { 'x-exact-tamber-token': wire.token }, socket: { remoteAddress: '127.0.0.1' } });
    queueMicrotask(() => req.end(JSON.stringify(body)));
    await wire.handle(req, Object.assign(new EventEmitter(), { writeHead(s) { status = s; }, setHeader() {}, write(t) { text += t; }, end(t) { text += t ?? ''; } }), new URL(`http://127.0.0.1${path}`), { allowed: true, local: true });
    return { status, ...JSON.parse(text) };
  };
  try {
    // P, two operations, accepted in part: its recompute is refused once released, so the accept would put P back.
    const p = (await call({ method: 'propose', hold: true, client: 'agent', ops: [{ op: 'set-token', name: 'a', value: '#refused' }, { op: 'set-token', name: 'b', value: '#000001' }] })).ok.proposal;
    for (let i = 1; i < 64; i++) await call({ method: 'propose', hold: true, client: `c-${i}`, ops: [{ op: 'set-token', name: 'a', value: '#000001' }] });
    const accepting = call({ method: 'accept', client: 'person', proposal: p, items: [0] });
    await Bun.sleep(20);
    // Its slot taken, then every held proposal (64) edited, each planning.
    await call({ method: 'propose', hold: true, client: 'late', ops: [{ op: 'set-token', name: 'a', value: '#000002' }] });
    const listed = (await call({ after: 0, wait: false, client: 'tool' }, '/__tamber/events')).proposals.map(q => q.proposal);
    expect(listed).toHaveLength(64);
    const edits = listed.map(id => call({ method: 'edit-held', client: 'person', proposal: id, index: 0, value: '#barrier' }));
    await Bun.sleep(30);
    // The accept's re-plan refused while all 64 still plan their edits: P is dropped, never held as a 65th.
    serve.refuse();
    const accepted = await accepting;
    expect(accepted.status).toBeGreaterThanOrEqual(400);
    const during = await call({ after: 0, wait: false, client: 'tool' }, '/__tamber/events');
    expect(during.proposals).toHaveLength(64);
    expect(during.proposals.map(q => q.proposal)).not.toContain(p);
    expect(during.events.some(e => e.event === 'proposal-dropped' && e.proposal === p && e.reason === 'too many held')).toBe(true);
    serve.release();
    await Promise.all(edits);
    expect((await call({ after: 0, wait: false, client: 'tool' }, '/__tamber/events')).proposals).toHaveLength(64);
  } finally { await wire.close(); }
}, 60_000);

test('a replaced proposal (1.17): its old id is refused 409 stale naming who replaced it, the latest of a chain; the new one is accepted', async () => {
  const held = await post({ method: 'propose', hold: true, client: 'bea', ops: [{ op: 'set-token', name: 'edge', value: '#0a0a0a' }] });
  const p1 = held.ok.proposal;
  // Ada edits Bea's held proposal: held again as a new one.
  const p2 = (await post({ method: 'edit-held', client: 'ada', proposal: p1, index: 0, value: '#0b0b0b' })).ok.proposal;
  for (const method of ['accept', 'drop', 'edit-held']) {
    const r = await post({ method, client: 'bea', proposal: p1, ...(method === 'edit-held' ? { index: 0, value: '#0c0c0c' } : {}) });
    expect([method, r.status, r.error?.stale, r.error?.replacedBy, r.error?.by], JSON.stringify({ r, p1, p2 })).toEqual([method, 409, true, p2, "ada"]);
  }
  // Cy edits Ada's: the old id answers with the latest replacement and its maker.
  const p3 = (await post({ method: 'edit-held', client: 'cy', proposal: p2, index: 0, value: '#0d0d0d' })).ok.proposal;
  const chained = await post({ method: 'accept', client: 'bea', proposal: p1 });
  expect([chained.status, chained.error.replacedBy, chained.error.by]).toEqual([409, p3, 'cy']);
  expect((await post({ method: 'accept', client: 'bea', proposal: p2 })).error.replacedBy).toBe(p3);
  // Bea accepts the one there is, knowingly; accepted, it is gone, not replaced: 404.
  const accepted = await post({ method: 'accept', client: 'bea', proposal: p3 });
  expect(accepted.status, JSON.stringify(accepted)).toBe(200);
  expect(readFileSync(join(dir, 'app.tamber'), 'utf8')).toContain('edge = #0d0d0d');
  expect((await post({ method: 'accept', client: 'bea', proposal: p3 })).status).toBe(404);
}, 60_000);

test('replaced by an accept\'s recompute (the file moved since it was held): the old id names the accepter, not the proposer', async () => {
  const held = await post({ method: 'propose', hold: true, client: 'bea', ops: [{ op: 'set-token', name: 'edge', value: '#0e0e0e' }] });
  const p1 = held.ok.proposal;
  // The design moves under it (a hand edit), so Ada's accept recomputes it rather than writing.
  writeFileSync(join(dir, 'app.tamber'), readFileSync(join(dir, 'app.tamber'), 'utf8').replace(/ink = #[0-9a-f]{6}/, 'ink = #010101'));
  await Bun.sleep(600);
  const recomputed = await post({ method: 'accept', client: 'ada', proposal: p1 });
  expect(recomputed.ok?.recomputed, JSON.stringify(recomputed)).toBe(true);
  const again = await post({ method: 'accept', client: 'bea', proposal: p1 });
  expect([again.status, again.error.replacedBy, again.error.by]).toEqual([409, recomputed.ok.proposal, 'ada']);
  await post({ method: 'drop', client: 'bea', proposal: recomputed.ok.proposal });
}, 60_000);
