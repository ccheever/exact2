// The design tool paired with a design server on another machine (LLP 1122
// T6, LLP 1121 stage 10): driven as a person would, through the server's
// trusted-tunnel door, which serves the tool itself, against a real `tamber
// serve`. Its stub-server tests are apps/design-tool/*.test.contract.
import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { DRIVEN, drive, lines, ROOT, stop, STUB_APP, TOOL_ORIGIN, toolTurns } from './design-tool-harness.mjs';
import { spawnOwned } from './test-children.mjs';
import { upstream } from './tamber-request.mjs';

toolTurns();

test('T6: a tool on another machine pairs through the trusted-tunnel door with a code, at propose: its edit is held for review, nothing above its scope goes through, the code and token are in no URL, log, state, browser profile or server output; revoked while it runs, it says so and sends no more', async () => {
  const app = mkdtempSync(join(tmpdir(), 'exact-design-tool-pair-'));
  writeFileSync(join(app, 'app.tamber'), readFileSync(resolve(ROOT, 'apps/design-tool/stub/dials.tamber'), 'utf8'));
  writeFileSync(join(app, 'app.contract'), STUB_APP);
  const git = (...args) => spawnSync('git', ['-c', 'user.email=t@example.test', '-c', 'user.name=t', ...args], { cwd: app });
  git('init', '-q'); git('add', '.'); git('commit', '-q', '-m', 'design');
  // The tool built for this port (as `tamber tool` builds it), then nothing at its origin: this browser is
  // the other machine's, which has no server of its own.
  // (Its own server, not --via, whose 2 s probe of a dev loop can time out under load.)
  const build = spawnOwned(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'tool', app, '--no-open'], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
  await lines(build).until(l => l.startsWith(`${TOOL_ORIGIN}/__tamber/tool/`), 600_000);
  await stop(build);
  // The server's trusted-tunnel door (stage 10 B2: routed as a remote one, its Origin check, its paths), on a
  // free loopback port.
  const free = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch: () => new Response('') }), tunnel = free.port;
  free.stop(true);
  const serve = spawnOwned(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'serve', app, '--trust-tunnel', String(tunnel)], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
  let said = '';
  serve.stderr.on('data', d => { said += d; });
  const storage = `design-tool-pair-${process.pid}`;
  let door;
  try {
    const out = lines(serve), origin = new URL(await out.until(l => /^http:\/\/127\.0\.0\.1:\d+\/__tamber$/.test(l), 600_000)).origin;
    const local = (await (await fetch(`${origin}/__tamber/token`)).json()).token;
    const ask = async body => (await fetch(`${origin}/__tamber`, { method: 'POST', headers: { 'content-type': 'application/json', 'x-exact-tamber-token': local }, body: JSON.stringify(body) })).json();
    // The other end of the tunnel, as ssh -L puts it on the client's loopback: everything passes as sent
    // (Origin included), recorded, with the token pairing hands out. Once the edit is held for review, the
    // developer revokes the client while its tool runs.
    const seen = [];
    let token = '';
    door = Bun.serve({ hostname: '127.0.0.1', port: 0, idleTimeout: 0, fetch: async req => {
      const url = new URL(req.url), body = req.method === 'POST' ? await req.text() : undefined;
      // (Its Host as the browser sent it, as a tunnel passes the bytes: the door answers to loopback at any port.)
      const headers = new Headers(req.headers);
      headers.delete('content-length');
      const res = await upstream(`http://127.0.0.1:${tunnel}${url.pathname}${url.search}`, { method: req.method, headers, body, redirect: 'manual' });
      const bytes = await res.arrayBuffer();
      let request = {};
      try { request = url.pathname === '/__tamber' && body ? JSON.parse(body) : {}; } catch { /* not the wire */ }
      seen.push({ path: `${url.pathname}${url.search}`, method: request.method ?? null, refresh: request.refresh === true, status: res.status, revoked: seen.some(s => s.revoke) });
      if (url.pathname === '/__tamber/pair' && res.ok) token = JSON.parse(new TextDecoder().decode(bytes)).ok.token;
      if (request.method === 'propose' && request.hold && res.ok) { await ask({ method: 'revoke', client: 'remote:Ada#1' }); seen.push({ revoke: true }); }
      const back = new Headers(res.headers);
      back.delete('content-encoding'); back.delete('content-length');
      return new Response(bytes, { status: res.status, headers: back });
    } });
    const { code } = (await ask({ method: 'pair-code', scope: 'propose', name: 'laptop' })).ok;
    const at = `http://127.0.0.1:${door.port}`;
    const replies = await drive(`${at}/__tamber/tool/`, storage, [
      'type name Ada', 'tap save-name', 'clock data',
      `type remote-at ${at}`, `type remote-code ${code}`, 'tap remote-pair', 'clock data', 'clock +3000', 'clock data',
      'type dial-hue-stepper 210', 'clock data', 'state', 'tap apply', 'clock data', 'state',
      // Revoked: the next poll is refused, and the tool says so.
      'clock +3000', 'clock data', 'clock +3000', 'clock data', 'clock +3000', 'clock data', 'state', 'logs --all',
    ]);
    const states = replies.filter(r => r.resources);
    expect(states[0].resources.hello).toMatchObject({ ok: true, scope: 'propose', local: false, client: 'remote:Ada#1' });
    expect(states[0].derives.canWrite).toBe(false);
    expect(states[1].derives.stack.note).toContain('Held for review as');
    expect(states.at(-1).resources.hello).toMatchObject({ ok: false, error: 'This client was revoked, or its pairing expired: nothing more is sent to the design server. Pair again with a new code.' });
    expect((await ask({ method: 'clients' })).ok.clients.map(c => c.client)).toEqual(['local']);
    // What went through the door: the pairing once; then only what a propose client may send (read, propose,
    // a read-only inspect refresh); after the revoke, only refusals, and soon nothing.
    const wire = seen.filter(s => s.path === '/__tamber');
    expect(seen.filter(s => s.path === '/__tamber/pair')).toHaveLength(1);
    expect(wire.map(s => s.method)).toContain('propose');
    for (const s of wire) expect([s.method, ['hello', 'describe', 'resolve', 'explain', 'uses', 'history', 'export', 'canvas', 'propose', 'import', 'revert'].includes(s.method) || (s.method === 'inspect' && s.refresh)]).toEqual([s.method, true]);
    const after = seen.filter(s => s.revoked && !s.revoke && s.path.startsWith('/__tamber') && !s.path.startsWith('/__tamber/tool'));
    for (const s of after) expect([s.path, s.status]).toEqual([s.path, 401]);
    // (The requests in flight when the revoke lands, a poll and a read, and none after: three 3 s retries passed.)
    expect(after.length).toBeLessThanOrEqual(2);
    // Never the code (once sent) or the token in a URL, a drive reply (the tool's logs and state), the server's
    // output, or any file of the browser's profile on disk (its local and session storage, IndexedDB, caches)
    // once it quit.
    expect(token.length).toBeGreaterThan(20);
    for (const secret of [token, code, code.replace('-', '')]) {
      for (const s of seen) expect(s.path ?? '').not.toContain(secret);
      expect(JSON.stringify(replies)).not.toContain(secret);
      expect(out.seen.join('\n') + said).not.toContain(secret);
    }
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
  } finally {
    door?.stop(true);
    await stop(serve);
    rmSync(app, { recursive: true, force: true });
  }
}, 900_000);
