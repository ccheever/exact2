// The Apple host's transfer carrier (LLP 1133.017 §14 stage 2, D8, D10),
// driven through the probe (`test/probe`), a module outside the tree that
// starts an upload with `context.transfers` and raises its ending as a wake
// event of its own. The fixture app's handler tells this suite's server what
// the ending carried.
//
// - In front: an upload's body reaches the server, and its ending is raised
//   at once with the response's status and body.
// - Across termination: the server holds its response while the agent kills
//   the app. On iOS the system relaunches the app in the background for the
//   session's events (`handleEventsForBackgroundURLSession`), the host
//   boots the session with no view (D7) and raises the ending; the suite
//   records whether the handler was heard, and whether the response body
//   survived (D10's `bodyLost`). On the Mac the next launch reconnects the
//   session and delivers what the system finished (D4's cell).
//
// What the measurement answers is printed as `note` lines; only the first
// drive is a check.
//
// usage: bun packages/background/test/transfer.mjs [macos] [ios --sim <udid>]
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { checker } from '../../camera/test/fixture.mjs';
import { until } from '../../photos/test/fixture.mjs';

const here = import.meta.dir, probe = resolve(here, 'probe'), root = resolve(here, '../../..');
const argv = process.argv.slice(2), sim = argv.includes('--sim') ? argv[argv.indexOf('--sim') + 1] : process.env.EXACT_SIM;
const hosts = argv.filter((a, i) => ['macos', 'ios'].includes(a) && argv[i - 1] !== '--sim');
const run = (cmd, args, cwd, ok = false) => {
  const r = spawnSync(cmd, args, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], maxBuffer: 1 << 26 });
  if (r.status !== 0 && !ok) throw new Error(`${cmd} ${args.join(' ')}: ${r.stdout.slice(-4000)}\n${r.stderr.slice(-4000)}`);
  return r.stdout;
};
const { check, failed } = checker();
const wait = (ms) => new Promise((r) => setTimeout(r, ms));
const poll = async (ok, ms) => { for (const end = Date.now() + ms; Date.now() < end; await wait(250)) if (ok()) return true; return ok(); };

// The uploads' bodies and query, by id; what each ending's handler said; how long `/upload` holds its answer.
const uploads = {}, heard = {};
let hold = 0;
const server = Bun.serve({ port: 0, hostname: '127.0.0.1', idleTimeout: 0, async fetch(req) {
  const url = new URL(req.url), id = url.searchParams.get('id');
  if (url.pathname === '/upload') {
    uploads[id] = { body: await req.text(), token: url.searchParams.get('token'), at: Date.now() };
    if (hold) await wait(hold);
    uploads[id].answered = Date.now();
    return new Response(`kept ${id}`, { status: 201, headers: { 'Content-Type': 'text/plain', Location: `/assets/${id}` } });
  }
  if (url.pathname === '/heard') heard[id] = { status: url.searchParams.get('status'), body: url.searchParams.get('body'), lost: url.searchParams.get('lost'), at: Date.now() };
  return new Response('ok');
} });
const origin = `http://127.0.0.1:${server.port}`;

function makeFixture() {
  const dir = mkdtempSync(join(tmpdir(), 'exact-transfer-')), app = join(dir, 'probefixture');
  run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
  for (const f of ['app.contract', 'app.ts']) cpSync(join(probe, 'fixture', f), join(app, f));
  rmSync(join(app, 'app.test.contract'), { force: true });
  const id = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).app.id;
  writeFileSync(join(app, 'app.ts'), readFileSync(join(app, 'app.ts'), 'utf8').replace('__APP_ID__', id).replace('__ORIGIN__', origin));
  const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
  manifest.dependencies = { ...manifest.dependencies, '@fixture/transfer-probe': `file:${probe}` };
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
    run('bun', [resolve(root, 'host/apple/build.mjs'), ...(host === 'ios' ? ['--ios', '--sim', sim] : []), 'probefixture-apple'], root);
    const { open } = await import(resolve(root, 'scripts/agent.mjs'));
    const session = () => open({ host, app: 'probefixture', bind: 'transfer-probe', storage: `probefixture-${process.pid}` });
    let s = await session();
    try {
      hold = 0;
      await s.tap('start-u1');
      const started = await until(s, 'started', (t) => t !== 'none');
      const noted = await until(s, 'noted', (t) => t !== 'none', 60_000);
      check(where, 'an upload started in front reaches the server, its body from the file', started === 'started' && uploads.u1?.body === 'hello u1' && uploads.u1?.token === 'secret',
        `${started} ${JSON.stringify(uploads.u1)}`);
      check(where, 'its ending is raised with the status and the response body', noted === 'u1 201 kept u1 false' && heard.u1?.body === 'kept u1', noted);
      const journal = (await s.logs({ all: true })).lines;
      if (failed()) console.log(journal.filter((l) => /probe|transfer|wake/.test(l)).join('\n'));

      hold = 20_000;
      await s.tap('start-u2');
      await until(s, 'started', (t) => t === 'started');
      const sent = await poll(() => uploads.u2, 30_000);
      console.log(`note ${where} u2's body ${sent ? 'reached the server' : 'did not reach the server in 30 s'}; the app is killed while its response is held`);
    } finally {
      await s.close();
    }
    const killed = Date.now();
    if (host === 'macos') console.log(`note ${where} the app's process at the close: ${run('pgrep', ['-f', fixture.id], root, true).trim() ? 'running' : 'gone'}`);
    const answered = await poll(() => uploads.u2?.answered, 40_000);
    const relaunched = host === 'ios' && answered && await poll(() => heard.u2, 90_000);
    if (host === 'ios') {
      const h = heard.u2;
      console.log(`note ${where} after the kill: ${h ? `the handler was heard ${Math.round((h.at - uploads.u2.answered) / 1000)} s after the response, in the background, status ${h.status}, body "${h.body}", bodyLost ${h.lost}` : 'no relaunch delivered the ending within 90 s of the response'}`);
      const running = run('xcrun', ['simctl', 'spawn', sim, 'launchctl', 'list'], root, true).split('\n').find((l) => l.includes(fixture.id));
      console.log(`note ${where} the app's process after the wake: ${running ?? 'none'}`);
      // The host's lines in the system's log: a relaunched process has no console.
      const lines = run('xcrun', ['simctl', 'spawn', sim, 'log', 'show', '--last', '4m', '--style', 'compact', '--predicate', 'subsystem == "dev.exact"'], root, true);
      console.log(lines.split('\n').filter((l) => /wakes|transfers/.test(l)).map((l) => `  ${l}`).join('\n'));
    }
    if (!relaunched) {
      s = await session();
      try {
        const noted = await until(s, 'noted', (t) => t.startsWith('u2'), 60_000);
        const h = heard.u2;
        console.log(`note ${where} at the next launch with a view (${Math.round((Date.now() - killed) / 1000)} s after the kill): ${h ? `status ${h.status}, body "${h.body}", bodyLost ${h.lost}` : `nothing delivered (${noted})`}`);
      } finally {
        await s.close();
      }
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
