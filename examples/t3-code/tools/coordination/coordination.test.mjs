import { afterEach, expect, test } from 'bun:test';
import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, symlinkSync, watch, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Coordination, GiB, command, processIdentity } from './state.mjs';

const fixtures = [];
afterEach(() => {
  for (const f of fixtures.splice(0)) {
    const stateFile = join(f.root, 'state.json');
    if (existsSync(stateFile)) {
      const state = JSON.parse(readFileSync(stateFile, 'utf8'));
      for (const task of Object.values(state.tasks)) for (const receipt of task.processes) {
        if (receipt.role !== 'guard' && receipt.started && processIdentity(receipt.pid)?.started === receipt.started) process.kill(receipt.pid, 'SIGTERM');
      }
    }
    for (const p of f.children) if (p.exitCode === null) p.kill();
    rmSync(f.dir, { recursive: true, force: true });
  }
});
function fixture() {
  const dir = realpathSync(mkdtempSync(join(tmpdir(), 't3-coordination-')));
  const main = join(dir, 'repo'), a = join(dir, 'a'), b = join(dir, 'b'), root = join(dir, 'shared');
  mkdirSync(main);
  const git = (...args) => command(['git', ...args], main);
  git('init', '-b', 'main'); git('config', 'user.email', 'fixture@example.invalid'); git('config', 'user.name', 'Fixture');
  writeFileSync(join(main, '.gitignore'), 'target/\n');
  writeFileSync(join(main, 'source'), 'source\n');
  git('add', '.'); git('commit', '-m', 'base');
  git('worktree', 'add', '-b', 'task-a', a); git('worktree', 'add', '-b', 'task-b', b);
  const f = { dir, main, a, b, root, git, children: [] };
  f.c = path => new Coordination(path, { root, freeBytes: () => 100 * GiB });
  f.cli = (path, args, free = 100 * GiB) => new Promise((resolve, reject) => {
    const p = spawn(process.execPath, [join(import.meta.dir, 'fixture.mjs'), ...args], {
      cwd: path, env: { ...process.env, COORDINATION_FIXTURE_ROOT: root, COORDINATION_FIXTURE_FREE: String(free) }, stdio: ['ignore', 'pipe', 'pipe'],
    });
    f.children.push(p);
    let out = '', err = '';
    p.stdout.on('data', d => out += d); p.stderr.on('data', d => err += d);
    p.once('error', reject); p.once('close', code => {
      const last = out.trim().split('\n').at(-1);
      let value; try { value = JSON.parse(last); } catch {}
      resolve({ code, out, err, value });
    });
  });
  f.start = async () => {
    await f.c(main).init('coordinator');
    const [ta, tb] = await Promise.all([f.cli(a, ['register', '--issue', 'A', '--thread', 'thread-a']), f.cli(b, ['register', '--issue', 'B', '--thread', 'thread-b'])]);
    expect(ta.code, ta.err).toBe(0); expect(tb.code, tb.err).toBe(0);
    f.ta = ta.value; f.tb = tb.value;
  };
  fixtures.push(f); return f;
}
function waitFile(file) {
  if (existsSync(file)) return Promise.resolve();
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => { watcher.close(); reject(new Error(`timeout: ${file}`)); }, 10000);
    const watcher = watch(join(file, '..'), () => { if (existsSync(file)) { clearTimeout(timer); watcher.close(); resolve(); } });
    if (existsSync(file)) { clearTimeout(timer); watcher.close(); resolve(); }
  });
}
const proof = task => ({ thread: { threadId: task.thread, worktreePath: task.worktree, status: 'completed', activeRunId: null, pendingRequestCount: 0, updatedAt: 'fixture' } });
const audit = (task, operation, processes = [], auditorThreadId = task.thread) => ({ taskId: task.id, worktree: task.worktree, auditorThreadId, operation, processes, noUnrecordedDescendants: true });
const own = task => ['--task', task.id, '--thread', task.thread];

test('independent worktrees share registration, device collisions, release/reuse and surplus retirement', async () => {
  const f = fixture(); await f.start();
  expect((await f.cli(f.a, ['register', '--issue', 'A', '--thread', 'thread-a'])).value.id).toBe(f.ta.id);
  expect((await f.cli(f.b, ['register', '--issue', 'A', '--thread', 'intruder'])).err).toContain('already owned');
  const inventory = [1, 2, 3].map(n => ({ host: 'fixture-host', udid: `phone-${n}`, state: 'Shutdown' }));
  inventory.push({ host: 'fixture-host', udid: 'nates-phone', state: 'Booted' });
  const inv = join(f.dir, 'inventory.json'); writeFileSync(inv, JSON.stringify(inventory));
  const enroll = async n => f.c(f.main).enroll('coordinator', { ...inventory[n - 1], class: 'phone', runtime: 'iOS-fixture' }, inventory, true);
  await expect(f.c(f.main).enroll('coordinator', { ...inventory[3], class: 'phone', runtime: 'iOS-fixture' }, inventory)).rejects.toThrow('shutdown');
  await enroll(1);
  const claim = t => f.cli(t.worktree, ['device-claim', ...own(t), '--host', 'fixture-host', '--class', 'phone', '--runtime', 'iOS-fixture']);
  const collision = await Promise.all([claim(f.ta), claim(f.tb)]);
  expect(collision.filter(r => r.code === 0)).toHaveLength(1);
  const winner = collision[0].code === 0 ? f.ta : f.tb, loser = winner === f.ta ? f.tb : f.ta;
  await enroll(2); const second = await claim(loser); expect(second.value.udid).toBe('phone-2');
  await expect(f.c(loser.worktree).releaseDevice(loser.id, loser.thread, 'fixture-host', 'phone-1', inventory)).rejects.toThrow('another task');
  await f.c(winner.worktree).releaseDevice(winner.id, winner.thread, 'fixture-host', 'phone-1', inventory);
  const reuse = await claim(winner); expect(reuse.value.udid).toBe('phone-1');
  await f.c(winner.worktree).releaseDevice(winner.id, winner.thread, 'fixture-host', 'phone-1', inventory);
  await f.c(loser.worktree).releaseDevice(loser.id, loser.thread, 'fixture-host', 'phone-2', inventory);
  await enroll(3);
  const devices = Object.values((await f.c(f.main).status()).devices);
  expect(devices.filter(d => d.status === 'idle' && d.class === 'phone')).toHaveLength(1);
  expect(devices.filter(d => d.status === 'retiring')).toHaveLength(2);
  await expect(f.c(f.main).retireDevice('coordinator', 'fixture-host', 'phone-2', inventory)).rejects.toThrow('delete the surplus');
  await f.c(f.main).retireDevice('coordinator', 'fixture-host', 'phone-2', inventory.filter(d => d.udid !== 'phone-2'));
  expect((await f.c(f.main).status()).devices['fixture-host:nates-phone']).toBeUndefined();
});

test('native slot spans repositories and low disk refuses before allocating output', async () => {
  const f = fixture(); await f.start();
  const other = join(f.dir, 'other'); mkdirSync(other); command(['git', 'init', '-b', 'main'], other);
  command(['git', 'config', 'user.email', 'fixture@example.invalid'], other); command(['git', 'config', 'user.name', 'Fixture'], other);
  writeFileSync(join(other, '.gitignore'), 'target/\n'); command(['git', 'add', '.'], other); command(['git', 'commit', '-m', 'base'], other);
  await f.c(other).init('coordinator'); const t = await f.c(other).register('OTHER', 'other-thread');
  const ready = join(f.dir, 'build-ready'), release = join(f.dir, 'build-release');
  const script = `import {writeFileSync,watch,existsSync} from 'node:fs';writeFileSync(${JSON.stringify(ready)},'ready');await new Promise(r=>{const w=watch(${JSON.stringify(f.dir)},()=>{if(existsSync(${JSON.stringify(release)})){w.close();r()}})});`;
  const first = f.cli(f.a, ['build', ...own(f.ta), '--output', 'target/owned', '--budget-gib', '10', '--', process.execPath, '-e', script]);
  await waitFile(ready);
  const second = await f.cli(other, ['build', ...own(t), '--output', 'target/owned', '--budget-gib', '10', '--', process.execPath, '-e', '']);
  expect(second.err).toContain('native build owned'); expect(existsSync(join(other, 'target'))).toBe(false);
  writeFileSync(release, 'go'); const completed = await first; expect(completed.code).toBe(0);
  expect((await f.c(f.main).status()).leases['native-build']).toBeDefined();
  expect((await f.c(f.a).auditRun(f.ta.id, f.ta.thread, audit(f.ta, completed.value.operation))).held).toBe(false);
  const low = await f.cli(other, ['build', ...own(t), '--output', 'target/owned', '--budget-gib', '10', '--', process.execPath, '-e', ''], 59 * GiB);
  expect(low.err).toContain('50 GiB reserve'); expect(existsSync(join(other, 'target'))).toBe(false);
  expect((await f.cli(other, ['build', ...own(t), '--output', 'target/owned', '--budget-gib', '10', '--', process.execPath, '-e', ''])).code).toBe(0);
});

test('ordinary pushes serialize against a bare remote and retain both task commits', async () => {
  const f = fixture(); await f.start();
  const remote = join(f.dir, 'remote.git'); command(['git', 'init', '--bare', remote]);
  f.git('remote', 'add', 'origin', remote); f.git('push', 'origin', 'HEAD:refs/heads/feat(example)/t3-code-ios');
  for (const [path, name] of [[f.a, 'A'], [f.b, 'B']]) {
    writeFileSync(join(path, name), name); command(['git', 'add', name], path); command(['git', 'commit', '-m', name], path);
  }
  await f.c(f.main).grant(f.ta.id, 'coordinator');
  const ready = join(f.dir, 'publish-ready'), release = join(f.dir, 'publish-release');
  const script = `import {writeFileSync,watch,existsSync} from 'node:fs';writeFileSync(${JSON.stringify(ready)},'ready');await new Promise(r=>{const w=watch(${JSON.stringify(f.dir)},()=>{if(existsSync(${JSON.stringify(release)})){w.close();r()}})});`;
  const first = f.cli(f.a, ['publish', ...own(f.ta), '--', process.execPath, '-e', script]);
  await waitFile(ready);
  await expect(f.c(f.main).grant(f.tb.id, 'coordinator')).rejects.toThrow('publishing turn owned');
  const second = await f.cli(f.b, ['publish', ...own(f.tb), '--', process.execPath, '-e', '']);
  expect(second.err).toContain('publishing turn');
  writeFileSync(release, 'go'); const result = await first; expect(result.code, result.err).toBe(0);
  await f.c(f.main).grant(f.tb.id, 'coordinator');
  const last = await f.cli(f.b, ['publish', ...own(f.tb), '--', process.execPath, '-e', '']); expect(last.code, last.err).toBe(0);
  const files = command(['git', '--git-dir', remote, 'ls-tree', '--name-only', 'refs/heads/feat(example)/t3-code-ios']);
  expect(files.split('\n')).toContain('A'); expect(files.split('\n')).toContain('B');
  expect(command(['git', '--git-dir', remote, 'log', '--format=%s', 'refs/heads/feat(example)/t3-code-ios']).split('\n')).toEqual(['B', 'A', 'base']);
  expect((await f.c(f.main).status()).leases).toEqual({});
});

test('cleanup preserves source and evidence, contains output, and retires only a terminal clean worktree', async () => {
  const f = fixture(); await f.start(); const c = f.c(f.a);
  const output = await c.recordOutput(f.ta.id, f.ta.thread, 'target/owned', 5);
  writeFileSync(join(output.path, 'cache'), 'disposable');
  const evidence = join(f.dir, 'evidence.log'); writeFileSync(evidence, 'test evidence');
  await expect(c.recordOutput(f.ta.id, f.ta.thread, 'source', 1)).rejects.toThrow('outside disposable');
  await expect(c.recordOutput(f.ta.id, f.ta.thread, '../b/target', 1)).rejects.toThrow('outside disposable');
  mkdirSync(join(f.a, 'target/unrelated')); writeFileSync(join(f.a, 'target/unrelated/keep'), 'unrelated');
  await c.preserve(f.ta.id, f.ta.thread, [evidence]);
  writeFileSync(join(f.a, 'source'), 'uncommitted');
  await expect(c.cleanup(f.ta.id, f.ta.thread)).rejects.toThrow('uncommitted');
  command(['git', 'restore', 'source'], f.a);
  await expect(f.c(f.main).retire(f.ta.id, 'coordinator', proof(f.ta))).rejects.toThrow('recover terminal');
  const fake = { ...proof(f.ta), thread: { ...proof(f.ta).thread, status: 'running' } };
  await expect(f.c(f.main).recover(f.ta.id, 'coordinator', fake)).rejects.toThrow('active or unknown');
  const cleanup = await c.cleanup(f.ta.id, f.ta.thread);
  expect(cleanup.receipts[0].kibRemoved).toBeGreaterThan(0);
  expect(existsSync(join(f.a, 'source'))).toBe(true); expect(existsSync(join(f.a, 'target/unrelated/keep'))).toBe(true);
  expect(readFileSync(cleanup.retained[0], 'utf8')).toBe('test evidence');
  await f.c(f.main).recover(f.ta.id, 'coordinator', proof(f.ta));
  await expect(f.c(f.main).retire(f.ta.id, 'coordinator', proof(f.ta))).rejects.toThrow('ignored files remain');
  rmSync(join(f.a, 'target/unrelated'), { recursive: true });
  const retired = await f.c(f.main).retire(f.ta.id, 'coordinator', proof(f.ta));
  expect(existsSync(f.a)).toBe(false); expect(existsSync(retired.evidence[0])).toBe(true);
  expect(f.git('show', `${retired.checkpoint.ref}:source`)).toBe('source');
});

test('symlinks, unknown process owners and replaced output identities refuse cleanup', async () => {
  const f = fixture(); await f.start(); const c = f.c(f.a);
  const outside = join(f.dir, 'outside'); mkdirSync(outside); writeFileSync(join(outside, 'keep'), 'keep');
  symlinkSync(outside, join(f.a, 'target'));
  await expect(c.recordOutput(f.ta.id, f.ta.thread, 'target/escape', 1)).rejects.toThrow('symlink');
  rmSync(join(f.a, 'target'));
  await c.recordOutput(f.ta.id, f.ta.thread, 'target/owned', 1);
  const evidence = join(f.dir, 'evidence.log'); writeFileSync(evidence, 'evidence'); await c.preserve(f.ta.id, f.ta.thread, [evidence]);
  await c.transaction(s => { s.tasks[f.ta.id].processes.push({ pid: 99, host: 'unknown-host', started: 'old' }); });
  await expect(c.cleanup(f.ta.id, f.ta.thread)).rejects.toThrow('unknown processes');
  await expect(f.c(f.main).recover(f.ta.id, 'coordinator', proof(f.ta))).rejects.toThrow('unknown processes');
  await c.transaction(s => { s.tasks[f.ta.id].processes = []; });
  rmSync(join(f.a, 'target/owned'), { recursive: true }); symlinkSync(outside, join(f.a, 'target/owned'));
  await expect(c.cleanup(f.ta.id, f.ta.thread)).rejects.toThrow('symlink');
  expect(readFileSync(join(outside, 'keep'), 'utf8')).toBe('keep');
});

test('only recorded processes can stop; live owners and unknown mutex owners cannot recover', async () => {
  const f = fixture(); await f.start();
  const ready = join(f.dir, 'process-ready');
  const running = f.cli(f.a, ['run', ...own(f.ta), '--', process.execPath, '-e', `require('fs').writeFileSync(${JSON.stringify(ready)},'ready');setInterval(()=>{},1000)`]);
  await waitFile(ready);
  const c = f.c(f.a); const state = await c.status();
  const receipt = state.tasks[f.ta.id].processes.find(p => p.role === 'child'); expect(receipt.started).toBeTruthy();
  await expect(c.stop(f.ta.id, f.ta.thread, process.pid)).rejects.toThrow('not launched');
  await expect(f.c(f.main).recover(f.ta.id, 'coordinator', proof(f.ta))).rejects.toThrow('live or unknown');
  await c.stop(f.ta.id, f.ta.thread, receipt.pid); await running;
  expect(processIdentity(process.pid)).toBeTruthy();
  await f.c(f.main).recover(f.ta.id, 'coordinator', proof(f.ta));
  const mutex = join(f.root, 'mutex'); mkdirSync(mutex);
  writeFileSync(join(mutex, 'owner.json'), JSON.stringify(processIdentity(process.pid)));
  expect(() => c.recoverMutex()).toThrow('live or unknown');
  writeFileSync(join(mutex, 'owner.json'), JSON.stringify({ pid: 99999999, host: 'unknown', started: 'old' }));
  expect(() => c.recoverMutex()).toThrow('live or unknown');
  writeFileSync(join(mutex, 'owner.json'), JSON.stringify({ ...processIdentity(process.pid), pid: 99999999 }));
  expect(c.recoverMutex().recovered.pid).toBe(99999999);
});

test('abandoned native guard retains its slot until terminal proof and recorded child exit', async () => {
  const f = fixture(); await f.start();
  const ready = join(f.dir, 'orphan-ready');
  const running = f.cli(f.a, ['build', ...own(f.ta), '--output', 'target/owned', '--budget-gib', '1', '--', process.execPath, '-e', `require('fs').writeFileSync(${JSON.stringify(ready)},'ready');setInterval(()=>{},1000)`]);
  const guard = f.children.at(-1); await waitFile(ready);
  const state = await f.c(f.a).status();
  const child = state.tasks[f.ta.id].processes.find(p => p.role === 'child'); expect(child).toBeDefined();
  guard.kill('SIGKILL');
  await expect(f.c(f.main).recover(f.ta.id, 'coordinator', proof(f.ta))).rejects.toThrow('live or unknown');
  expect((await f.cli(f.b, ['build', ...own(f.tb), '--output', 'target/owned', '--budget-gib', '1', '--', process.execPath, '-e', ''])).err).toContain('native build owned');
  await f.c(f.a).stop(f.ta.id, f.ta.thread, child.pid); await running;
  await f.c(f.main).recover(f.ta.id, 'coordinator', proof(f.ta));
  const lease = (await f.c(f.main).status()).leases['native-build'];
  expect(lease).toBeDefined();
  expect((await f.c(f.main).auditRun(f.ta.id, 'coordinator', audit(f.ta, lease.operation, [], 'coordinator'))).held).toBe(false);
  expect((await f.c(f.main).status()).leases).toEqual({});
});

test('port and storage receipts collide across processes and release for reuse', async () => {
  const f = fixture(); await f.start();
  const claim = task => f.cli(task.worktree, ['resource-claim', ...own(task), '--kind', 'port', '--value', 'fixture-host:49161']);
  const results = await Promise.all([claim(f.ta), claim(f.tb)]);
  expect(results.filter(r => r.code === 0)).toHaveLength(1);
  const winner = results[0].code === 0 ? f.ta : f.tb, loser = winner === f.ta ? f.tb : f.ta;
  await expect(f.c(loser.worktree).resource(loser.id, loser.thread, 'port', 'fixture-host:49161', true)).rejects.toThrow('another task');
  await f.c(winner.worktree).resource(winner.id, winner.thread, 'port', 'fixture-host:49161', true);
  expect((await claim(loser)).code).toBe(0);
  await f.c(winner.worktree).resource(winner.id, winner.thread, 'storage', `fixture-host:t3-${winner.id}`);
});

test('daemonized escape retains native ownership after successful launcher exit until explicit audit', async () => {
  const f = fixture(); await f.start(); const c = f.c(f.a);
  const pidFile = join(f.dir, 'detached-pid');
  const launcher = `import {spawn} from 'node:child_process';import {writeFileSync} from 'node:fs';const c=spawn(process.execPath,['-e','setInterval(()=>{},1000)'],{detached:true,stdio:'ignore'});writeFileSync(${JSON.stringify(pidFile)},String(c.pid));c.unref();`;
  const result = await f.cli(f.a, ['build', ...own(f.ta), '--output', 'target/owned', '--budget-gib', '1', '--', process.execPath, '-e', launcher]);
  expect(result.code, result.err).toBe(0);
  const descendant = processIdentity(Number(readFileSync(pidFile, 'utf8'))); expect(descendant.started).toBeTruthy();
  try {
    const state = await c.status(); expect(state.leases['native-build']).toBeDefined();
    expect(state.tasks[f.ta.id].processes).not.toHaveLength(0);
    const incomplete = audit(f.ta, result.value.operation); incomplete.noUnrecordedDescendants = false;
    expect((await c.auditRun(f.ta.id, f.ta.thread, incomplete)).held).toBe(true);
    expect((await c.auditRun(f.ta.id, f.ta.thread, audit(f.ta, result.value.operation, [descendant]))).held).toBe(true);
    await c.stop(f.ta.id, f.ta.thread, descendant.pid);
    const deadline = Date.now() + 5000;
    while (processIdentity(descendant.pid) && Date.now() < deadline) await new Promise(r => setTimeout(r, 50));
    expect(processIdentity(descendant.pid)).toBeNull();
    expect((await c.auditRun(f.ta.id, f.ta.thread, audit(f.ta, result.value.operation, [descendant]))).held).toBe(false);
    expect((await c.status()).leases['native-build']).toBeUndefined();
  } finally {
    if (processIdentity(descendant.pid)?.started === descendant.started) process.kill(descendant.pid, 'SIGTERM');
  }
});

test('surviving foreground-group descendants are automatically recorded and block release', async () => {
  const f = fixture(); await f.start(); const c = f.c(f.a);
  const pidFile = join(f.dir, 'group-pid');
  const launcher = `import {spawn} from 'node:child_process';import {writeFileSync} from 'node:fs';const c=spawn(process.execPath,['-e','setInterval(()=>{},1000)'],{stdio:'ignore'});writeFileSync(${JSON.stringify(pidFile)},String(c.pid));c.unref();`;
  const result = await f.cli(f.a, ['build', ...own(f.ta), '--output', 'target/owned', '--budget-gib', '1', '--', process.execPath, '-e', launcher]);
  expect(result.code, result.err).toBe(0);
  const descendant = Number(readFileSync(pidFile, 'utf8'));
  const state = await c.status(); expect(state.tasks[f.ta.id].processes.some(p => p.pid === descendant && p.role === 'descendant')).toBe(true);
  expect((await c.auditRun(f.ta.id, f.ta.thread, audit(f.ta, result.value.operation))).held).toBe(true);
  await c.stop(f.ta.id, f.ta.thread, descendant);
  const deadline = Date.now() + 5000;
  while (processIdentity(descendant) && Date.now() < deadline) await new Promise(r => setTimeout(r, 50));
  expect((await c.auditRun(f.ta.id, f.ta.thread, audit(f.ta, result.value.operation))).held).toBe(false);
});
