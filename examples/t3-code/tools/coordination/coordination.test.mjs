import { afterEach, expect, test } from 'bun:test';
import { spawn } from 'node:child_process';
import { existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, symlinkSync, watch, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Coordination, GiB, command, processHostMatches, processIdentity, terminalProof } from './state.mjs';

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

test('boot identity survives a hostname change without trusting another boot or unbound legacy host', () => {
  const now = { host: 'after', bootId: 'boot-a' };
  expect(processHostMatches({ host: 'before', bootId: 'boot-a' }, now)).toBe(true);
  expect(processHostMatches({ host: 'after', bootId: 'boot-b' }, now)).toBe(false);
  expect(processHostMatches({ host: 'before' }, now)).toBe(false);
  expect(processHostMatches({ host: 'before' }, now, { before: { bootId: 'boot-a' } })).toBe(true);
  expect(processHostMatches({ host: 'before' }, now, { before: { bootId: 'boot-b' } })).toBe(false);
  expect(processHostMatches({ host: 'after' }, now)).toBe(true);
  expect(processHostMatches({ host: 'before', bootId: 'boot-b' }, now, { before: { bootId: 'boot-a' } })).toBe(false);
  expect(processHostMatches({ host: 'after' }, now, { after: { bootId: 'boot-b' } })).toBe(false);
  expect(processHostMatches({ host: 'after' }, now, { after: { bootId: 'boot-a' } })).toBe(true);
});

test('legacy hostname recovery requires a live owned launcher and never releases its process lease', async () => {
  const f = fixture(); await f.start();
  const pidFile = join(f.dir, 'rename-child.pid');
  const running = f.cli(f.a, ['run', ...own(f.ta), '--', process.execPath, '-e', `await Bun.write(${JSON.stringify(pidFile)}, String(process.pid)); await Bun.sleep(300000)`]);
  await waitFile(pidFile);
  const stateFile = join(f.root, 'state.json');
  const state = await f.c(f.main).status();
  const [key, lease] = Object.entries(state.leases).find(([, l]) => l.task === f.ta.id);
  const current = processIdentity(lease.guard.pid);
  expect(current.bootId).toBeTruthy();
  for (const p of [...state.tasks[f.ta.id].processes, lease.guard]) { p.host = 'before-rename'; delete p.bootId; }
  writeFileSync(stateFile, JSON.stringify(state));
  const witness = { fromHost: 'before-rename', toHost: current.host, bootId: current.bootId,
    anchor: { taskId: f.ta.id, operation: lease.operation, pid: lease.guard.pid, started: lease.guard.started } };
  await expect(f.c(f.main).bindProcessHost('thread-a', witness)).rejects.toThrow('coordinator');
  await expect(f.c(f.main).bindProcessHost('coordinator', { ...witness, bootId: 'another-boot' })).rejects.toThrow('current boot');
  await expect(f.c(f.main).bindProcessHost('coordinator', { ...witness, toHost: 'elsewhere' })).rejects.toThrow('current boot');
  await expect(f.c(f.main).bindProcessHost('coordinator', { ...witness, anchor: { ...witness.anchor, started: 'wrong' } })).rejects.toThrow('matching legacy');
  const changedStart = structuredClone(state); changedStart.leases[key].guard.started = 'reused PID';
  writeFileSync(stateFile, JSON.stringify(changedStart));
  await expect(f.c(f.main).bindProcessHost('coordinator', { ...witness, anchor: { ...witness.anchor, started: 'reused PID' } })).rejects.toThrow('dead, changed or unknown');
  const foreignWorktree = structuredClone(state); foreignWorktree.tasks[f.ta.id].worktree = f.b;
  writeFileSync(stateFile, JSON.stringify(foreignWorktree));
  await expect(f.c(f.main).bindProcessHost('coordinator', witness)).rejects.toThrow('worktree mismatch');
  const foreignThread = structuredClone(state); foreignThread.tasks[f.ta.id].thread = 'another-thread';
  writeFileSync(stateFile, JSON.stringify(foreignThread));
  await expect(f.c(f.main).bindProcessHost('coordinator', witness)).rejects.toThrow('ownership arguments');
  writeFileSync(stateFile, JSON.stringify(state));
  await expect(f.c(f.a).stop(f.ta.id, f.ta.thread, Number(readFileSync(pidFile, 'utf8')))).rejects.toThrow('unknown process');
  const bound = await f.c(f.main).bindProcessHost('coordinator', witness);
  expect(bound.action).toBe('process-host-bound');
  expect((await f.c(f.main).status()).leases[key]).toBeDefined();
  await expect(f.c(f.a).auditRun(f.ta.id, f.ta.thread, audit(f.ta, lease.operation))).rejects.toThrow('launcher still live');
  const saved = (await f.c(f.main).status()).tasks[f.ta.id].processes;
  await f.c(f.a).stop(f.ta.id, f.ta.thread, Number(readFileSync(pidFile, 'utf8')));
  const result = await running;
  await expect(f.c(f.main).bindProcessHost('coordinator', witness)).rejects.toThrow('dead, changed or unknown');
  const descendants = saved.filter(p => p.role !== 'guard');
  await expect(f.c(f.a).auditRun(f.ta.id, f.ta.thread, audit(f.ta, result.value.operation, [{ ...descendants[0], host: 'foreign' }]))).rejects.toThrow('unknown audited');
  expect((await f.c(f.a).auditRun(f.ta.id, f.ta.thread, audit(f.ta, result.value.operation, descendants))).held).toBe(false);
});

test('only empty read-only native investigations accept matching terminal native proof', async () => {
  const f = fixture(); await f.c(f.main).init('coordinator');
  const t = await f.c(f.a).register('NATIVE', '/root/investigation');
  const native = { worktreePath: t.worktree, nativeCollaboration: { agents: [{ agent_name: t.thread, agent_status: { completed: 'read-only report' } }] } };
  expect(() => terminalProof({ ...native, worktreePath: f.b }, t)).toThrow('active or unknown');
  expect(() => terminalProof({ ...native, nativeCollaboration: { agents: [{ agent_name: t.thread, agent_status: 'running' }] } }, t)).toThrow('active or unknown');
  expect(() => terminalProof(native, { ...t, thread: 't3-worker' })).toThrow('active or unknown');
  for (const key of ['outputs', 'processes', 'pending']) expect(() => terminalProof(native, { ...t, [key]: [{}] })).toThrow('active or unknown');
  const evidence = join(f.dir, 'report.log'); writeFileSync(evidence, 'read-only evidence');
  await f.c(f.a).preserve(t.id, t.thread, [evidence]);
  await f.c(f.a).resource(t.id, t.thread, 'storage', 'fixture:native');
  await expect(f.c(f.main).recover(t.id, 'coordinator', native)).rejects.toThrow('owns resources');
  expect((await f.c(f.main).status()).leases['storage:fixture:native']).toBeDefined();
  await f.c(f.a).resource(t.id, t.thread, 'storage', 'fixture:native', true);
  const inventory = [{ host: 'fixture', udid: 'native-phone', state: 'Shutdown' }];
  await f.c(f.main).enroll('coordinator', { host: 'fixture', udid: 'native-phone', class: 'phone', runtime: 'fixture' }, inventory, true);
  await f.c(f.a).claimDevice(t.id, t.thread, 'fixture', 'phone', 'fixture');
  await expect(f.c(f.main).recover(t.id, 'coordinator', native)).rejects.toThrow('owns resources');
  await f.c(f.a).releaseDevice(t.id, t.thread, 'fixture', 'native-phone', inventory);
  await f.c(f.main).recover(t.id, 'coordinator', native);
  await f.c(f.main).retire(t.id, 'coordinator', native);
  expect(existsSync(f.a)).toBe(false);
});

test('fresh output directories match trailing-slash ignore rules before creation', async () => {
  const f = fixture(); await f.start();
  writeFileSync(join(f.a, '.gitignore'), 'target/\nexamples/t3-code/mobile/assets/browser-mobile/\n');
  for (const path of ['target', 'examples/t3-code/mobile/assets/browser-mobile']) {
    expect(existsSync(join(f.a, path))).toBe(false);
    const output = await f.c(f.a).recordOutput(f.ta.id, f.ta.thread, path, 1);
    expect(output.path).toBe(join(f.a, path));
    expect(existsSync(output.path)).toBe(true);
    await expect(f.c(f.a).recordOutput(f.ta.id, f.ta.thread, path, 1)).rejects.toThrow('refuse adopting');
  }
  await expect(f.c(f.a).recordOutput(f.ta.id, f.ta.thread, 'examples/t3-code/mobile/dist', 1)).rejects.toThrow();
  expect(existsSync(join(f.a, 'examples/t3-code/mobile/dist'))).toBe(false);
});

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

test('coordinator enrolls a fresh simulator directly to its worker without changing the idle pool', async () => {
  const f = fixture(); await f.start();
  const old = { host: 'fixture-host', udid: 'pooled-phone', class: 'phone', runtime: 'iOS-fixture' };
  const fresh = { ...old, udid: 'fresh-phone' };
  const pooled = await f.c(f.main).enroll('coordinator', old, [{ ...old, state: 'Shutdown' }], true);
  const inventory = [{ ...old, state: 'Booted' }, { ...fresh, state: 'Shutdown' }];
  const inv = join(f.dir, 'inventory.json'); writeFileSync(inv, JSON.stringify(inventory));
  const assigned = await f.cli(f.main, ['device-enroll', '--thread', 'coordinator', '--task', f.ta.id,
    '--host', fresh.host, '--udid', fresh.udid, '--class', fresh.class, '--runtime', fresh.runtime,
    '--inventory', inv, '--origin', 'created']);
  expect(assigned.code, assigned.err).toBe(0);
  expect(assigned.value).toMatchObject({ ...fresh, status: 'owned', task: f.ta.id, thread: f.ta.thread, worktree: f.a, created: true });
  const state = await f.c(f.main).status();
  expect(state.devices['fixture-host:pooled-phone']).toEqual(pooled);
  expect(Object.values(state.devices).filter(d => d.task === f.ta.id)).toHaveLength(1);
  await expect(f.c(f.a).claimDevice(f.ta.id, f.ta.thread, old.host, old.class, old.runtime)).rejects.toThrow('already owns');
  await expect(f.c(f.b).releaseDevice(f.tb.id, f.tb.thread, fresh.host, fresh.udid, inventory)).rejects.toThrow('another task');
  await f.c(f.a).releaseDevice(f.ta.id, f.ta.thread, fresh.host, fresh.udid, inventory);
  const released = await f.c(f.main).status();
  expect(released.devices['fixture-host:pooled-phone']).toEqual(pooled);
  expect(released.devices['fixture-host:fresh-phone'].status).toBe('retiring');
  expect(released.devices['fixture-host:fresh-phone'].task).toBeUndefined();
});

test('direct enrollment refuses invalid owners and inventory without changing shared state', async () => {
  const f = fixture(); await f.start();
  await f.c(f.main).recover(f.tb.id, 'coordinator', proof(f.tb));
  const other = fixture();
  const foreign = new Coordination(other.a, { root: f.root });
  await foreign.init('other-coordinator');
  const foreignTask = await foreign.register('FOREIGN', 'foreign-worker');
  const c = f.c(f.main);
  const device = { host: 'fixture-host', udid: 'fresh-phone', class: 'phone', runtime: 'iOS-fixture' };
  const inventory = [{ ...device, state: 'Shutdown' }];
  const before = await c.status();
  const refused = async (thread, candidate, rows, created, id, message) => {
    await expect(c.enroll(thread, candidate, rows, created, id)).rejects.toThrow(message);
    expect(await c.status()).toEqual(before);
  };
  await refused(f.ta.thread, device, inventory, true, f.ta.id, 'coordinator');
  await refused('coordinator', device, inventory, true, 'unknown', 'unknown task');
  await refused('coordinator', device, inventory, true, foreignTask.id, 'different repository');
  await refused('coordinator', device, inventory, true, f.tb.id, 'inactive task');
  await refused('coordinator', device, inventory, false, f.ta.id, 'newly created');
  for (const rows of [null, {}, [], [{ ...inventory[0], host: 'another-host' }],
    [{ ...inventory[0], udid: 'another-phone' }], [{ ...inventory[0], class: 'ipad' }],
    [{ ...inventory[0], runtime: 'another-runtime' }], [{ host: device.host, udid: device.udid, state: 'Shutdown' }],
    [...inventory, ...inventory]]) {
    await refused('coordinator', device, rows, true, f.ta.id, 'fresh inventory');
  }
  for (const field of ['host', 'udid', 'runtime']) {
    for (const value of ['', ' ', 42]) {
      const invalid = { ...device, [field]: value };
      await refused('coordinator', invalid, [{ ...invalid, state: 'Shutdown' }], true, f.ta.id, 'fresh inventory');
    }
  }
  await refused('coordinator', { ...device, class: 'watch' }, [{ ...inventory[0], class: 'watch' }], true, f.ta.id, 'compatible class/runtime');
  await refused('coordinator', device, [{ ...inventory[0], state: 'Booted' }], true, f.ta.id, 'shutdown');
  const inv = join(f.dir, 'inventory.json'); writeFileSync(inv, JSON.stringify(inventory));
  const args = ['device-enroll', '--thread', 'coordinator', '--task', f.ta.id, '--host', device.host,
    '--udid', device.udid, '--class', device.class, '--runtime', device.runtime, '--inventory', inv, '--origin', 'audited-unused'];
  expect((await f.cli(f.main, args)).err).toContain('newly created');
  expect(await c.status()).toEqual(before);
  await c.enroll('coordinator', device, inventory, true, f.ta.id);
  const owned = await c.status();
  await expect(c.enroll('coordinator', { ...device, udid: 'second-phone' }, [{ ...inventory[0], udid: 'second-phone' }], true, f.ta.id)).rejects.toThrow('already owns');
  expect(await c.status()).toEqual(owned);
});

test('concurrent direct enrollment assigns one owner and never exposes the new simulator to pool claims', async () => {
  const f = fixture(); await f.start();
  const device = { host: 'fixture-host', udid: 'fresh-ipad', class: 'ipad', runtime: 'iOS-fixture' };
  const inventory = [{ ...device, state: 'Shutdown' }];
  const inv = join(f.dir, 'inventory.json'); writeFileSync(inv, JSON.stringify(inventory));
  const enroll = task => f.cli(f.main, ['device-enroll', '--thread', 'coordinator', '--task', task.id,
    '--host', device.host, '--udid', device.udid, '--class', device.class, '--runtime', device.runtime,
    '--inventory', inv, '--origin', 'created']);
  const claim = task => f.cli(task.worktree, ['device-claim', ...own(task), '--host', device.host, '--class', device.class, '--runtime', device.runtime]);
  const [a, b, pooled] = await Promise.all([enroll(f.ta), enroll(f.tb), claim(f.tb)]);
  expect([a, b].filter(r => r.code === 0)).toHaveLength(1);
  expect([a, b].find(r => r.code !== 0).err).toContain('already registered');
  expect(pooled.err).toMatch(/no released compatible|already owns/);
  const winner = a.code === 0 ? f.ta : f.tb, loser = winner === f.ta ? f.tb : f.ta;
  const state = await f.c(f.main).status();
  expect(Object.values(state.devices)).toHaveLength(1);
  expect(state.devices['fixture-host:fresh-ipad']).toMatchObject({ status: 'owned', task: winner.id, thread: winner.thread, worktree: winner.worktree });
  expect(state.events.filter(e => e.action === 'device-claimed')).toHaveLength(1);
  await f.c(winner.worktree).releaseDevice(winner.id, winner.thread, device.host, device.udid, inventory);
  expect((await f.c(f.main).status()).devices['fixture-host:fresh-ipad'].status).toBe('idle');
  expect((await claim(loser)).value).toMatchObject({ udid: device.udid, task: loser.id });
});

test('coordinator designates only an exact released local surplus and retains the compatible pool', async () => {
  const f = fixture(); await f.start(); const c = f.c(f.main);
  const devices = [
    { host: 'local', udid: 'kept-phone', class: 'phone', runtime: 'iOS-fixture' },
    { host: 'local', udid: 'kept-ipad', class: 'ipad', runtime: 'iOS-fixture' },
    { host: 'Mac', udid: 'surplus-ipad', class: 'ipad', runtime: 'iOS-fixture' },
  ];
  const inventory = devices.map(d => ({ ...d, state: 'Shutdown' }));
  const directory = join(f.dir, 'Devices'); mkdirSync(directory);
  for (const d of devices) {
    mkdirSync(join(directory, d.udid));
    await c.enroll('coordinator', d, inventory, true);
  }
  const evidence = join(f.dir, 'surplus-evidence.json'); writeFileSync(evidence, '{}');
  const preserved = await f.c(f.a).preserve(f.ta.id, f.ta.thread, [evidence]);
  const initial = await c.status(), target = 'Mac:surplus-ipad';
  expect(initial.devices[target].status).toBe('idle');
  const identity = d => { const path = join(directory, d.udid), s = lstatSync(path); return { path, dev: s.dev, ino: s.ino }; };
  const proof = { observedAtMs: Date.now(), reason: 'Retain the original pool; remove the released device enrolled with another host label.',
    expectedDevice: initial.devices[target], identity: identity(devices[2]),
    retainedDevices: devices.slice(0, 2).map(d => ({ expected: initial.devices[`${d.host}:${d.udid}`], identity: identity(d) })),
    inventory, evidence: preserved.evidence };
  await expect(c.retireDevice('coordinator', 'Mac', 'surplus-ipad', inventory)).rejects.toThrow('only surplus');
  await expect(c.designateDeviceSurplus('thread-a', proof)).rejects.toThrow('coordinator');
  for (const invalid of [
    { ...proof, observedAtMs: Date.now() - 60001 }, { ...proof, observedAtMs: Date.now() + 60000 },
    { ...proof, reason: '' }, { ...proof, retainedDevices: [] },
    { ...proof, expectedDevice: { ...proof.expectedDevice, runtime: 'changed' } },
    { ...proof, identity: { ...proof.identity, ino: proof.identity.ino + 1 } },
    { ...proof, inventory: inventory.filter(d => d.udid !== 'surplus-ipad') },
    { ...proof, inventory: [...inventory, inventory[2]] },
    { ...proof, inventory: inventory.map(d => ({ ...d, state: 'Booted' })) },
    { ...proof, evidence: [] }, { ...proof, evidence: [evidence] },
    { ...proof, retainedDevices: [{ expected: proof.expectedDevice, identity: proof.identity }] },
  ]) {
    await expect(c.designateDeviceSurplus('coordinator', invalid)).rejects.toThrow();
    expect((await c.status()).devices).toEqual(initial.devices);
  }
  const stateFile = join(f.root, 'state.json');
  for (const change of [{ task: f.ta.id }, { created: false }, { repo: 'foreign' }, { status: 'owned' }]) {
    const changed = structuredClone(initial); Object.assign(changed.devices[target], change);
    writeFileSync(stateFile, JSON.stringify(changed));
    await expect(c.designateDeviceSurplus('coordinator', proof)).rejects.toThrow('released task-created');
  }
  writeFileSync(stateFile, JSON.stringify(initial));
  const duplicate = { ...initial.devices[target], host: 'local' };
  const aliased = structuredClone(initial); aliased.devices['local:surplus-ipad'] = duplicate;
  writeFileSync(stateFile, JSON.stringify(aliased));
  await expect(c.designateDeviceSurplus('coordinator', { ...proof,
    retainedDevices: [{ expected: duplicate, identity: proof.identity }],
    inventory: [...inventory, { ...duplicate, state: 'Shutdown' }],
  })).rejects.toThrow('distinct local simulator directories');
  expect((await c.status()).devices).toEqual(aliased.devices);
  writeFileSync(stateFile, JSON.stringify(initial));
  const alias = join(f.dir, 'alias', 'Devices'); mkdirSync(alias, { recursive: true });
  symlinkSync(join(directory, 'surplus-ipad'), join(alias, 'surplus-ipad'));
  await expect(c.designateDeviceSurplus('coordinator', { ...proof, identity: { ...proof.identity, path: join(alias, 'surplus-ipad') } })).rejects.toThrow('symlink');
  const proofFile = join(f.dir, 'surplus-proof.json'); writeFileSync(proofFile, JSON.stringify(proof));
  const result = await f.cli(f.main, ['device-surplus', '--thread', 'coordinator', '--proof', proofFile]);
  expect(result.code, result.err).toBe(0); expect(result.value.action).toBe('device-surplus-designated');
  const after = await c.status();
  expect(after.devices[target]).toEqual({ ...initial.devices[target], status: 'retiring' });
  for (const d of devices.slice(0, 2)) expect(after.devices[`${d.host}:${d.udid}`]).toEqual(initial.devices[`${d.host}:${d.udid}`]);
  expect(existsSync(join(directory, 'surplus-ipad'))).toBe(true);
  await expect(c.retireDevice('coordinator', 'Mac', 'surplus-ipad', inventory)).rejects.toThrow('delete the surplus');
  rmSync(join(directory, 'surplus-ipad'), { recursive: true });
  await c.retireDevice('coordinator', 'Mac', 'surplus-ipad', inventory.slice(0, 2));
  expect(Object.keys((await c.status()).devices).sort()).toEqual(['local:kept-ipad', 'local:kept-phone']);
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

test('missing process start times block live owners but release after same-boot PID absence', async () => {
  const f = fixture(); await f.start(); const c = f.c(f.a);
  const ready = join(f.dir, 'missing-start-ready');
  const running = f.cli(f.a, ['run', ...own(f.ta), '--', process.execPath, '-e', `require('fs').writeFileSync(${JSON.stringify(ready)}, 'ready');setInterval(()=>{},1000)`]);
  await waitFile(ready);
  const state = await c.status();
  const [key, lease] = Object.entries(state.leases).find(([, l]) => l.task === f.ta.id);
  const child = state.tasks[f.ta.id].processes.find(p => p.role === 'child');
  const guard = state.tasks[f.ta.id].processes.find(p => p.role === 'guard');
  expect(child.started).toBeTruthy(); expect(guard.started).toBeTruthy();
  try {
    await c.transaction(s => {
      s.leases[key].guard.started = null;
      for (const p of s.tasks[f.ta.id].processes) p.started = null;
    });
    await expect(c.stop(f.ta.id, f.ta.thread, child.pid)).rejects.toThrow('unknown process identity');
    await expect(c.auditRun(f.ta.id, f.ta.thread, audit(f.ta, lease.operation))).rejects.toThrow('launcher still live or unknown');
    expect(processIdentity(child.pid)?.started).toBe(child.started);
    process.kill(child.pid, 'SIGTERM'); await running;
    expect(processIdentity(child.pid)).toBeNull(); expect(processIdentity(guard.pid)).toBeNull();
    // A failed final census can leave the dead launcher in the running phase.
    await c.transaction(s => {
      s.leases[key].phase = 'running';
      s.leases[key].guard.bootId = 'another-boot';
      s.tasks[f.ta.id].processes.push({ ...guard, started: null });
    });
    await expect(c.auditRun(f.ta.id, f.ta.thread, audit(f.ta, lease.operation))).rejects.toThrow('launcher still live or unknown');
    await c.transaction(s => { s.leases[key].guard.bootId = guard.bootId; });
    expect((await c.auditRun(f.ta.id, f.ta.thread, audit(f.ta, lease.operation))).held).toBe(false);
    expect((await c.status()).leases).toEqual({});
  } finally {
    if (processIdentity(child.pid)?.started === child.started) process.kill(child.pid, 'SIGTERM');
    await running;
  }
});

test('confirmed same-host restart retires only the audited execution and preserves unknown history', async () => {
  const f = fixture(); await f.start(); const c = f.c(f.main);
  const current = processIdentity(process.pid); expect(current?.bootId).toBeTruthy();
  await f.c(f.a).recordOutput(f.ta.id, f.ta.thread, 'target/retained', 1);
  await f.c(f.a).resource(f.ta.id, f.ta.thread, 'storage', 'fixture:retained');
  const operation = 'prior-boot-native';
  const guard = { pid: 2147483646, host: current.host, bootId: 'prior-boot', started: 'prior boot launcher' };
  const child = { ...guard, pid: 2147483645, operation, role: 'child', started: 'prior boot child' };
  await c.transaction(s => {
    // Any accidental census of this old group fails without inspecting real PIDs.
    s.leases['native-build'] = { task: f.ta.id, operation, guard, kind: 'native-build', phase: 'awaiting-audit', group: 0 };
    s.tasks[f.ta.id].processes = [{ ...guard, operation, role: 'guard' }, child, { ...child, operation: 'retained-operation' }];
    s.events.push({ action: 'process-audit-held', operation, held: true, audit: { noUnrecordedDescendants: false } });
  });
  const baseline = await c.status();
  const accepted = { ...audit(f.ta, operation, [guard, child], 'coordinator'), noUnrecordedDescendants: false,
    newBootBoundary: { fromBootId: guard.bootId, toBootId: current.bootId, host: current.host,
      sameHostRestartConfirmed: true, confirmationReference: 'fixture human confirmation' } };
  const refusals = [
    ['same boot', (s, a) => { s.leases['native-build'].guard.bootId = current.bootId; a.newBootBoundary.fromBootId = current.bootId; }],
    ['missing recorded boot', s => { delete s.leases['native-build'].guard.bootId; }],
    ['wrong prior boot', (s, a) => { a.newBootBoundary.fromBootId = 'forged'; }],
    ['wrong current boot', (s, a) => { a.newBootBoundary.toBootId = 'forged'; }],
    ['missing current boot', (s, a) => { delete a.newBootBoundary.toBootId; }],
    ['unconfirmed restart', (s, a) => { a.newBootBoundary.sameHostRestartConfirmed = false; }],
    ['missing confirmation reference', (s, a) => { delete a.newBootBoundary.confirmationReference; }],
    ['empty confirmation reference', (s, a) => { a.newBootBoundary.confirmationReference = ' '; }],
    ['wrong current host', (s, a) => { a.newBootBoundary.host = 'another-host'; }],
    ['wrong prior host', s => { s.leases['native-build'].guard.host = 'another-host'; }],
    ['running launcher', s => { s.leases['native-build'].phase = 'running'; }],
    ['pending launch', s => { s.tasks[f.ta.id].pending.push('pending-operation'); }],
    ['mixed boot process', s => { s.tasks[f.ta.id].processes[1].bootId = current.bootId; }],
    ['legacy process', s => { delete s.tasks[f.ta.id].processes[1].bootId; }],
    ['missing process start', s => { s.tasks[f.ta.id].processes[1].started = null; }],
    ['invalid process PID', s => { s.tasks[f.ta.id].processes[1].pid = 0; }],
    ['no recorded operation', s => { s.tasks[f.ta.id].processes = []; }],
    ['omitted child', (s, a) => { a.processes = [guard]; }],
    ['omitted guard', (s, a) => { a.processes = [child]; }],
    ['unrecorded identity', (s, a) => { a.processes.push({ ...child, pid: 2147483644 }); }],
    ['false lifetime claim', (s, a) => { a.noUnrecordedDescendants = true; }],
    ['noncoordinator', (s, a) => { a.auditorThreadId = f.ta.thread; }],
    ['mismatched process key', s => { const l = s.leases['native-build']; delete s.leases['native-build']; s.leases['process:old'] = { ...l, kind: 'process' }; }],
    ['mismatched native kind', s => { s.leases['native-build'].kind = 'process'; }],
    ['publishing lease', s => { const l = s.leases['native-build']; delete s.leases['native-build']; s.leases['publish:old'] = { ...l, kind: 'publish' }; }],
    ['ordinary audit remains closed', (s, a) => { delete a.newBootBoundary; }],
    ['mismatched task', (s, a) => { a.taskId = f.tb.id; }],
  ];
  const stateFile = join(f.root, 'state.json');
  for (const [name, mutate] of refusals) {
    const state = structuredClone(baseline), attempt = structuredClone(accepted);
    mutate(state, attempt); writeFileSync(stateFile, JSON.stringify(state));
    const before = readFileSync(stateFile, 'utf8');
    await expect(c.auditRun(f.ta.id, attempt.auditorThreadId, attempt), name).rejects.toThrow();
    expect(readFileSync(stateFile, 'utf8'), name).toBe(before);
  }
  writeFileSync(stateFile, JSON.stringify(baseline));
  const released = await c.auditRun(f.ta.id, 'coordinator', accepted);
  expect(released).toMatchObject({ action: 'process-audit-released', operation, held: false,
    historicalLifetime: 'unknown', releaseBasis: 'confirmed-same-host-restart', guard, audit: accepted });
  expect(released.retiredProcesses).toEqual(baseline.tasks[f.ta.id].processes.filter(p => p.operation === operation));
  const after = await c.status();
  expect(after.events.slice(0, -1)).toEqual(baseline.events);
  expect(after.events.at(-1)).toEqual(released);
  const expected = structuredClone(baseline);
  delete expected.leases['native-build'];
  expected.tasks[f.ta.id].processes = expected.tasks[f.ta.id].processes.filter(p => p.operation !== operation);
  expected.events.push(released);
  expect(after).toEqual(expected);

  const processBaseline = structuredClone(baseline);
  const processLease = { ...processBaseline.leases['native-build'], kind: 'process' };
  delete processBaseline.leases['native-build'];
  processBaseline.leases[`process:${operation}`] = processLease;
  const sameBootProcess = structuredClone(processBaseline), sameBootAudit = structuredClone(accepted);
  sameBootProcess.leases[`process:${operation}`].guard.bootId = current.bootId;
  sameBootAudit.newBootBoundary.fromBootId = current.bootId;
  writeFileSync(stateFile, JSON.stringify(sameBootProcess));
  const sameBootBytes = readFileSync(stateFile, 'utf8');
  await expect(c.auditRun(f.ta.id, 'coordinator', sameBootAudit)).rejects.toThrow('confirmed same-host restart');
  expect(readFileSync(stateFile, 'utf8')).toBe(sameBootBytes);
  writeFileSync(stateFile, JSON.stringify(processBaseline));
  const processReleased = await c.auditRun(f.ta.id, 'coordinator', accepted);
  expect(processReleased).toMatchObject({ operation, held: false, historicalLifetime: 'unknown',
    releaseBasis: 'confirmed-same-host-restart', audit: accepted });
  const processExpected = structuredClone(processBaseline);
  delete processExpected.leases[`process:${operation}`];
  processExpected.tasks[f.ta.id].processes = processExpected.tasks[f.ta.id].processes.filter(p => p.operation !== operation);
  processExpected.events.push(processReleased);
  expect(await c.status()).toEqual(processExpected);
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
