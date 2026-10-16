// Scrub sessions (LLP 1121 D13, LLP 1122 D6; stage 4, commit 6): a scrub
// previews on the builds and writes once; each way it ends restores the
// committed design; a rebuild-class edit, a second client and a
// `propose`-scope client open none. Through the real design server too.
import { afterAll, beforeAll, expect, test } from 'bun:test';
import { createHash } from 'node:crypto';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { developmentGate } from './install-page.mjs';
import { scrubSessions } from './tamber-scrub.mjs';
import { tamberServer } from './tamber-server.mjs';

const sha = text => createHash('sha256').update(text).digest('hex');
const op = value => [{ op: 'set-token', name: 'ink', value }];

/** A session over stand-ins: `class` the server's answer, every preview,
 * restore, write and end kept. */
function sessions({ cls = 'live', writeOk = true, idleMs } = {}) {
  const seen = { previews: [], restores: 0, writes: [], ended: [] };
  const s = scrubSessions({
    ask: async r => ({ ok: { class: cls, texts: { 'app.tamber': `ink = ${r.ops[0].value}` }, changed: [], synthetic: [], files: r.files } }),
    write: async r => { seen.writes.push(r); return writeOk ? { ok: { inverse: 'inv-1' } } : { error: { conflict: true, message: 'moved' } }; },
    preview: async texts => { seen.previews.push(texts['app.tamber']); return {}; },
    restore: () => { seen.restores++; },
    ended: e => seen.ended.push(e.reason),
    idleMs,
  });
  return { s, seen };
}

test('a scrub previews each value and writes once, on release, on the digests it opened with', async () => {
  const { s, seen } = sessions();
  const first = await s.update({ client: 'tool', ops: op('#111111'), files: { 'app.tamber': 'd0' } });
  expect(first.ok.scrub).toBe('scrub-1');
  await s.update({ client: 'tool', scrub: 'scrub-1', ops: op('#222222') });
  expect(seen.previews).toEqual(['ink = #111111', 'ink = #222222']);
  expect(seen.writes).toEqual([]);
  const released = await s.release({ client: 'tool', scrub: 'scrub-1' });
  expect(released.ok.scrub).toBe('scrub-1');
  expect(seen.writes).toEqual([{ method: 'transaction', client: 'tool', files: { 'app.tamber': 'd0' }, ops: op('#222222') }]);
  expect(seen.restores).toBe(0);
  expect(s.current()).toBe(null);
});

test('each way a scrub ends without release restores the committed design', async () => {
  for (const [how, reason] of [
    [s => s.cancel({ client: 'tool', scrub: 'scrub-1' }), 'cancelled'],
    [s => s.compete(), 'competing write'],
    [s => s.absent(() => false), 'disconnected'],
    [s => s.lost(), 'server lost'],
  ]) {
    const { s, seen } = sessions();
    await s.update({ client: 'tool', ops: op('#111111'), files: {} });
    how(s);
    expect(seen.restores).toBe(1);
    expect(seen.ended).toEqual([reason]);
    expect(seen.writes).toEqual([]);
    expect(s.current()).toBe(null);
  }
  const { s, seen } = sessions({ idleMs: 30 });
  await s.update({ client: 'tool', ops: op('#111111'), files: {} });
  await new Promise(ok => setTimeout(ok, 80));
  expect(seen.ended).toEqual(['idle']);
  expect(seen.restores).toBe(1);
  // A present client keeps it.
  const kept = sessions();
  await kept.s.update({ client: 'tool', ops: op('#111111'), files: {} });
  kept.s.absent(() => true);
  expect(kept.s.current()).toEqual({ scrub: 'scrub-1', client: 'tool' });
  kept.s.cancel({ client: 'tool', scrub: 'scrub-1' });
});

test('a release that conflicts writes nothing and restores', async () => {
  const { s, seen } = sessions({ writeOk: false });
  await s.update({ client: 'tool', ops: op('#111111'), files: {} });
  const released = await s.release({ client: 'tool', scrub: 'scrub-1' });
  expect(released.error.conflict).toBe(true);
  expect(seen.restores).toBe(1);
  expect(seen.ended).toEqual(['refused']);
});

test('no scrub for a rebuild-class edit, and one at a time', async () => {
  const rebuild = sessions({ cls: 'rebuild' });
  const refused = await rebuild.s.update({ client: 'tool', ops: op('#111111'), files: {} });
  expect(refused.status).toBe(409);
  expect(rebuild.seen.previews).toEqual([]);
  const { s } = sessions();
  await s.update({ client: 'one', ops: op('#111111'), files: {} });
  const second = await s.update({ client: 'two', ops: op('#222222'), files: {} });
  expect(second.status).toBe(409);
  expect((await s.release({ client: 'two', scrub: 'scrub-1' })).status).toBe(409);
});

test('updates while one previews: only the latest is shown', async () => {
  const shown = [];
  let unblock;
  const s = scrubSessions({
    ask: async r => ({ ok: { class: 'live', texts: { f: r.ops[0].value } } }),
    write: async () => ({ ok: {} }),
    preview: async texts => { shown.push(texts.f); if (shown.length === 1) await new Promise(ok => { unblock = ok; }); return {}; },
    restore: () => {},
  });
  const a = s.update({ client: 'tool', ops: op('#1'), files: {} });
  await new Promise(ok => setTimeout(ok, 5));
  const b = s.update({ client: 'tool', ops: op('#2') });
  const c = s.update({ client: 'tool', ops: op('#3') });
  unblock();
  expect((await b).ok.superseded).toBe(true);
  expect((await a).ok.scrub).toBe('scrub-1');
  expect((await c).ok.scrub).toBe('scrub-1');
  expect(shown).toEqual(['#1', '#3']);
  s.cancel({ client: 'tool', scrub: 'scrub-1' });
});

// Through the wire and the real design server.
let dir, server, tamber, origin, headers, scope = 'write';
const previews = [];
let restores = 0;
beforeAll(async () => {
  dir = mkdtempSync(join(tmpdir(), 'exact-tamber-scrub-'));
  writeFileSync(join(dir, 'app.contract'), 'component App\n  view\n    text "a" color="ink"\n');
  writeFileSync(join(dir, 'app.tamber'), 'tamber t\n\ntheme\n  ink = #000000\n');
  server = createServer();
  await new Promise(ok => server.listen(0, '127.0.0.1', ok));
  const port = server.address().port;
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  tamber = tamberServer({
    appDir: dir, log: () => {}, settle: 100, scopeOf: () => scope,
    scrub: { preview: async texts => { previews.push(texts['app.tamber']); return {}; }, restore: () => { restores++; } },
  });
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
const post = async request => {
  const res = await fetch(`${origin}/__tamber`, { method: 'POST', headers, body: JSON.stringify(request) });
  return { status: res.status, ...(await res.json()) };
};
/** A subscription naming `client`, which makes it present; `close()` ends it. */
async function present(client) {
  const abort = new AbortController();
  const res = await fetch(`${origin}/__tamber/subscribe`, { method: 'POST', headers, body: JSON.stringify({ client }), signal: abort.signal });
  const reader = res.body.getReader();
  void (async () => { for (;;) { const { done } = await reader.read().catch(() => ({ done: true })); if (done) return; } })();
  return { close: () => abort.abort() };
}

test('through the server: previews on the builds, the file untouched until release, written once', async () => {
  const file = join(dir, 'app.tamber');
  const before = readFileSync(file, 'utf8');
  const tool = await present('tool');
  const opened = await post({ method: 'scrub', client: 'tool', ops: op('#111111'), files: { 'app.tamber': sha(before) } });
  expect(opened.ok?.scrub, JSON.stringify(opened)).toBeString();
  await post({ method: 'scrub', client: 'tool', scrub: opened.ok.scrub, ops: op('#222222') });
  expect(previews.at(-1)).toContain('ink = #222222');
  expect(readFileSync(file, 'utf8')).toBe(before);
  const released = await post({ method: 'scrub-release', client: 'tool', scrub: opened.ok.scrub });
  expect(released.ok?.inverse, JSON.stringify(released)).toBeString();
  expect(readFileSync(file, 'utf8')).toContain('ink = #222222');
  tool.close();
}, 60_000);

test('through the server: a competing write and a disconnect each end a scrub and restore', async () => {
  const file = join(dir, 'app.tamber');
  const tool = await present('tool');
  const at = restores;
  const opened = await post({ method: 'scrub', client: 'tool', ops: op('#333333'), files: { 'app.tamber': sha(readFileSync(file, 'utf8')) } });
  expect(opened.ok?.scrub, JSON.stringify(opened)).toBeString();
  const other = await post({ method: 'transaction', client: 'agent', files: { 'app.tamber': sha(readFileSync(file, 'utf8')) }, ops: op('#444444') });
  expect(other.ok?.inverse, JSON.stringify(other)).toBeString();
  expect(restores).toBe(at + 1);
  expect((await post({ method: 'scrub-release', client: 'tool', scrub: opened.ok.scrub })).status).toBe(409);
  const again = await post({ method: 'scrub', client: 'tool', ops: op('#555555'), files: { 'app.tamber': sha(readFileSync(file, 'utf8')) } });
  expect(again.ok?.scrub).toBeString();
  tool.close();
  await new Promise(ok => setTimeout(ok, 200));
  expect(restores).toBe(at + 2);
}, 60_000);

test('through the server: a propose-scope client opens no scrub, and an absent one neither', async () => {
  const file = join(dir, 'app.tamber');
  const files = { 'app.tamber': sha(readFileSync(file, 'utf8')) };
  scope = 'propose';
  const refused = await post({ method: 'scrub', client: 'tool', ops: op('#666666'), files });
  scope = 'write';
  expect(refused.status).toBe(403);
  expect((await post({ method: 'scrub', client: 'nobody', ops: op('#666666'), files })).status).toBe(409);
});
