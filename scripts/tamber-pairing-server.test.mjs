// Pairing through the wire (LLP 1121 stage 10 A2): a code minted locally, a
// token for it once, at its scope, under the client name its token gives;
// local-only methods; no cookie and no remote request but `pair` without a
// token; revoke and expiry ending the client's streams, polls, scrub, pins
// and any other end (the canvas gateway's).
import { afterAll, beforeAll, expect, test } from 'bun:test';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { developmentGate } from './install-page.mjs';
import { tamberServer } from './tamber-server.mjs';

let dir, server, tamber, origin, local, logged = [], remote = false, flags = null, now = Date.now(), restores = 0;
beforeAll(async () => {
  dir = mkdtempSync(join(tmpdir(), 'exact-tamber-pairing-'));
  writeFileSync(join(dir, 'app.contract'), 'component App\n  view\n    text "a" color="ink"\n');
  writeFileSync(join(dir, 'app.tamber'), 'tamber t\n\ntheme\n  ink = #000000\n');
  server = createServer();
  await new Promise(ok => server.listen(0, '127.0.0.1', ok));
  const port = server.address().port;
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  tamber = tamberServer({
    appDir: dir, log: line => logged.push(line), settle: 100, pairTtlMs: 3600_000, clock: () => now,
    scrub: { preview: async () => ({}), restore: () => { restores++; } },
  });
  // `remote` stands in for B's listener: the request came from off this machine.
  server.on('request', async (req, res) => {
    const access = flags ?? (remote ? { local: false, remote: true, peer: '192.0.2.7' } : gate.check(req));
    if (!(await tamber.handle(req, res, new URL(req.url, 'http://x'), access))) { res.writeHead(404); res.end(); }
  });
  origin = `http://127.0.0.1:${port}`;
  local = (await (await fetch(`${origin}/__tamber/token`)).json()).token;
  await tamber.ready;
}, 600_000); // a cold checkout builds the compiler first
afterAll(async () => {
  await tamber?.close();
  server?.close();
  if (dir) rmSync(dir, { recursive: true, force: true });
});
const post = async (request, token = local, path = '/__tamber', more = {}) => {
  const res = await fetch(`${origin}${path}`, { method: 'POST', headers: { 'content-type': 'application/json', ...(token ? { 'x-exact-tamber-token': token } : {}), ...more }, body: JSON.stringify(request) });
  const text = await res.text();
  let body = {};
  try { body = JSON.parse(text); } catch {}
  return { status: res.status, ...body };
};
// Each pairing from a peer of its own: a peer gets five attempts a minute.
let peers = 0;
// Each from B's TLS door, as a remote peer comes (plain HTTP from off this
// machine reaches nothing of the wire: the test below).
const fromPeer = async fn => { flags = { local: false, remote: true, peer: `203.0.113.${++peers}` }; try { return await fn(); } finally { flags = null; } };
const paired = async (scope, name) => {
  const { code } = (await post({ method: 'pair-code', scope, name })).ok;
  return (await fromPeer(() => post({ code }, null, '/__tamber/pair'))).ok;
};
/** A subscription naming `client`; `ended` resolves when the server ends it. */
const subscribe = async (token, client) => {
  const res = await fetch(`${origin}/__tamber/subscribe`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: JSON.stringify({ client }) });
  const reader = res.body.getReader();
  const ended = (async () => { for (;;) { const { done } = await reader.read().catch(() => ({ done: true })); if (done) return true; } })();
  return { status: res.status, ended };
};

test("the local token's body may be 16 MiB (a CLI import's sources); a paired client's is 1 MiB, refused before it is read; a remote importer-marked hold gets no exemption from the ops cap", async () => {
  // About 10 MiB of DTCG: 2,500 colours, each with a long description.
  const note = 'n'.repeat(4000);
  const tokens = Object.fromEntries(Array.from({ length: 2600 }, (_, k) => [`c${k}`, { $type: 'color', $value: `#${(k + 0x300000).toString(16).slice(-6)}`, $description: note }]));
  const source = JSON.stringify(tokens);
  expect(source.length).toBeGreaterThan(10 << 20);
  const local = await post({ method: 'import', kind: 'dtcg', client: 'big-local', name: 'tokens.json', source });
  expect(local.status, JSON.stringify(local).slice(0, 300)).toBe(200);
  expect(local.ok.ops.length).toBeGreaterThan(2000);
  await post({ method: 'drop', client: 'big-local', proposal: local.ok.proposal });
  // Past 1 MiB the local token reaches only an import (Astra's round 2).
  const review = { importer: 'dtcg', tokens: 1, offers: [], note: 'n'.repeat(2 << 20) };
  const big = await post({ method: 'propose', hold: true, client: 'big-local', ops: [{ op: 'set-token', name: 'ink', value: '#111111' }], review });
  expect(big.status).toBe(413);
  const p = await paired('write', 'far');
  const remote = await post({ method: 'import', kind: 'dtcg', client: 'far', name: 'tokens.json', source }, p.token);
  expect(remote.status).toBe(413);
  // An importer-marked hold from a paired client: capped as any.
  const ops = Array.from({ length: 1025 }, (_, k) => ({ op: 'add-token', name: `r.c${k}`, value: '#000000' }));
  const held = await post({ method: 'propose', hold: true, client: 'far', ops, review: { importer: 'dtcg', tokens: 1025, offers: [] } }, p.token);
  expect([held.status, held.error?.id]).toEqual([400, 'tamber-ops-size']);
  await post({ method: 'revoke', client: p.client });
}, 300_000);

test("a paired client's request past 1024 operations is refused (tamber-ops-size), whatever held proposal it names", async () => {
  const css = `@theme {\n${Array.from({ length: 1500 }, (_, k) => `  --color-r${k}: #${(k + 0x200000).toString(16).slice(-6)};`).join('\n')}\n}\n`;
  const imported = await post({ method: 'import', kind: 'tailwind', client: 'owner', source: css });
  expect(imported.status, JSON.stringify(imported).slice(0, 300)).toBe(200);
  const held = imported.ok;
  const p = await paired('propose', 'tablet');
  const ops = held.ops.map(o => o.op);
  const theirs = await post({ method: 'propose', hold: true, client: 'tablet', review: { ...held.review, from: held.proposal }, ops }, p.token);
  expect([theirs.status, theirs.error?.id]).toEqual([400, 'tamber-ops-size']);
  await post({ method: 'drop', client: 'owner', proposal: held.proposal });
  await post({ method: 'revoke', client: p.client });
}, 120_000);

test('a paired client works at its scope, as the client its token names; local methods are the local token\'s alone', async () => {
  const p = await paired('propose', 'laptop');
  expect(p).toMatchObject({ client: 'remote:laptop#1', scope: 'propose' });
  const hello = await post({ method: 'hello' }, p.token);
  expect([hello.ok.scope, hello.ok.local, hello.ok.client]).toEqual(['propose', false, 'remote:laptop#1']);
  const refused = await post({ method: 'transaction', client: 'tool', ops: [] }, p.token);
  expect([refused.status, refused.error.needs]).toEqual([403, 'write']);
  // It names another client: the held proposal is its own all the same.
  const held = await post({ method: 'propose', hold: true, client: 'tool', ops: [{ op: 'set-token', name: 'ink', value: '#111111' }] }, p.token);
  const events = await post({ after: 0, wait: false }, local, '/__tamber/events');
  expect(events.proposals.find(x => x.proposal === held.ok.proposal)?.client).toBe('remote:laptop#1');
  const w = await paired('write', 'desk');
  for (const request of [{ method: 'clients' }, { method: 'revoke', client: 'remote:laptop#1' }, { method: 'pair-code', scope: 'write' }]) {
    const no = await post(request, w.token);
    expect([request.method, no.status, no.error.needs]).toEqual([request.method, 403, 'local']);
  }
  const list = (await post({ method: 'clients' })).ok.clients;
  expect(list.map(c => [c.client, c.scope, c.local ?? false])).toEqual([['local', 'write', true], ['remote:laptop#1', 'propose', false], ['remote:desk#1', 'write', false]]);
  expect(JSON.stringify(list)).not.toContain(p.token);
  // Neither token nor code is ever logged.
  expect(logged.join('\n')).not.toContain(p.token);
});

test('no cookie authenticates; from off this machine only `pair` answers without a paired token, and the local token is no one', async () => {
  const p = await paired('read', 'phone');
  expect((await post({ method: 'hello' }, null, '/__tamber', { cookie: `x-exact-tamber-token=${p.token}; tamber=${local}` })).status).toBe(404);
  remote = true;
  try {
    expect((await post({ method: 'hello' }, null)).status).toBe(404);
    expect((await post({ method: 'hello' }, local)).status).toBe(404);
    expect((await fetch(`${origin}/__tamber/token`)).status).toBe(404);
    expect((await post({ method: 'hello' }, p.token)).ok.scope).toBe('read');
    // The pairing endpoint answers, and its refusals say nothing of codes.
    const wrong = await post({ code: 'AAAAA-AAAAA' }, null, '/__tamber/pair');
    expect(wrong.status).toBe(403);
  } finally { remote = false; }
});

test('revoke ends the client\'s subscription, long poll, scrub and pins at once, keeps its held proposals marked, and tells every other end', async () => {
  const p = await paired('write', 'remote');
  const sub = await subscribe(p.token, 'ignored');
  expect(sub.status).toBe(200);
  // A build to pin, as a dev loop registers one.
  const frames = [];
  const build = tamber.channel.open(f => frames.push(f));
  tamber.channel.report(JSON.stringify({ nonce: build.nonce, kind: 'register', id: 'web-1', host: 'web', capabilities: ['pin'] }));
  expect((await post({ method: 'pin', client: 'x', build: 'web-1', pin: { scheme: 'dark' } }, p.token)).status).toBe(200);
  expect(tamber.channel.owners()).toContain(p.client);
  const files = { 'app.tamber': (await post({ method: 'hello' })).ok.files['app.tamber'] };
  const scrub = await post({ method: 'scrub', client: 'x', ops: [{ op: 'set-token', name: 'ink', value: '#222222' }], files }, p.token);
  expect(scrub.ok).toBeTruthy();
  const held = await post({ method: 'propose', hold: true, client: 'x', ops: [{ op: 'set-token', name: 'ink', value: '#333333' }] }, p.token);
  // The canvas gateway's end, as stage 10 C subscribes it.
  const gateway = [];
  tamber.pairing.onEnd(p.client, why => gateway.push(why));
  // Once the scrub's and the proposal's events are out, a poll that waits.
  await new Promise(ok => setTimeout(ok, 500));
  const seq = (await post({ after: 0, wait: false }, local, '/__tamber/events')).seq;
  const poll = post({ after: seq }, p.token, '/__tamber/events');
  await new Promise(ok => setTimeout(ok, 200));
  const t0 = Date.now(), restored = restores;
  expect((await post({ method: 'revoke', client: p.client })).ok).toEqual({ revoked: p.client });
  // Its scrub ended, the builds given the committed design back.
  expect(restores).toBeGreaterThan(restored);
  expect((await poll).status).toBe(401);
  expect(await sub.ended).toBe(true);
  expect(Date.now() - t0).toBeLessThan(2000);
  expect(gateway).toEqual(['revoked']);
  expect(tamber.channel.owners()).not.toContain(p.client);
  expect(frames.at(-1)).toMatchObject({ kind: 'pin', pin: null });
  const after = await post({ after: seq, wait: false }, local, '/__tamber/events');
  expect(after.events.map(e => e.event)).toEqual(expect.arrayContaining(['revoked', 'scrub']));
  expect(after.proposals.find(x => x.proposal === held.ok.proposal)).toMatchObject({ client: p.client, revoked: true });
  // Its next request is told it was revoked.
  expect((await post({ method: 'hello' }, p.token)).status).toBe(401);
  expect((await post({ method: 'revoke', client: p.client })).status).toBe(404);
});

test('a token past --pair-ttl ends as a revoke does', async () => {
  const p = await paired('read', 'tablet');
  const ends = [];
  tamber.pairing.onEnd(p.client, why => ends.push(why));
  now += 3600_000 + 1;
  try {
    expect((await post({ method: 'hello' }, p.token)).status).toBe(401);
    expect(ends).toEqual(['expired']);
  } finally { now -= 3600_000 + 1; }
});

test('a request revoked while its body comes is refused, and a local client may not take a paired name', async () => {
  const p = await paired('write', 'slow');
  const { request } = await import('node:http');
  const url = new URL(origin);
  const reply = new Promise(ok => {
    const req = request({ host: url.hostname, port: url.port, path: '/__tamber', method: 'POST', headers: { 'content-type': 'application/json', 'x-exact-tamber-token': p.token } }, res => {
      let text = ''; res.on('data', d => { text += d; }); res.on('end', () => ok({ status: res.statusCode, text }));
    });
    req.write('{"method": "transaction", ');
    globalThis.slowRequest = req;
  });
  await new Promise(ok => setTimeout(ok, 200));
  await post({ method: 'revoke', client: p.client });
  globalThis.slowRequest.end('"client": "x", "ops": []}');
  expect((await reply).status).toBe(401);
  const taken = await post({ method: 'propose', hold: true, client: 'remote:slow#2', ops: [] });
  expect([taken.status, taken.error.message]).toEqual([400, expect.stringContaining('paired client')]);
});

test('through a tunnel or the remote listener nothing is local, whatever else access says; a malformed code is 400', async () => {
  for (const f of [{ local: true, tunnel: true }, { local: true, remote: true }]) {
    flags = f;
    try {
      expect([f, (await fetch(`${origin}/__tamber/token`)).status]).toEqual([f, 404]);
      expect([f, (await fetch(`${origin}/__tamber/tool/`)).status]).toEqual([f, 404]);
      expect([f, (await post({ method: 'hello' })).status]).toEqual([f, 404]);
    } finally { flags = null; }
  }
  for (const code of [{ toString: null }, 12345, 'A'.repeat(40)]) expect((await fromPeer(() => post({ code }, null, '/__tamber/pair'))).status).toBe(400);
});

test('a token\'s own timer ends it, past a timer\'s reach too', async () => {
  const { pairing } = await import('./tamber-pairing.mjs');
  const short = pairing({ ttlMs: 400 });
  const p = short.pair({ code: short.mint({ scope: 'read' }).ok.code }, 'peer').ok;
  const ends = [];
  short.onEnd(p.client, why => ends.push(why));
  await new Promise(ok => setTimeout(ok, 700));
  expect(ends).toEqual(['expired']);
  expect(short.has(p.client)).toBe(false);
  // 30 days: armed in steps, alive until its time.
  let now = 0;
  const long = pairing({ ttlMs: 30 * 864e5, clock: () => now });
  const q = long.pair({ code: long.mint({ scope: 'read' }).ok.code }, 'peer').ok;
  now = 25 * 864e5;
  expect(long.lookup(q.token)?.client).toBe(q.client);
  now = 30 * 864e5 + 1;
  expect(long.lookup(q.token)).toBe(null);
  long.close();
});

test('plain HTTP from off this machine (a --lan bind\'s peer) reaches nothing of the wire: no pairing, no paired token, the code not spent', async () => {
  const { code } = (await post({ method: 'pair-code', scope: 'write', name: 'plain' })).ok;
  const p = await paired('write', 'plain-held');
  flags = { allowed: true, local: false, peer: '192.0.2.40' };
  try {
    expect((await post({ code }, null, '/__tamber/pair')).status).toBe(404);
    // Six misses from it: past both the limiter's five and the burn's five, had they counted.
    for (let i = 0; i < 6; i++) expect((await post({ code: 'x'.repeat(String(code).length) }, null, '/__tamber/pair')).status).toBe(404);
    expect((await post({ method: 'hello' }, p.token)).status).toBe(404);
    expect((await post({ after: 0, wait: false }, p.token, '/__tamber/events')).status).toBe(404);
    const res = await fetch(`${origin}/__tamber/subscribe`, { method: 'POST', headers: { 'x-exact-tamber-token': p.token }, body: '{}' });
    expect(res.status).toBe(404);
    await res.body?.cancel();
  } finally { flags = null; }
  // Nothing was spent: the same code pairs through the door from the same peer, and the token still works there.
  flags = { allowed: true, local: false, remote: true, peer: '192.0.2.40' };
  try { expect((await post({ code }, null, '/__tamber/pair')).ok.client).toBe('remote:plain#1'); } finally { flags = null; }
  expect((await fromPeer(() => post({ method: 'hello' }, p.token))).ok.client).toBe(p.client);
});

test('a LAN peer holding the local token is no one; a token past its deadline mid-body is 401; a deeply nested request is 400', async () => {
  flags = { allowed: true, local: false };
  try { expect((await post({ method: 'clients' })).status).toBe(404); } finally { flags = null; }
  const p = await paired('write', 'late');
  const { request } = await import('node:http');
  const url = new URL(origin);
  const reply = new Promise(ok => {
    const req = request({ host: url.hostname, port: url.port, path: '/__tamber', method: 'POST', headers: { 'x-exact-tamber-token': p.token } }, res => {
      let text = ''; res.on('data', d => { text += d; }); res.on('end', () => ok(res.statusCode));
    });
    req.write('{"method": "hello"');
    globalThis.lateRequest = req;
  });
  await new Promise(ok => setTimeout(ok, 200));
  now += 3600_000 + 1;
  try {
    globalThis.lateRequest.end('}');
    expect(await reply).toBe(401);
  } finally { now -= 3600_000 + 1; }
  let deep = 'null';
  for (let i = 0; i < 20000; i++) deep = `[${deep}]`;
  expect((await (() => fetch(`${origin}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': local }, body: `{"method": "hello", "deep": ${deep}}` }))()).status).toBe(400);
});

test('a scrub release whose client ends while the builds take the preview writes nothing', async () => {
  const { scrubSessions } = await import('./tamber-scrub.mjs');
  let let_go, writes = 0, ok = true;
  const sessions = scrubSessions({
    ask: async () => ({ ok: { class: 'live', files: { 'app.tamber': 'b' }, texts: { 'app.tamber': 't' } } }),
    write: async () => { writes++; return { ok: {} }; },
    preview: async () => ({}), restore: () => {},
    released: () => new Promise(done => { let_go = done; }),
    allowed: () => ok,
  });
  const opened = await sessions.update({ client: 'remote:a#1', ops: [{ op: 'set-token', name: 'ink', value: '#111111' }], files: { 'app.tamber': 'a' } });
  const release = sessions.release({ client: 'remote:a#1', scrub: opened.ok.scrub });
  await new Promise(done => setTimeout(done, 50));
  ok = false;
  let_go();
  expect((await release).status).toBe(401);
  expect(writes).toBe(0);
});

test('a long poll woken past its client\'s deadline is 401; a nested review is refused before it is kept; `revoke`\'s name is the RPC\'s alone', async () => {
  const p = await paired('read', 'poller');
  const seq = (await post({ after: 0, wait: false }, local, '/__tamber/events')).seq;
  const poll = post({ after: seq }, p.token, '/__tamber/events');
  await new Promise(ok => setTimeout(ok, 200));
  now += 3600_000 + 1;
  try {
    // An event wakes it, after its deadline and before its timer.
    await post({ method: 'propose', hold: true, client: 'tool', ops: [{ op: 'set-token', name: 'ink', value: '#141414' }] });
    expect((await poll).status).toBe(401);
  } finally { now -= 3600_000 + 1; }
  let deep = '0';
  for (let i = 0; i < 100; i++) deep = `[${deep}]`;
  const q = await paired('propose', 'nester');
  const nested = await fetch(`${origin}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': q.token }, body: `{"method": "propose", "hold": true, "ops": [], "review": {"extra": ${deep}}}` });
  expect(nested.status).toBe(400);
  // The streams still answer, every client's.
  expect((await post({ after: 0, wait: false }, local, '/__tamber/events')).status).toBe(200);
  const taken = await post({ method: 'revoke', client: 'remote:poller#1', wait: false }, local, '/__tamber/events');
  expect(taken.status).toBe(400);
  for (const x of (await post({ after: 0, wait: false }, local, '/__tamber/events')).proposals) await post({ method: 'drop', proposal: x.proposal, client: 'tool' });
});

test('a paired subscription past its deadline is sent nothing more, and ends', async () => {
  const p = await paired('read', 'watcher');
  const res = await fetch(`${origin}/__tamber/subscribe`, { method: 'POST', headers: { 'x-exact-tamber-token': p.token }, body: '{}' });
  const reader = res.body.getReader();
  let text = '';
  const ended = (async () => { for (;;) { const { done, value } = await reader.read().catch(() => ({ done: true })); if (done) return true; text += new TextDecoder().decode(value); } })();
  await new Promise(ok => setTimeout(ok, 200));
  now += 3600_000 + 1;
  try {
    await post({ method: 'propose', hold: true, client: 'tool', ops: [{ op: 'set-token', name: 'ink', value: '#151515' }] });
    expect(await ended).toBe(true);
    expect(text).not.toContain('#151515');
  } finally { now -= 3600_000 + 1; }
  for (const x of (await post({ after: 0, wait: false }, local, '/__tamber/events')).proposals) await post({ method: 'drop', proposal: x.proposal, client: 'tool' });
});

test('an expiry ended by the next request keeps every other subscriber\'s events in order', async () => {
  const p = await paired('read', 'orderly');
  const res = await fetch(`${origin}/__tamber/subscribe`, { method: 'POST', headers: { 'x-exact-tamber-token': local }, body: '{"client": "watch"}' });
  const reader = res.body.getReader();
  let text = '';
  void (async () => { for (;;) { const { done, value } = await reader.read().catch(() => ({ done: true })); if (done) return; text += new TextDecoder().decode(value); } })();
  await new Promise(ok => setTimeout(ok, 200));
  now += 3600_000 + 1;
  try {
    await post({ method: 'propose', hold: true, client: 'tool', ops: [{ op: 'set-token', name: 'ink', value: '#161616' }] });
    await new Promise(ok => setTimeout(ok, 300));
    const seqs = text.split('\n').filter(l => l.startsWith('data: ')).map(l => JSON.parse(l.slice(6))).filter(e => typeof e.seq === 'number' && e.event !== 'subscribed').map(e => [e.seq, e.event]);
    expect(seqs.some(([, e]) => e === 'revoked')).toBe(true);
    expect(seqs.map(([n]) => n)).toEqual([...seqs.map(([n]) => n)].sort((a, b) => a - b));
    expect(p.client).toEqual(expect.stringContaining('orderly'));
  } finally { now -= 3600_000 + 1; await reader.cancel().catch(() => {}); }
  for (const x of (await post({ after: 0, wait: false }, local, '/__tamber/events')).proposals) await post({ method: 'drop', proposal: x.proposal, client: 'tool' });
});

test('a scrub update or a held proposal past its client\'s deadline: the scrub ends, the proposal is marked', async () => {
  const p = await paired('write', 'scrubber');
  const sub = await subscribe(p.token, 'x');
  expect(sub.status).toBe(200);
  const files = { 'app.tamber': (await post({ method: 'hello' })).ok.files['app.tamber'] };
  const op = v => [{ op: 'set-token', name: 'ink', value: v }];
  expect((await post({ method: 'scrub', client: 'x', ops: op('#171717'), files }, p.token)).ok).toBeTruthy();
  const restored = restores;
  // Past the deadline, the scrub's own module asks before it plans.
  const { scrubSessions } = await import('./tamber-scrub.mjs');
  let ok = true;
  const sessions = scrubSessions({ ask: async () => ({ ok: { class: 'live', files: {}, texts: {} } }), write: async () => ({ ok: {} }), preview: async () => ({}), restore: () => {}, allowed: () => ok });
  const opened = await sessions.update({ client: 'remote:z#1', ops: op('#181818'), files: {} });
  ok = false;
  expect((await sessions.update({ client: 'remote:z#1', scrub: opened.ok.scrub, ops: op('#191919'), files: {} })).status).toBe(401);
  // Its client ends while the update plans: 401 after the wait too.
  let planned;
  const slow = scrubSessions({ ask: () => new Promise(done => { planned = done; }), write: async () => ({ ok: {} }), preview: async () => ({}), restore: () => {}, allowed: () => ok });
  ok = true;
  const pending = slow.update({ client: 'remote:z#2', ops: op('#1b1b1b'), files: {} });
  await new Promise(done => setTimeout(done, 20));
  ok = false;
  planned({ ok: { class: 'live', files: {}, texts: {} } });
  expect((await pending).status).toBe(401);
  // An opening scrub that fails with another update queued: that one is 409, not a crash.
  let first;
  const failing = scrubSessions({ ask: () => new Promise(done => { first = done; }), write: async () => ({ ok: {} }), preview: async () => ({}), restore: () => {}, allowed: () => true });
  const one = failing.update({ client: 'remote:z#3', ops: op('#1c1c1c'), files: {} });
  await new Promise(done => setTimeout(done, 20));
  const two = failing.update({ client: 'remote:z#3', ops: op('#1d1d1d'), files: {} });
  first({ status: 400, error: { message: 'refused' } });
  expect((await one).status).toBe(400);
  expect([409, 400]).toContain((await two).status);
  // Through the wire: the request after the deadline ends the client first (its scrub restored).
  now += 3600_000 + 1;
  try {
    expect((await post({ method: 'scrub', client: 'x', ops: op('#1a1a1a'), files }, p.token)).status).toBe(401);
    expect(restores).toBeGreaterThan(restored);
  } finally { now -= 3600_000 + 1; }
});


// LLP 1122 T7 (*1.14*): `navigate` needs write and a present client, goes to
// one build that can, a location on its own origin only, and is journaled
// with its client; a build that can't is 409 naming it (the tool then
// reloads that copy's frame instead).
test('navigate: write scope, a present client, one build that can, a location on its own origin', async () => {
  const frames = [], other = [];
  const s = tamber.channel.open(f => frames.push(f)), t = tamber.channel.open(f => other.push(f));
  tamber.channel.report(JSON.stringify({ nonce: s.nonce, kind: 'register', id: 'web-nav', host: 'web', capabilities: ['pin', 'navigate'] }));
  tamber.channel.report(JSON.stringify({ nonce: t.nonce, kind: 'register', id: 'ios-nav', host: 'ios', capabilities: ['pin'] }));
  const reader = await paired('propose', 'reader-nav');
  expect((await post({ method: 'navigate', client: 'x', build: 'web-nav', location: '/x' }, reader.token)).status).toBe(403);
  const p = await paired('write', 'writer-nav');
  expect((await post({ method: 'navigate', client: 'x', build: 'web-nav', location: '/x' }, p.token)).status).toBe(409);
  const sub = await subscribe(p.token, 'x');
  expect((await post({ method: 'navigate', client: 'x', location: '/x' }, p.token)).status).toBe(400);
  for (const bad of ['https://evil.example/x', '//evil.example/x', 'x', '/a b']) expect((await post({ method: 'navigate', client: 'x', build: 'web-nav', location: bad }, p.token)).status).toBe(400);
  const ios = await post({ method: 'navigate', client: 'x', build: 'ios-nav', location: '/x' }, p.token);
  expect([ios.status, ios.error.unsupported]).toEqual([409, 'navigate']);
  expect(other.some(f => f.kind === 'navigate')).toBe(false);
  const done = await post({ method: 'navigate', client: 'x', build: 'web-nav', location: '/posts/7#top' }, p.token);
  expect(done.ok).toEqual({ navigated: true, build: 'web-nav', seq: done.ok.seq });
  expect(frames.at(-1)).toEqual({ kind: 'navigate', seq: done.ok.seq, location: '/posts/7#top' });
  expect(logged.some(l => l.includes(`${p.client} navigated web-nav to "/posts/7#top"`))).toBe(true);
  await post({ method: 'revoke', client: p.client });
  await sub.ended;
  s.close(); t.close();
});
