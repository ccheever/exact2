// The design tool paired with a design server on another machine (LLP 1122
// T6, LLP 1121 stage 10): driven as a person would, through a door that
// serves the tool itself, against a real `tamber serve`. Its stub-server
// tests are in apps/design-tool/app.test.contract.
import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { DRIVEN, drive, lines, proxied, ROOT, stop, STUB_APP, toolTurns } from './design-tool-harness.mjs';
import { spawnOwned } from './test-children.mjs';

toolTurns();

test('T6: a tool on another machine pairs through the door with a code, at propose: its edit is held for review, what writes is never sent, and the code and token are in no URL, log, state or storage; a revoke ends it', async () => {
  const app = mkdtempSync(join(tmpdir(), 'exact-design-tool-pair-'));
  writeFileSync(join(app, 'app.tamber'), readFileSync(resolve(ROOT, 'apps/design-tool/stub/dials.tamber'), 'utf8'));
  writeFileSync(join(app, 'app.contract'), STUB_APP);
  const git = (...args) => spawnSync('git', ['-c', 'user.email=t@example.test', '-c', 'user.name=t', ...args], { cwd: app });
  git('init', '-q'); git('add', '.'); git('commit', '-q', '-m', 'design');
  // The tool built for this port (as `tamber tool` builds it), then nothing at its origin: this browser is
  // the other machine's, which has no server of its own.
  await (await proxied(app)).close();
  const serve = spawnOwned(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'serve', app], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
  const storage = `design-tool-pair-${process.pid}`;
  let door;
  try {
    const origin = new URL(await lines(serve).until(l => /^http:\/\/127\.0\.0\.1:\d+\/__tamber$/.test(l), 600_000)).origin;
    // The other machine's door: a tunnel to it on this machine's loopback, recording what passes (the
    // server's own tool files, its wire), and the token pairing hands out, to look for it after.
    const seen = [];
    let token = '';
    door = Bun.serve({ hostname: '127.0.0.1', port: 0, idleTimeout: 0, fetch: async req => {
      const url = new URL(req.url), body = req.method === 'POST' ? await req.text() : undefined;
      seen.push({ path: `${url.pathname}${url.search}`, method: req.method, body: url.pathname === '/__tamber/pair' ? '' : body ?? '' });
      const headers = new Headers(req.headers);
      headers.delete('host'); headers.delete('content-length'); headers.delete('origin');
      const res = await fetch(`${origin}${url.pathname}${url.search}`, { method: req.method, headers, body, redirect: 'manual' });
      const bytes = await res.arrayBuffer();
      if (url.pathname === '/__tamber/pair' && res.ok) token = JSON.parse(new TextDecoder().decode(bytes)).ok.token;
      const out = new Headers(res.headers);
      out.delete('content-encoding'); out.delete('content-length');
      return new Response(bytes, { status: res.status, headers: out });
    } });
    const local = (await (await fetch(`${origin}/__tamber/token`)).json()).token;
    const ask = async body => (await fetch(`${origin}/__tamber`, { method: 'POST', headers: { 'content-type': 'application/json', 'x-exact-tamber-token': local }, body: JSON.stringify(body) })).json();
    const { code } = (await ask({ method: 'pair-code', scope: 'propose', name: 'laptop' })).ok;
    const at = `http://127.0.0.1:${door.port}`;
    const replies = await drive(`${at}/__tamber/tool/`, storage, [
      'type name Ada', 'tap save-name', 'clock data',
      `type remote-at ${at}`, `type remote-code ${code}`, 'tap remote-pair', 'clock data', 'clock +3000', 'clock data',
      'type dial-hue-stepper 210', 'clock data', 'tap apply', 'clock data',
      'state', 'logs --all',
    ]);
    const state = replies.filter(r => r.resources).at(-1);
    expect(state.resources.hello).toMatchObject({ ok: true, scope: 'propose', local: false, client: 'remote:Ada#1' });
    expect(state.derives.canWrite).toBe(false);
    expect(state.derives.stack.note).toContain('Held for review as');
    expect((await ask({ method: 'clients' })).ok.clients.map(c => [c.client, c.scope])).toEqual([['local', 'write'], ['remote:Ada#1', 'propose']]);
    // What went through the door: the pairing once, then the paired client's own requests; no write (above
    // its scope), and never the code or the token in a URL.
    const methods = seen.filter(s => s.path === '/__tamber').map(s => JSON.parse(s.body).method);
    expect(seen.filter(s => s.path === '/__tamber/pair')).toHaveLength(1);
    expect(methods).toContain('propose');
    for (const m of ['transaction', 'pin', 'hold', 'outline', 'commit', 'pair-code', 'clients']) expect(methods).not.toContain(m);
    expect(token.length).toBeGreaterThan(20);
    for (const s of seen) for (const secret of [token, code]) expect(s.path).not.toContain(secret);
    // Nor in the drive's every reply (the tool's logs, its state), nor anywhere in the browser's profile
    // on disk (its local and session storage, IndexedDB, caches), once that browser has quit.
    const said = JSON.stringify(replies);
    for (const secret of [token, code, code.replace('-', '')]) expect(said).not.toContain(secret);
    const { driveStore } = await import('./agent-launch.mjs');
    const { resolveApp } = await import('./app.mjs');
    const before = process.env.EXACT_APP_DIR;
    process.env.EXACT_APP_DIR = DRIVEN.env.EXACT_APP_DIR ?? resolve(ROOT, 'apps/design-tool');
    const profile = driveStore(resolveApp(), storage, DRIVEN.env).profile;
    if (before === undefined) delete process.env.EXACT_APP_DIR; else process.env.EXACT_APP_DIR = before;
    const files = [];
    const walk = dir => { for (const e of readdirSync(dir, { withFileTypes: true })) { const p = join(dir, e.name); if (e.isDirectory()) walk(p); else if (e.isFile()) files.push(p); } };
    walk(profile);
    expect(files.length).toBeGreaterThan(0);
    for (const f of files) { const bytes = readFileSync(f); for (const secret of [token, code]) expect(bytes.includes(secret) ? f : '').toBe(''); }
    // Revoked here: the token is refused at once (401), and the server lists the client no more.
    expect((await ask({ method: 'revoke', client: 'remote:Ada#1' })).ok).toEqual({ revoked: 'remote:Ada#1' });
    expect((await fetch(`${origin}/__tamber`, { method: 'POST', headers: { 'content-type': 'application/json', 'x-exact-tamber-token': token }, body: '{"method":"hello"}' })).status).toBe(401);
  } finally {
    door?.stop(true);
    await stop(serve);
    rmSync(app, { recursive: true, force: true });
  }
}, 900_000);
