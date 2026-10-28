// @exact/background-transfer's conformance suite on Apple (LLP 1133.017 §14
// stage 2, D3, D4, D10): the fixture app (`test/fixture`) built for macOS or
// an iOS simulator and driven by the agent. The interface is its own
// substitute, so the host's carrier runs as in production against this
// suite's server.
//
// - The status: available and `granted`.
// - An upload: its body read from an app file, its headers and query sent
//   and never shown; its ending raised as `backgroundfetchsuccess`, the
//   response body in `app:/data/transfers/<id>/<index>`, the URL shown
//   without its query, the bytes counted; retired by the handler.
// - A download to `saveTo`: the body in that file.
// - A 404: `backgroundfetchfail`, `bad-status`.
// - A URL no `net.fetch` line grants: refused before anything is sent.
// - A download held by the server and aborted: `backgroundfetchabort`,
//   `aborted`.
// - Nothing the journal shows carries the credential.
//
// usage: bun packages/background-transfer/test/apple.mjs [macos] [ios --sim <udid>]
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { checker } from '../../camera/test/fixture.mjs';
import { until } from '../../photos/test/fixture.mjs';

const here = import.meta.dir, pkg = resolve(here, '..'), root = resolve(pkg, '../..');
const argv = process.argv.slice(2), sim = argv.includes('--sim') ? argv[argv.indexOf('--sim') + 1] : process.env.EXACT_SIM;
const hosts = argv.filter((a, i) => ['macos', 'ios'].includes(a) && argv[i - 1] !== '--sim');
const run = (cmd, args, cwd, ok = false) => {
  const r = spawnSync(cmd, args, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], maxBuffer: 1 << 26 });
  if (r.status !== 0 && !ok) throw new Error(`${cmd} ${args.join(' ')}: ${r.stdout.slice(-4000)}\n${r.stderr.slice(-4000)}`);
  return r.stdout;
};
const { check, failed } = checker();
const wait = (ms) => new Promise((r) => setTimeout(r, ms));

// What each request carried, by id; what each handler told.
const sent = {}, heard = {};
const server = Bun.serve({ port: 0, hostname: '127.0.0.1', idleTimeout: 0, async fetch(req) {
  const url = new URL(req.url), id = url.searchParams.get('id');
  if (url.pathname === '/heard') {
    heard[id] = Object.fromEntries(url.searchParams);
    return new Response('ok');
  }
  sent[id] = { path: url.pathname, method: req.method, body: await req.text(), token: url.searchParams.get('token'), authorization: req.headers.get('authorization') };
  if (url.pathname === '/upload') return new Response(`kept ${id}`, { status: 201, headers: { 'Content-Type': 'text/plain' } });
  if (url.pathname === '/file' && url.searchParams.get('hold')) await wait(120_000);
  if (url.pathname === '/file') return new Response(`file ${id}`, { headers: { 'Content-Type': 'text/plain' } });
  return new Response('missing', { status: 404 });
} });
const origin = `http://127.0.0.1:${server.port}`;

function makeFixture() {
  const dir = mkdtempSync(join(tmpdir(), 'exact-transfer-')), app = join(dir, 'bgtfixture');
  run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
  for (const f of ['app.contract', 'app.ts']) cpSync(join(here, 'fixture', f), join(app, f));
  rmSync(join(app, 'app.test.contract'), { force: true });
  const id = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).app.id;
  writeFileSync(join(app, 'app.ts'), readFileSync(join(app, 'app.ts'), 'utf8').replace('__APP_ID__', id).replace('__ORIGIN__', origin));
  const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
  manifest.dependencies = { ...manifest.dependencies, '@exact/background-transfer': `file:${pkg}` };
  writeFileSync(join(app, 'package.json'), JSON.stringify(manifest, null, 2));
  run('bun', ['install'], app);
  return { app, id, remove: () => rmSync(dir, { recursive: true, force: true }) };
}

for (const host of hosts.length ? hosts : ['macos']) {
  const where = host === 'ios' ? 'ios-simulator' : 'macos';
  if (host === 'ios' && !sim) throw new Error('ios: name a simulator you booted (--sim <udid>)');
  const fixture = makeFixture();
  Object.assign(process.env, { EXACT_APP_DIR: fixture.app, EXACT2: root, EXACT_IDENTITY: process.env.EXACT_IDENTITY ?? '-' });
  if (sim) process.env.EXACT_SIM = sim;
  try {
    run('bun', [resolve(root, 'host/apple/build.mjs'), ...(host === 'ios' ? ['--ios', '--sim', sim] : []), 'bgtfixture-apple'], root);
    const { open } = await import(resolve(root, 'scripts/agent.mjs'));
    const s = await open({ host, app: 'bgtfixture', storage: `bgtfixture-${process.pid}` });
    try {
      const status = await until(s, 'status', (t) => t !== '');
      check(where, 'the status: available and granted', status === 'true granted', status);

      await s.tap('upload-u1');
      const begun = await until(s, 'started', (t) => t !== 'none', 20_000);
      check(where, 'data code starts an upload', begun === 'started', begun);
      const u1 = await until(s, 'noted', (t) => t.startsWith('u1'), 60_000);
      const h = heard.u1 ?? {};
      check(where, 'an upload sends its file with its headers and query', sent.u1?.body === 'hello u1' && sent.u1?.token === 'secret' && sent.u1?.authorization === 'Bearer secret',
        JSON.stringify(sent.u1));
      check(where, 'its ending is backgroundfetchsuccess, the response body in app:/data/transfers', u1 === 'u1 success success  201 kept u1' && h.path === 'app:/data/transfers/u1/0', `${u1} ${JSON.stringify(h)}`);
      check(where, 'its record shows the URL without its query, and the bytes sent', h.url === `${origin}/upload` && h.uploaded === '8' && h.uploadTotal === '8', JSON.stringify(h));
      const ids = await until(s, 'ids', (t) => t === '', 10_000);
      check(where, 'the handler retired it: no id is listed', ids === '', ids);

      await s.tap('download-d1');
      const d1 = await until(s, 'noted', (t) => t.startsWith('d1'), 60_000);
      check(where, 'a download lands in its saveTo file', d1 === 'd1 success success  200 file d1' && heard.d1?.path === 'app:/data/saved/d1.txt' && heard.d1?.downloaded === '7',
        `${d1} ${JSON.stringify(heard.d1)}`);

      await s.tap('missing-m1');
      const m1 = await until(s, 'noted', (t) => t.startsWith('m1'), 60_000);
      check(where, 'a 404 ends backgroundfetchfail, bad-status', m1.startsWith('m1 fail failure bad-status 404'), m1);

      await s.tap('ungranted-x1');
      const x1 = await until(s, 'started', (t) => t !== 'started', 20_000);
      check(where, 'a URL no net.fetch line grants is refused before anything is sent', x1 !== 'started' && x1 !== 'none' && !sent.x1, x1);

      await s.tap('hold-h1');
      await until(s, 'started', (t) => t === 'started', 20_000);
      for (const end = Date.now() + 20_000; !sent.h1 && Date.now() < end;) await wait(250);
      await s.tap('abort-h1');
      const aborted = await until(s, 'aborted', (t) => t !== 'none', 20_000);
      const h1 = await until(s, 'noted', (t) => t.startsWith('h1'), 60_000);
      check(where, 'an aborted transfer ends backgroundfetchabort, aborted', aborted === 'true' && h1.startsWith('h1 abort failure aborted'), `${aborted} ${h1}`);

      const journal = (await s.logs({ all: true })).lines;
      check(where, 'the journal never shows the credential', !journal.some((l) => l.includes('secret')), journal.filter((l) => l.includes('secret')).join('\n'));
      if (failed()) console.log(journal.filter((l) => /transfer|wake|grant|fs|refused/.test(l)).join('\n'));
    } finally {
      await s.close();
    }
  } finally {
    if (host === 'ios') run('xcrun', ['simctl', 'uninstall', sim, fixture.id], root, true);
    else rmSync(join(homedir(), 'Library/Application Support', fixture.id), { recursive: true, force: true });
    fixture.remove();
  }
}
server.stop(true);
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
