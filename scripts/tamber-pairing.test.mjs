// The wire's scopes (LLP 1121 stage 10 A1; scripts/tamber-pairing.mjs): one
// table, checked in the dispatcher before any handler, failing closed.
import { afterAll, beforeAll, expect, test } from 'bun:test';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { developmentGate } from './install-page.mjs';
import { allows, METHOD_SCOPE, needs, SCOPES } from './tamber-pairing.mjs';
import { METHODS, tamberServer } from './tamber-server.mjs';

test('every method the wire serves has a scope, and the table names no other', () => {
  for (const method of METHODS) expect([method, METHOD_SCOPE[method]]).toEqual([method, expect.any(String)]);
  const streams = new Set(['subscribe', 'events']);
  for (const method of Object.keys(METHOD_SCOPE)) expect(METHODS.has(method) || streams.has(method)).toBe(true);
  for (const scope of Object.values(METHOD_SCOPE)) expect([...SCOPES, 'local']).toContain(scope);
  // Ruling 5: inspect reads; what changes the developer's screens writes.
  expect(['inspect', 'hold', 'pin', 'outline'].map(m => METHOD_SCOPE[m])).toEqual(['read', 'write', 'write', 'write']);
});

test('scopes are ordered, `local` is the local token\'s alone, and an unknown scope allows nothing', () => {
  expect(allows('write', 'propose')).toBe(true);
  expect(allows('propose', 'write')).toBe(false);
  expect(allows('read', 'read')).toBe(true);
  expect(allows('write', 'local', false)).toBe(false);
  expect(allows('read', 'local', true)).toBe(true);
  expect(allows('admin', 'read')).toBe(false);
  expect(allows('write', 'nothing')).toBe(false);
  // A method with no entry needs a scope no client has.
  expect(needs({ method: 'shutdown' })).toBe(null);
  // `drop`: one's own held proposal is `propose`, another's `write`.
  const owner = id => (id === 'p1' ? 'tool' : null);
  expect(needs({ method: 'drop', proposal: 'p1', client: 'tool' }, owner)).toBe('propose');
  expect(needs({ method: 'drop', proposal: 'p1', client: 'other' }, owner)).toBe('write');
});

let dir, server, tamber, origin, headers, scope = 'write';
beforeAll(async () => {
  dir = mkdtempSync(join(tmpdir(), 'exact-tamber-scopes-'));
  writeFileSync(join(dir, 'app.contract'), 'component App\n  view\n    text "a" color="ink"\n');
  writeFileSync(join(dir, 'app.tamber'), 'tamber t\n\ntheme\n  ink = #000000\n');
  server = createServer();
  await new Promise(ok => server.listen(0, '127.0.0.1', ok));
  const port = server.address().port;
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  tamber = tamberServer({ appDir: dir, log: () => {}, settle: 100, scopeOf: () => scope });
  server.on('request', async (req, res) => {
    if (!(await tamber.handle(req, res, new URL(req.url, 'http://x'), gate.check(req)))) { res.writeHead(404); res.end(); }
  });
  origin = `http://127.0.0.1:${port}`;
  const { token } = await (await fetch(`${origin}/__tamber/token`)).json();
  headers = { 'content-type': 'application/json', 'x-exact-tamber-token': token };
  await tamber.ready;
}, 600_000); // a cold checkout builds the compiler first
afterAll(async () => {
  await tamber?.close();
  server?.close();
  if (dir) rmSync(dir, { recursive: true, force: true });
});
const post = async (request, as = 'write', path = '/__tamber') => {
  scope = as;
  try {
    const res = await fetch(`${origin}${path}`, { method: 'POST', headers, body: JSON.stringify(request) });
    return { status: res.status, ...(path === '/__tamber' ? await res.json() : {}) };
  } finally { scope = 'write'; }
};

test('through the server: each client is refused every method above its scope, naming both scopes', async () => {
  for (const [as, below] of [['read', ['propose', 'write']], ['propose', ['write']]]) {
    for (const [method, need] of Object.entries(METHOD_SCOPE)) {
      if (!below.includes(need) || !METHODS.has(method)) continue;
      const refused = await post({ method, client: 'remote' }, as);
      expect([as, method, refused.status, refused.error?.scope, refused.error?.needs]).toEqual([as, method, 403, as, need]);
      expect(refused.error.message).toContain(`\`${need}\` scope`);
    }
  }
});

test('through the server: hello says the client\'s scope; a read client streams; a propose client drops only its own proposal', async () => {
  const hello = await post({ method: 'hello' }, 'propose');
  expect([hello.ok.scope, hello.ok.local]).toEqual(['propose', true]);
  expect((await post({ after: 0, wait: false }, 'read', '/__tamber/events')).status).toBe(200);
  const ops = [{ op: 'set-token', name: 'ink', value: '#111111' }];
  const mine = await post({ method: 'propose', hold: true, client: 'remote', ops }, 'propose');
  expect(mine.ok.proposal).toEqual(expect.any(String));
  const theirs = await post({ method: 'propose', hold: true, client: 'tool', ops });
  const refused = await post({ method: 'drop', proposal: theirs.ok.proposal, client: 'remote' }, 'propose');
  expect([refused.status, refused.error.needs]).toEqual([403, 'write']);
  expect((await post({ method: 'drop', proposal: mine.ok.proposal, client: 'remote' }, 'propose')).ok).toEqual({ dropped: mine.ok.proposal });
  expect((await post({ method: 'drop', proposal: theirs.ok.proposal, client: 'tool' })).ok).toEqual({ dropped: theirs.ok.proposal });
});

test('through the server: inspect below write reads the last selections and asks no build; `on` is refused', async () => {
  const on = await post({ method: 'inspect', client: 'remote', on: true }, 'read');
  expect([on.status, on.error.needs]).toEqual([403, 'write']);
  const frames = [];
  const build = tamber.channel.open(f => frames.push(f));
  tamber.channel.report(JSON.stringify({ nonce: build.nonce, kind: 'register', id: 'web-1', host: 'web', capabilities: ['inspect'] }));
  const sent = frames.length;
  const read = await post({ method: 'inspect', client: 'remote', refresh: true }, 'read');
  expect([read.status, read.ok]).toEqual([200, { selections: [] }]);
  // Nothing went to the build: its answer would outline on the developer's screens.
  expect(frames.length).toBe(sent);
});

test('through the server: the change stream is by the table, refused in the same envelope; no inherited name is a method', async () => {
  const refused = await post({ client: 'x' }, 'nobody', '/__tamber/subscribe');
  expect(refused.status).toBe(403);
  const res = await fetch(`${origin}/__tamber/events`, { method: 'POST', headers: { ...headers }, body: '{"wait": false}' });
  expect(res.status).toBe(200);
  scope = 'nobody';
  try {
    const body = await (await fetch(`${origin}/__tamber/events`, { method: 'POST', headers, body: '{"wait": false}' })).json();
    expect(body.error).toMatchObject({ scope: 'nobody', needs: 'read' });
  } finally { scope = 'write'; }
  for (const method of ['__proto__', 'constructor', 'toString', 'Hello']) expect((await post({ method })).status).toBe(400);
  expect(needs({ method: '__proto__' })).toBe(null);
});

test('through the server: a client below write makes room only from its own held proposals', async () => {
  const ops = [{ op: 'set-token', name: 'ink', value: '#121212' }];
  const tools = await post({ method: 'propose', hold: true, client: 'tool', ops });
  let first = null;
  for (let i = 0; i < 64; i++) {
    const r = await post({ method: 'propose', hold: true, client: 'remote', ops }, 'propose');
    if (r.status !== 200) break;
    first ??= r.ok.proposal;
  }
  const ids = (await (await fetch(`${origin}/__tamber/events`, { method: 'POST', headers, body: '{"wait": false}' })).json()).proposals.map(p => p.proposal);
  expect(ids).toContain(tools.ok.proposal);
  expect(ids).not.toContain(first);
  const none = await post({ method: 'propose', hold: true, client: 'other', ops }, 'propose');
  expect(none.status).toBe(409);
  for (const id of ids) await post({ method: 'drop', proposal: id, client: 'tool' });
});

test('through the server: at the cap, overlapping holds below write still drop no one else\'s; a hold that fails drops nothing', async () => {
  const ops = [{ op: 'set-token', name: 'ink', value: '#131313' }];
  const tools = [];
  for (let i = 0; i < 63; i++) tools.push((await post({ method: 'propose', hold: true, client: 'tool', ops })).ok.proposal);
  // Two at once, from two clients with none of their own: one fits, one is refused.
  scope = 'propose';
  const both = await Promise.all(['remote-a', 'remote-b'].map(client => fetch(`${origin}/__tamber`, { method: 'POST', headers, body: JSON.stringify({ method: 'propose', hold: true, client, ops }) }).then(r => r.status)));
  scope = 'write';
  expect(both.sort()).toEqual([200, 409]);
  const ids = () => fetch(`${origin}/__tamber/events`, { method: 'POST', headers, body: '{"wait": false}' }).then(r => r.json()).then(b => b.proposals.map(p => p.proposal));
  for (const id of tools) expect(await ids()).toContain(id);
  // At the cap, a hold that is refused or proposes nothing keeps the caller's own.
  const mine = (await ids()).find(id => !tools.includes(id));
  const owner = (await (await fetch(`${origin}/__tamber/events`, { method: 'POST', headers, body: '{"wait": false}' })).json()).proposals.find(p => p.proposal === mine).client;
  expect((await post({ method: 'import', client: owner, kind: 'nothing', source: '' }, 'propose')).status).toBe(400);
  expect((await post({ method: 'propose', hold: true, client: owner, ops: [{ op: 'set-token', name: 'nope', value: '#000000' }] }, 'propose')).status).not.toBe(200);
  expect(await ids()).toContain(mine);
  for (const id of await ids()) await post({ method: 'drop', proposal: id, client: 'tool' });
});

test('through the server: an export below write keeps no baseline', async () => {
  const read = await post({ method: 'export', kind: 'dtcg' }, 'read');
  expect([read.status, read.ok.baseline]).toEqual([200, null]);
  const local = await post({ method: 'export', kind: 'dtcg' });
  expect(local.ok.baseline).toEqual(expect.stringContaining('.exact/tamber-dtcg/'));
});

test('a scope the wire can\'t name allows nothing; a read client reads a build\'s selection until the build has none', async () => {
  scope = undefined;
  try {
    const res = await fetch(`${origin}/__tamber`, { method: 'POST', headers, body: JSON.stringify({ method: 'transaction', client: 'x', ops: [] }) });
    expect(res.status).toBe(403);
  } finally { scope = 'write'; }
  // A second server, in process, with a map to join a selection to.
  const { PassThrough } = await import('node:stream');
  const { EventEmitter } = await import('node:events');
  const other = mkdtempSync(join(tmpdir(), 'exact-tamber-scopes-sel-'));
  writeFileSync(join(other, 'app.contract'), 'component App\n  view\n    text "a" color="ink"\n');
  writeFileSync(join(other, 'app.tamber'), 'tamber t\n\ntheme\n  ink = #000000\n');
  const map = { digest: 'p1', sources: [{ file: 'exact:app', sha256: 'x' }], nodes: [{ file: 'exact:app', line: 3, col: 5, end_col: 9, component: 'App', chain: [], bindings: [{ row: 'background_color', origin: 'own' }] }] };
  let as = 'write';
  const sel = tamberServer({ appDir: other, log: () => {}, scopeOf: () => as, sourceMap: () => Buffer.from(JSON.stringify(map)) });
  try {
    await sel.ready;
    const ask = async (request) => {
      const req = Object.assign(new PassThrough(), { method: 'POST', headers: { 'x-exact-tamber-token': sel.token }, socket: { remoteAddress: '127.0.0.1' } });
      let status = 0, text = '';
      const res = Object.assign(new EventEmitter(), { destroyed: false, writableEnded: false, writeHead(s) { status = s; }, setHeader() {}, write(t) { text += t; }, end(t) { if (t) text += t; this.writableEnded = true; } });
      const done = sel.handle(req, res, new URL('http://x/__tamber'), { local: true });
      req.end(JSON.stringify(request));
      await done;
      return { status, ...JSON.parse(text || '{}') };
    };
    const { nonce } = sel.channel.open(() => {});
    const report = body => sel.channel.report(JSON.stringify({ nonce, ...body }));
    report({ kind: 'register', id: 'web-1', host: 'web', capabilities: ['inspect'], plan: 'g1', design: 'd2' });
    report({ kind: 'status', scheme: 'light', revision: 0, design: 'd2' });
    sel.channel.inspect('tool', null, { on: true });
    const selected = { node: 2, site: 0, instance: [], planDigest: 'p1', tuple: {}, rows: [] };
    expect(report({ kind: 'selected', selected, plan: 'g1', design: 'd2', revision: 0 }).status).toBe(204);
    as = 'read';
    expect((await ask({ method: 'inspect', client: 'remote', refresh: true })).ok.selections.length).toBe(1);
    // Inspect mode left on the build, which reports no selection: none to
    // read. (The ask above found `tool` absent and ended its inspect mode,
    // the command the build's report now names: 2.)
    expect(report({ kind: 'selected', selected: null, plan: 'g1', design: 'd2', revision: 0, inspect: 2 }).status).toBe(204);
    expect((await ask({ method: 'inspect', client: 'remote', refresh: true })).ok.selections).toEqual([]);
  } finally {
    await sel.close();
    rmSync(other, { recursive: true, force: true });
  }
});

