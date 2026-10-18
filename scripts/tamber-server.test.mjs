// The design server's wire (LLP 1121 D13, stage 3 commit 4): the token only
// to a local request, every read and write behind it, two clients' writes to
// one token where one conflicts, a hand edit, a new file and a `git checkout`
// reaching a subscriber and a long poll, a write announced once, a server
// whose lock is another's answering 503, and the dev loop answering `hello`.
import { afterAll, beforeAll, expect, test } from 'bun:test';
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, realpathSync, renameSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { PassThrough } from 'node:stream';
import { EventEmitter } from 'node:events';
import { developmentGate } from './install-page.mjs';
import { FRAME_ANCESTORS, frameOnlyByTool, tamberServer } from './tamber-server.mjs';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
const DESIGN = 'tamber t\n\ntheme\n  ink = #000000\n  edge = #101010\n';
const BRAND = 'tamber brand\n\ntheme\n  brand.ink = #202020\n';
const sha = text => createHash('sha256').update(text).digest('hex');

/** A stand-in for `contract tamber serve` that answers `hello`. */
function fakeServe() {
  const child = Object.assign(new EventEmitter(), { stdin: new PassThrough(), stdout: new PassThrough(), stderr: new PassThrough(), exitCode: null, signalCode: null });
  child.kill = () => { child.exitCode = 0; child.emit('exit', 0); };
  let buf = '';
  child.stdin.on('data', chunk => {
    buf += chunk; const lines = buf.split('\n'); buf = lines.pop();
    for (const line of lines) child.stdout.write(`${JSON.stringify({ id: JSON.parse(line).id, ok: { files: {} } })}\n`);
  });
  return child;
}

/** A request as the gate and the handler see it, and what it was answered. */
async function ask(tamber, gate, { path, method = 'GET', headers = {}, peer = '127.0.0.1', body = '' }) {
  const req = Object.assign(new PassThrough(), { method, headers, socket: { remoteAddress: peer } });
  let status = 0, text = '';
  const res = { writeHead(s) { status = s; }, write(t) { text += t; }, end(t) { text += t ?? ''; } };
  queueMicrotask(() => req.end(body));
  const handled = await tamber.handle(req, res, new URL(path, 'http://x'), gate.check(req));
  return { handled, status, body: text ? JSON.parse(text) : null };
}

test('close() signals the server\'s group only by a pid of its own: a missing, 0 or 1 pid ends the child by its own kill', async () => {
  const kill = process.kill;
  const groups = [];
  // Recorded, never sent: a pid of 0 or 1 here would reach this test's own group, or every process.
  process.kill = (pid, signal) => { if (signal === 0 || signal === undefined) return kill.call(process, pid, signal); groups.push(pid); return true; };
  try {
    for (const pid of [undefined, 0, 1]) {
      let child;
      const spawnServe = () => (child = Object.assign(fakeServe(), pid === undefined ? {} : { pid }));
      const tamber = tamberServer({ appDir: '/nowhere', spawnServe, log: () => {} });
      await tamber.ready;
      await tamber.close();
      expect(groups).toEqual([]);
      expect(child.exitCode).toBe(0);
    }
  } finally {
    process.kill = kill;
  }
});

test('the server reads the app\'s component params, again for a new plan or after a failed read, and a selection names a param row on the param rung; neither event carries an IR token (LLP 1122 T3)', async () => {
  const IR = 'param.f1a2b3c4d.Card.fill';
  const components = [{ identity: 'app.contract#Card', name: 'Card', params: [{ name: 'app.contract#Card.fill', short: 'Card.fill', token: IR, default: 'ink', set: null }] }];
  let describes = 0;
  const serve = () => {
    const child = fakeServe();
    child.stdin.removeAllListeners('data');
    let buf = '';
    child.stdin.on('data', chunk => {
      buf += chunk; const lines = buf.split('\n'); buf = lines.pop();
      for (const line of lines) {
        const r = JSON.parse(line);
        // The first describe fails: the next selection's plan reads again.
        const reply = r.method !== 'describe' ? { ok: { files: { 'app.tamber': 'd1' } } }
          : ++describes === 1 ? { error: { message: 'not now' } }
          : { ok: { files: { 'app.tamber': 'd1' }, ir: { tokens: [{ name: 'ink' }], components } } };
        child.stdout.write(`${JSON.stringify({ id: r.id, ...reply })}\n`);
      }
    });
    return child;
  };
  const map = { digest: 'p1', sources: [{ file: 'exact:app', sha256: 'x' }], nodes: [{ file: 'exact:app', line: 3, col: 5, end_col: 9, component: 'Card', chain: [], bindings: [{ row: 'background_color', origin: 'own' }] }] };
  const port = 8803;
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  const tamber = tamberServer({ appDir: '/nowhere', spawnServe: serve, log: () => {}, sourceMap: digest => Buffer.from(JSON.stringify({ ...map, digest: digest ?? 'p1' })) });
  await tamber.ready;
  const settled = async () => { for (let i = 0; i < 100 && describes < 1; i++) await Bun.sleep(5); await Bun.sleep(5); };
  await settled();
  const headers = { host: `127.0.0.1:${port}`, 'x-exact-tamber-token': tamber.token };
  await ask(tamber, gate, { path: '/__tamber/events', method: 'POST', headers, body: JSON.stringify({ wait: false, client: 'tool' }) });
  const { nonce } = tamber.channel.open(() => {});
  const report = body => tamber.channel.report(JSON.stringify({ nonce, ...body }));
  report({ kind: 'register', id: 'web-1', host: 'web', capabilities: ['pin', 'inspect'], plan: 'g1', design: 'd1' });
  report({ kind: 'status', scheme: 'light', revision: 0 });
  tamber.channel.inspect('tool', null, { on: true });
  const selected = { node: 2, site: 0, instance: [], planDigest: 'p1', tuple: {}, rows: [{ name: 'background_color', value: '#fff', source: 'authored', token: { names: [IR], text: IR, modes: '', implied: false } }] };
  const events = async () => (await ask(tamber, gate, { path: '/__tamber/events', method: 'POST', headers, body: JSON.stringify({ after: 0, wait: false, client: 'tool' }) })).body.events;
  // The read failed: read-only, said so; this plan reads them again.
  expect(report({ kind: 'selected', selected, plan: 'g1', design: 'd1', revision: 0 }).status).toBe(204);
  expect((await events()).filter(e => e.event === 'selection').at(-1).rows[0].editable).toMatchObject({ rung: null, reason: expect.stringContaining('has not read yet') });
  for (let i = 0; i < 100 && describes < 2; i++) await Bun.sleep(5);
  await Bun.sleep(5);
  expect(report({ kind: 'selected', selected, plan: 'g1', design: 'd1', revision: 0 }).status).toBe(204);
  const all = await events();
  expect(all.filter(e => e.event === 'selection').at(-1).rows[0].editable).toEqual({ rung: 'param', param: 'app.contract#Card.fill', short: 'Card.fill', token: 'ink', modes: '' });
  // The report as published too (`selected`): every one named by the param, or left out.
  expect(all.filter(e => e.event === 'selected').at(-1).selected.rows[0].token.names).toEqual(['app.contract#Card.fill']);
  expect(JSON.stringify(all)).not.toContain(IR);
  // A new plan: read again, its names kept meanwhile but no alias offered.
  const p2 = { ...selected, planDigest: 'p2' };
  expect(report({ kind: 'selected', selected: p2, plan: 'g1', design: 'd1', revision: 0 }).status).toBe(204);
  expect((await events()).filter(e => e.event === 'selection').at(-1).rows[0].editable).toEqual({ rung: 'param', param: 'app.contract#Card.fill', short: 'Card.fill', modes: '' });
  for (let i = 0; i < 100 && describes < 3; i++) await Bun.sleep(5);
  await Bun.sleep(5);
  expect(report({ kind: 'selected', selected: p2, plan: 'g1', design: 'd1', revision: 0 }).status).toBe(204);
  expect((await events()).filter(e => e.event === 'selection').at(-1).rows[0].editable.token).toBe('ink');
  await tamber.close();
});

test('a stale selection asks its build again once per status, its status first; a current one is joined with the map the dev loop gives', async () => {
  const map = { digest: 'p1', sources: [{ file: 'exact:app', sha256: 'x' }], nodes: [{ file: 'exact:app', line: 3, col: 5, end_col: 9, component: 'App', chain: [], bindings: [{ row: 'background_color', origin: 'own' }] }] };
  const tamber = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {}, sourceMap: () => Buffer.from(JSON.stringify(map)) });
  const frames = [];
  const { nonce } = tamber.channel.open(f => frames.push(f));
  const report = body => tamber.channel.report(JSON.stringify({ nonce, ...body }));
  report({ kind: 'register', id: 'web-1', host: 'web', capabilities: ['pin', 'inspect'], plan: 'g1', design: 'd1' });
  report({ kind: 'status', scheme: 'light', revision: 0 });
  tamber.channel.inspect('tool', null, { on: true });
  const selected = { node: 2, site: 0, instance: [], planDigest: 'p1', tuple: {}, rows: [{ name: 'background_color', value: '#fff', source: 'authored', token: { names: ['ink'], text: 'ink', modes: '', implied: false } }] };
  const asks = () => frames.filter(f => f.kind === 'inspect' && f.refresh).length;
  // Stale twice under one status: asked again once, status first.
  expect(report({ kind: 'selected', selected, plan: 'g1', design: 'd0', revision: 0 }).status).toBe(409);
  expect(report({ kind: 'selected', selected, plan: 'g1', design: 'd0', revision: 0 }).status).toBe(409);
  expect(asks()).toBe(1);
  expect(frames.find(f => f.kind === 'inspect' && f.refresh)).toEqual({ kind: 'inspect', refresh: true, status: true });
  // Its status moves: asked again under it.
  report({ kind: 'status', scheme: 'dark', revision: 0, design: 'd2' });
  expect(report({ kind: 'selected', selected, plan: 'g1', design: 'd0', revision: 0 }).status).toBe(409);
  expect(asks()).toBe(2);
  // Current: joined, the token rung offered, every copy outlined.
  expect(report({ kind: 'selected', selected, plan: 'g1', design: 'd2', revision: 0 }).status).toBe(204);
  expect(frames.at(-1)).toEqual({ kind: 'outline', sites: [0], plan: 'p1' });
  // A click's selection: the outline answering it names its token.
  expect(report({ kind: 'selected', selected, click: '7-123', plan: 'g1', design: 'd2', revision: 0 }).status).toBe(204);
  expect(frames.at(-1)).toEqual({ kind: 'outline', sites: [0], plan: 'p1', click: '7-123' });
  expect(report({ kind: 'selected', selected, click: '', plan: 'g1', design: 'd2', revision: 0 }).status).toBe(400);
  await tamber.close();
});

test('the token goes only to a local request, and every request needs it', async () => {
  const port = 8800;
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }, { origin: `http://192.168.1.20:${port}` }], port);
  const tamber = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {} });
  const local = { host: `127.0.0.1:${port}` };
  const given = await ask(tamber, gate, { path: '/__tamber/token', headers: local });
  expect(given.status).toBe(200);
  expect(given.body.token).toBe(tamber.token);
  // A forwarded request, a `--lan` host, a peer off this machine: the install endpoint's not-found.
  for (const refused of [
    { headers: { ...local, 'x-forwarded-for': '203.0.113.9' } },
    { headers: { ...local, forwarded: 'for=203.0.113.9' } },
    { headers: { host: `192.168.1.20:${port}` }, peer: '192.168.1.30' },
    { headers: local, peer: '192.168.1.30' },
  ]) {
    const answer = await ask(tamber, gate, { path: '/__tamber/token', ...refused });
    expect(answer.status).toBe(404);
    expect(answer.body).toEqual({ message: 'Not found.' });
  }
  // A read or a write without the token, or with another.
  for (const headers of [local, { ...local, 'x-exact-tamber-token': 'f'.repeat(64) }]) {
    const answer = await ask(tamber, gate, { path: '/__tamber', method: 'POST', headers, body: '{"method":"hello"}' });
    expect(answer.status).toBe(404);
  }
  const hello = await ask(tamber, gate, { path: '/__tamber', method: 'POST', headers: { ...local, 'x-exact-tamber-token': tamber.token }, body: '{"method":"hello"}' });
  expect(hello.status).toBe(200);
  // No dev loop runs it: no page to frame.
  expect(hello.body.ok.page).toBe(null);
  // A path that is not the wire's is left to the dev loop.
  expect((await ask(tamber, gate, { path: '/__dev', headers: local })).handled).toBe(false);
  tamber.close();
});

// LLP 1121 stage 4: a dev loop's wire says its builds take a value-only edit
// live; a write is held until the server answers, however long (a read
// still times out), so a write never lands after its client was told it
// failed.
test('hello says live, and a slow write is answered, not timed out', async () => {
  const port = 8803;
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  // Answers a transaction after 300 ms, a describe never.
  const slowServe = () => {
    const child = fakeServe();
    child.stdin.removeAllListeners('data');
    let buf = '';
    child.stdin.on('data', chunk => {
      buf += chunk; const lines = buf.split('\n'); buf = lines.pop();
      for (const line of lines) {
        const r = JSON.parse(line);
        const answer = () => child.stdout.write(`${JSON.stringify({ id: r.id, ok: { files: {}, written: [], changed: [] } })}\n`);
        if (r.method === 'transaction') setTimeout(answer, 300);
        else if (r.method !== 'describe') answer();
      }
    });
    return child;
  };
  const tamber = tamberServer({ appDir: '/nowhere', spawnServe: slowServe, log: () => {}, live: true, answerMs: 100 });
  const headers = { host: `127.0.0.1:${port}`, 'x-exact-tamber-token': tamber.token };
  const post = body => ask(tamber, gate, { path: '/__tamber', method: 'POST', headers, body: JSON.stringify(body) });
  expect((await post({ method: 'hello' })).body.ok.live).toBe(true);
  const write = await post({ method: 'transaction', client: 'agent', files: {}, ops: [] });
  expect(write.status).toBe(200);
  expect((await post({ method: 'describe' })).status).toBe(504);
  tamber.close();
});

// LLP 1122 T2c: a map per plan digest. Each build is joined and outlined
// from its own plan's map; a build on a plan with no map here is sent none.
test('each build is joined and outlined from its own plan\'s map', async () => {
  const port = 8806, P1 = '1'.repeat(64), P2 = '2'.repeat(64), P3 = '3'.repeat(64);
  const mapOf = (digest, line) => ({ digest, sources: [{ file: 'exact:app', sha256: 'x' }], nodes: [{ file: 'exact:app', line, col: 5, end_col: 9, component: 'App', chain: [], bindings: [{ row: 'background_color', origin: 'own' }] }, { file: 'exact:app', line: line + 100, col: 1, end_col: 2, component: 'App', chain: [], bindings: [] }] });
  const maps = { [P1]: mapOf(P1, 3), [P2]: mapOf(P2, 9) };
  // The same source span at another site of P1: the same sources, another plan.
  maps[P1].nodes.push({ file: 'exact:app', line: 109, col: 1, end_col: 2, component: 'App', chain: [], bindings: [] });
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  const tamber = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {}, sourceMap: digest => { const m = digest ? maps[digest] : maps[P1]; return m ? Buffer.from(JSON.stringify(m)) : null; } });
  const headers = { host: `127.0.0.1:${port}`, 'x-exact-tamber-token': tamber.token };
  const post = body => ask(tamber, gate, { path: '/__tamber', method: 'POST', headers, body: JSON.stringify(body) });
  const open = (id, planDigest) => { const frames = []; const s = tamber.channel.open(f => frames.push(f)); tamber.channel.report(JSON.stringify({ nonce: s.nonce, kind: 'register', id, host: 'ios', capabilities: ['pin', 'inspect'], plan: 'g', planDigest })); tamber.channel.report(JSON.stringify({ nonce: s.nonce, kind: 'status', scheme: 'light', revision: 0, planDigest })); return { frames, nonce: s.nonce }; };
  const a = open('web-1', P1), b = open('ios-1', P2), c = open('mac-1', P3);
  expect((await post({ method: 'hello' })).body.ok.builds.map(x => [x.id, x.planDigest])).toEqual([['web-1', P1], ['ios-1', P2], ['mac-1', P3]]);
  await ask(tamber, gate, { path: '/__tamber/events', method: 'POST', headers, body: JSON.stringify({ wait: false, client: 'tool' }) });
  tamber.channel.inspect('tool', null, { on: true });
  // A selection on ios-1 joins with P2's map, and is outlined on every
  // build from its own plan's map: P1 has that span at site 2.
  const selected = { node: 2, site: 1, instance: [], planDigest: P2, tuple: {}, rows: [] };
  expect(tamber.channel.report(JSON.stringify({ nonce: b.nonce, kind: 'selected', selected, inspect: b.frames.filter(f => f.kind === 'inspect').at(-1).seq, plan: 'g', revision: 0 })).status).toBe(204);
  expect(b.frames.at(-1)).toEqual({ kind: 'outline', sites: [1], plan: P2 });
  expect(a.frames.at(-1)).toEqual({ kind: 'outline', sites: [2], plan: P1 });
  expect(c.frames.some(f => f.kind === 'outline')).toBe(false);
  expect((await ask(tamber, gate, { path: '/__tamber/events', method: 'POST', headers, body: JSON.stringify({ after: 0, wait: false, client: 'tool' }) })).body.events.find(e => e.event === 'selection').source.line).toBe(109);
  // The tool outlines line 109: each build its own plan's sites; mac-1's
  // plan has no map here, so it is sent nothing.
  const outlined = (await post({ method: 'outline', client: 'tool', at: [{ file: 'exact:app', line: 109, col: 1 }] })).body.ok;
  expect(outlined.builds).toEqual(['web-1', 'ios-1']);
  expect(a.frames.at(-1)).toEqual({ kind: 'outline', sites: [2], plan: P1 });
  expect(b.frames.at(-1)).toEqual({ kind: 'outline', sites: [1], plan: P2 });
  expect(c.frames.some(f => f.kind === 'outline')).toBe(false);
  // The running generation without a map: builds with their own still outline.
  const running = maps[P1];
  delete maps[P1];
  expect((await post({ method: 'outline', client: 'tool', at: [{ file: 'exact:app', line: 109, col: 1 }] })).body.ok.builds).toEqual(['ios-1']);
  // web-1, which this client outlined, loses that outline rather than keeping it.
  expect(a.frames.at(-1)).toEqual({ kind: 'outline', clear: true });
  maps[P1] = running;
  // A map that moved under the same plan digest (an edit of locations only): the new one is used.
  maps[P2] = mapOf(P2, 9);
  maps[P2].nodes[1].line = 120;
  await post({ method: 'outline', client: 'tool', at: [{ file: 'exact:app', line: 120, col: 1 }] });
  expect(b.frames.at(-1)).toEqual({ kind: 'outline', sites: [1], plan: P2 });
  // A selection from another plan than the build says it runs now: not current, asked again.
  expect(tamber.channel.report(JSON.stringify({ nonce: b.nonce, kind: 'selected', selected: { ...selected, planDigest: P1 }, inspect: b.frames.filter(f => f.kind === 'inspect').at(-1).seq, plan: 'g', revision: 0 })).status).toBe(409);
  // A selection that names no plan joins with no map: read-only.
  expect(tamber.channel.report(JSON.stringify({ nonce: b.nonce, kind: 'selected', selected: { ...selected, planDigest: null }, inspect: b.frames.filter(f => f.kind === 'inspect').at(-1).seq, plan: 'g', revision: 0 })).status).toBe(204);
  const last = (await ask(tamber, gate, { path: '/__tamber/events', method: 'POST', headers, body: JSON.stringify({ after: 0, wait: false, client: 'tool' }) })).body.events.filter(e => e.event === 'selection').at(-1);
  expect(last.source).toBe(null);
  expect(typeof last.readOnly).toBe('string');
  await tamber.close();
});

// LLP 1122 D4: the dev loop's builds in `hello`, and a pin a client may
// set only while it subscribes by name, cleared on every build when it goes.
test('a pin is its client\'s while it is present, and builds are in hello', async () => {
  const port = 8804;
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  const tamber = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {} });
  const headers = { host: `127.0.0.1:${port}`, 'x-exact-tamber-token': tamber.token };
  const post = body => ask(tamber, gate, { path: '/__tamber', method: 'POST', headers, body: JSON.stringify(body) });
  const frames = [];
  const build = tamber.channel.open(f => frames.push(f));
  tamber.channel.report(JSON.stringify({ nonce: build.nonce, kind: 'register', id: 'web-1', host: 'web', capabilities: ['pin'] }));
  expect((await post({ method: 'hello' })).body.ok.builds.map(b => b.id)).toEqual(['web-1']);
  // Not present: refused.
  expect((await post({ method: 'pin', client: 'tool', build: 'web-1', pin: { scheme: 'dark' } })).status).toBe(409);
  // A long poll naming the client makes it present for 35 s after it is answered.
  const polled = await ask(tamber, gate, { path: '/__tamber/events', method: 'POST', headers, body: JSON.stringify({ wait: false, client: 'poller' }) });
  expect(polled.status).toBe(200);
  expect((await post({ method: 'pin', client: 'poller', build: 'web-1', pin: { scheme: 'light' } })).status).toBe(200);
  // 35 s on with no poll: absent, and its pin handed back.
  const realNow = Date.now;
  Date.now = () => realNow() + 36_000;
  try { await post({ method: 'hello' }); } finally { Date.now = realNow; }
  expect(frames.at(-1)).toMatchObject({ kind: 'pin', pin: null });
  // Subscribed by name: taken, and a frame goes to that build.
  const req = Object.assign(new PassThrough(), { method: 'POST', headers, socket: { remoteAddress: '127.0.0.1' } });
  const res = Object.assign(new EventEmitter(), { destroyed: false, writableEnded: false, writeHead() {}, write() {}, end() {} });
  queueMicrotask(() => req.end(JSON.stringify({ client: 'tool' })));
  await tamber.handle(req, res, new URL('/__tamber/subscribe', 'http://x'), gate.check(req));
  const taken = await post({ method: 'preview', client: 'tool', build: 'web-1', axis: 'dark-style', value: 'dim' });
  expect(taken.status).toBe(200);
  expect(frames.at(-1)).toMatchObject({ kind: 'pin', pin: { mode: { 'dark-style': 'dim' } } });
  // The client goes: its pin is handed back.
  res.emit('close');
  expect(frames.at(-1)).toMatchObject({ kind: 'pin', pin: null });
  tamber.close();
});

// The real server (`contract tamber serve`, built once by cargo) behind an
// HTTP listener and the development gate, as a dev loop mounts it.
let dir, server, tamber, origin, headers;
beforeAll(async () => {
  dir = mkdtempSync(join(tmpdir(), 'exact-tamber-wire-'));
  writeFileSync(join(dir, 'app.contract'), 'component App\n  view\n    text "a"\n');
  writeFileSync(join(dir, 'app.tamber'), DESIGN);
  writeFileSync(join(dir, 'brand.tamber'), BRAND);
  const git = (...args) => spawnSync('git', ['-c', 'user.email=t@example.test', '-c', 'user.name=t', ...args], { cwd: dir });
  git('init', '-q'); git('config', 'user.email', 't@example.test'); git('config', 'user.name', 't'); git('add', '.'); git('commit', '-q', '-m', 'design');
  server = createServer();
  await new Promise(ok => server.listen(0, '127.0.0.1', ok));
  const port = server.address().port;
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  // The watcher settles slower than a request lands, so a write's own
  // reply, not the watcher, is what tells a hand edit before it apart.
  tamber = tamberServer({ appDir: dir, log: () => {}, settle: 250 });
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

/** The wire stopped and started again: a new session, a new token. */
async function freshWire() {
  await tamber.close();
  tamber = tamberServer({ appDir: dir, log: () => {}, settle: 250 });
  await tamber.ready;
  const { token } = await (await fetch(`${origin}/__tamber/token`)).json();
  headers = { 'content-type': 'application/json', 'x-exact-tamber-token': token };
}

const post = async request => {
  const res = await fetch(`${origin}/__tamber`, { method: 'POST', headers, body: JSON.stringify(request) });
  return { status: res.status, ...(await res.json()) };
};

/** The change stream's events as they come. */
async function subscribe() {
  const res = await fetch(`${origin}/__tamber/subscribe`, { method: 'POST', headers });
  const reader = res.body.getReader(), decoder = new TextDecoder(), events = [];
  let buf = '';
  (async () => {
    for (;;) {
      const { value, done } = await reader.read().catch(() => ({ done: true }));
      if (done) return;
      buf += decoder.decode(value, { stream: true });
      const parts = buf.split('\n\n'); buf = parts.pop();
      for (const part of parts) if (part.startsWith('data: ')) events.push(JSON.parse(part.slice(6)));
    }
  })();
  const until = async (pass, ms = 5000) => {
    const end = Date.now() + ms;
    while (!pass(events)) { if (Date.now() > end) throw new Error(`no such event in ${JSON.stringify(events)}`); await Bun.sleep(20); }
  };
  return { events, until, close: () => reader.cancel() };
}

const disk = file => sha(readFileSync(join(dir, file)));

test('two clients writing one token: the second conflicts, and the write is announced once', async () => {
  const stream = await subscribe();
  await stream.until(e => e[0]?.event === 'subscribed');
  const seen = { 'app.tamber': disk('app.tamber') };
  const anonymous = await post({ method: 'transaction', files: seen, ops: [{ op: 'set-token', name: 'ink', value: '#111111' }] });
  expect(anonymous.status).toBe(400);
  const first = await post({ method: 'transaction', client: 'one', files: seen, ops: [{ op: 'set-token', name: 'ink', value: '#111111' }] });
  expect(first.status).toBe(200);
  expect(first.ok.written).toEqual(['app.tamber']);
  const second = await post({ method: 'transaction', client: 'two', files: seen, ops: [{ op: 'set-token', name: 'ink', value: '#222222' }] });
  expect(second.status).toBe(409);
  expect(second.error.conflict).toBe(true);
  expect(readFileSync(join(dir, 'app.tamber'), 'utf8')).toContain('ink = #111111');
  // A hand edit after it: the watcher's next read, which would also carry
  // any second announcement of the write.
  writeFileSync(join(dir, 'brand.tamber'), BRAND.replace('#202020', '#212121'));
  await stream.until(e => e.some(x => x.client === null && x.edited?.includes('brand.tamber')));
  const changes = stream.events.filter(x => x.event === 'changed');
  expect(changes.map(x => [x.client, x.edited])).toEqual([['one', ['app.tamber']], [null, ['brand.tamber']]]);
  expect(changes[0].changed).toEqual([{ file: 'app.tamber', token: 'ink' }]);
  const undone = await post({ method: 'undo', client: 'one', inverse: first.ok.inverse });
  expect(undone.status).toBe(200);
  expect(readFileSync(join(dir, 'app.tamber'), 'utf8')).toBe(DESIGN);
  writeFileSync(join(dir, 'brand.tamber'), BRAND);
  await stream.until(e => e.at(-1)?.files['brand.tamber'] === sha(BRAND));
  stream.close();
}, 30_000);

test('a hand edit made before a write is its own event, not the writer\'s', async () => {
  const stream = await subscribe();
  await stream.until(e => e[0]?.event === 'subscribed');
  const at = stream.events.length;
  // Too soon for the watcher: the write's reply carries the moved digest.
  writeFileSync(join(dir, 'brand.tamber'), BRAND.replace('#202020', '#232323'));
  const done = await post({ method: 'transaction', client: 'one', files: { 'app.tamber': disk('app.tamber') }, ops: [{ op: 'set-token', name: 'edge', value: '#131313' }] });
  expect(done.status).toBe(200);
  await stream.until(e => e.slice(at).some(x => x.client === 'one'));
  const mine = stream.events.slice(at).find(x => x.client === 'one');
  expect(mine.edited).toEqual(['app.tamber']);
  await stream.until(e => e.slice(at).some(x => x.client === null && x.edited.includes('brand.tamber')));
  await post({ method: 'undo', client: 'one', inverse: done.ok.inverse });
  writeFileSync(join(dir, 'brand.tamber'), BRAND);
  await stream.until(e => e.at(-1)?.files['brand.tamber'] === sha(BRAND) && e.at(-1)?.files['app.tamber'] === sha(DESIGN));
  stream.close();
}, 30_000);

test('a hand edit and a git checkout reach a subscriber', async () => {
  const stream = await subscribe();
  await stream.until(e => e[0]?.event === 'subscribed');
  writeFileSync(join(dir, 'brand.tamber'), BRAND.replace('#202020', '#303030'));
  await stream.until(e => e.some(x => x.event === 'changed' && x.client === null && x.edited.includes('brand.tamber')));
  writeFileSync(join(dir, 'app.tamber'), DESIGN.replace('#101010', '#121212'));
  await stream.until(e => e.at(-1)?.files['app.tamber'] === sha(DESIGN.replace('#101010', '#121212')));
  const before = stream.events.length;
  // A checkout puts both files back: one change, or two, each a hand edit.
  spawnSync('git', ['checkout', '--', 'app.tamber', 'brand.tamber'], { cwd: dir });
  await stream.until(e => {
    const last = e.at(-1);
    return e.length > before && last.files['app.tamber'] === sha(DESIGN) && last.files['brand.tamber'] === sha(BRAND);
  });
  expect(stream.events.slice(before).every(x => x.client === null)).toBe(true);
  // A design file that appears is found, and a long poll hears of it.
  const { seq } = await (await fetch(`${origin}/__tamber/events`, { method: 'POST', headers, body: '{"after":0}' })).json();
  // `wait: false` answers at once, with nothing new.
  const now = await (await fetch(`${origin}/__tamber/events`, { method: 'POST', headers, body: JSON.stringify({ after: seq, wait: false }), signal: AbortSignal.timeout(5000) })).json();
  expect(now).toMatchObject({ events: [], seq, missed: false });
  const polled = fetch(`${origin}/__tamber/events`, { method: 'POST', headers, body: JSON.stringify({ after: seq }) }).then(r => r.json());
  writeFileSync(join(dir, 'late.tamber'), 'tamber late\n\ntheme\n  late.ink = #404040\n');
  await stream.until(e => e.some(x => x.edited?.includes('late.tamber')));
  const answer = await polled;
  expect(answer.events.some(x => x.edited.includes('late.tamber'))).toBe(true);
  const late = await post({ method: 'propose', ops: [{ op: 'set-token', file: 'late.tamber', name: 'late.ink', value: '#414141' }] });
  expect(late.status).toBe(200);
  rmSync(join(dir, 'late.tamber'));
  await stream.until(e => !('late.tamber' in e.at(-1)?.files));
  // The design itself removed and checked out again.
  rmSync(join(dir, 'app.tamber'));
  await stream.until(e => !('app.tamber' in e.at(-1)?.files) && e.at(-1)?.edited.includes('app.tamber'));
  spawnSync('git', ['checkout', '--', 'app.tamber'], { cwd: dir });
  await stream.until(e => e.at(-1)?.files['app.tamber'] === sha(DESIGN));
  stream.close();
}, 30_000);

test('a second wire for the app, whose lock is the first\'s, answers 503', async () => {
  const second = tamberServer({ appDir: dir, log: () => {} });
  await second.ready;
  const gate = developmentGate([{ origin: 'http://127.0.0.1:8801' }], 8801);
  const local = { host: '127.0.0.1:8801', 'x-exact-tamber-token': second.token };
  const subscribed = await ask(second, gate, { path: '/__tamber/subscribe', method: 'POST', headers: local });
  expect(subscribed.status).toBe(503);
  expect(subscribed.body.error.message).toContain('another tamber server');
  const hello = await ask(second, gate, { path: '/__tamber', method: 'POST', headers: local, body: '{"method":"hello"}' });
  expect(hello.status).toBe(503);
  // A long poll gets the `down` event once, then is told so, never
  // answered empty to ask again at once.
  const first = await ask(second, gate, { path: '/__tamber/events', method: 'POST', headers: local, body: '{"after":0}' });
  expect(first.body.events.map(e => e.event)).toEqual(['down']);
  const polled = await ask(second, gate, { path: '/__tamber/events', method: 'POST', headers: local, body: JSON.stringify({ after: first.body.seq }) });
  expect(polled.status).toBe(503);
  await second.close();
}, 600_000);

/** A dev loop for `app` (the theme fixture) on a free port (`flags` say
 * which loop), and its origin; `stop` ends it. */
async function devLoop(flags = [], app = 'theme-fixture') {
  const listener = createServer();
  await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
  const port = listener.address().port;
  await new Promise(ok => listener.close(ok));
  const dist = mkdtempSync(join(tmpdir(), 'exact-tamber-dev-'));
  const dev = spawn(process.execPath, [resolve(ROOT, 'host/web/dev.mjs'), '--app', app, '--port', String(port), ...flags], { cwd: ROOT, env: { ...process.env, EXACT_WEB_DIST: dist }, stdio: ['ignore', 'pipe', 'pipe'] });
  const stop = async () => {
    if (dev.exitCode === null && dev.signalCode === null) {
      const exit = new Promise(ok => dev.once('exit', ok));
      dev.kill('SIGTERM');
      if (!(await Promise.race([exit.then(() => true), new Promise(ok => setTimeout(() => ok(false), 30_000).unref())]))) dev.kill('SIGKILL');
    }
    rmSync(dist, { recursive: true, force: true });
  };
  let out = '', err = '';
  dev.stderr.on('data', d => { err = (err + d).slice(-4000); });
  try {
    await new Promise((ok, fail) => {
      dev.stdout.on('data', d => { out += d; if (out.includes(`127.0.0.1:${port}/`)) ok(); });
      dev.on('error', fail);
      dev.on('exit', code => fail(new Error(`the dev loop exited ${code}: ${out}${err}`)));
    });
  } catch (e) { await stop(); throw e; }
  return { base: `http://127.0.0.1:${port}`, stop, said: () => out };
}

const helloOf = async base => {
  const { token } = await (await fetch(`${base}/__tamber/token`)).json();
  return (await (await fetch(`${base}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: '{"method":"hello"}' })).json()).ok;
};

test('frameOnlyByTool: every HTML head carries frame-ancestors, however it is written; nothing else does', async () => {
  const routes = {
    '/head': res => { res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' }); res.end('<p>'); },
    '/message': res => { res.writeHead(200, 'OK', { 'content-type': 'text/html' }); res.end('<p>'); },
    '/set': res => { res.setHeader('content-type', 'text/html'); res.writeHead(200); res.end('<p>'); },
    '/implicit': res => { res.setHeader('content-type', 'text/html'); res.end('<p>'); },
    '/flat': res => { res.writeHead(200, ['Content-Type', 'text/html', 'X-Frame-Options', 'DENY']); res.end('<p>'); },
    '/repeated': res => { res.writeHead(200, ['Content-Type', 'text/plain', 'Content-Type', 'text/html']); res.end('<p>'); },
    '/pairs': res => { res.writeHead(200, [['content-type', 'text/html'], ['set-cookie', 'a=1'], ['set-cookie', 'b=2']]); res.end('<p>'); },
    '/forwarded': res => { res.writeHead(200, { 'content-type': 'text/html', 'content-security-policy': 'frame-ancestors *' }); res.end('<p>'); },
    '/preset': res => { res.setHeader('content-security-policy', "default-src 'self'"); res.writeHead(200, { 'content-type': 'text/html' }); res.end('<p>'); },
    '/json': res => { res.writeHead(200, { 'content-type': 'application/json' }); res.end('{}'); },
    '/none': res => { res.writeHead(204); res.end(); },
  };
  const server = createServer((req, res) => { frameOnlyByTool(res); routes[req.url](res); });
  await new Promise(ok => server.listen(0, '127.0.0.1', ok));
  try {
    const at = path => fetch(`http://127.0.0.1:${server.address().port}${path}`);
    const csp = async path => (await at(path)).headers.get('content-security-policy');
    for (const path of ['/head', '/message', '/set', '/implicit', '/flat', '/repeated', '/pairs']) expect([path, await csp(path)]).toEqual([path, FRAME_ANCESTORS]);
    // A policy already there stays beside it: both are enforced, so the
    // tool's is never widened.
    expect(await csp('/forwarded')).toBe(`frame-ancestors *, ${FRAME_ANCESTORS}`);
    expect(await csp('/preset')).toBe(`default-src 'self', ${FRAME_ANCESTORS}`);
    // X-Frame-Options goes; the head's other headers stay, repeats too.
    expect((await at('/flat')).headers.get('x-frame-options')).toBe(null);
    expect((await at('/pairs')).headers.getSetCookie()).toEqual(['a=1', 'b=2']);
    for (const path of ['/json', '/none']) expect([path, await csp(path)]).toEqual([path, null]);
  } finally { await new Promise(ok => server.close(ok)); }
});

// The wasm loop's HTML (caltrain: the theme fixture has no wasm build).
// LLP 1122 T2b: inspect is a development page's only: no file a web build
// ships (every host file, `webHostFiles`) carries it; dev.js, which does, is
// served by the dev loop alone.
test('no web build ships inspect: it lives in dev.js, which no build copies', async () => {
  const { webHostFiles } = await import('./app.mjs');
  const shipped = webHostFiles();
  expect(Object.keys(shipped)).not.toContain('dev.js');
  for (const [name, source] of Object.entries(shipped)) if (name.endsWith('.js')) expect([name, readFileSync(resolve(ROOT, source), 'utf8').includes('exact-inspect')]).toEqual([name, false]);
  expect(readFileSync(resolve(ROOT, 'host/web/dev.js'), 'utf8')).toContain('exact-inspect');
  // The JS target's half, and its table, are served by its dev loop
  // (inspect-dev.js; `exact-web-js inspect-table` when first asked): no
  // build makes or copies either.
  const build = readFileSync(resolve(ROOT, 'host/web-js/build.mjs'), 'utf8');
  expect(build).not.toContain('inspect-dev.js');
  expect(build).not.toContain('dev-inspect');
  expect(readFileSync(resolve(ROOT, 'host/web-js/dev.mjs'), 'utf8')).toContain("url.pathname === '/dev-inspect.json'");
  expect(readFileSync(resolve(ROOT, 'host/web-js/inspect-dev.js'), 'utf8')).toContain('exact-inspect');
});

// LLP 1122 T2b: inspect mode in the wasm host, on the theme fixture's page.
test('inspect on the wasm page: no tap reaches the app, a click selects the node the hit test finds and reports its site, instance and rows, a refresh reports it again, and Escape leaves', async () => {
  const { base, stop, said } = await devLoop(['--wasm'], 'theme-fixture');
  const { chromium } = await import('playwright-core');
  const browser = await chromium.launch({ channel: 'chrome', headless: true });
  let polling = true;
  try {
    const { token } = await (await fetch(`${base}/__tamber/token`)).json();
    const post = async (body, path = '/__tamber') => (await fetch(`${base}${path}`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: JSON.stringify(body) })).json();
    // The client, present by its long poll; the events it is told.
    const events = [];
    const poll = (async () => { let after = 0, epoch; while (polling) { const r = await post(epoch ? { after, epoch, client: 'tool' } : { after: 0, wait: false, client: 'tool' }, '/__tamber/events'); events.push(...(r.events ?? [])); after = r.seq ?? after; epoch = r.epoch; } })();
    const until = async (pass, ms = 30_000) => { const end = Date.now() + ms; for (;;) { const got = await pass(); if (got) return got; if (Date.now() > end) throw new Error('not seen'); await new Promise(ok => setTimeout(ok, 200)); } };
    const page = await browser.newPage({ viewport: { width: 420, height: 900 }, hasTouch: true });
    await page.goto(`${base}/`);
    const build = await until(async () => (await post({ method: 'hello' })).ok.builds.find(b => b.host === 'web'));
    // The resident compiler's first plan, and its source map, are in.
    await until(() => said().includes('plan ready'), 300_000);
    expect(build.capabilities).toContain('inspect');
    const dim = () => page.locator('[data-testid="dim-preview"]').evaluate(e => getComputedStyle(e).backgroundColor);
    const before = await dim();
    expect((await post({ method: 'inspect', client: 'tool', on: true })).ok).toEqual({ builds: [build.id], unsupported: [] });
    await page.locator('#exact-inspect').waitFor();
    // No control takes focus while inspecting (its keys would reach the app).
    expect(await page.locator('[data-testid="swap"]').evaluate(e => { e.focus(); return document.activeElement === e; })).toBe(false);
    // A hovered node is outlined.
    const card = page.locator('[data-testid="card"]');
    const box = await card.boundingBox();
    await page.mouse.move(box.x + 4, box.y + 4);
    await until(async () => (await page.evaluate(() => [...document.body.children].some(e => e.style?.outline?.includes('solid') && e.style.display === 'block'))));
    // A click on Swap pins: the app never hears it, and the node the hit
    // test finds there is selected and reported.
    const swap = page.locator('[data-testid="swap"]');
    const at = await swap.boundingBox();
    // (Counting the page's scrolls into view from here.)
    await page.evaluate(() => { globalThis.__scrolls = 0; const was = Element.prototype.scrollIntoView; Element.prototype.scrollIntoView = function (...a) { globalThis.__scrolls += 1; return was.apply(this, a); }; });
    await page.mouse.click(at.x + at.width / 2, at.y + at.height / 2);
    const selected = await until(() => events.find(e => e.event === 'selected'));
    expect(await dim()).toBe(before);
    const hit = await page.evaluate(([x, y]) => document.elementsFromPoint(x, y).find(e => e.closest('[data-view]'))?.closest('[data-view]').dataset.view, [at.x + at.width / 2, at.y + at.height / 2]);
    expect(selected.build).toBe(build.id);
    expect(selected.selected).toMatchObject({ node: Number(hit), trimmed: false });
    expect(typeof selected.selected.site).toBe('number');
    expect(Array.isArray(selected.selected.instance)).toBe(true);
    expect(selected.selected.rows.length).toBeGreaterThan(0);
    // Each row as the runner's `layout` gives it: its value and source.
    expect(selected.selected.rows.every(r => typeof r.name === 'string' && ['authored', 'dynamic', 'inherited', 'initial'].includes(r.source))).toBe(true);
    // The server joins it to source (the theme fixture's `text "Swap
    // pins"`), says each row's editability, and outlines every copy.
    const contract = resolve(ROOT, 'apps/theme-fixture/app.contract');
    const line = readFileSync(contract, 'utf8').split('\n').findIndex(l => l.includes('text "Swap pins"')) + 1;
    const selection = await until(() => events.find(e => e.event === 'selection'));
    expect(selection).toMatchObject({ build: build.id, node: Number(hit), readOnly: null, source: { line, component: 'App' } });
    expect(realpathSync(selection.source.file)).toBe(realpathSync(contract));
    expect(selection.rows.every(r => r.editable && (r.editable.rung === 'token' || typeof r.editable.reason === 'string'))).toBe(true);
    // Its colour is the page's, inherited: read-only, a link to the token
    // the page's own row reads.
    const color = selection.rows.find(r => r.name === 'text_color');
    expect(color).toMatchObject({ source: 'inherited', editable: { rung: null, link: 'text.primary' } });
    await until(async () => (await page.locator('[data-exact-outline]').count()) >= 1);
    // The outline of the copy just clicked scrolls nothing (it is under the
    // pointer); the tool's Outline below brings its first copy in.
    expect(await page.evaluate(() => globalThis.__scrolls)).toBe(0);
    // Selecting from the tool: a token's uses outline their views (the
    // card's surface, each row reading surface.card).
    const uses = (await post({ method: 'uses', token: 'surface.card' })).ok;
    expect(uses.sites.length).toBeGreaterThan(0);
    const outlined = (await post({ method: 'outline', client: 'tool', at: uses.sites })).ok;
    expect(outlined.builds).toEqual([build.id]);
    expect(outlined.sites.length).toBeGreaterThan(0);
    // Every copy: one box per view at those sites, on that view's box.
    const boxes = () => page.evaluate(sites => {
      const x = globalThis.exact, want = new Set(sites);
      const views = JSON.parse(x.readOut(x.wasm.exact_agent(x.writeIn(JSON.stringify({ op: 'sites' })))));
      const at = e => { const r = e.getBoundingClientRect(); return [r.left, r.top, r.width, r.height].map(Math.round).join(','); };
      return {
        views: views.sites.filter(([, site]) => want.has(site)).map(([id]) => x.views.get(id)).filter(e => e?.isConnected).map(at).sort(),
        marks: [...document.querySelectorAll('[data-exact-outline]')].map(at).sort(),
      };
    }, outlined.sites);
    await until(async () => { const b = await boxes(); return b.views.length > 0 && JSON.stringify(b.marks) === JSON.stringify(b.views); });
    expect(await page.evaluate(() => globalThis.__scrolls)).toBe(1);
    // An outline of nothing clears them.
    await post({ method: 'outline', client: 'tool', at: [] });
    await until(async () => (await page.locator('[data-exact-outline]').count()) === 0);
    // The window's modes: the app starts in Black (`dark-style`'s first).
    expect(selected.selected.tuple).toEqual({ 'dark-style': 'black' });
    // No key, no finger reaches the app either: Enter and Space on the
    // button focused before, and a tap, leave the previews as they were.
    await page.keyboard.press('Enter');
    expect(await dim()).toBe(before);
    await page.keyboard.press(' ');
    expect(await dim()).toBe(before);
    await page.touchscreen.tap(at.x + at.width / 2, at.y + at.height / 2);
    await until(() => events.filter(e => e.event === 'selected').length >= 2);
    expect(await dim()).toBe(before);
    // A tap on the Dim preview selects it, in the modes it is in: Dim, its
    // pin, while the window is in Black.
    const pinned = await page.locator('[data-testid="dim-preview"]').boundingBox();
    await page.touchscreen.tap(pinned.x + 6, pinned.y + 6);
    const inDim = await until(() => events.filter(e => e.event === 'selected').find(e => e.selected.testId === 'dim-preview' || e.selected.tuple?.['dark-style'] === 'dim'));
    expect(inDim.selected.tuple).toEqual({ 'dark-style': 'dim' });
    // A refresh reports the selection again.
    const count = events.filter(e => e.event === 'selected').length;
    expect((await post({ method: 'inspect', client: 'tool', refresh: true })).ok.builds).toEqual([build.id]);
    await until(() => events.filter(e => e.event === 'selected').length > count);
    // A reload keeps it: the page registers again and is told it is on.
    await page.reload();
    await page.locator('#exact-inspect').waitFor();
    // Escape leaves: the banner goes, the server's clients are told (a
    // selection of none), and a tap reaches the app again.
    await page.keyboard.press('Escape');
    await page.locator('#exact-inspect').waitFor({ state: 'detached' });
    await until(() => events.find(e => e.event === 'selected' && e.selected === null));
    // And keys again: focused, Enter swaps (the positive control for the
    // keys inspect mode took).
    await swap.focus();
    expect(await swap.evaluate(e => e.contains(document.activeElement) || document.activeElement?.contains(e))).toBe(true);
    await page.keyboard.press('Enter');
    await until(async () => (await dim()) !== before);
    polling = false;
    await poll.catch(() => {});
  } finally {
    polling = false;
    await browser.close();
    await stop();
  }
}, 900_000);

// LLP 1122 T2b, the JS target: the same node on the theme fixture answers
// the same on the JS loop as on the wasm loop (the gate's line), its rows
// built in the page from the build's inspect table, and the join and
// outline drawn from the page's own plan's map.
test('inspect on the JS page: no tap reaches the app, and each node answers as on the wasm page, row for row, joined to the same source, with the same editability', async () => {
  const { chromium } = await import('playwright-core');
  const browser = await chromium.launch({ channel: 'chrome', headless: true });
  const ids = ['page', 'dim', 'swap', 'card', 'light', 'dim-preview', 'black-preview'];
  const polls = [], stops = [];
  try {
    const answers = [];
    // One loop at a time (each builds; two would wait on each other's Cargo).
    for (const flags of [['--wasm'], []]) {
      const loop = await devLoop(flags, 'theme-fixture'), { base, said } = loop;
      stops.push(loop.stop);
      const { token } = await (await fetch(`${base}/__tamber/token`)).json();
      const post = async (body, path = '/__tamber') => (await fetch(`${base}${path}`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: JSON.stringify(body) })).json();
      const events = [], state = { polling: true };
      polls.push(state);
      const poll = (async () => { let after = 0, epoch; while (state.polling) { const r = await post(epoch ? { after, epoch, client: 'tool' } : { after: 0, wait: false, client: 'tool' }, '/__tamber/events'); events.push(...(r.events ?? [])); after = r.seq ?? after; epoch = r.epoch; } })();
      const until = async (pass, ms = 30_000, what = '') => { const end = Date.now() + ms; for (;;) { const got = await pass(); if (got) return got; if (Date.now() > end) throw new Error(`not seen: ${what} on ${base}: ${JSON.stringify(events.slice(-3)).slice(0, 600)}`); await new Promise(ok => setTimeout(ok, 200)); } };
      const page = await browser.newPage({ viewport: { width: 420, height: 900 } });
      await page.goto(`${base}/`);
      await until(() => said().includes('plan ready'), 300_000);
      const build = await until(async () => (await post({ method: 'hello' })).ok?.builds?.find(b => b.host.startsWith('web')), 120_000);
      expect(build.capabilities).toContain('inspect');
      const dim = () => page.locator('[data-testid="dim-preview"]').evaluate(e => getComputedStyle(e).backgroundColor);
      const before = await dim();
      expect((await post({ method: 'inspect', client: 'tool', on: true })).ok).toEqual({ builds: [build.id], unsupported: [] });
      await page.locator('#exact-inspect').waitFor();
      const got = {};
      for (const id of ids) {
        const box = await page.locator(`[data-testid="${id}"]`).first().boundingBox();
        const seen = events.length;
        await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
        const selected = await until(() => events.slice(seen).find(e => e.event === 'selected'), 30_000, `${id} selected`);
        const selection = await until(() => events.slice(seen).find(e => e.event === 'selection'), 30_000, `${id} selection`);
        got[id] = { selected, selection };
      }
      // Swap pins was clicked: the app never heard it.
      expect(await dim()).toBe(before);
      // The selection's copies are outlined on the page.
      await until(async () => (await page.locator('[data-exact-outline]').count()) >= 1);
      // Escape leaves, and the server's clients are told.
      await page.keyboard.press('Escape');
      await page.locator('#exact-inspect').waitFor({ state: 'detached' });
      await until(() => events.find(e => e.event === 'selected' && e.selected === null));
      answers.push({ build, got });
      state.polling = false;
      await poll.catch(() => {});
      await page.close();
      await loop.stop();
    }
    const [wasm, js] = answers;
    expect(js.build.host).toBe('web-js');
    expect(js.build.planDigest).toBe(wasm.build.planDigest);
    // A row as both say it: its value, source and token (a view id differs by page).
    // (And no row the wasm page reads from the kernel is the JS page's computed CSS.)
    const rows = (s) => s.rows.map(({ name, value, source, token, computed }) => ({ name, value, source, token: token ?? null, computed: !!computed }));
    for (const id of ids) {
      const [a, b] = [wasm.got[id], js.got[id]];
      expect([id, b.selected.selected.site, b.selected.selected.type, b.selected.selected.testId, b.selected.selected.instance, b.selected.selected.tuple]).toEqual([id, a.selected.selected.site, a.selected.selected.type, a.selected.selected.testId, a.selected.selected.instance, a.selected.selected.tuple]);
      expect([id, rows(b.selected.selected)]).toEqual([id, rows(a.selected.selected)]);
      // The same source, and the same editability, row for row.
      expect([id, b.selection.source.line, b.selection.readOnly]).toEqual([id, a.selection.source.line, a.selection.readOnly]);
      expect([id, b.selection.rows.map(r => [r.name, r.editable])]).toEqual([id, a.selection.rows.map(r => [r.name, r.editable])]);
    }
  } finally {
    for (const p of polls) p.polling = false;
    await browser.close();
    for (const stop of stops) await stop();
  }
}, 900_000);

test('the JS loop: its own design server\'s lock is no edit, and an atomic save over the design is a value batch', async () => {
  // An app of its own, so no other test's fixture moves under it.
  const dir = mkdtempSync(join(tmpdir(), 'exact-atomic-save-'));
  writeFileSync(join(dir, 'app.json'), JSON.stringify({ name: 'Atomic save', app: { id: 'com.exact.atomic-save', name: 'Atomic save' } }));
  writeFileSync(join(dir, 'app.contract'), 'component App\n  view\n    column background-color="surface.card" min-height=40\n      text "card"\n');
  const file = join(dir, 'app.tamber'), was = 'tamber atomic\n\ntheme\n  surface.card = light-dark(#f1f3f5, #16181c)\n';
  writeFileSync(file, was);
  let loop = null;
  try {
    loop = await devLoop([], dir);
    const { said } = loop;
    const until = async (pass, ms) => { const end = Date.now() + ms; while (!pass()) { if (Date.now() > end) throw new Error(`not seen in: ${said().slice(-2000)}`); await new Promise(ok => setTimeout(ok, 200)); } };
    // The design server has its lock in .exact/ and the reader is built:
    // the loop's own start made no edit.
    await until(() => said().includes('plan ready') && said().includes('design reader ready') && existsSync(join(dir, '.exact', 'tamber.lock')), 900_000);
    expect(said()).not.toContain('edit →');
    // An editor's atomic save: a temporary beside the design, renamed over it.
    writeFileSync(`${file}.tmp`, was.replace('#16181c)', '#101010)'));
    renameSync(`${file}.tmp`, file);
    // The batch, then its verifying build's word that it stands.
    await until(() => said().includes('edit → checked'), 300_000);
    expect(said()).toMatch(/edit → batch ready/);
    expect(said()).not.toContain('edit → plan ready');
  } finally {
    await loop?.stop().catch(() => {});
    rmSync(dir, { recursive: true, force: true });
  }
}, 1_200_000);

test('the wasm loop\'s HTML, its page, install page and /__dev/open, may be framed only by the design tool\'s origin and its own', async () => {
  const { base, stop } = await devLoop(['--wasm'], 'caltrain');
  try {
    for (const path of ['/', '/.exact/install/', '/__dev/open']) {
      const page = await fetch(`${base}${path}`, { headers: { accept: 'text/html' } });
      expect([path, page.status, page.headers.get('content-type')?.split(';')[0]]).toEqual([path, 200, 'text/html']);
      expect([path, page.headers.get('content-security-policy'), page.headers.get('x-frame-options')]).toEqual([path, FRAME_ANCESTORS, null]);
    }
    // Nothing else carries it.
    expect((await fetch(`${base}/exact.json`)).headers.get('content-security-policy')).toBe(null);
  } finally { await stop(); }
}, 900_000);

for (const [loop, flags] of [['the JS loop', []]]) {
  test(`${loop} mounts the wire, its hello names its page, and only the design tool's origin and the page's own may frame its HTML`, async () => {
    const { base, stop } = await devLoop(flags);
    try {
      const hello = await helloOf(base);
      expect(hello.protocol).toBe('tamber/1.11');
      expect(Object.keys(hello.files)).toContain('app.tamber');
      // The loop's own origin, which the design tool frames, and the page's
      // header that lets only the tool (and itself) frame it.
      expect(hello.page).toBe(base);
      const page = await fetch(`${base}/`, { headers: { accept: 'text/html' } });
      expect(page.status).toBe(200);
      expect(page.headers.get('content-security-policy')).toBe("frame-ancestors 'self' http://127.0.0.1:7740");
      expect(page.headers.get('x-frame-options')).toBe(null);
      // Every other HTML the loop answers: an install page (the fixture has
      // no auth callback, nor a resident loop for `/__dev/open`; the wasm
      // loop's test has that, and the helper's own a name without .html).
      for (const path of ['/.exact/install/']) {
        const other = await fetch(`${base}${path}`, { headers: { accept: 'text/html' } });
        expect([path, other.status, other.headers.get('content-type')?.split(';')[0]]).toEqual([path, 200, 'text/html']);
        expect([path, other.headers.get('content-security-policy')]).toEqual([path, FRAME_ANCESTORS]);
      }
    } finally { await stop(); }
  }, 900_000);
}

// LLP 1122 D9 and D10 on the wire (stage 3 commit 5): who changed a file,
// `commit`'s guard, held proposals.
const gitIn = (...args) => spawnSync('git', ['-c', 'user.email=t@example.test', '-c', 'user.name=t', ...args], { cwd: dir, encoding: 'utf8' });
const head = () => gitIn('rev-parse', 'HEAD').stdout.trim();

test('a conflict names who last changed the file', async () => {
  const seen = { 'app.tamber': disk('app.tamber') };
  const first = await post({ method: 'transaction', client: 'agent', files: seen, ops: [{ op: 'set-token', name: 'ink', value: '#151515' }] });
  const stale = await post({ method: 'transaction', client: 'person', files: seen, ops: [{ op: 'set-token', name: 'ink', value: '#161616' }] });
  expect(stale.status).toBe(409);
  expect(stale.error.by).toEqual({ 'app.tamber': 'agent' });
  // A hand edit the watcher has not yet said: the conflict says it.
  writeFileSync(join(dir, 'app.tamber'), DESIGN);
  const again = await post({ method: 'undo', client: 'agent', inverse: first.ok.inverse });
  expect(again.status).toBe(409);
  expect(again.error.by['app.tamber']).toBe('changed on disk');
  const stream = await subscribe();
  await stream.until(e => e.at(-1)?.files?.['app.tamber'] === sha(DESIGN));
  stream.close();
}, 30_000);

test("commit takes the session's files only, keeps other staging, and never pushes", async () => {
  // Every file as HEAD has it, a remote, and something else staged.
  gitIn('checkout', '--', '.');
  const bare = mkdtempSync(join(tmpdir(), 'exact-tamber-remote-'));
  spawnSync('git', ['init', '-q', '--bare', bare]);
  gitIn('remote', 'add', 'origin', bare);
  writeFileSync(join(dir, 'notes.txt'), 'staged elsewhere\n');
  gitIn('add', 'notes.txt');
  // Earlier tests wrote these files through this wire: a session begins
  // with a wire of its own.
  await freshWire();
  const stream = await subscribe();
  await stream.until(e => e[0]?.event === 'subscribed');
  const before = head();
  const first = await post({ method: 'transaction', client: 'agent', files: { 'app.tamber': disk('app.tamber') }, ops: [{ op: 'set-token', name: 'edge', value: '#161616' }] });
  expect(first.status).toBe(200);
  // A scope read before a further write no longer holds.
  const early = await post({ method: 'commit', client: 'person', dry: true });
  const done = await post({ method: 'transaction', client: 'agent', files: { 'app.tamber': disk('app.tamber') }, ops: [{ op: 'set-token', name: 'edge', value: '#171717' }] });
  expect(done.status).toBe(200);
  const stale = await post({ method: 'commit', client: 'person', scope: early.ok.scope });
  expect([stale.status, stale.error.stale]).toEqual([409, true]);
  // A dry run: what the commit would take, each edit with its write's
  // number, and nothing written.
  const dry = await post({ method: 'commit', client: 'person', dry: true });
  expect(dry.status, JSON.stringify(dry)).toBe(200);
  expect(dry.ok).toMatchObject({ files: ['app.tamber'], edits: [{ file: 'app.tamber', client: 'agent', tokens: ['edge'], write: first.ok.write }, { file: 'app.tamber', client: 'agent', tokens: ['edge'], write: done.ok.write }] });
  expect(head()).toBe(before);
  const committed = await post({ method: 'commit', client: 'person', message: 'design: a darker edge', scope: dry.ok.scope });
  expect(committed.status, JSON.stringify(committed)).toBe(200);
  expect(committed.ok.files).toEqual(['app.tamber']);
  expect(committed.ok.edits).toEqual(dry.ok.edits);
  expect(gitIn('rev-parse', 'HEAD~1').stdout.trim()).toBe(before);
  const message = gitIn('log', '-1', '--format=%B').stdout;
  expect(message).toContain('design: a darker edge');
  expect(message).toContain('- agent: edge (app.tamber)');
  expect(gitIn('show', '--name-only', '--format=', 'HEAD').stdout.trim()).toBe(`${''}${gitIn('ls-files', '--full-name', 'app.tamber').stdout.trim()}`);
  // notes.txt is still staged, and not in the commit; app.tamber is clean.
  expect(gitIn('diff', '--cached', '--name-only').stdout.trim()).toBe('notes.txt');
  expect(gitIn('status', '--porcelain', 'app.tamber').stdout).toBe('');
  expect(spawnSync('git', ['for-each-ref'], { cwd: bare, encoding: 'utf8' }).stdout).toBe('');
  await stream.until(e => e.some(x => x.event === 'committed'));
  stream.close();
  rmSync(bare, { recursive: true, force: true });
}, 30_000);

test('commit refuses a file dirty at its baseline, and one changed by hand between writes', async () => {
  // brand.tamber differs from HEAD before the session writes it.
  writeFileSync(join(dir, 'brand.tamber'), BRAND.replace('#202020', '#242424'));
  const stream = await subscribe();
  await stream.until(e => e.at(-1)?.files?.['brand.tamber'] === sha(BRAND.replace('#202020', '#242424')));
  await post({ method: 'transaction', client: 'agent', files: { 'brand.tamber': disk('brand.tamber') }, ops: [{ op: 'set-token', file: 'brand.tamber', name: 'brand.ink', value: '#252525' }] });
  // app.tamber: written, edited by hand, written again.
  const at = head();
  await post({ method: 'transaction', client: 'agent', files: { 'app.tamber': disk('app.tamber') }, ops: [{ op: 'set-token', name: 'ink', value: '#181818' }] });
  writeFileSync(join(dir, 'app.tamber'), readFileSync(join(dir, 'app.tamber'), 'utf8').replace('#181818', '#191919'));
  await stream.until(e => e.at(-1)?.client === null && e.at(-1)?.edited.includes('app.tamber'));
  await post({ method: 'transaction', client: 'agent', files: { 'app.tamber': disk('app.tamber') }, ops: [{ op: 'set-token', name: 'ink', value: '#1a1a1a' }] });
  const refused = await post({ method: 'commit', client: 'person' });
  expect(refused.status).toBe(400);
  expect(refused.error.refused.map(r => r.file).sort()).toEqual(['app.tamber', 'brand.tamber']);
  expect(refused.error.refused.find(r => r.file === 'brand.tamber').why).toContain('differed from HEAD');
  expect(refused.error.refused.find(r => r.file === 'app.tamber').why).toContain('someone other than this server');
  expect(head()).toBe(at);
  stream.close();
}, 30_000);

test('a held proposal is accepted, and one gone stale or taken in part is recomputed', async () => {
  gitIn('checkout', '--', 'app.tamber');
  const stream = await subscribe();
  await stream.until(e => e.at(-1)?.files?.['app.tamber'] === disk('app.tamber'));
  const held = await post({ method: 'propose', hold: true, client: 'agent', ops: [{ op: 'set-token', name: 'ink', value: '#1b1b1b' }] });
  expect(held.status).toBe(200);
  expect(readFileSync(join(dir, 'app.tamber'), 'utf8')).not.toContain('#1b1b1b');
  await stream.until(e => e.some(x => x.event === 'proposal' && x.proposal === held.ok.proposal));
  const taken = await post({ method: 'accept', client: 'person', proposal: held.ok.proposal });
  expect(taken.status).toBe(200);
  expect(readFileSync(join(dir, 'app.tamber'), 'utf8')).toContain('ink = #1b1b1b');
  await stream.until(e => e.some(x => x.client === 'agent' && x.acceptedBy === 'person'));
  await stream.until(e => e.some(x => x.event === 'proposal-dropped' && x.proposal === held.ok.proposal && x.reason === 'accepted'));
  expect((await post({ method: 'accept', client: 'person', proposal: held.ok.proposal })).status).toBe(404);
  // Stale: the file moves after review; the accept gives a new diff to accept.
  const second = await post({ method: 'propose', hold: true, client: 'agent', ops: [{ op: 'set-token', name: 'edge', value: '#1c1c1c' }] });
  writeFileSync(join(dir, 'app.tamber'), readFileSync(join(dir, 'app.tamber'), 'utf8').replace('#1b1b1b', '#1d1d1d'));
  await stream.until(e => e.at(-1)?.client === null && e.at(-1)?.edited.includes('app.tamber'));
  const stale = await post({ method: 'accept', client: 'person', proposal: second.ok.proposal });
  expect(stale.ok.recomputed).toBe(true);
  expect(stale.ok.proposal).not.toBe(second.ok.proposal);
  await stream.until(e => e.some(x => x.event === 'proposal-dropped' && x.proposal === second.ok.proposal && x.reason === 'replaced' && x.replacedBy === stale.ok.proposal));
  expect(stream.events.find(x => x.event === 'proposal' && x.proposal === stale.ok.proposal).replaces).toBe(second.ok.proposal);
  expect(readFileSync(join(dir, 'app.tamber'), 'utf8')).not.toContain('#1c1c1c');
  expect((await post({ method: 'accept', client: 'person', proposal: stale.ok.proposal })).status).toBe(200);
  expect(readFileSync(join(dir, 'app.tamber'), 'utf8')).toContain('edge = #1c1c1c');
  // A held proposal checks the files its client requires, as a transaction
  // does, and so does a part of it accepted later.
  const guarded = await post({ method: 'propose', hold: true, client: 'agent', requires: { 'brand.tamber': 'f'.repeat(64) }, ops: [{ op: 'set-token', name: 'ink', value: '#1e1e1e' }] });
  expect(guarded.status).toBe(409);
  const brand = disk('brand.tamber');
  // A `requires` digest and a `files` one for the same file must agree.
  const clash = await post({ method: 'propose', hold: true, client: 'agent', files: { 'brand.tamber': brand }, requires: { 'brand.tamber': 'f'.repeat(64) }, ops: [{ op: 'set-token', name: 'ink', value: '#1e1e1e' }] });
  expect(clash.status).toBe(400);
  const kept = await post({ method: 'propose', hold: true, client: 'agent', requires: { 'brand.tamber': brand }, ops: [{ op: 'set-token', name: 'ink', value: '#1e1e1e' }, { op: 'set-token', name: 'edge', value: '#1f1f1f' }] });
  expect(kept.status).toBe(200);
  const before = readFileSync(join(dir, 'brand.tamber'), 'utf8');
  writeFileSync(join(dir, 'brand.tamber'), `${before}// moved\n`);
  await stream.until(e => e.some(x => x.client === null && x.edited?.includes('brand.tamber') && x.files['brand.tamber'] !== brand));
  expect((await post({ method: 'accept', client: 'person', proposal: kept.ok.proposal, items: [1] })).status).toBe(409);
  writeFileSync(join(dir, 'brand.tamber'), before);
  await stream.until(e => e.at(-1)?.files?.['brand.tamber'] === brand);
  // In part: the one item kept is a new proposal.
  const two = await post({ method: 'propose', hold: true, client: 'agent', ops: [{ op: 'set-token', name: 'ink', value: '#1e1e1e' }, { op: 'set-token', name: 'edge', value: '#1f1f1f' }] });
  const part = await post({ method: 'accept', client: 'person', proposal: two.ok.proposal, items: [1] });
  expect(part.ok.recomputed).toBe(true);
  expect(part.ok.ops).toEqual([{ index: 0, op: { op: 'set-token', name: 'edge', value: '#1f1f1f' } }]);
  expect(part.ok.diff).toContain('#1f1f1f');
  expect(part.ok.diff).not.toContain('#1e1e1e');
  expect((await post({ method: 'drop', proposal: part.ok.proposal })).status).toBe(400);
  expect((await post({ method: 'drop', client: 'person', proposal: part.ok.proposal })).status).toBe(200);
  await stream.until(e => e.some(x => x.event === 'proposal-dropped' && x.proposal === part.ok.proposal && x.client === 'person'));
  stream.close();
}, 30_000);

test('commit refuses a file staged at its baseline, and one whose HEAD moved; a locked index moves nothing', async () => {
  gitIn('checkout', '--', 'app.tamber', 'brand.tamber');
  gitIn('reset', '-q');
  await freshWire();
  // brand.tamber staged (and the working tree back as HEAD has it).
  writeFileSync(join(dir, 'brand.tamber'), BRAND.replace('#202020', '#262626'));
  gitIn('add', 'brand.tamber');
  spawnSync('sh', ['-c', 'git show HEAD:brand.tamber > brand.tamber'], { cwd: dir });
  const stream = await subscribe();
  await stream.until(e => e[0]?.event === 'subscribed');
  await post({ method: 'transaction', client: 'agent', files: { 'brand.tamber': disk('brand.tamber') }, ops: [{ op: 'set-token', file: 'brand.tamber', name: 'brand.ink', value: '#272727' }] });
  const staged = await post({ method: 'commit', client: 'person' });
  expect(staged.error.refused).toEqual([{ file: 'brand.tamber', why: 'it was staged when the session first wrote it' }]);
  // HEAD moves under the session.
  gitIn('reset', '-q');
  spawnSync('sh', ['-c', 'git show HEAD:brand.tamber > brand.tamber'], { cwd: dir });
  await freshWire();
  await post({ method: 'transaction', client: 'agent', files: { 'app.tamber': disk('app.tamber') }, ops: [{ op: 'set-token', name: 'ink', value: '#282828' }] });
  gitIn('commit', '-q', '--allow-empty', '-m', 'elsewhere');
  const moved = await post({ method: 'commit', client: 'person' });
  expect(moved.error.refused).toEqual([{ file: 'app.tamber', why: 'HEAD has moved since the session first wrote it' }]);
  // A locked index: the commit fails whole, HEAD where it was.
  gitIn('checkout', '--', 'app.tamber');
  await freshWire();
  await post({ method: 'transaction', client: 'agent', files: { 'app.tamber': disk('app.tamber') }, ops: [{ op: 'set-token', name: 'ink', value: '#292929' }] });
  const at = head();
  const lock = join(dir, '.git', 'index.lock');
  writeFileSync(lock, '');
  const locked = await post({ method: 'commit', client: 'person' });
  rmSync(lock);
  expect(locked.status, JSON.stringify(locked)).toBe(400);
  expect(head()).toBe(at);
  const retried = await post({ method: 'commit', client: 'person' });
  expect(retried.status, JSON.stringify(retried)).toBe(200);
  expect(gitIn('status', '--porcelain', 'app.tamber').stdout).toBe('');
  stream.close();
}, 30_000);

test("an import is held as proposals: the tokens as one, each role override apart; again, it proposes nothing", async () => {
  const { importTheme } = await import('./tamber-server.mjs');
  gitIn('checkout', '--', 'app.tamber');
  const stream = await subscribe();
  await stream.until(e => e[0]?.event === 'subscribed');
  const css = resolve(ROOT, 'tamber/tests/import/shadcn-neutral.css');
  const log = console.log;
  const said = [];
  console.log = line => said.push(line);
  try {
    expect(await importTheme(['shadcn', css, origin, '--app', dir])).toBe(0);
  } finally { console.log = log; }
  expect(said[0]).toBe('shadcn: 47 source values, 47 mapped, 0 reported');
  await stream.until(e => e.some(x => x.event === 'proposal' && x.client === 'import:shadcn'));
  const held = stream.events.find(x => x.event === 'proposal' && x.client === 'import:shadcn');
  // 23 colours, `--radius` a length and its derived `lg` (stage 6 H).
  expect(held.ops.length).toBe(30);
  expect(held.review.tokens).toBe(25);
  expect(held.review.offers.map(o => [o.role, o.items])).toEqual([['-exact-background', [25]], ['-exact-label', [26]], ['-exact-secondary-background', [27]], ['-exact-separator', [28]], ['AccentColor', [29]]]);
  expect(held.review.unmapped.map(u => u.name)).toContain('--radius-sm');
  // The overrides left off: the tokens alone, recomputed, then accepted.
  const part = await post({ method: 'accept', client: 'person', proposal: held.proposal, items: [...Array(25).keys()] });
  expect(part.ok.recomputed).toBe(true);
  // Its review kept, the overrides' items gone with them.
  expect(part.ok.review.unmapped.map(u => u.name)).toContain('--radius-sm');
  expect(part.ok.review.offers.every(o => o.items.length === 0)).toBe(true);
  expect((await post({ method: 'accept', client: 'person', proposal: part.ok.proposal })).status).toBe(200);
  const text = readFileSync(join(dir, 'app.tamber'), 'utf8');
  expect(text).toContain('shadcn.background = light-dark(oklch(1 0 0), oklch(0.145 0 0))');
  expect(text).not.toContain('-exact-background');
  expect(text).toContain('shadcn.radius: length = 0.625rem');
  said.length = 0;
  console.log = line => said.push(line);
  try {
    expect(await importTheme(['shadcn', css, origin, '--app', dir])).toBe(0);
  } finally { console.log = log; }
  expect(said).toContain('nothing to propose: the design has every mapped value already');
  stream.close();
}, 120_000);

// LLP 1122 T1's server additions (`tamber/1.1`): `history`, `revert`,
// `import`, and the design tool's files.
test('the design tool is served to a local request only, its base at its path', async () => {
  const port = 8802;
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }, { origin: `http://192.168.1.20:${port}` }], port);
  const toolDir = mkdtempSync(join(tmpdir(), 'exact-tamber-tool-'));
  const unbuilt = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {}, toolDir: join(toolDir, 'none') });
  const local = { host: `127.0.0.1:${port}` };
  const missing = await ask(unbuilt, gate, { path: '/__tamber/tool/', headers: local });
  expect(missing.status).toBe(404);
  expect(missing.body.message).toContain('bun exact.mjs tamber tool');
  unbuilt.close();
  writeFileSync(join(toolDir, 'index.html'), '<!doctype html>\n<base href="/">\n<script type="module" src="./app.js"></script>\n');
  writeFileSync(join(toolDir, 'app.js'), 'export {};\n');
  const tamber = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {}, toolDir });
  const raw = async (path, extra = {}) => {
    const req = Object.assign(new PassThrough(), { method: 'GET', headers: local, socket: { remoteAddress: '127.0.0.1' }, ...extra });
    let status = 0, head = {}, text = '';
    const res = { writeHead(s, h = {}) { status = s; head = h; }, write(t) { text += t; }, end(t) { text += t ?? ''; } };
    req.end();
    await tamber.handle(req, res, new URL(path, 'http://x'), gate.check(req));
    return { status, head, text };
  };
  expect((await raw('/__tamber/tool')).head.location).toBe('/__tamber/tool/');
  const page = await raw('/__tamber/tool/');
  expect(page.status).toBe(200);
  expect(page.head['content-type']).toContain('text/html');
  expect(page.text).toContain('<base href="/__tamber/tool/">');
  expect((await raw('/__tamber/tool/app.js')).head['content-type']).toBe('text/javascript');
  // No dot path, no way out, no write, and nothing to a request from off this machine.
  // A link out of the build, or to a dot file in it, serves nothing.
  writeFileSync(join(toolDir, '.secret'), 'dot\n');
  symlinkSync(join(ROOT, 'package.json'), join(toolDir, 'out.json'));
  symlinkSync(join(toolDir, '.secret'), join(toolDir, 'alias.txt'));
  for (const path of ['/__tamber/tool/.exact-build.json', '/__tamber/tool/a%2f..%2f..%2fsecret', '/__tamber/tool/nope.js', '/__tamber/tool/out.json', '/__tamber/tool/alias.txt']) expect((await raw(path)).status, path).toBe(404);
  expect((await raw('/__tamber/tool/', { method: 'POST' })).status).toBe(404);
  expect((await raw('/__tamber/tool/', { headers: { host: `192.168.1.20:${port}` }, socket: { remoteAddress: '192.168.1.30' } })).status).toBe(404);
  tamber.close();
  rmSync(toolDir, { recursive: true, force: true });
});

test('history reads three design commits as token edits; a revert applies, is recomputed on moved files, and is refused', async () => {
  const V1 = 'tamber t\n\ntheme\n  ink = #000000\n  edge = #101010\n';
  const V2 = V1.replace('#101010', '#202020');
  const V3 = V2.replace('#000000', '#111111') + '  card = #303030\n';
  const repo = mkdtempSync(join(tmpdir(), 'exact-tamber-history-'));
  const ada = { GIT_AUTHOR_NAME: 'Ada', GIT_AUTHOR_EMAIL: 'ada@example.test', GIT_COMMITTER_NAME: 'Ada', GIT_COMMITTER_EMAIL: 'ada@example.test' };
  const g = (...args) => spawnSync('git', args, { cwd: repo, encoding: 'utf8', env: { ...process.env, ...ada } });
  writeFileSync(join(repo, 'app.contract'), 'component App\n  view\n    text "a"\n');
  g('init', '-q');
  for (const [text, message] of [[V1, 'a design'], [V2, 'a darker edge'], [V3, 'ink and a card']]) {
    writeFileSync(join(repo, 'app.tamber'), text);
    g('add', '.'); g('commit', '-q', '-m', message);
  }
  // A commit that touches no design file is not the design's history.
  writeFileSync(join(repo, 'notes.txt'), 'n\n'); g('add', '.'); g('commit', '-q', '-m', 'notes');
  const sha = rev => g('rev-parse', rev).stdout.trim();
  const listener = createServer();
  await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
  const port = listener.address().port;
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  const wire = tamberServer({ appDir: repo, log: () => {}, settle: 250 });
  listener.on('request', async (req, res) => {
    if (!(await wire.handle(req, res, new URL(req.url, 'http://x'), gate.check(req)))) { res.writeHead(404); res.end(); }
  });
  try {
    await wire.ready;
    const at = `http://127.0.0.1:${port}`;
    const { token } = await (await fetch(`${at}/__tamber/token`)).json();
    const call = async request => {
      const res = await fetch(`${at}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: JSON.stringify(request) });
      return { status: res.status, ...(await res.json()) };
    };
    const log = await call({ method: 'history' });
    expect(log.status, JSON.stringify(log)).toBe(200);
    expect(log.ok.commits.map(c => c.message.split('\n')[0])).toEqual(['ink and a card', 'a darker edge', 'a design']);
    expect(log.ok.more).toBe(false);
    const [third, second, first] = log.ok.commits;
    expect(second).toMatchObject({ commit: sha('HEAD~2'), author: 'Ada', email: 'ada@example.test' });
    expect(second.files).toHaveLength(1);
    expect(second.files[0]).toMatchObject({ file: 'app.tamber', status: 'M', edits: [{ what: 'token', name: 'edge', when: [], before: '#101010', after: '#202020' }] });
    expect(second.files[0].diff).toContain('+  edge = #202020');
    expect(third.files[0].edits.map(e => [e.name, e.before, e.after])).toEqual([['ink', '#000000', '#111111'], ['card', null, '#303030']]);
    // Its app.contract has no state part: not the design's (stage 8 D).
    expect(first.files.map(f => f.file)).toEqual(['app.tamber']);
    expect(first.files[0].status).toBe('A');
    expect(first.files[0].edits.map(e => [e.what, e.name])).toEqual([['other', 'name'], ['token', 'ink'], ['token', 'edge']]);
    const page = await call({ method: 'history', limit: 1, skip: 1 });
    expect(page.ok.commits.map(c => c.message.split('\n')[0])).toEqual(['a darker edge']);
    expect(page.ok.more).toBe(true);
    expect((await call({ method: 'history', limit: 0 })).status).toBe(400);
    // Refused: the file moved on the commit's own edit.
    const digest = () => createHash('sha256').update(readFileSync(join(repo, 'app.tamber'))).digest('hex');
    const moved = await call({ method: 'transaction', client: 'agent', files: { 'app.tamber': digest() }, ops: [{ op: 'set-token', name: 'edge', value: '#2a2a2a' }] });
    expect(moved.status).toBe(200);
    expect((await call({ method: 'revert', commit: second.commit })).status).toBe(400);
    const refused = await call({ method: 'revert', client: 'design-tool:ada', commit: second.commit });
    expect(refused.status).toBe(409);
    expect(refused.error.message).toContain('edge');
    expect(refused.error.refused).toEqual([{ file: 'app.tamber', edit: { what: 'token', name: 'edge', when: [], before: '#101010', after: '#202020' }, now: '#2a2a2a', why: 'changed since' }]);
    // Recomputed: the third commit reverted on a file moved since, the later edit kept.
    const held = await call({ method: 'revert', client: 'design-tool:ada', commit: third.commit });
    expect(held.status, JSON.stringify(held)).toBe(200);
    expect(held.ok.review.revert).toMatchObject({ commit: third.commit, subject: 'ink and a card', moved: ['app.tamber'] });
    expect(held.ok.ops.map(o => o.op.op)).toEqual(['set-token', 'remove-token']);
    expect(readFileSync(join(repo, 'app.tamber'), 'utf8')).toContain('card');
    expect((await call({ method: 'accept', client: 'design-tool:ada', proposal: held.ok.proposal })).status).toBe(200);
    expect(readFileSync(join(repo, 'app.tamber'), 'utf8')).toBe(V1.replace('#101010', '#2a2a2a'));
    // Applied: the edit put back as the commit left it, then reverted.
    await call({ method: 'transaction', client: 'agent', files: { 'app.tamber': digest() }, ops: [{ op: 'set-token', name: 'edge', value: '#202020' }] });
    const again = await call({ method: 'revert', client: 'design-tool:ada', commit: second.commit });
    expect(again.status, JSON.stringify(again)).toBe(200);
    expect((await call({ method: 'accept', client: 'design-tool:ada', proposal: again.ok.proposal })).status).toBe(200);
    expect(readFileSync(join(repo, 'app.tamber'), 'utf8')).toBe(V1);
    const nothing = await call({ method: 'revert', client: 'design-tool:ada', commit: second.commit });
    expect(nothing.status).toBe(409);
    expect(nothing.error.message).toContain('nothing to revert');
    expect((await call({ method: 'revert', client: 'design-tool:ada', commit: sha('HEAD') })).error.message).toContain('changed no design file');
    // Held, then the files move: on the revert's own edit, the accept is
    // refused; elsewhere, it is planned again from the commit.
    const setEdge = value => call({ method: 'transaction', client: 'agent', files: { 'app.tamber': digest() }, ops: [{ op: 'set-token', name: 'edge', value }] });
    await setEdge('#202020');
    const h1 = await call({ method: 'revert', client: 'design-tool:ada', commit: second.commit });
    await setEdge('#303030');
    const stale = await call({ method: 'accept', client: 'design-tool:ada', proposal: h1.ok.proposal });
    expect(stale.status).toBe(409);
    expect(stale.error.refused[0]).toMatchObject({ edit: { name: 'edge' }, now: '#303030' });
    expect(readFileSync(join(repo, 'app.tamber'), 'utf8')).toContain('edge = #303030');
    await setEdge('#202020');
    const h2 = await call({ method: 'revert', client: 'design-tool:ada', commit: second.commit });
    await call({ method: 'transaction', client: 'agent', files: { 'app.tamber': digest() }, ops: [{ op: 'set-token', name: 'ink', value: '#121212' }] });
    const replanned = await call({ method: 'accept', client: 'design-tool:ada', proposal: h2.ok.proposal });
    expect(replanned.ok?.recomputed, JSON.stringify(replanned)).toBe(true);
    expect(replanned.ok.ops).toEqual([{ index: 0, op: { op: 'set-token', file: 'app.tamber', name: 'edge', when: [], value: '#101010', comment: null } }]);
    expect((await call({ method: 'accept', client: 'design-tool:ada', proposal: replanned.ok.proposal })).status).toBe(200);
    expect(readFileSync(join(repo, 'app.tamber'), 'utf8')).toBe(V1.replace('#000000', '#121212'));
    expect((await call({ method: 'revert', client: 'design-tool:ada', commit: '--all' })).status).toBe(400);
    expect((await call({ method: 'revert', client: 'design-tool:ada', commit: 'f'.repeat(40) })).status).toBe(404);
    // An import the tool starts from a file's text, held with its review.
    const css = readFileSync(resolve(ROOT, 'tamber/tests/import/shadcn-neutral.css'), 'utf8');
    expect((await call({ method: 'import', kind: 'shadcn', source: css })).status).toBe(400);
    const imported = await call({ method: 'import', client: 'design-tool:ada', kind: 'shadcn', source: css, name: 'globals.css' });
    expect(imported.status, JSON.stringify(imported)).toBe(200);
    expect(imported.ok.client).toBe('design-tool:ada');
    // 23 colours, `--radius` a length and its derived `lg` (LLP 1121 stage 6 H).
    expect(imported.ok.ops).toHaveLength(30);
    expect(imported.ok.review).toMatchObject({ importer: 'shadcn', counted: { source: 47, mapped: 47, reported: 0 }, tokens: 25 });
    expect(imported.ok.review.offers.map(o => o.role)).toEqual(['-exact-background', '-exact-label', '-exact-secondary-background', '-exact-separator', 'AccentColor']);
    expect(readFileSync(join(repo, 'app.tamber'), 'utf8')).toBe(V1.replace('#000000', '#121212'));
    // A stylesheet's importer takes one sheet, never silently the first of two.
    const two = await call({ method: 'import', client: 'design-tool:ada', kind: 'shadcn', sources: [{ name: 'a.css', text: css }, { name: 'b.css', text: css }] });
    expect(two.status).toBe(400);
    expect(two.error.message).toContain('imports one stylesheet');
    // Several DTCG files at once: a Figma export's modes, named as picked.
    const figma = resolve(ROOT, 'tamber/tests/import/figma');
    const sources = readdirSync(figma).map(name => ({ name, text: readFileSync(join(figma, name), 'utf8') }));
    const modes = await call({ method: 'import', client: 'design-tool:ada', kind: 'dtcg', sources });
    expect(modes.status, JSON.stringify(modes).slice(0, 400)).toBe(200);
    expect(modes.ok.review.counted).toEqual({ source: 104, mapped: 101, reported: 3 });
    // A design file whose name git quotes still has its diff.
    writeFileSync(join(repo, 'ünï.tamber'), 'tamber u\n\ntheme\n  u.ink = #010101\n');
    g('add', 'ünï.tamber'); g('commit', '-q', '-m', 'a named file');
    const named = await call({ method: 'history', limit: 1 });
    expect(named.ok.commits[0].files).toHaveLength(1);
    expect(named.ok.commits[0].files[0]).toMatchObject({ file: 'ünï.tamber', status: 'A' });
    expect(named.ok.commits[0].files[0].diff).toContain('+  u.ink = #010101');
    const astryx = await call({ method: 'import', client: 'design-tool:ada', kind: 'astryx', source: '{}' });
    expect(astryx.status).toBe(400);
    expect(astryx.error.message).toContain('bun exact.mjs tamber import astryx');
  } finally {
    await wire.close();
    listener.close();
    rmSync(repo, { recursive: true, force: true });
  }
}, 600_000);

test("a held revert keeps its untouched files' digests and a person's selection when it is planned again", async () => {
  const A1 = 'tamber t\n\ntheme\n  ink = #000000\n  edge = #101010\n';
  const B1 = 'tamber b\n\ntheme\n  b.ink = #010101\n';
  const repo = mkdtempSync(join(tmpdir(), 'exact-tamber-revert-'));
  const env = { ...process.env, GIT_AUTHOR_NAME: 'Ada', GIT_AUTHOR_EMAIL: 'ada@example.test', GIT_COMMITTER_NAME: 'Ada', GIT_COMMITTER_EMAIL: 'ada@example.test' };
  const g = (...args) => spawnSync('git', args, { cwd: repo, encoding: 'utf8', env });
  writeFileSync(join(repo, 'app.contract'), 'component App\n  view\n    text "a"\n');
  g('init', '-q');
  for (const [a, b] of [[A1, B1], [A1.replace('#000000', '#222222').replace('#101010', '#202020'), B1.replace('#010101', '#020202')]]) {
    writeFileSync(join(repo, 'app.tamber'), a); writeFileSync(join(repo, 'b.tamber'), b);
    g('add', '.'); g('commit', '-q', '-m', 'design');
  }
  const commit = g('rev-parse', 'HEAD').stdout.trim();
  const listener = createServer();
  await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
  const port = listener.address().port;
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  const wire = tamberServer({ appDir: repo, log: () => {}, settle: 250 });
  listener.on('request', async (req, res) => {
    if (!(await wire.handle(req, res, new URL(req.url, 'http://x'), gate.check(req)))) { res.writeHead(404); res.end(); }
  });
  try {
    await wire.ready;
    const at = `http://127.0.0.1:${port}`;
    const { token } = await (await fetch(`${at}/__tamber/token`)).json();
    const call = async request => {
      const res = await fetch(`${at}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: JSON.stringify(request) });
      return { status: res.status, ...(await res.json()) };
    };
    const digest = f => createHash('sha256').update(readFileSync(join(repo, f))).digest('hex');
    const set = (file, name, value) => call({ method: 'transaction', client: 'agent', files: { [file]: digest(file) }, ops: [{ op: 'set-token', file, name, value }] });
    // b.tamber already as before: no operation there, but its digest guards the plan.
    await set('b.tamber', 'b.ink', '#010101');
    const held = await call({ method: 'revert', client: 'ada', commit });
    expect(held.ok.ops.map(o => o.op.file)).toEqual(['app.tamber', 'app.tamber']);
    await set('b.tamber', 'b.ink', '#030303');
    const stale = await call({ method: 'accept', client: 'ada', proposal: held.ok.proposal });
    expect(stale.status, JSON.stringify(stale)).toBe(409);
    expect(stale.error.refused[0]).toMatchObject({ file: 'b.tamber', edit: { name: 'b.ink' }, now: '#030303' });
    expect(readFileSync(join(repo, 'app.tamber'), 'utf8')).toContain('ink = #222222');
    // Only `ink` kept; then the file moves on the item left out: planned
    // again, it is still only `ink`.
    await set('b.tamber', 'b.ink', '#010101');
    const two = await call({ method: 'revert', client: 'ada', commit });
    const inkAt = two.ok.ops.findIndex(o => o.op.name === 'ink');
    const part = await call({ method: 'accept', client: 'ada', proposal: two.ok.proposal, items: [inkAt] });
    expect(part.ok.ops.map(o => o.op.name)).toEqual(['ink']);
    await set('app.tamber', 'edge', '#2b2b2b');
    const again = await call({ method: 'accept', client: 'ada', proposal: part.ok.proposal });
    expect(again.ok?.recomputed, JSON.stringify(again)).toBe(true);
    expect(again.ok.ops.map(o => o.op.name)).toEqual(['ink']);
    expect((await call({ method: 'accept', client: 'ada', proposal: again.ok.proposal })).status).toBe(200);
    expect(readFileSync(join(repo, 'app.tamber'), 'utf8')).toBe(A1.replace('#101010', '#2b2b2b'));
    // An axis's values are each their own item: only `dim` kept stays only `dim`.
    const AX = v => `tamber b\n\naxes\n  tone = ${v}\n\ntheme\n  b.ink = #010101\n`;
    for (const v of ['black | dim | navy', 'black']) { writeFileSync(join(repo, 'b.tamber'), AX(v)); g('add', 'b.tamber'); g('commit', '-q', '-m', `tone ${v}`); }
    // The server reads the checkout.
    for (let i = 0; i < 100 && !(await call({ method: 'hello' })).ok.files['b.tamber']?.startsWith(digest('b.tamber').slice(0, 8)); i++) await Bun.sleep(50);
    const axes = await call({ method: 'revert', client: 'ada', commit: g('rev-parse', 'HEAD').stdout.trim() });
    expect(axes.ok.ops.map(o => o.op.value)).toEqual(['dim', 'navy']);
    const dimOnly = await call({ method: 'accept', client: 'ada', proposal: axes.ok.proposal, items: [0] });
    await set('b.tamber', 'b.ink', '#040404');
    const again2 = await call({ method: 'accept', client: 'ada', proposal: dimOnly.ok.proposal });
    expect(again2.ok?.ops.map(o => o.op.value), JSON.stringify(again2)).toEqual(['dim']);
    // A client's own review cannot pose as a revert.
    expect((await call({ method: 'propose', hold: true, client: 'agent', ops: [{ op: 'set-token', name: 'ink', value: '#090909' }], review: { revert: { commit, only: {} } } })).status).toBe(400);
  } finally {
    await wire.close();
    listener.close();
    rmSync(repo, { recursive: true, force: true });
  }
}, 600_000);

// LLP 1121 stage 8 D: the app's own `.contract` files are the server's too:
// a hand edit of a state part is announced, a `set-state` is a session's
// write that `commit` takes, and a file dirty at its baseline is refused.
test("the app's .contract: a hand edit is announced, set-state is committed, and one dirty at its baseline refused", async () => {
  const VIEW = 'style Post\n  background-color="#0a7aff"\n\ncomponent App\n  view\n    column class=Post\n      text "Post"\n';
  const repo = mkdtempSync(join(tmpdir(), 'exact-tamber-contract-'));
  const env = { ...process.env, GIT_AUTHOR_NAME: 'Ada', GIT_AUTHOR_EMAIL: 'ada@example.test', GIT_COMMITTER_NAME: 'Ada', GIT_COMMITTER_EMAIL: 'ada@example.test' };
  const g = (...args) => spawnSync('git', args, { cwd: repo, encoding: 'utf8', env });
  writeFileSync(join(repo, 'app.contract'), VIEW);
  writeFileSync(join(repo, 'app.tamber'), 'tamber t\n\ntheme\n  ink = #000000\n');
  g('init', '-q'); g('config', 'user.email', 'ada@example.test'); g('config', 'user.name', 'Ada'); g('add', '.'); g('commit', '-q', '-m', 'app');
  const listener = createServer();
  await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
  const port = listener.address().port;
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  let wire = tamberServer({ appDir: repo, log: () => {}, settle: 250 });
  listener.on('request', async (req, res) => {
    if (!(await wire.handle(req, res, new URL(req.url, 'http://x'), gate.check(req)))) { res.writeHead(404); res.end(); }
  });
  const at = `http://127.0.0.1:${port}`;
  let token;
  const call = async request => {
    const res = await fetch(`${at}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: JSON.stringify(request) });
    return { status: res.status, ...(await res.json()) };
  };
  const digest = f => createHash('sha256').update(readFileSync(join(repo, f))).digest('hex');
  const active = value => ({ op: 'set-state', style: 'Post', state: 'active', row: 'background-color', value });
  try {
    await wire.ready;
    ({ token } = await (await fetch(`${at}/__tamber/token`)).json());
    const res = await fetch(`${at}/__tamber/subscribe`, { method: 'POST', headers: { 'x-exact-tamber-token': token } });
    const reader = res.body.getReader(), decoder = new TextDecoder(), events = [];
    let buf = '';
    (async () => { for (;;) { const { value, done } = await reader.read().catch(() => ({ done: true })); if (done) return; buf += decoder.decode(value, { stream: true }); const parts = buf.split('\n\n'); buf = parts.pop(); for (const p of parts) if (p.startsWith('data: ')) events.push(JSON.parse(p.slice(6))); } })();
    const until = async pass => { const end = Date.now() + 5000; while (!pass(events)) { if (Date.now() > end) throw new Error(JSON.stringify(events)); await Bun.sleep(20); } };
    await until(e => e[0]?.event === 'subscribed');
    // A hand edit of the Contract source, a state part added: announced.
    writeFileSync(join(repo, 'app.contract'), VIEW.replace('  background-color="#0a7aff"\n', '  background-color="#0a7aff"\n  :hover\n    opacity=0.9\n'));
    await until(e => e.some(x => x.event === 'changed' && x.client === null && x.edited?.includes('app.contract')));
    writeFileSync(join(repo, 'app.contract'), VIEW);
    await until(e => e.at(-1)?.files?.['app.contract'] === digest('app.contract'));
    // A set-state: the session's write, which commit takes.
    const done = await call({ method: 'transaction', client: 'agent', files: { 'app.contract': digest('app.contract') }, ops: [active('#0050c8')] });
    expect(done.status, JSON.stringify(done)).toBe(200);
    expect(done.ok.class).toBe('rebuild');
    expect(readFileSync(join(repo, 'app.contract'), 'utf8')).toContain('  :active\n    background-color="#0050c8"\n');
    const committed = await call({ method: 'commit', client: 'person', message: 'design: a pressed post' });
    expect(committed.status, JSON.stringify(committed)).toBe(200);
    expect(committed.ok.files).toEqual(['app.contract']);
    // Dirty at its baseline: a session that writes it refuses to commit it.
    writeFileSync(join(repo, 'app.contract'), readFileSync(join(repo, 'app.contract'), 'utf8').replace('#0a7aff', '#0b7bff'));
    await until(e => e.at(-1)?.files?.['app.contract'] === digest('app.contract'));
    await wire.close();
    wire = tamberServer({ appDir: repo, log: () => {}, settle: 250 });
    await wire.ready;
    ({ token } = await (await fetch(`${at}/__tamber/token`)).json());
    const again = await call({ method: 'transaction', client: 'agent', files: { 'app.contract': digest('app.contract') }, ops: [active('#003399')] });
    expect(again.status, JSON.stringify(again)).toBe(200);
    const refused = await call({ method: 'commit', client: 'person' });
    expect(refused.status).toBe(400);
    expect(refused.error.refused.find(r => r.file === 'app.contract').why).toContain('differed from HEAD');
    reader.cancel();
  } finally {
    await wire.close();
    listener.close();
    rmSync(repo, { recursive: true, force: true });
  }
}, 600_000);
// Stage 8 D, review round 2: a held set-state with no `file` is guarded by
// the Contract sources it does not write; a source two directories down is
// still watched after its directory is replaced empty and filled again.
test("a held set-state is guarded by the other Contract sources, and a replaced directory's source stays watched", async () => {
  const repo = mkdtempSync(join(tmpdir(), 'exact-tamber-contract2-'));
  mkdirSync(join(repo, 'src/ui'), { recursive: true });
  const POST = 'style Post\n  background-color="#0a7aff"\n\ncomponent PostButton\n  view\n    column class=Post\n      text "Post"\n';
  writeFileSync(join(repo, 'src/ui/post.contract'), POST);
  writeFileSync(join(repo, 'app.contract'), 'use PostButton from "./src/ui/post.contract"\ncomponent App\n  view\n    PostButton()\n');
  writeFileSync(join(repo, 'app.tamber'), 'tamber t\n\ntheme\n  ink = #000000\n');
  const listener = createServer();
  await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
  const port = listener.address().port;
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  const wire = tamberServer({ appDir: repo, log: () => {}, settle: 250 });
  listener.on('request', async (req, res) => {
    if (!(await wire.handle(req, res, new URL(req.url, 'http://x'), gate.check(req)))) { res.writeHead(404); res.end(); }
  });
  const at = `http://127.0.0.1:${port}`;
  let token;
  const call = async request => {
    const res = await fetch(`${at}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: JSON.stringify(request) });
    return { status: res.status, ...(await res.json()) };
  };
  const digest = f => createHash('sha256').update(readFileSync(join(repo, f))).digest('hex');
  try {
    await wire.ready;
    ({ token } = await (await fetch(`${at}/__tamber/token`)).json());
    const res = await fetch(`${at}/__tamber/subscribe`, { method: 'POST', headers: { 'x-exact-tamber-token': token } });
    const reader = res.body.getReader(), decoder = new TextDecoder(), events = [];
    let buf = '';
    (async () => { for (;;) { const { value, done } = await reader.read().catch(() => ({ done: true })); if (done) return; buf += decoder.decode(value, { stream: true }); const parts = buf.split('\n\n'); buf = parts.pop(); for (const p of parts) if (p.startsWith('data: ')) events.push(JSON.parse(p.slice(6))); } })();
    const until = async pass => { const end = Date.now() + 5000; while (!pass(events)) { if (Date.now() > end) throw new Error(JSON.stringify(events)); await Bun.sleep(20); } };
    await until(e => e[0]?.event === 'subscribed');
    const files = { 'app.contract': digest('app.contract'), 'src/ui/post.contract': digest('src/ui/post.contract') };
    const held = await call({ method: 'propose', hold: true, client: 'agent', files, ops: [{ op: 'set-state', style: 'Post', state: 'active', row: 'opacity', value: 0.8 }] });
    expect(held.status, JSON.stringify(held)).toBe(200);
    expect(held.ok.texts).toBeUndefined();
    // A hand edit of the source it does not write: the accept is refused.
    writeFileSync(join(repo, 'app.contract'), `${readFileSync(join(repo, 'app.contract'), 'utf8')}// moved\n`);
    await until(e => e.some(x => x.client === null && x.edited?.includes('app.contract')));
    const taken = await call({ method: 'accept', client: 'person', proposal: held.ok.proposal });
    expect(taken.status, JSON.stringify(taken)).toBe(409);
    expect(readFileSync(join(repo, 'src/ui/post.contract'), 'utf8')).toBe(POST);
    // `src` replaced empty (a read taken then), then filled again.
    renameSync(join(repo, 'src'), join(repo, 'src.old'));
    mkdirSync(join(repo, 'src'));
    await until(e => e.some(x => x.event === 'changed' && x.files && !('src/ui/post.contract' in x.files)));
    mkdirSync(join(repo, 'src/ui'));
    writeFileSync(join(repo, 'src/ui/post.contract'), POST);
    await until(e => e.at(-1)?.files?.['src/ui/post.contract'] === digest('src/ui/post.contract'));
    await Bun.sleep(400);
    writeFileSync(join(repo, 'src/ui/post.contract'), POST.replace('#0a7aff', '#0b7bff'));
    await until(e => e.at(-1)?.files?.['src/ui/post.contract'] === digest('src/ui/post.contract'));
    reader.cancel();
  } finally {
    await wire.close();
    listener.close();
    rmSync(repo, { recursive: true, force: true });
  }
}, 600_000);
// The app's own .contract files are in history and revert too (stage 8 D's
// follow-up): a commit's state rows are edits, its other lines `code`,
// which a revert leaves alone.
test("history reads a .contract commit's state rows, and a revert undoes them, leaving its code", async () => {
  const V1 = 'style Post\n  background-color="#0a7aff"\n\ncomponent App\n  view\n    column class=Post\n      text "Post"\n';
  const V2 = 'style Post\n  background-color="#0a7aff"\n  :active\n    opacity=0.5\n\ncomponent App\n  view\n    column class=Post\n      text "Posted"\n';
  const repo = mkdtempSync(join(tmpdir(), 'exact-tamber-history-contract-'));
  const env = { ...process.env, GIT_AUTHOR_NAME: 'Ada', GIT_AUTHOR_EMAIL: 'ada@example.test', GIT_COMMITTER_NAME: 'Ada', GIT_COMMITTER_EMAIL: 'ada@example.test' };
  const g = (...args) => spawnSync('git', args, { cwd: repo, encoding: 'utf8', env });
  writeFileSync(join(repo, 'app.tamber'), 'tamber t\n\ntheme\n  ink = #000000\n');
  writeFileSync(join(repo, 'app.contract'), V1);
  g('init', '-q'); g('add', '.'); g('commit', '-q', '-m', 'app');
  writeFileSync(join(repo, 'app.contract'), V2); g('add', '.'); g('commit', '-q', '-m', 'a pressed post');
  // A commit that changes only a Contract source's code is no design commit.
  writeFileSync(join(repo, 'app.contract'), V2.replace('Posted', 'Sent')); g('add', '.'); g('commit', '-q', '-m', 'reword');
  const listener = createServer();
  await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
  const port = listener.address().port;
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  const wire = tamberServer({ appDir: repo, log: () => {}, settle: 250 });
  listener.on('request', async (req, res) => {
    if (!(await wire.handle(req, res, new URL(req.url, 'http://x'), gate.check(req)))) { res.writeHead(404); res.end(); }
  });
  try {
    await wire.ready;
    const at = `http://127.0.0.1:${port}`;
    const { token } = await (await fetch(`${at}/__tamber/token`)).json();
    const call = async request => {
      const res = await fetch(`${at}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: JSON.stringify(request) });
      return { status: res.status, ...(await res.json()) };
    };
    await call({ method: 'hello', client: 'test' });
    const log = await call({ method: 'history' });
    expect(log.ok.commits.map(c => c.message.split('\n')[0])).toEqual(['a pressed post', 'app']);
    expect(log.status, JSON.stringify(log)).toBe(200);
    const pressed = log.ok.commits.find(c => c.message.startsWith('a pressed post'));
    const file = pressed.files.find(f => f.file === 'app.contract');
    expect(file.edits).toEqual([{ what: 'state', name: 'Post', state: 'active', row: 'opacity', when: [], before: null, after: '0.5' }]);
    expect(file.code).toBe(true);
    const held = await call({ method: 'revert', client: 'person', commit: pressed.commit });
    expect(held.status, JSON.stringify(held)).toBe(200);
    expect(held.ok.ops).toEqual([{ index: 0, op: { op: 'unset-state', style: 'Post', state: 'active', row: 'opacity', file: 'app.contract' } }]);
    expect(held.ok.review.revert.code).toEqual(['app.contract']);
    const taken = await call({ method: 'accept', client: 'person', proposal: held.ok.proposal });
    expect(taken.status, JSON.stringify(taken)).toBe(200);
    const now = readFileSync(join(repo, 'app.contract'), 'utf8');
    expect(now).not.toContain(':active');
    expect(now).toContain('text "Sent"');
    // Again: nothing left to revert.
    expect((await call({ method: 'revert', client: 'person', commit: pressed.commit })).status).toBe(409);
  } finally {
    await wire.close();
    listener.close();
    rmSync(repo, { recursive: true, force: true });
  }
}, 600_000);

// LLP 1122 D4, D8; LLP 1121 stage 8 E (*1.11*): `hold`, a present client's
// with write scope, sent to its build; replayed after a rebuild at the one
// site at the same source location in the new plan's map; the build's `held`
// is every client's event.
test('a hold reaches its build and follows its site into the build\'s next plan', async () => {
  const port = 8807, P1 = '4'.repeat(64), P2 = '5'.repeat(64);
  const node = (line) => ({ file: 'exact:app', line, col: 5, end_col: 9, component: 'App', chain: [], bindings: [] });
  const maps = { [P1]: { digest: P1, sources: [{ file: 'exact:app', sha256: 'x' }], nodes: [node(3), node(7)] }, [P2]: { digest: P2, sources: [{ file: 'exact:app', sha256: 'x' }], nodes: [node(1), node(2), node(7)] } };
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  const tamber = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {}, sourceMap: digest => (maps[digest] ? Buffer.from(JSON.stringify(maps[digest])) : null) });
  const headers = { host: `127.0.0.1:${port}`, 'x-exact-tamber-token': tamber.token };
  const post = body => ask(tamber, gate, { path: '/__tamber', method: 'POST', headers, body: JSON.stringify(body) });
  const frames = [], s = tamber.channel.open(f => frames.push(f));
  tamber.channel.report(JSON.stringify({ nonce: s.nonce, kind: 'register', id: 'web-1', host: 'web', capabilities: ['pin', 'hold'], plan: 'g', planDigest: P1 }));
  // Not present: refused.
  expect((await post({ method: 'hold', client: 'tool', build: 'web-1', site: 1, instance: [], state: 'active' })).status).toBe(409);
  await ask(tamber, gate, { path: '/__tamber/events', method: 'POST', headers, body: JSON.stringify({ wait: false, client: 'tool' }) });
  const held = await post({ method: 'hold', client: 'tool', build: 'web-1', site: 1, instance: [], state: 'active' });
  expect(held.status).toBe(200);
  expect(frames.at(-1)).toEqual({ kind: 'hold', revision: held.body.ok.revision, hold: { site: 1, instance: [], state: 'active' }, plan: P1 });
  // Its build says it holds it: every client is told.
  tamber.channel.report(JSON.stringify({ nonce: s.nonce, kind: 'held', state: 'active', revision: held.body.ok.revision }));
  const events = (await ask(tamber, gate, { path: '/__tamber/events', method: 'POST', headers, body: JSON.stringify({ after: 0, wait: false, client: 'tool' }) })).body.events;
  expect(events.find(e => e.event === 'held')).toMatchObject({ build: 'web-1', state: 'active' });
  // A rebuild: site 1 (line 7) is site 2 of the new plan.
  tamber.channel.report(JSON.stringify({ nonce: s.nonce, kind: 'status', scheme: 'light', revision: 0, planDigest: P2 }));
  expect(frames.at(-1)).toMatchObject({ kind: 'hold', hold: { site: 2, instance: [], state: 'active' } });
  expect(frames.at(-1).revision).toBeGreaterThan(held.body.ok.revision);
  // Let go.
  expect((await post({ method: 'hold', client: 'tool', build: 'web-1', state: null })).status).toBe(200);
  expect(frames.at(-1)).toMatchObject({ kind: 'hold', hold: null });
  // A hold refused is its owner's answer: another client never sees it.
  const hover = await post({ method: 'hold', client: 'tool', build: 'web-1', site: 2, instance: [], state: 'hover' });
  tamber.channel.report(JSON.stringify({ nonce: s.nonce, kind: 'unsupported', what: 'hold', detail: 'no hover here', revision: hover.body.ok.revision }));
  const seen = async (client) => (await ask(tamber, gate, { path: '/__tamber/events', method: 'POST', headers, body: JSON.stringify({ after: 0, wait: false, client }) })).body.events.filter(e => e.event === 'unsupported');
  expect(await seen('tool')).toMatchObject([{ build: 'web-1', what: 'hold', detail: 'no hover here', only: 'tool' }]);
  expect(await seen('other')).toEqual([]);
  await tamber.close();
});
