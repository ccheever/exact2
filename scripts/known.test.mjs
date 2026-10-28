// What fails on main, said beside a local run (LLP 1146 D1): the lane's note and its reading.
import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { classify, mainNotes, noteOf, publish, report } from './known.mjs';

const result = (sha, failures, checks = { ios: { command: 'bun host/apple/build.mjs --test --ios', status: 1 } }) => ({ sha, checks, failures });

test('a note says which checks ran to the end', () => {
  const note = noteOf(result('a', ['ios: ExactKitTests.A b failed', 'test: exact-x (lib) does not compile'], {
    ios: { command: 'bun host/apple/build.mjs --test --ios', status: 1 },
    test: { command: 'cargo test --workspace', status: 101 },
    difftest: { command: 'cargo run -p contract-difftest', skipped: "inputs of abc's pass" },
  }), 1);
  expect(note.checks.ios).toEqual({ command: 'bun host/apple/build.mjs --test --ios', status: 1, complete: true });
  expect(note.checks.test.complete).toBe(false);
  expect(note.checks.difftest.skipped).toBeDefined();
});

test('a failure is known, new or unknown against the nearest note that ran its kind of check', () => {
  const history = [
    { sha: 'c3', notes: [noteOf(result('c3', [], { lint: { command: 'cargo clippy', status: 0 } }), 1)] },
    { sha: 'c2', notes: [noteOf(result('c2', ['ios: ExactKitTests.Iframe pinch failed']), 1)] },
    { sha: 'c1', notes: [noteOf(result('c1', ['ios: ExactKitTests.Iframe pinch failed', 'ios: ExactKitTests.Nav pop failed']), 1)] },
  ];
  const sorted = classify(['local: ExactKitTests.Iframe pinch failed', 'local: ExactKitTests.Nav pop failed', 'local: ExactKitTests.Mine failed'], 'xctest', history);
  expect(sorted.at.sha).toBe('c2');
  expect(sorted.rows.map((r) => [r.verdict, r.rate])).toEqual([['known', '2 of its last 2 lane runs'], ['new', '1 of the last 2 lane runs'], ['new', undefined]]);
  expect(report(sorted)[0]).toMatch(/^3 failed: 1 known on origin\/main at c2 \(lane, .*\), 2 new$/);
  expect(classify(['local: x'], 'cargo', history).rows[0].verdict).toBe('unknown');
  expect(report(classify(['local: x'], 'cargo', history))[0]).toContain('unknown on origin/main');
});

test('the lane publishes a note to origin and another checkout reads it', () => {
  const dir = mkdtempSync(join(tmpdir(), 'known-'));
  const run = (cwd, ...args) => { const r = spawnSync('git', args, { cwd, encoding: 'utf8' }); if (r.status !== 0) throw new Error(`git ${args.join(' ')}: ${r.stderr}`); return r.stdout.trim(); };
  try {
    const origin = join(dir, 'origin.git'), lane = join(dir, 'lane'), mine = join(dir, 'mine');
    run(dir, 'init', '-q', '--bare', '-b', 'main', origin);
    run(dir, 'clone', '-q', origin, lane);
    for (const k of ['user.email', 'user.name']) run(lane, 'config', k, 'lane');
    writeFileSync(join(lane, 'a'), '1'); run(lane, 'add', 'a'); run(lane, 'commit', '-qm', 'one');
    run(lane, 'push', '-q', 'origin', 'HEAD:main');
    const sha = run(lane, 'rev-parse', 'HEAD');
    expect(publish(noteOf(result(sha, ['ios: ExactKitTests.Iframe pinch failed']), 1), lane)).toBeNull();
    run(dir, 'clone', '-q', origin, mine);
    const history = mainNotes({ cwd: mine });
    expect(history.map((h) => h.sha)).toEqual([sha]);
    expect(classify(['local: ExactKitTests.Iframe pinch failed'], 'xctest', history).rows[0].verdict).toBe('known');
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
