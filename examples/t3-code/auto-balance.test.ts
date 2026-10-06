// Auto balance (task auto-balance). Ports of T3 Code 1e2ecbd975 (MIT, see LICENSE-T3):
// packages/client-runtime/src/state/projectGrouping.test.ts "load balancing shared project
// machines" (every case, original names) and apps/web/src/components/ServerUpdateAction.test.tsx
// "ServerUpdatesAction" (every case, original names; the clone's update is a T3Fleet job, so a
// machine's success toast follows its reconnect, as server-update.test.ts shows). Then the
// clone's wiring: ChatView's automatic-environment state, the load, the Run on menu, Send, and
// the multi-machine banner.
import { afterEach, beforeEach, describe, expect, it, test } from 'bun:test';
import { obj, type Obj } from './domain';
import type { Native } from './protocol';
import { T3Client } from './client';
import { toasts } from './toast';
import { EnvironmentFleet, type FleetEntry } from './settings-b-fleet';
import { setJobs, type OutdatedJob } from './settings-b-outdated';
import { noteNow } from './composer-controls';
import { chooseLoadBalancedEnvironment, decodeHostResources } from './load-balancing';
import { autoBalancePrepare, autoBalanceSend, autoBalanceState, chooseAutoEnvironment, draftSelection, loadHostResources, noteBalancePrefs, resetHostResources,
  setDraftSelection, withAutoOption } from './auto-balance';
import { snapshot } from './presentation';
import { autoBalanceUpdateBanner, autoBalanceUpdateMachines, desktopAppsConfirmMessage, machineRow, updateServers, updatesPending,
  type BannerEnvironment } from './auto-balance-banner';
import type { ServerUpdateTarget, UpdateDeps } from './server-update';
import { draftContext } from './composer-controls-branch';
import { CLIENT_VERSION } from './version-skew';

describe("load balancing shared project machines", () => {
  const now = 100_000;
  const resources = {
    sampledAt: now,
    cpuUtilization: 0.2,
    cpuCount: 8,
    availableMemoryBytes: 8_000,
    totalMemoryBytes: 16_000,
  };

  it("compares three machines using free capacity and preference", () => {
    const candidates = [
      { environmentId: "busy", resources: { ...resources, cpuUtilization: 0.9 }, weight: 1 },
      { environmentId: "idle", resources, weight: 1 },
      { environmentId: "preferred", resources: { ...resources, cpuCount: 4 }, weight: 3 },
    ];
    expect(chooseLoadBalancedEnvironment(candidates, now)).toBe("preferred");
    expect(chooseLoadBalancedEnvironment(candidates.slice(0, 2), now)).toBe("idle");
  });

  it("rejects stale, unknown, excluded and saturated machines", () => {
    expect(
      chooseLoadBalancedEnvironment(
        [
          {
            environmentId: "stale",
            resources: { ...resources, sampledAt: now - 15_001 },
            weight: 1,
          },
          { environmentId: "unknown", resources: null, weight: 1 },
          {
            environmentId: "no-cpu-sample",
            resources: { ...resources, cpuUtilization: null },
            weight: 1,
          },
          { environmentId: "excluded", resources, weight: 0 },
          {
            environmentId: "cpu-full",
            resources: { ...resources, cpuUtilization: 0.95 },
            weight: 1,
          },
          {
            environmentId: "memory-full",
            resources: { ...resources, availableMemoryBytes: 100 },
            weight: 1,
          },
        ],
        now,
      ),
    ).toBeNull();
  });

  it("uses client receipt time when host clocks differ", () => {
    const candidate = {
      environmentId: "different-clock",
      resources: { ...resources, sampledAt: now + 60_000 },
      receivedAt: now,
      weight: 1,
    };
    expect(chooseLoadBalancedEnvironment([candidate], now)).toBe("different-clock");
    expect(chooseLoadBalancedEnvironment([candidate], now + 15_001)).toBeNull();
  });
});

describe('HostResourcesSnapshot', () => {
  test('a reply that does not match the schema is no sample', () => {
    expect(decodeHostResources({ sampledAt: 1, cpuUtilization: null, cpuCount: 8, availableMemoryBytes: 1, totalMemoryBytes: 2 })).toMatchObject({ cpuUtilization: null });
    expect(decodeHostResources({ sampledAt: 1, cpuUtilization: 1.2, cpuCount: 8, availableMemoryBytes: 1, totalMemoryBytes: 2 })).toBeNull();
    expect(decodeHostResources({ sampledAt: 1, cpuUtilization: 0.1, cpuCount: -1, availableMemoryBytes: 1, totalMemoryBytes: 2 })).toBeNull();
    expect(decodeHostResources(null)).toBeNull();
  });
});

// ── ServerUpdatesAction ────────────────────────────────────────────────────
describe("ServerUpdatesAction", () => {
  const targets: ReadonlyArray<ServerUpdateTarget> = [
    { environmentKey: 'http://10.0.0.2:3773\nbatch-a', environmentId: 'batch-a', serverLabel: 'Laptop', selfUpdate: 'boot-service', targetVersion: '0.0.31', fromVersion: '0.0.30',
      threadContinuation: true, continueThreadsAfterServerUpdate: true },
    { environmentKey: 'http://10.0.0.3:3773\nbatch-b', environmentId: 'batch-b', serverLabel: 'Office', selfUpdate: 'respawn', targetVersion: '0.0.31', fromVersion: '0.0.30',
      threadContinuation: true, continueThreadsAfterServerUpdate: false },
    { environmentKey: 'http://10.0.0.4:3773\nbatch-c', environmentId: 'batch-c', serverLabel: 'Manual', selfUpdate: null, targetVersion: '0.0.31', fromVersion: '0.0.30' },
  ];
  type Harness = { deps: UpdateDeps; started: Obj[]; toasted: Obj[]; completions: Array<() => void> };
  function harness(start?: (key: string) => Promise<Obj>): Harness {
    const state: Harness = { started: [], toasted: [], completions: [], deps: null as unknown as UpdateDeps };
    state.deps = {
      start: (key, request) => { state.started.push({ key, ...request }); return start ? start(key) : Promise.resolve({ started: true, attempt: `${key}#1` }); },
      copy: async () => {}, toast: toast => { state.toasted.push(toast); },
    };
    return state;
  }
  beforeEach(() => setJobs({}));

  it("updates both supported machines with their own continuation preference and skips the manual machine", async () => {
    const state = harness(), owner = {};
    expect(await updateServers(owner, targets, state.deps, async () => true)).toBe('started');
    expect(state.started.map(call => [String(call.key).split('\n')[1], call.targetVersion, call.extras])).toEqual([
      ['batch-a', '0.0.31', { continueRunningThreads: true }],
      ['batch-b', '0.0.31', {}],
    ]);
    // Each job's "<label> updated" toast follows its reconnect (server-update.test.ts, announceServerUpdates).
    expect(state.toasted).toEqual([]);
  });

  it("names a failed machine while letting the other machine complete", async () => {
    const state = harness(async key => { if (key.endsWith('batch-a')) throw new Error('Download failed'); return { started: true, attempt: 'b1' }; }), owner = {};
    expect(await updateServers(owner, targets, state.deps, async () => true)).toBe('started');
    expect(state.started).toHaveLength(2);
    expect(state.toasted).toEqual([{ kind: 'error', title: 'Laptop update failed', description: 'Download failed', stacked: true }]);
    expect(updatesPending(owner)).toBe(false);
  });

  it("starts each machine once when double-clicked and disables the action until both finish", async () => {
    const state = harness(); const owner = {};
    state.deps.start = (key, request) => { state.started.push({ key, ...request }); return new Promise(resolve => state.completions.push(() => resolve({ started: true, attempt: key }))); };
    const first = updateServers(owner, targets, state.deps, async () => true), second = updateServers(owner, targets, state.deps, async () => true);
    expect(await second).toBe('busy');
    await Promise.resolve();
    expect(state.started).toHaveLength(2);
    expect(updatesPending(owner)).toBe(true);
    state.completions[0]!();
    await Promise.resolve();
    expect(updatesPending(owner)).toBe(true);
    state.completions[1]!();
    expect(await first).toBe('started');
    expect(updatesPending(owner)).toBe(false);
  });

  it("asks once for desktop machines and cancels the entire batch", async () => {
    const state = harness(), owner = {}, asked: string[] = [];
    const desktop = targets.map((target, index) => index < 2 ? { ...target, selfUpdate: 'desktop-managed', desktopAppUpdate: true } : target);
    expect(await updateServers(owner, desktop, state.deps, async message => { asked.push(message); return false; })).toBe('cancelled');
    expect(asked).toEqual(['Update the T3 Code desktop apps on Laptop, Office? They will close and relaunch on those machines.']);
    expect(asked[0]).toContain('Laptop, Office');
    expect(state.started).toEqual([]);
    expect(updatesPending(owner)).toBe(false);
    expect(desktopAppsConfirmMessage(['A', 'B'])).toBe('Update the T3 Code desktop apps on A, B? They will close and relaunch on those machines.');
  });
});

// ── The clone's wiring ─────────────────────────────────────────────────────
const identity = { canonicalKey: 'git.example.invalid/acme/shared', rootPath: '/repos/shared' };
const provider = { instanceId: 'codex', driver: 'codex', enabled: true, installed: true, status: 'ready', auth: { status: 'authenticated' }, models: [{ slug: 'gpt-5' }] };
const configFor = (label: string, extra: Obj = {}): Obj => ({ environment: { label, serverVersion: CLIENT_VERSION, ...obj(extra.environment) }, providers: [provider], ...extra, ...(extra.environment ? { environment: { label, serverVersion: CLIENT_VERSION, ...obj(extra.environment) } } : {}) });
const NOW = 1_800_000_000_000;
const sample = (cpu: number | null, sampledAt = NOW) => ({ sampledAt, cpuUtilization: cpu, cpuCount: 8, availableMemoryBytes: 8_000, totalMemoryBytes: 16_000 });

function entry(id: string, label: string, phase: FleetEntry['phase'] = 'connected', config: Obj = configFor(label)): FleetEntry {
  return { key: `http://127.0.0.1:${id.charCodeAt(0)}\n${id}`, origin: `http://127.0.0.1:${id.charCodeAt(0)}`, environmentId: id, phase, message: '', traceId: '',
    generation: 1, synchronized: 1, lastEvent: 0, subscriptions: {}, config, shell: { projects: [{ id: `p${id}`, workspaceRoot: '/repos/shared', repositoryIdentity: identity }], threads: [], sequence: 0 } as never,
    scopes: [], error: '', requested: true };
}
type Setup = { client: T3Client; source: EnvironmentFleet; native: Native; calls: Obj[]; replies: Record<string, unknown>; hooks: { connect?: (request: Obj) => unknown } };
function setup(machines: FleetEntry[] = [entry('b', 'Studio'), entry('c', 'Build box')], replies: Record<string, unknown> = {}): Setup {
  const client = new T3Client(), source = new EnvironmentFleet(), calls: Obj[] = [], hooks: Setup['hooks'] = {};
  for (const machine of machines) source.entries.set(machine.key, machine);
  Object.assign(client, { environmentId: 'a', origin: 'http://192.0.2.10:3773', projectId: 'pa', threadId: '', connection: 'connected', configLive: true, shellLive: true,
    config: configFor('Laptop'), providerId: 'codex', modelId: 'gpt-5' });
  client.shell.projects = [{ id: 'pa', workspaceRoot: '/repos/shared', repositoryIdentity: identity }] as never;
  noteNow(client, NOW);
  noteBalancePrefs(client, { loadBalancingEnabled: true, loadBalancingWeights: {}, githubRouting: {} });
  const native: Native = { available: true, watch() {}, later: async input => {
    const request = obj(input); calls.push(request);
    if (request.op === 'connect') {
      // The Run on move refocuses the window's connection on the chosen machine (T3Transport connect).
      const failure = await hooks.connect?.(request);
      if (failure instanceof Error) return { ok: false, generation: 0, error: { kind: 'Connection', message: failure.message } };
      const target = machines.find(machine => machine.origin === request.origin);
      return { ok: true, generation: 2, value: { state: 'connected', origin: String(request.origin), environmentId: target?.environmentId ?? '', message: '' } };
    }
    if (request.op !== 'request') return { ok: true, generation: Number(request.generation ?? 0), value: {} };
    const target = String(request.fleet ?? '').split('\n')[1] ?? 'a', reply = replies[target || 'a'];
    if (reply instanceof Error) return { ok: false, generation: Number(request.generation ?? 0), error: { kind: 'Timeout', message: reply.message, uncertain: true } };
    return { ok: true, generation: Number(request.generation ?? 0), value: reply ?? sample(0.5) };
  } };
  return { client, source, native, calls, replies, hooks };
}

/** What the window's next synchronization of the machine the draft moved to brings (synchronize: config, shell, defaults). */
function synchronizedOn(client: T3Client, label: string, model = 'gpt-5'): void {
  Object.assign(client, { connection: 'connected', configLive: true, shellLive: true, threadLive: true, config: configFor(label) });
  client.shell.projects = [{ id: client.projectId, workspaceRoot: '/repos/shared', repositoryIdentity: identity }] as never;
  Object.assign(client, { providerId: 'codex', modelId: model, runtimeMode: 'approval-required', interactionMode: 'default' }); // chooseDefaults
}

describe('Auto balance (ChatView automaticEnvironment, useLoadBalancedEnvironment)', () => {
  beforeEach(() => { resetHostResources(); setJobs({}); });
  afterEach(() => resetHostResources());

  test('a draft on a project three machines hold is automatic and checks machines until each one answers', async () => {
    const { client, source, native, calls } = setup(undefined, { a: sample(0.5), b: sample(0.96), c: sample(0.1, NOW - 20_000) });
    const state = autoBalanceState(client, NOW, source);
    expect([state.offered, state.automatic, state.needs, state.pending, state.label]).toEqual([true, true, true, true, 'Checking machines…']);
    expect(state.candidates).toEqual(['c', 'b', 'a']); // loopback machines lead (primary), then by label
    expect(state.fetch).not.toBe('');
    await loadHostResources(client, native, state.fetch, NOW, source);
    // One server.getHostResources per candidate, each with the 5 s deadline, over its own transport.
    expect(calls.filter(call => call.method === 'server.getHostResources').map(call => [call.fleet ? String(call.fleet).split('\n')[1] : 'a', call.timeout])).toEqual([['c', 5], ['b', 5], ['a', 5]]);
    // b is busy (0.96); c's sample is stale by its own clock, but receipt time is the client's: c is idle and wins,
    // and the draft moves there with its auto selection (setDraftThreadContext's projectRef).
    expect(client.environmentId).toBe('c');
    expect(client.draftKey).toBe('c:new:pc');
    expect(draftSelection(client)).toEqual({ selection: 'auto', choice: 'c' });
    expect(draftSelection(client, 'a:new:pa')).toEqual({ selection: '', choice: '' });
    synchronizedOn(client, 'Build box');
    const after = autoBalanceState(client, NOW, source);
    expect([after.automatic, after.needs, after.chosen, after.label, after.fetch]).toEqual([true, false, 'c', 'Auto balance', '']);
  });

  test('a choice that resolves while the user types moves the draft and keeps its text, owner, workspace and model', async () => {
    const { client, source, native, hooks } = setup(undefined, { a: sample(0.5), b: sample(0.96), c: sample(0.1) });
    const typed = 'Draft typed while machines are checked';
    client.local.drafts[client.draftKey] = typed;
    (client.local.composerControls.contexts ??= {})[client.draftKey] = { envMode: 'worktree', branch: '', worktreePath: '' };
    Object.assign(client, { modelId: 'gpt-5', runtimeMode: 'full-access', interactionMode: 'plan' });
    const owner = snapshot(client).composerOwner;
    expect(owner).toBe('http://192.0.2.10:3773:pa:');
    // A keystroke reaches the old key while the window's connection moves (the 'draft' op ran first).
    hooks.connect = () => { client.local.drafts['a:new:pa'] = `${typed}!`; };
    await loadHostResources(client, native, autoBalanceState(client, NOW, source).fetch, NOW, source);
    expect([client.environmentId, client.projectId, client.draftKey]).toEqual(['c', 'pc', 'c:new:pc']);
    expect(client.local.drafts).toEqual({ 'c:new:pc': `${typed}!` });
    expect(draftContext(client)).toMatchObject({ envMode: 'worktree' });
    expect(draftSelection(client)).toEqual({ selection: 'auto', choice: 'c' });
    // app.contract's composerText keeps the window's typed text while the owner holds: the field is not
    // rewritten, so its caret and focus stay where the user left them.
    expect(snapshot(client).composerOwner).toBe(owner);
    synchronizedOn(client, 'Build box');
    expect(snapshot(client)).toMatchObject({ composerOwner: owner, draft: `${typed}!`, projectId: 'pc' });
    // The moved draft keeps its model and modes over the machine's defaults.
    await autoBalancePrepare(client, null);
    expect([client.providerId, client.modelId, client.runtimeMode, client.interactionMode]).toEqual(['codex', 'gpt-5', 'full-access', 'plan']);
    // Leaving the draft drops the kept owner: another draft's owner is its own.
    Object.assign(client, { threadId: 't9' });
    expect(snapshot(client).composerOwner).toBe(`${client.origin}:pc:t9`);
  });

  test('the composer reads its typed text through the draft owner, which a moved draft keeps', async () => {
    const contract = await Bun.file(new URL('./app.contract', import.meta.url)).text();
    expect(contract).toContain('derive composerOwner = data.composerOwner');
    expect(contract).toContain('derive composerText = draftOwner == `${composerOwner}${data.requestKey}` ? draft : data.draft');
    // The load's result lands as data only: no focus move, no draft owner reset.
    const load = contract.slice(contract.indexOf('action balanceLoadNow'), contract.indexOf('\n', contract.indexOf('action balanceLoadNow') + 30) + 1);
    expect(load).not.toContain('focus(');
    expect(load).not.toContain('draftOwner');
  });

  test('a move whose connection fails leaves the draft, its text and its unresolved Auto selection where they were', async () => {
    const { client, source, native, hooks } = setup(undefined, { a: sample(0.5), b: sample(0.96), c: sample(0.1) });
    client.local.drafts[client.draftKey] = 'Keep me';
    hooks.connect = () => new Error('The connection was refused.');
    await loadHostResources(client, native, autoBalanceState(client, NOW, source).fetch, NOW, source);
    expect([client.environmentId, client.draftKey, client.local.drafts['a:new:pa'], client.local.drafts['c:new:pc']]).toEqual(['a', 'a:new:pa', 'Keep me', undefined]);
    expect(draftSelection(client).choice).toBe('');
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: 'Could not switch machine', description: 'The connection was refused.' });
  });

  test('a send waits for a move in flight and sends from the machine the draft moved to', async () => {
    const { client, source, native, hooks } = setup(undefined, { a: sample(0.5), b: sample(0.96), c: sample(0.1) });
    let release = () => {};
    const gate = new Promise<void>(resolve => { release = resolve; });
    let connecting = false;
    hooks.connect = () => { connecting = true; return gate; };
    const load = loadHostResources(client, native, autoBalanceState(client, NOW, source).fetch, NOW, source);
    for (let turn = 0; turn < 100 && !connecting; turn++) await Promise.resolve();
    expect(connecting).toBe(true);
    let sentFrom = '';
    const send = autoBalanceSend(client, native, async () => { sentFrom = client.environmentId; }, source);
    await Promise.resolve();
    expect(sentFrom).toBe('');
    release();
    await Promise.all([load, send]);
    expect(sentFrom).toBe('c');
  });

  test('a draft with no provider chosen yet balances over the requested driver (Codex)', () => {
    const { client, source } = setup();
    Object.assign(client, { providerId: '', modelId: '' });
    expect(autoBalanceState(client, NOW, source).candidates).toEqual(['c', 'b', 'a']);
  });

  test('a weight of 0 is never chosen and every failure reads "Auto balance unavailable"', async () => {
    const { client, source, native } = setup(undefined, { a: new Error('timed out'), b: new Error('timed out'), c: sample(0.1) });
    noteBalancePrefs(client, { loadBalancingEnabled: true, loadBalancingWeights: { c: 0 }, githubRouting: {} });
    const state = autoBalanceState(client, NOW, source);
    expect(state.candidates).toEqual(['b', 'a']);
    await loadHostResources(client, native, state.fetch, NOW, source);
    const after = autoBalanceState(client, NOW, source);
    expect([after.needs, after.pending, after.failed, after.label]).toEqual([true, false, true, 'Auto balance unavailable']);
    expect(draftSelection(client).choice).toBe('');
  });

  test('picking a machine is manual and clears the choice; picking Auto asks every machine again', async () => {
    const { client, source, native } = setup();
    await loadHostResources(client, native, autoBalanceState(client, NOW, source).fetch, NOW, source);
    synchronizedOn(client, client.environmentId === 'c' ? 'Build box' : client.environmentId === 'b' ? 'Studio' : 'Laptop');
    expect(draftSelection(client).selection).toBe('auto');
    setDraftSelection(client, { selection: 'manual', choice: '' });
    const manual = autoBalanceState(client, NOW, source);
    expect([manual.offered, manual.automatic, manual.label, manual.fetch]).toEqual([true, false, '', '']);
    expect(withAutoOption(client, [{ id: 'a', label: 'Laptop', machine: 'laptop', primary: false, projectId: 'pa', selected: true }], manual).map(option => [option.id, option.selected]))
      .toEqual([['a', true]]); // a single machine offers no Auto item
    chooseAutoEnvironment(client, source);
    expect(draftSelection(client)).toEqual({ selection: 'auto', choice: '' });
    const again = autoBalanceState(client, NOW, source);
    expect([again.pending, again.label]).toEqual([true, 'Checking machines…']);
    const options = withAutoOption(client, [{ id: 'a', label: 'Laptop', machine: 'laptop', primary: false, projectId: 'pa', selected: true },
      { id: 'b', label: 'Studio', machine: 'server', primary: false, projectId: 'pb', selected: false }], again);
    expect(options.map(option => [option.id, option.label, option.selected])).toEqual([['auto', 'Checking machines…', true], ['a', 'Laptop', false], ['b', 'Studio', false]]);
  });

  test('attachments keep the draft on its machine: picking Auto warns, and an unresolved draft is not automatic', () => {
    const { client, source } = setup();
    client.local.snapshotDrafts[client.draftKey] = [{ id: '00000000-0000-0000-0000-000000000001', mimeType: 'image/png', sizeBytes: 1 }];
    expect(autoBalanceState(client, NOW, source).automatic).toBe(false);
    chooseAutoEnvironment(client, source);
    expect(toasts(client).map(toast => [toast.kind, toast.title, toast.description])).toEqual([['warning', 'Keep attachments on this machine',
      'Remove attachments before choosing automatic routing, then attach them on the selected machine.']]);
    expect(draftSelection(client).selection).toBe('');
  });

  test('Load balancing off, a branch chosen by hand or a started thread is not automatic', () => {
    const { client, source } = setup();
    noteBalancePrefs(client, { loadBalancingEnabled: false, loadBalancingWeights: {}, githubRouting: {} });
    expect(autoBalanceState(client, NOW, source).offered).toBe(false);
    noteBalancePrefs(client, { loadBalancingEnabled: true, loadBalancingWeights: {}, githubRouting: {} });
    (client.local.composerControls.contexts ??= {})[client.draftKey] = { envMode: 'local', branch: 'feature', worktreePath: '' };
    expect(autoBalanceState(client, NOW, source).automatic).toBe(false);
    setDraftSelection(client, { selection: 'auto', choice: '' });
    expect(autoBalanceState(client, NOW, source).automatic).toBe(true);
    chooseAutoEnvironment(client, source);
    expect(draftContext(client)).toMatchObject({ branch: '', worktreePath: '' });
    Object.assign(client, { threadId: 't1', threadLive: true });
    expect(autoBalanceState(client, NOW, source).offered).toBe(false);
  });

  test('Send waits while machines are checked, and a send in flight blocks the retarget', async () => {
    const { client, source, native } = setup();
    let sent = 0;
    await autoBalanceSend(client, native, async () => { sent++; }, source);
    expect(sent).toBe(0);
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'warning', title: 'Checking machine resources', description: 'Resource checks are still running. You can choose a machine in the composer.' });
    // A send of a resolved draft that stays here runs at once; a load that lands meanwhile does not retarget.
    setDraftSelection(client, { selection: 'auto', choice: 'a' });
    await autoBalanceSend(client, native, async () => {
      sent++;
      setDraftSelection(client, { selection: 'auto', choice: '' });
      await loadHostResources(client, native, autoBalanceState(client, NOW, source).fetch, NOW, source);
      expect(draftSelection(client).choice).toBe('');
    }, source);
    expect(sent).toBe(1);
  });

  test('every machine failing its check: Send asks for a machine', async () => {
    const { client, source, native } = setup(undefined, { a: new Error('x'), b: new Error('x'), c: new Error('x') });
    await loadHostResources(client, native, autoBalanceState(client, NOW, source).fetch, NOW, source);
    await autoBalanceSend(client, native, async () => { throw new Error('must not send'); }, source);
    expect(toasts(client).at(-1)).toMatchObject({ title: 'Choose a machine to continue', description: 'No eligible machine has available resources. Choose a machine in the composer to override.' });
  });
});

// ── useAutoBalanceUpdateBanner ─────────────────────────────────────────────
describe('useAutoBalanceUpdateBanner', () => {
  const old = '0.0.40';
  const env = (environmentId: string, label: string, capabilities: Obj, connected = true): BannerEnvironment => ({ environmentId, key: `http://192.0.2.1:3773\n${environmentId}`, label,
    connected, config: { environment: { serverVersion: old, capabilities }, settings: {} } });
  const machinesOf = (list: BannerEnvironment[], jobs: Record<string, OutdatedJob> = {}) => autoBalanceUpdateMachines(list, { versionMismatchDismissals: [] }, jobs);
  const job = (patch: Partial<OutdatedJob>): OutdatedJob => ({ status: 'running', stage: 'downloading', fromVersion: old, targetVersion: CLIENT_VERSION, message: '', resultVersion: '',
    label: 'x', mode: 'connected', attempt: 'j1', ...patch });

  test('one item for every machine with a notice: the title counts all, the manual ones and the reconnect row', () => {
    const list = [env('m1', 'Manual box', { serverSelfUpdate: null, serverInstallation: { kind: 'npm-global', prefix: '/opt/node' } }),
      env('m2', 'Offline box', { serverSelfUpdate: 'boot-service' }, false),
      env('m3', 'Desk', { serverSelfUpdate: 'desktop-managed' }),
      env('m4', 'Studio', { serverSelfUpdate: 'respawn' })];
    const machines = machinesOf(list);
    const banner = autoBalanceUpdateBanner(machines)!;
    expect([banner.title, banner.description, banner.action, banner.actionLabel, banner.dismiss, banner.dismissLabel, banner.icon, banner.variant]).toEqual([
      'Update available for 4 machines', '2 need a manual update', 'ab:update-all', 'Update 1 machine', 'ab:dismiss', 'Dismiss update notice', 'download', 'default']);
    expect(banner.menuLabel).toBe('Update available for 4 machines. View machines');
    expect(banner.machines.map(row => [row.label, row.line, row.copyLabel, row.note])).toEqual([
      ['Manual box', 'Manual update required', 'Copy update command', ''],
      ['Offline box', 'Reconnect this machine to update', '', ''],
      ['Desk', 'Manual update required', '', 'Update the desktop app on that machine to update this server.'],
      ['Studio', `Ready to update to ${CLIENT_VERSION}`, '', ''],
    ]);
    expect(autoBalanceUpdateBanner(machinesOf([env('m4', 'Studio', { serverSelfUpdate: 'respawn' })]))).toMatchObject({ title: 'Update available for 1 machine', actionLabel: 'Update all' });
    expect(autoBalanceUpdateBanner([])).toBeNull();
  });

  test('a running machine counts as Updating with no action or dismiss; a failure offers Retry', () => {
    const list = [env('m4', 'Studio', { serverSelfUpdate: 'respawn' }), env('m5', 'Lab', { serverSelfUpdate: 'boot-service' })];
    const running = autoBalanceUpdateBanner(machinesOf(list, { [`http://192.0.2.1:3773\nm4`]: job({}) }))!;
    expect([running.title, running.action, running.dismiss, running.icon, running.priority]).toEqual(['Updating 1 machine', undefined, undefined, 'spinner', 1]);
    expect(running.machines[0]).toMatchObject({ status: 'running', line: 'Downloading…' });
    const failed = autoBalanceUpdateBanner(machinesOf(list, { [`http://192.0.2.1:3773\nm4`]: job({ status: 'failed', message: 'The package could not be verified.' }) }))!;
    expect([failed.title, failed.actionLabel, failed.variant, failed.icon, failed.iconTone]).toEqual(['Could not update 1 machine', 'Retry', 'error', 'circle-alert', 'error']);
    expect(machineRow(machinesOf(list, { [`http://192.0.2.1:3773\nm4`]: job({ status: 'failed', message: 'nope' }) })[0]!)).toMatchObject({ status: 'failed', line: 'nope' });
  });

  test('a dismissed version notice drops its machine', () => {
    const list = [env('m4', 'Studio', { serverSelfUpdate: 'respawn' })];
    expect(autoBalanceUpdateMachines(list, { versionMismatchDismissals: [`m4:${CLIENT_VERSION}:${old}`] }, {})).toEqual([]);
  });
});
