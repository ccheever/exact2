// `exact contract build`'s app.ts check, bounded (the design tool's `build.mjs --check` spun 4.7 days, found
// 2026-10-26; theme2's conditions): a builder that outlasts the bound is stopped with its whole process group (a
// `tsc` under it too), as it is when the check is told to stop or its caller is killed outright; a `tsc` that never
// returns is stopped at the bound; a builder that finishes is passed through; the bound only shortens, and only under
// the test switch.
import { expect, test } from 'bun:test';
import { spawn, spawnSync } from 'node:child_process';
import { chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { checkBound, CHECK_BOUND_MS } from '../js/bake/src/typescript.mjs';

const ROOT = resolve(import.meta.dir, '..'), SUPERVISE = resolve(ROOT, 'scripts/check-supervise.mjs');
const timed = { ...process.env, EXACT_TEST_TIMING: '1', EXACT_CHECK_BOUND_MS: '1500' };
const alive = pid => { try { process.kill(pid, 0); return true; } catch { return false; } };
const scratch = () => mkdtempSync(join(tmpdir(), 'exact-check-watchdog-'));

/** A builder stand-in that spins, with a child of its own that sleeps (a `tsc` under it); both pids written to `pids`. */
function spinner(dir) {
  const path = join(dir, 'spin.mjs');
  writeFileSync(path, `import { spawn } from 'node:child_process';
import { writeFileSync } from 'node:fs';
const kid = spawn(process.execPath, ['-e', 'setInterval(() => {}, 1000)'], { stdio: 'ignore' });
writeFileSync(${JSON.stringify(join(dir, 'pids'))}, JSON.stringify([process.pid, kid.pid]));
for (;;) {}
`);
  return path;
}
/** A builder stand-in with a sleeping child that finishes as soon as `go` exists. */
function finisher(dir) {
  const path = join(dir, 'finish.mjs');
  writeFileSync(path, `import { spawn } from 'node:child_process';
import { existsSync, writeFileSync } from 'node:fs';
const kid = spawn(process.execPath, ['-e', 'setInterval(() => {}, 1000)'], { stdio: 'ignore' });
writeFileSync(${JSON.stringify(join(dir, 'pids'))}, JSON.stringify([process.pid, kid.pid]));
setInterval(() => { if (existsSync(${JSON.stringify(join(dir, 'go'))})) process.exit(0); }, 10);
`);
  return path;
}
const ps = (field, pid) => Number(spawnSync('ps', ['-o', `${field}=`, '-p', String(pid)], { encoding: 'utf8' }).stdout.trim());
async function pidsOf(dir) {
  for (let i = 0; i < 200; i++) {
    try { return JSON.parse(readFileSync(join(dir, 'pids'), 'utf8')); } catch { await Bun.sleep(50); }
  }
  throw new Error('the stand-in never started');
}
/** The pids still running after up to 5 s. */
async function left(pids) {
  for (let i = 0; i < 100 && pids.some(alive); i++) await Bun.sleep(50);
  return pids.filter(alive);
}

test('the bound only shortens, and only under the test switch', () => {
  expect(checkBound({})).toBe(CHECK_BOUND_MS);
  expect(CHECK_BOUND_MS).toBe(600_000);
  expect(checkBound({ EXACT_CHECK_BOUND_MS: '1500' })).toBe(CHECK_BOUND_MS);
  expect(checkBound({ EXACT_TEST_TIMING: 'true', EXACT_CHECK_BOUND_MS: '1500' })).toBe(CHECK_BOUND_MS);
  expect(checkBound({ EXACT_TEST_TIMING: '1', EXACT_CHECK_BOUND_MS: '1500' })).toBe(1500);
  for (const bad of ['700000', '600000', '49', '2s', '', '-1', '1.5'])
    expect(checkBound({ EXACT_TEST_TIMING: '1', EXACT_CHECK_BOUND_MS: bad })).toBe(CHECK_BOUND_MS);
});

test('a builder that spins past the bound is stopped, its group (a tsc under it) with it, and the check fails saying so', async () => {
  const dir = scratch(), began = Date.now();
  const r = spawnSync(process.execPath, [SUPERVISE, String(process.pid), spinner(dir), 'app', '--check'], { encoding: 'utf8', env: timed, timeout: 60_000 });
  expect(r.status).toBe(1);
  expect(Date.now() - began).toBeLessThan(15_000);
  expect(r.stderr).toContain("error: app: app.ts's type check did not finish in 1.5 s, so it was stopped");
  expect(await left(await pidsOf(dir))).toEqual([]);
}, 60_000);

test('the builder leads its own process group, so it cannot leave it (a group leader cannot setsid)', async () => {
  const dir = scratch();
  const supervisor = spawn(process.execPath, [SUPERVISE, String(process.pid), spinner(dir), 'app', '--check'], { stdio: 'ignore' });
  const [builder, kid] = await pidsOf(dir);
  expect(ps('pgid', builder)).toBe(builder);
  expect(ps('pgid', kid)).toBe(builder);
  supervisor.kill('SIGTERM');
  expect(await left([supervisor.pid, builder, kid])).toEqual([]);
}, 60_000);

test('the leader killed alone (SIGKILL): the supervisor fails the check and kills the builder group by the pid it was sent', async () => {
  const dir = scratch();
  const supervisor = spawn(process.execPath, [SUPERVISE, String(process.pid), spinner(dir), 'app', '--check'], { stdio: 'ignore' });
  const exited = new Promise(done => supervisor.on('exit', code => done(code)));
  const pids = await pidsOf(dir), leader = ps('ppid', pids[0]);
  await Bun.sleep(300); // (the builder's pid read by the supervisor)
  process.kill(leader, 'SIGKILL');
  expect(await exited).toBe(1);
  expect(await left(pids)).toEqual([]);
}, 60_000);

test('a stopped leader (SIGSTOP) does not hold the check past its bound: the caller gets its answer, nothing left', async () => {
  const dir = scratch();
  const supervisor = spawn(process.execPath, [SUPERVISE, String(process.pid), spinner(dir), 'app', '--check'], { stdio: ['ignore', 'pipe', 'pipe'], env: timed });
  const answered = new Promise(done => supervisor.on('close', code => done(code)));
  const pids = await pidsOf(dir), leader = ps('ppid', pids[0]);
  await Bun.sleep(300); // (the builder's pid read by the supervisor)
  process.kill(leader, 'SIGSTOP');
  expect(await answered).toBe(1);
  expect(await left([leader, ...pids])).toEqual([]);
}, 60_000);

test('the supervisor killed outright just as the builder finishes: what the builder left running goes too', async () => {
  const dir = scratch();
  const supervisor = spawn(process.execPath, [SUPERVISE, String(process.pid), finisher(dir), 'app', '--check'], { stdio: 'ignore' });
  const pids = await pidsOf(dir);
  supervisor.kill('SIGKILL');
  writeFileSync(join(dir, 'go'), '');
  expect(await left([supervisor.pid, ...pids])).toEqual([]);
}, 60_000);

test('the check told to stop mid-way (SIGTERM, SIGINT, SIGHUP), or killed outright (SIGKILL), takes its group with it', async () => {
  for (const signal of ['SIGTERM', 'SIGINT', 'SIGHUP', 'SIGKILL']) {
    const dir = scratch();
    const supervisor = spawn(process.execPath, [SUPERVISE, String(process.pid), spinner(dir), 'app', '--check'], { stdio: 'ignore' });
    const pids = await pidsOf(dir);
    supervisor.kill(signal);
    expect(await left([supervisor.pid, ...pids]), signal).toEqual([]);
  }
}, 60_000);

test('its caller killed past any handler (SIGKILL) leaves nothing running: the supervisor sees its parent go', async () => {
  const dir = scratch(), caller = join(dir, 'caller.mjs');
  writeFileSync(caller, `import { spawnSync } from 'node:child_process';
spawnSync(process.execPath, [${JSON.stringify(SUPERVISE)}, String(process.pid), ${JSON.stringify(spinner(dir))}, 'app', '--check'], { stdio: 'ignore' });
`);
  const outer = spawn(process.execPath, [caller], { stdio: 'ignore' });
  const pids = await pidsOf(dir);
  outer.kill('SIGKILL');
  expect(await left(pids)).toEqual([]);
}, 60_000);

test("its caller's whole process group killed (SIGKILL: the supervisor with it) leaves nothing: the builder's group kills itself", async () => {
  const dir = scratch(), caller = join(dir, 'caller.mjs');
  writeFileSync(caller, `import { spawnSync } from 'node:child_process';
spawnSync(process.execPath, [${JSON.stringify(SUPERVISE)}, String(process.pid), ${JSON.stringify(spinner(dir))}, 'app', '--check'], { stdio: 'ignore' });
`);
  // (The caller leads a group of its own, which the supervisor joins, as a shell's job does.)
  const outer = spawn(process.execPath, [caller], { stdio: 'ignore', detached: true });
  const pids = await pidsOf(dir);
  process.kill(-outer.pid, 'SIGKILL');
  expect(await left(pids)).toEqual([]);
}, 60_000);

test('a supervisor whose caller is already gone (not the pid it was given) starts nothing', () => {
  const dir = scratch();
  const r = spawnSync(process.execPath, [SUPERVISE, '1', spinner(dir), 'app', '--check'], { encoding: 'utf8', timeout: 30_000 });
  expect(r.status).toBe(1);
  expect(existsSync(join(dir, 'pids'))).toBe(false);
});

test('a builder that finishes is passed through: its exit status and its output, whole', () => {
  const dir = scratch(), builder = join(dir, 'done.mjs'), big = 'x'.repeat(200_000);
  writeFileSync(builder, `process.stdout.write(${JSON.stringify(big)}); process.stderr.write('said\\n'); process.exit(3);`);
  const r = spawnSync(process.execPath, [SUPERVISE, String(process.pid), builder, 'app', '--check'], { encoding: 'utf8', maxBuffer: 1 << 26 });
  expect(r.status).toBe(3);
  expect(r.stdout).toBe(big);
  expect(r.stderr).toBe('said\n');
});

test('js/bake/src/typescript.mjs imports nothing beside it: the native bake copies it alone into its stage', async () => {
  const dir = scratch();
  copyFileSync(resolve(ROOT, 'js/bake/src/typescript.mjs'), join(dir, '__exact_config.mjs'));
  const { checkBound: staged } = await import(join(dir, '__exact_config.mjs'));
  expect(staged({})).toBe(CHECK_BOUND_MS);
});

test('a tsc that never returns is stopped at the bound (js/bake/src/typescript.mjs check)', async () => {
  const dir = scratch(), stage = join(dir, 'stage'), libraries = join(dir, 'lib'), tsc = join(dir, 'tsc');
  mkdirSync(stage); mkdirSync(libraries);
  writeFileSync(join(stage, '__exact_tsconfig.json'), '{}');
  writeFileSync(tsc, `#!/usr/bin/env bun\nrequire('node:fs').writeFileSync(${JSON.stringify(join(dir, 'tsc.pid'))}, String(process.pid));\nsetInterval(() => {}, 1000);\n`);
  chmodSync(tsc, 0o755);
  const was = { timing: process.env.EXACT_TEST_TIMING, bound: process.env.EXACT_CHECK_BOUND_MS };
  process.env.EXACT_TEST_TIMING = '1'; process.env.EXACT_CHECK_BOUND_MS = '1500';
  try {
    const { check } = await import('../js/bake/src/typescript.mjs');
    const began = Date.now();
    await expect(check(stage, tsc, libraries)).rejects.toThrow('the TypeScript check did not finish in 1.5 s (tsc stopped)');
    expect(Date.now() - began).toBeLessThan(15_000);
    // (And the stand-in itself is gone.)
    expect(await left([Number(readFileSync(join(dir, 'tsc.pid'), 'utf8'))])).toEqual([]);
  } finally {
    for (const [k, v] of [['EXACT_TEST_TIMING', was.timing], ['EXACT_CHECK_BOUND_MS', was.bound]]) if (v === undefined) delete process.env[k]; else process.env[k] = v;
  }
  expect(existsSync(stage)).toBe(true);
}, 60_000);
