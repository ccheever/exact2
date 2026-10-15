// The design tool (LLP 1122 T1) driven as a person would, against a real
// design server: `bun exact.mjs tamber tool` on a copy of the theme fixture
// serves it at its own URL, and the agent drives that page (`agent web
// --url`). The tool shows the server's hello, and an agent's write arrives
// on its long poll attributed. Its stub-server tests are
// apps/design-tool/app.test.contract.
import { expect, test } from 'bun:test';
import { spawn, spawnSync } from 'node:child_process';
import { cpSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createHash } from 'node:crypto';
import { TOOL_ORIGIN } from './tamber-server.mjs';

const ROOT = resolve(new URL('..', import.meta.url).pathname);

/** Lines of `child`'s stdout as they come; `until(pass)` waits for one. */
function lines(child) {
  const seen = [];
  let buf = '';
  child.stdout.setEncoding('utf8');
  child.stdout.on('data', d => { buf += d; const all = buf.split('\n'); buf = all.pop(); seen.push(...all); });
  const until = async (pass, ms) => {
    const end = Date.now() + ms;
    while (!seen.some(pass)) { if (Date.now() > end || child.exitCode !== null) throw new Error(`not seen: ${seen.join('\n')}`); await Bun.sleep(50); }
    return seen.find(pass);
  };
  return { seen, until };
}

const stop = async child => {
  if (child.exitCode !== null || child.signalCode !== null) return;
  const exit = new Promise(ok => child.once('exit', ok));
  child.kill('SIGTERM');
  if (!(await Promise.race([exit.then(() => true), Bun.sleep(15_000).then(() => false)]))) child.kill('SIGKILL');
};

test('the tool opens on the theme fixture, shows its hello, and an agent\'s write arrives attributed', async () => {
  const app = mkdtempSync(join(tmpdir(), 'exact-design-tool-'));
  for (const f of ['app.contract', 'app.tamber', 'app.json']) cpSync(resolve(ROOT, 'apps/theme-fixture', f), join(app, f));
  const git = (...args) => spawnSync('git', ['-c', 'user.email=t@example.test', '-c', 'user.name=t', ...args], { cwd: app });
  git('init', '-q'); git('add', '.'); git('commit', '-q', '-m', 'design');
  const tool = spawn(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'tool', app, '--no-open'], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
  let said = '';
  tool.stderr.on('data', d => { said = (said + d).slice(-4000); });
  let drive;
  try {
    const url = await lines(tool).until(l => l.startsWith(`${TOOL_ORIGIN}/__tamber/tool/`), 600_000).catch(e => { throw new Error(`${e.message}\n${said}`); });
    drive = spawn(process.execPath, [resolve(ROOT, 'scripts/agent.mjs'), 'web', '--app', 'design-tool', '--url', url, '--storage', `design-tool-${process.pid}`, '--json',
      'type name Ada', 'tap save-name', 'clock data', 'clock +1', 'clock data', 'state'], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
    let driveSaid = '';
    drive.stderr.on('data', d => { driveSaid = (driveSaid + d).slice(-4000); });
    const out = lines(drive);
    // The first answer is in (the third op's reply, `clock data`): the next
    // poll will carry what comes after it.
    await out.until(() => out.seen.filter(l => l.startsWith('{')).length >= 3, 300_000).catch(e => { throw new Error(`${e.message}\n${driveSaid}`); });
    const digest = createHash('sha256').update(readFileSync(join(app, 'app.tamber'))).digest('hex');
    const write = spawnSync(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'op', TOOL_ORIGIN, JSON.stringify({ method: 'transaction', client: 'agent:codex', files: { 'app.tamber': digest }, ops: [{ op: 'set-token', name: 'surface.card', value: 'light-dark(#f1f3f5, #17191d)' }] })], { cwd: ROOT, encoding: 'utf8' });
    expect(write.status, write.stdout + write.stderr).toBe(0);
    await new Promise(ok => drive.once('exit', ok));
    expect(drive.exitCode, driveSaid).toBe(0);
    const replies = out.seen.filter(l => l.startsWith('{')).map(l => JSON.parse(l));
    const { hello, changes } = replies.findLast(r => r.resources).resources;
    expect(hello).toMatchObject({ ok: true, protocol: 'tamber/1.1', app });
    expect(hello.files.map(f => f.path)).toEqual(['app.tamber']);
    expect(changes.recent[0]).toMatchObject({ kind: 'changed', who: 'agent:codex', files: ['app.tamber'], tokens: ['surface.card'] });
  } finally {
    if (drive) await stop(drive);
    await stop(tool);
    rmSync(app, { recursive: true, force: true });
  }
}, 900_000);

test('--via reaches only a loopback dev loop serving this app, and forwards to it', async () => {
  const copy = () => {
    const dir = mkdtempSync(join(tmpdir(), 'exact-design-tool-via-'));
    for (const f of ['app.contract', 'app.tamber', 'app.json']) cpSync(resolve(ROOT, 'apps/theme-fixture', f), join(dir, f));
    return dir;
  };
  const [mine, other] = [copy(), copy()];
  const run = args => spawnSync(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'tool', ...args, '--no-open'], { cwd: ROOT, encoding: 'utf8' });
  // The design server for `mine` alone, as a dev loop would hold it.
  const serve = spawn(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'serve', mine], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
  let tool;
  try {
    const at = new URL(await lines(serve).until(l => /^http:\/\/127\.0\.0\.1:\d+\/__tamber$/.test(l), 600_000)).origin;
    for (const [via, said] of [['http://example.com', 'loopback URL'], [TOOL_ORIGIN, 'loopback URL'], ['http://localhost:9', 'loopback URL']]) {
      const r = run([mine, '--via', via]);
      expect(r.status, r.stderr).toBe(2);
      expect(r.stderr).toContain(said);
    }
    const wrong = run([other, '--via', at]);
    expect(wrong.status).toBe(1);
    expect(wrong.stderr).toContain(`the design of ${mine}`);
    tool = spawn(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'tool', mine, '--via', at, '--no-open'], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
    await lines(tool).until(l => l.startsWith(`${TOOL_ORIGIN}/__tamber/tool/`), 600_000);
    const { token } = await (await fetch(`${TOOL_ORIGIN}/__tamber/token`)).json();
    const hello = await (await fetch(`${TOOL_ORIGIN}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: '{"method":"hello"}' })).json();
    expect(hello.ok.app).toBe(mine);
    expect((await fetch(`${TOOL_ORIGIN}/__tamber/tool/`)).status).toBe(200);
  } finally {
    if (tool) await stop(tool);
    await stop(serve);
    rmSync(mine, { recursive: true, force: true });
    rmSync(other, { recursive: true, force: true });
  }
}, 900_000);
