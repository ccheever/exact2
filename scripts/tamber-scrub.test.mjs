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
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
import { developmentGate } from './install-page.mjs';
import { scrubSessions } from './tamber-scrub.mjs';
import { tamberServer } from './tamber-server.mjs';

const sha = text => createHash('sha256').update(text).digest('hex');
const op = value => [{ op: 'set-token', name: 'ink', value }];

/** A session over stand-ins: `class` the server's answer, every preview,
 * restore, write and end kept. */
function sessions({ cls = 'live', writeOk = true, idleMs } = {}) {
  const seen = { previews: [], restores: 0, writes: [], ended: [], released: 0 };
  const s = scrubSessions({
    ask: async r => ({ ok: { class: cls, texts: { 'app.tamber': `ink = ${r.ops[0].value}` }, changed: [], synthetic: [], files: r.files } }),
    write: async r => { seen.writes.push({ ...r, released: seen.released }); return writeOk ? { ok: { inverse: 'inv-1' } } : { error: { conflict: true, message: 'moved' } }; },
    preview: async texts => { seen.previews.push(texts['app.tamber']); return {}; },
    restore: () => { seen.restores++; },
    released: () => { seen.released++; },
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
  // The builds were told to keep the preview before the write.
  expect(seen.writes).toEqual([{ method: 'transaction', client: 'tool', files: { 'app.tamber': 'd0' }, ops: op('#222222'), released: 1 }]);
  expect(seen.restores).toBe(0);
  expect(s.current()).toBe(null);
});

/** A session whose dev loop patches: each frame folded (`scrub-fold`) and
 * patched; `fallback` makes the loop refuse a patch, `mismatch` its verify. */
function patching({ fallback = false, mismatch = false, paceMs = 0, verifyMs = 30 } = {}) {
  const seen = { asks: [], patches: [], previews: [], verified: [], restores: 0, ended: [], starts: [] };
  const s = scrubSessions({
    ask: async r => {
      seen.asks.push(r.method);
      return { ok: { class: 'live', values: [{ name: 'ink', css: [r.ops[0].value] }], texts: { 'app.tamber': `ink = ${r.ops[0].value}` }, changed: [], synthetic: [], files: r.files } };
    },
    write: async () => ({ ok: { inverse: 'inv-1' } }),
    patch: async (values, frame) => { seen.starts.push(Date.now()); seen.patches.push([values[0].css[0], frame.rev]); return fallback ? { fallback: true } : {}; },
    verify: async texts => { seen.verified.push(texts['app.tamber']); return mismatch ? { mismatch: 'ink' } : {}; },
    preview: async texts => { seen.previews.push(texts['app.tamber']); return {}; },
    restore: () => { seen.restores++; },
    ended: e => seen.ended.push(e.reason),
    paceMs,
    verifyMs,
  });
  return { s, seen };
}

test('a frame is folded and patched, compiling nothing, and verified once the gesture rests', async () => {
  const { s, seen } = patching();
  await s.update({ client: 'tool', ops: op('#111111'), files: { 'app.tamber': 'd0' } });
  await s.update({ client: 'tool', scrub: 'scrub-1', ops: op('#222222') });
  expect(seen.asks).toEqual(['scrub-fold', 'scrub-fold']);
  expect(seen.patches).toEqual([['#111111', 1], ['#222222', 2]]);
  expect(seen.previews).toEqual([]);
  await new Promise(ok => setTimeout(ok, 80));
  // Only the frame shown last, planned in full and compiled.
  expect(seen.verified).toEqual(['ink = #222222']);
  expect(s.current()).not.toBe(null);
});

test('a frame the loop cannot patch is planned and compiled, and not verified', async () => {
  const { s, seen } = patching({ fallback: true });
  await s.update({ client: 'tool', ops: op('#111111'), files: { 'app.tamber': 'd0' } });
  expect(seen.asks).toEqual(['scrub-fold', 'propose']);
  expect(seen.previews).toEqual(['ink = #111111']);
  await new Promise(ok => setTimeout(ok, 80));
  expect(seen.verified).toEqual([]);
});

test('a patched frame its compile disagrees with ends the scrub, restored', async () => {
  const { s, seen } = patching({ mismatch: true });
  await s.update({ client: 'tool', ops: op('#111111'), files: { 'app.tamber': 'd0' } });
  await new Promise(ok => setTimeout(ok, 80));
  expect(seen.ended).toEqual(['mismatch']);
  expect(seen.restores).toBe(1);
  expect(s.current()).toBe(null);
});

test('previews start at most one per pace, the latest value waiting', async () => {
  const { s, seen } = patching({ paceMs: 40 });
  const first = s.update({ client: 'tool', ops: op('#1'), files: { 'app.tamber': 'd0' } });
  await first;
  const second = s.update({ client: 'tool', scrub: 'scrub-1', ops: op('#2') });
  const third = s.update({ client: 'tool', scrub: 'scrub-1', ops: op('#3') });
  expect((await second).ok.superseded).toBe(true);
  await third;
  expect(seen.patches.map(([v]) => v)).toEqual(['#1', '#3']);
  expect(seen.starts[1] - seen.starts[0]).toBeGreaterThanOrEqual(38);
});

// Each way a scrub ends without release restores the committed design, and
// says so, with no write.
const opened = async (opts) => {
  const t = sessions(opts);
  await t.s.update({ client: 'tool', ops: op('#111111'), files: {} });
  return t;
};
const restored = ({ s, seen }, reason) => {
  expect(seen.restores).toBe(1);
  expect(seen.ended).toEqual([reason]);
  expect(seen.writes).toEqual([]);
  expect(s.current()).toBe(null);
};
test('a cancel ends a scrub and restores', async () => {
  const t = await opened();
  t.s.cancel({ client: 'tool', scrub: 'scrub-1' });
  restored(t, 'cancelled');
});
test('a competing write ends a scrub and restores', async () => {
  const t = await opened();
  t.s.compete();
  restored(t, 'competing write');
});
test('its client gone ends a scrub and restores; a present one keeps it', async () => {
  const kept = await opened();
  kept.s.absent(() => true);
  expect(kept.s.current()).toEqual({ scrub: 'scrub-1', client: 'tool' });
  const t = await opened();
  t.s.absent(() => false);
  restored(t, 'disconnected');
});
test('the design server lost ends a scrub and restores', async () => {
  const t = await opened();
  t.s.lost();
  restored(t, 'server lost');
});
test('30 s with no update ends a scrub and restores; each update\'s arrival counts', async () => {
  const t = await opened({ idleMs: 60 });
  await new Promise(ok => setTimeout(ok, 40));
  await t.s.update({ client: 'tool', scrub: 'scrub-1', ops: op('#222222') });
  await new Promise(ok => setTimeout(ok, 40));
  expect(t.s.current()).not.toBe(null);
  await new Promise(ok => setTimeout(ok, 60));
  restored(t, 'idle');
});
test('an end while the first update previews: no scrub opens or is announced, and the builds are restored', async () => {
  let release;
  const seen = { restores: 0, opened: 0, ended: 0 };
  const s = scrubSessions({
    ask: async r => ({ ok: { class: 'live', texts: { f: r.ops[0].value } } }),
    write: async () => ({ ok: {} }),
    preview: async () => { await new Promise(ok => { release = ok; }); return {}; },
    restore: () => { seen.restores++; },
    opened: () => { seen.opened++; },
    ended: () => { seen.ended++; },
  });
  const first = s.update({ client: 'tool', ops: op('#1'), files: {} });
  await new Promise(ok => setTimeout(ok, 5));
  const queued = s.update({ client: 'tool', ops: op('#2') });
  s.compete();
  release();
  expect((await first).status).toBe(409);
  expect((await queued).ok.ended).toBe('competing write');
  expect(s.current()).toBe(null);
  expect(seen).toEqual({ restores: 1, opened: 0, ended: 0 });
});

test('idle counts from the last update\'s arrival, however long its preview', async () => {
  const t0 = Date.now();
  let at = null;
  const s = scrubSessions({
    ask: async r => ({ ok: { class: 'live', texts: { f: r.ops[0].value } } }),
    write: async () => ({ ok: {} }),
    preview: async () => { await new Promise(ok => setTimeout(ok, 50)); return {}; },
    restore: () => {},
    ended: e => { if (e.reason === 'idle') at = Date.now() - t0; },
    idleMs: 80,
  });
  await s.update({ client: 'tool', ops: op('#1'), files: {} });
  await new Promise(ok => setTimeout(ok, 150));
  expect(at).toBeGreaterThanOrEqual(75);
  expect(at).toBeLessThan(125);
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
  // It names the scrub's opener.
  expect(second.error.message).toContain('`one`');
  expect((await s.release({ client: 'two', scrub: 'scrub-1' })).status).toBe(409);
});

test('a burst of 50 values: one previews at a time, and the last always lands', async () => {
  const shown = [];
  let inflight = 0, most = 0;
  const s = scrubSessions({
    ask: async r => ({ ok: { class: 'live', texts: { f: r.ops[0].value } } }),
    write: async () => ({ ok: {} }),
    preview: async texts => { inflight++; most = Math.max(most, inflight); await new Promise(ok => setTimeout(ok, 3)); shown.push(texts.f); inflight--; return {}; },
    restore: () => {},
  });
  const replies = [];
  for (let i = 0; i < 50; i++) {
    replies.push(s.update({ client: 'tool', ops: op(`#${i}`), files: {} }));
    await new Promise(ok => setTimeout(ok, i % 7));
  }
  const answered = await Promise.all(replies);
  expect(most).toBe(1);
  expect(shown.at(-1)).toBe('#49');
  expect(answered.at(-1).ok.superseded).toBeUndefined();
  expect(answered.every(a => a.ok)).toBe(true);
  s.cancel({ client: 'tool', scrub: 'scrub-1' });
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

test('through the server: a hand edit ends a scrub and restores', async () => {
  const file = join(dir, 'app.tamber');
  const tool = await present('tool');
  const files = () => ({ 'app.tamber': sha(readFileSync(file, 'utf8')) });
  const at = restores;
  const a = await post({ method: 'scrub', client: 'tool', ops: op('#121212'), files: files() });
  expect(a.ok?.scrub, JSON.stringify(a)).toBeString();
  writeFileSync(file, readFileSync(file, 'utf8').replace(/ink = .*/, 'ink = #343434'));
  await new Promise(ok => setTimeout(ok, 600));
  expect(restores).toBe(at + 1);
  expect((await post({ method: 'scrub-release', client: 'tool', scrub: a.ok.scrub })).status).toBe(409);
  tool.close();
  await new Promise(ok => setTimeout(ok, 300));
}, 60_000);

// A file edited by hand after the scrub opened, which the watcher has not
// said yet (its settle long here): the release conflicts, writes nothing,
// names who moved it, and restores.
test('through the server: a release whose file moved conflicts naming who moved it', async () => {
  const app = mkdtempSync(join(tmpdir(), 'exact-tamber-scrub-conflict-'));
  writeFileSync(join(app, 'app.contract'), 'component App\n  view\n    text "a" color="ink"\n');
  writeFileSync(join(app, 'app.tamber'), 'tamber t\n\ntheme\n  ink = #000000\n');
  let restored = 0;
  const local = createServer();
  await new Promise(ok => local.listen(0, '127.0.0.1', ok));
  const port = local.address().port;
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  const wire = tamberServer({ appDir: app, log: () => {}, settle: 30_000, scrub: { preview: async () => ({}), restore: () => { restored++; } } });
  local.on('request', async (req, res) => { if (!(await wire.handle(req, res, new URL(req.url, 'http://x'), gate.check(req)))) { res.writeHead(404); res.end(); } });
  const at = `http://127.0.0.1:${port}`;
  const { token } = await (await fetch(`${at}/__tamber/token`)).json();
  const h = { 'content-type': 'application/json', 'x-exact-tamber-token': token };
  const ask = async q => { const r = await fetch(`${at}/__tamber`, { method: 'POST', headers: h, body: JSON.stringify(q) }); return { status: r.status, ...(await r.json()) }; };
  await wire.ready;
  const abort = new AbortController();
  const sub = await fetch(`${at}/__tamber/subscribe`, { method: 'POST', headers: h, body: JSON.stringify({ client: 'tool' }), signal: abort.signal });
  void sub.body.getReader().read().catch(() => {});
  const file = join(app, 'app.tamber');
  const opened = await ask({ method: 'scrub', client: 'tool', ops: op('#565656'), files: { 'app.tamber': sha(readFileSync(file, 'utf8')) } });
  expect(opened.ok?.scrub, JSON.stringify(opened)).toBeString();
  writeFileSync(file, readFileSync(file, 'utf8').replace(/ink = .*/, 'ink = #787878'));
  const released = await ask({ method: 'scrub-release', client: 'tool', scrub: opened.ok.scrub });
  expect(released.status).toBe(409);
  expect(released.error?.conflict, JSON.stringify(released)).toBe(true);
  expect(released.error.by).toEqual({ 'app.tamber': 'changed on disk' });
  expect(readFileSync(file, 'utf8')).toContain('ink = #787878');
  expect(restored).toBe(1);
  abort.abort();
  await wire.close();
  local.close();
  rmSync(app, { recursive: true, force: true });
}, 600_000);

test('through the server: a cancel ends a scrub and restores', async () => {
  const file = join(dir, 'app.tamber');
  const tool = await present('tool');
  const files = () => ({ 'app.tamber': sha(readFileSync(file, 'utf8')) });
  const at = restores;
  const a = await post({ method: 'scrub', client: 'tool', ops: op('#9a9a9a'), files: files() });
  expect((await post({ method: 'scrub-cancel', client: 'tool', scrub: a.ok.scrub })).ok.ended).toBe('cancelled');
  expect(restores).toBe(at + 1);
  tool.close();
}, 60_000);

// The design server's loss, and idle, through a wire over a stand-in server
// whose death the test chooses.
test('the design server dying, and idle, each end a scrub and restore', async () => {
  let child;
  const spawnServe = () => {
    const { PassThrough } = require('node:stream');
    const { EventEmitter } = require('node:events');
    child = Object.assign(new EventEmitter(), { stdin: new PassThrough(), stdout: new PassThrough(), stderr: new PassThrough(), exitCode: null, signalCode: null, pid: 0 });
    child.kill = () => {};
    let buf = '';
    child.stdin.on('data', chunk => {
      buf += chunk; const lines = buf.split('\n'); buf = lines.pop();
      for (const line of lines) {
        const r = JSON.parse(line);
        const ok = r.method === 'propose' ? { class: 'live', texts: { 'app.tamber': 'x' }, changed: [], synthetic: [], files: {} } : { files: {} };
        child.stdout.write(`${JSON.stringify({ id: r.id, ok })}\n`);
      }
    });
    return child;
  };
  let restored = 0;
  const local = createServer();
  await new Promise(ok => local.listen(0, '127.0.0.1', ok));
  const port = local.address().port;
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  const wire = tamberServer({ appDir: '/nowhere', spawnServe, log: () => {}, scrubIdleMs: 150, scrub: { preview: async () => ({}), restore: () => { restored++; } } });
  local.on('request', async (req, res) => { if (!(await wire.handle(req, res, new URL(req.url, 'http://x'), gate.check(req)))) { res.writeHead(404); res.end(); } });
  const at = `http://127.0.0.1:${port}`;
  const { token } = await (await fetch(`${at}/__tamber/token`)).json();
  const h = { 'content-type': 'application/json', 'x-exact-tamber-token': token };
  const ask = async q => (await fetch(`${at}/__tamber`, { method: 'POST', headers: h, body: JSON.stringify(q) })).json();
  const abort = new AbortController();
  const sub = await fetch(`${at}/__tamber/subscribe`, { method: 'POST', headers: h, body: JSON.stringify({ client: 'tool' }), signal: abort.signal });
  void sub.body.getReader().read().catch(() => {});
  expect((await ask({ method: 'scrub', client: 'tool', ops: op('#1'), files: {} })).ok?.scrub).toBeString();
  await new Promise(ok => setTimeout(ok, 300));
  expect(restored).toBe(1);
  expect((await ask({ method: 'scrub', client: 'tool', ops: op('#2'), files: {} })).ok?.scrub).toBeString();
  child.exitCode = 1;
  child.emit('exit', 1);
  expect(restored).toBe(2);
  abort.abort();
  await wire.close();
  local.close();
}, 30_000);

test('an update naming a scrub that ended opens none', async () => {
  const t = await opened();
  t.s.cancel({ client: 'tool', scrub: 'scrub-1' });
  const late = await t.s.update({ client: 'tool', scrub: 'scrub-1', ops: op('#999999'), files: {} });
  expect(late.status).toBe(409);
  expect(t.s.current()).toBe(null);
  expect(t.seen.previews).toEqual(['ink = #111111']);
});

// `propose` through a loop whose pages write some tokens inline (the JS
// target): a live edit that moves one is `rebuild` there; one that moves
// none stays live; `texts` only when asked.
test('propose through a loop that writes a token inline is rebuild there, else as the session says', async () => {
  const asked = [];
  const spawnServe = () => {
    const { PassThrough } = require('node:stream');
    const { EventEmitter } = require('node:events');
    // A pid no process has (above macOS's and Linux's default limits): close
    // signals its group, and pid 0's group would be this test's own.
    const child = Object.assign(new EventEmitter(), { stdin: new PassThrough(), stdout: new PassThrough(), stderr: new PassThrough(), exitCode: null, signalCode: null, pid: 2 ** 30 });
    child.kill = () => {};
    let buf = '';
    child.stdin.on('data', chunk => {
      buf += chunk; const lines = buf.split('\n'); buf = lines.pop();
      for (const line of lines) {
        const r = JSON.parse(line);
        if (r.method === 'propose') asked.push(r.texts);
        const value = r.ops?.[0]?.value ?? '';
        const ok = r.method === 'propose' ? { class: value === 'rebuilds' ? 'rebuild' : 'live', texts: { 'app.tamber': `ink = ${value}` }, changed: [], synthetic: [], files: {}, diff: '' } : { files: {} };
        child.stdout.write(`${JSON.stringify({ id: r.id, ok })}\n`);
      }
    });
    return child;
  };
  const inlined = [];
  const inline = async texts => { inlined.push(texts['app.tamber']); return texts['app.tamber'].includes('600') ? '`ink` is written inline on this page (`font-weight`)' : null; };
  const local = createServer();
  await new Promise(ok => local.listen(0, '127.0.0.1', ok));
  const port = local.address().port;
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  const wire = tamberServer({ appDir: '/nowhere', spawnServe, log: () => {}, scrub: { preview: async () => ({}), restore: () => {}, inline } });
  local.on('request', async (req, res) => { if (!(await wire.handle(req, res, new URL(req.url, 'http://x'), gate.check(req)))) { res.writeHead(404); res.end(); } });
  const at = `http://127.0.0.1:${port}`;
  const { token } = await (await fetch(`${at}/__tamber/token`)).json();
  const h = { 'content-type': 'application/json', 'x-exact-tamber-token': token };
  const ask = async q => (await fetch(`${at}/__tamber`, { method: 'POST', headers: h, body: JSON.stringify(q) })).json();
  const weight = await ask({ method: 'propose', ops: [{ op: 'set-token', name: 'ink', value: '600' }] });
  expect(weight.ok.class).toBe('rebuild');
  expect(weight.ok.texts).toBeUndefined();
  const plain = await ask({ method: 'propose', ops: [{ op: 'set-token', name: 'ink', value: '#111111' }], texts: true });
  expect(plain.ok).toMatchObject({ class: 'live', texts: { 'app.tamber': 'ink = #111111' } });
  // A rebuild already: the loop is not asked.
  expect((await ask({ method: 'propose', ops: [{ op: 'set-token', name: 'ink', value: 'rebuilds' }] })).ok.class).toBe('rebuild');
  expect(inlined).toEqual(['ink = 600', 'ink = #111111']);
  expect(asked).toEqual([true, true, true]);
  await wire.close();
  local.close();
}, 30_000);
