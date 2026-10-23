// The tool's changes on the server's GET long poll (tamber/1.17, LLP 1122 stage 14 D) through LLP 1016.000 D8's
// `exactLongPoll`, against a real design server: idle, its poll is held, so `clock settle` doesn't wait on it and
// each heartbeat (the server's 204 after 25 s) is a reply `clock held changes` returns on that commits nothing; an
// agent's write then shows within one reply. A file of its own, so a lane runs its minute of waiting beside the others.
import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { cpSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createHash } from 'node:crypto';
import { spawnOwned } from './test-children.mjs';
import { DRIVEN, ROOT, TOOL_ORIGIN, lines, stop, toolTurns } from './design-tool-harness.mjs';

toolTurns();

test('an idle tool holds its long poll: clock settle returns at once, three heartbeats over a minute commit nothing, and a write shows within one reply', async () => {
  const app = mkdtempSync(join(tmpdir(), 'exact-design-tool-poll-'));
  for (const f of ['app.contract', 'app.tamber', 'app.json']) cpSync(resolve(ROOT, 'apps/theme-fixture', f), join(app, f));
  const tool = spawnOwned(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'tool', app, '--no-open'], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
  let said = '', drive;
  tool.stderr.on('data', d => { said = (said + d).slice(-4000); });
  try {
    const url = await lines(tool).until(l => l.startsWith(`${TOOL_ORIGIN}/__tamber/tool/`), 600_000).catch(e => { throw new Error(`${e.message}\n${said}`); });
    // The first read is a POST; its follow task (`clock +1`) opens the stream, whose first ask is answered at once.
    // (`clock held` waits up to 30 s here: a heartbeat comes 25 s after an ask.)
    const ops = ['clock data', 'clock +1', 'clock data', 'clock +1', 'clock data', 'state',
      'clock settle', 'clock held changes 30s', 'clock held changes 30s', 'clock held changes 30s', 'state',
      'clock held changes 30s', 'clock data', 'state'];
    drive = spawnOwned(process.execPath, [resolve(ROOT, 'scripts/agent.mjs'), 'web', ...DRIVEN.args, '--url', url, '--storage', `design-tool-poll-${process.pid}`, '--json', ...ops], { cwd: ROOT, env: DRIVEN.env, stdio: ['ignore', 'pipe', 'pipe'] });
    let driveSaid = '';
    drive.stderr.on('data', d => { driveSaid = (driveSaid + d).slice(-4000); });
    const out = lines(drive), at = [];
    // When each reply came (the n-th JSON line's arrival).
    drive.stdout.on('data', () => { const n = out.seen.filter(l => l.startsWith('{')).length; while (at.length < n) at.push(Date.now()); });
    const replies = () => out.seen.filter(l => l.startsWith('{'));
    // The third heartbeat is in (the tenth reply): an agent writes, and the next reply carries it.
    await out.until(() => replies().length >= 11, 300_000).catch(e => { throw new Error(`${e.message}\n${driveSaid}`); });
    const digest = createHash('sha256').update(readFileSync(join(app, 'app.tamber'))).digest('hex');
    const write = spawnSync(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'op', TOOL_ORIGIN, JSON.stringify({ method: 'transaction', client: 'agent:codex', files: { 'app.tamber': digest }, ops: [{ op: 'set-token', name: 'surface.card', value: 'light-dark(#f1f3f5, #17191d)' }] })], { cwd: ROOT, encoding: 'utf8' });
    expect(write.status, write.stdout + write.stderr).toBe(0);
    await new Promise(ok => drive.once('exit', ok));
    expect(drive.exitCode, driveSaid).toBe(0);
    const r = replies().map(l => JSON.parse(l));
    expect(r.length).toBe(ops.length);
    const opened = r[5].resources.changes, idle = r[10].resources.changes, after = r[13].resources.changes;
    expect(r[5].resources.hello).toMatchObject({ ok: true, protocol: 'tamber/1.17' });
    expect(opened.stream).toBe(true);
    // Settled with the poll held, not waited on: named, and back well inside the server's 25 s.
    expect(r[6]).toMatchObject({ settled: true, held: 1, heldTargets: ['changes'] });
    expect(at[6] - at[5]).toBeLessThan(10_000);
    // Each held clock came back on a heartbeat (no sooner than the server's wait), over a minute in all.
    for (const i of [7, 8, 9]) expect(r[i].settled, JSON.stringify(r[i])).toBe(true);
    expect(at[9] - at[6]).toBeGreaterThanOrEqual(60_000);
    // ...and nothing came of them: no answer folded, nothing recent, the same place.
    expect([idle.answer, idle.seq, idle.epoch, idle.recent]).toEqual([opened.answer, opened.seq, opened.epoch, opened.recent]);
    expect(idle.stream).toBe(true);
    // The write: one reply on, folded from the stream, attributed.
    expect(after.answer).toBe(idle.answer + 1);
    expect(after.stream).toBe(true);
    expect(after.recent[0]).toMatchObject({ kind: 'changed', who: 'agent:codex', files: ['app.tamber'], tokens: ['surface.card'] });
  } finally {
    if (drive) await stop(drive);
    await stop(tool);
    rmSync(app, { recursive: true, force: true });
  }
}, 900_000);
