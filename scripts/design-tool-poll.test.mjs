// The tool's changes on the server's GET long poll (tamber/1.17, LLP 1122 stage 14 D) through LLP 1016.000 D8's
// `exactLongPoll`, against a real design server: idle, its poll is held, so `clock settle` doesn't wait on it and
// each heartbeat (the server's 204 after its wait, 25 s) is a reply `clock held changes` returns on that commits
// nothing; an agent's write then shows within one reply. The server's wait is 5 s here (tamber-timing.mjs's test
// knob, every bound scaled from 25 s's), and the real 25 s in the slow lane (EXACT_SLOW_TESTS=1).
import { expect, onTestFinished, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { cpSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createHash } from 'node:crypto';
import { spawnOwned } from './test-children.mjs';
import { DRIVEN, HEARTBEAT_RACE, ROOT, SLOW, TOOL_ORIGIN, lines, signals, stop, timed, toolTurns } from './design-tool-harness.mjs';

toolTurns();

// The server's wait before a heartbeat, and a held clock's bound past it (30 s for 25 s).
const WAIT = SLOW ? 25_000 : 5_000, HELD = `clock held changes ${Math.ceil(WAIT * 1.2 / 1000)}s`;

test('an idle tool holds its long poll: clock settle returns at once, heartbeats over three waits commit nothing, and a write shows within one reply', async () => {
  const app = mkdtempSync(join(tmpdir(), 'exact-design-tool-poll-'));
  for (const f of ['app.contract', 'app.tamber', 'app.json']) cpSync(resolve(ROOT, 'apps/theme-fixture', f), join(app, f));
  const tool = spawnOwned(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'tool', app, '--no-open'], { cwd: ROOT, env: timed({ POLL: WAIT }), stdio: ['ignore', 'pipe', 'pipe'] });
  let said = '', drive;
  tool.stderr.on('data', d => { said = (said + d).slice(-4000); });
  try {
    const url = await lines(tool).until(l => l.startsWith(`${TOOL_ORIGIN}/__tamber/tool/`), 600_000).catch(e => { throw new Error(`${e.message}\n${said}`); });
    // The first read is a POST; its follow task (`clock +1`) opens the stream, whose first ask is answered at once.
    // (`clock held` waits up to 1.2 waits here: a heartbeat comes a wait after an ask.) The write lands while the drive
    // waits on its signal, so no heartbeat can come between: then one reply on, it is folded.
    const written = signals('poll');
    onTestFinished(written.drop);
    // (Named first: the tool reads nothing for no one, stage 15 D1.6. `N` ops before the poll's own.)
    const named = ['type name Ada', 'tap save-name'], N = named.length;
    const ops = [...named, 'clock data', 'clock +1', 'clock data', 'clock +1', 'clock data', 'state',
      'clock settle', HELD, HELD, HELD, HELD, 'state',
      written.pause(60_000, 1), HELD, 'clock data', 'state'];
    drive = spawnOwned(process.execPath, [resolve(ROOT, 'scripts/agent.mjs'), 'web', ...DRIVEN.args, '--url', url, '--storage', `design-tool-poll-${process.pid}`, '--json', ...ops], { cwd: ROOT, env: DRIVEN.env, stdio: ['ignore', 'pipe', 'pipe'] });
    let driveSaid = '';
    drive.stderr.on('data', d => { driveSaid = (driveSaid + d).slice(-4000); });
    const out = lines(drive), at = [];
    // When each reply came (the n-th JSON line's arrival).
    drive.stdout.on('data', () => { const n = out.seen.filter(l => l.startsWith('{')).length; while (at.length < n) at.push(Date.now()); });
    const replies = () => out.seen.filter(l => l.startsWith('{'));
    // The fourth heartbeat is in (the twelfth reply is the state after it): an agent writes, and the next reply carries it.
    await out.until(() => replies().length >= N + 12, 300_000).catch(e => { throw new Error(`${e.message}\n${driveSaid}`); });
    const digest = createHash('sha256').update(readFileSync(join(app, 'app.tamber'))).digest('hex');
    const write = spawnSync(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'op', TOOL_ORIGIN, JSON.stringify({ method: 'transaction', client: 'agent:codex', files: { 'app.tamber': digest }, ops: [{ op: 'set-token', name: 'surface.card', value: 'light-dark(#f1f3f5, #17191d)' }] })], { cwd: ROOT, encoding: 'utf8' });
    expect(write.status, write.stdout + write.stderr).toBe(0);
    written.fire(1);
    await new Promise(ok => drive.once('exit', ok));
    expect(drive.exitCode, driveSaid).toBe(0);
    const r = replies().map(l => JSON.parse(l));
    expect(r.length).toBe(ops.length);
    const opened = r[N + 5].resources.changes, idle = r[N + 11].resources.changes, after = r[N + 15].resources.changes;
    expect(r[N + 5].resources.hello).toMatchObject({ ok: true, protocol: 'tamber/1.18' });
    expect(opened.stream).toBe(true);
    // Settled with the poll held, not waited on: named, and back well inside the server's wait (10 s of 25 s).
    expect(r[N + 6]).toMatchObject({ settled: true, held: 1, heldTargets: ['changes'] });
    expect(at[N + 6] - at[N + 5]).toBeLessThan(WAIT * 0.4);
    // Each held clock came back on a heartbeat: the first on the wait in flight (any part of it left), each after
    // it a whole wait on, so over 2.4 waits idle from the first to the last (a minute of 25 s waits).
    for (const i of [7, 8, 9, 10].map(i => N + i)) expect(r[i].settled, JSON.stringify(r[i])).toBe(true);
    expect(at[N + 10] - at[N + 7]).toBeGreaterThanOrEqual(WAIT * 2.4);
    // ...and nothing came of them: no answer folded, nothing recent, the same place.
    expect([idle.answer, idle.seq, idle.epoch, idle.recent]).toEqual([opened.answer, opened.seq, opened.epoch, opened.recent]);
    expect(idle.stream).toBe(true);
    // The write: one reply on, folded from the stream, attributed.
    expect(after.answer, HEARTBEAT_RACE).toBe(idle.answer + 1);
    expect(after.stream).toBe(true);
    expect(after.recent[0]).toMatchObject({ kind: 'changed', who: 'agent:codex', files: ['app.tamber'], tokens: ['surface.card'] });
  } finally {
    if (drive) await stop(drive);
    await stop(tool);
    rmSync(app, { recursive: true, force: true });
  }
}, 900_000);
