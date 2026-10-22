import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { noPrimary, resetPrimary } from '../support/local-primary-fixture';
// These cases are the hosted rules (resolveHostedFirstRunDecision): no embedded server runs on this Mac.
beforeEach(noPrimary);
afterEach(resetPrimary);
import { splitOnboardingProjectPath, hostedDecision, transition, agentRows, providerState, decodeCandidates, recentCandidates, welcomeView, welcomeLocal, importProjects, welcomeShowing } from '../../src/pages/pages-welcome';
import { pagesPrefs } from '../../src/pages/pages-prefs';
import { fleet, environmentKey } from '../../src/connections/settings-b-fleet';
import { initialShell } from '../../src/client/domain';
import type { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import type { Obj } from '../../src/client/domain';

test('a fresh client with no saved environment gets the wizard, and keeps it until it finishes', () => {
  expect(hostedDecision({ hydrated: true, completed: false, catalogReady: true, environmentCount: 0 })).toEqual({ decision: 'wizard', persistCompletion: false });
  expect(hostedDecision({ hydrated: true, completed: false, catalogReady: true, environmentCount: 1 })).toEqual({ decision: 'app', persistCompletion: true });
  expect(hostedDecision({ hydrated: true, completed: true, catalogReady: false, environmentCount: 0 }).decision).toBe('app');
  expect(hostedDecision({ hydrated: true, completed: false, catalogReady: false, environmentCount: 0 }).decision).toBe('pending');
  expect(transition('pending', 'wizard')).toBe('wizard');
  expect(transition('wizard', 'app')).toBe('wizard');
  expect(transition('app', 'pending')).toBe('app');
  expect(transition('app', 'wizard')).toBe('wizard');
});

const fixtureConfig = {
  settings: { providers: { codex: { enabled: false }, claudeAgent: { enabled: false } },
    providerInstances: { exact_fixture: { driver: 'codex', enabled: true, config: { setupMode: 'existing' } } } },
  providers: [
    { instanceId: 'codex', driver: 'codex', displayName: 'Codex', enabled: false, installed: true, status: 'disabled', auth: { status: 'unknown' } },
    { instanceId: 'exact_fixture', driver: 'codex', displayName: 'Exact verification fixture', enabled: true, installed: true, status: 'ready', auth: { status: 'authenticated' } },
    { instanceId: 'claudeAgent', driver: 'claudeAgent', displayName: 'Claude', enabled: false, installed: true, status: 'disabled', auth: { status: 'unknown' }, message: 'Claude is disabled in T3 Code settings.' },
  ],
};

test('agents read as the reference cards: Codex setup until pointed at a CLI, readiness, Claude disabled', () => {
  const rows = agentRows(fixtureConfig);
  expect(rows.map(row => [row.terminalOpen, row.terminalAvailable])).toEqual([[false, false], [false, false], [false, false]]);
  expect(rows.map(row => [row.kind, row.name, row.summary, row.badge])).toEqual([
    ['codex-setup', 'Codex', 'Code with your ChatGPT subscription.', ''],
    ['card', 'Exact verification fixture', 'Ready to code.', 'Ready'],
    ['card', 'Claude', 'Disabled · Claude is disabled in T3 Code settings.', 'Disabled'],
  ]);
  expect(providerState({ enabled: true, installed: false, status: 'warning', auth: { status: 'unknown' } })).toBe('checking');
  expect(providerState({ enabled: true, installed: true, status: 'ready', auth: { status: 'unauthenticated' } })).toBe('signIn');
  expect(agentRows({ settings: {}, providers: [] }).map(row => [row.name, row.badge])).toEqual([['Codex', ''], ['Claude Code', 'Checking...']]);
});

test('scan candidates decode, and only busy recent repositories start selected', () => {
  const now = Date.parse('2026-10-04T00:00:00.000Z');
  const candidates = decodeCandidates('env', { candidates: [
    { path: '/a', title: 'a', sources: ['codex'], threadCount: 5, lastActiveAt: '2026-10-01T00:00:00.000Z', alreadyImported: false, git: { root: '/a' } },
    { path: '/b', title: 'b', sources: ['claudeAgent'], threadCount: 1, lastActiveAt: '2026-10-01T00:00:00.000Z', alreadyImported: false, git: { root: '/b' } },
    { path: '/c', title: 'c', sources: [], threadCount: 9, lastActiveAt: '2026-10-01T00:00:00.000Z', alreadyImported: false, git: null },
    { path: '/d', title: 'd', sources: [], threadCount: 9, lastActiveAt: '2026-08-01T00:00:00.000Z', alreadyImported: false },
  ] });
  expect(candidates.map(candidate => [candidate.key, candidate.git])).toEqual([['["env","/a"]', true], ['["env","/b"]', true], ['["env","/c"]', false], ['["env","/d"]', true]]);
  expect([...recentCandidates(candidates, now)]).toEqual(['["env","/a"]']);
});

function fakeNative(saved: Obj[]): Native {
  return { available: true, watch: () => {}, later: async (request: Obj) => {
    if (request.op === 'environments') return { ok: true, value: { saved }, generation: 1 };
    if (request.op === 'ids') return { ok: true, value: ['command-1', 'project-1'], generation: 1 };
    return { ok: true, value: {}, generation: 1 };
  } } as unknown as Native;
}

test('the wizard lists the paired computer, sets it up, scans, and finishing lands in the imported project', async () => {
  const calls: { method: string; payload: Obj }[] = [];
  const client = {
    local: { deviceSettings: { appearanceMode: 'system' } } as Record<string, unknown>, origin: 'http://127.0.0.1:14807', environmentId: 'env-1', connection: 'connected', statusMessage: '', scopes: [],
    config: { ...fixtureConfig, environment: { label: 'Morgan’s MacBook Pro' } }, shell: { projects: [] as Obj[], threads: [], sequence: 0 }, threadId: '', projectId: '',
    rpc: async (_native: Native, method: string, payload: Obj) => {
      calls.push({ method, payload });
      if (method === 'agentSessions.scan') return { candidates: [{ path: '/work/app', title: 'app', sources: ['codex'], threadCount: 4, lastActiveAt: new Date(Date.now() - 86_400_000).toISOString(), alreadyImported: false, git: { root: '/work/app' } }], scannedAt: new Date().toISOString() };
      if (method === 'agentSessions.import') return { importedCount: 4, skippedCount: 0 };
      return {};
    },
    restAccess: () => ({ http: async () => ({ snapshotSequence: 2, projects: [{ id: 'project-1', title: 'app', workspaceRoot: '/work/app' }], threads: [] }) }),
  } as unknown as T3Client;
  const native = fakeNative([]);
  const connect = await welcomeView(client, native, { step: 'connect', now: Date.now() });
  expect(connect.show).toBe(true);
  // No local server runs (the hosted rules): "Add a computer" starts open (expandPairingInitially).
  expect(connect.pairStartsOpen).toBe(true);
  expect(welcomeShowing(client)).toBe(true);
  expect(connect.computers).toEqual([{ key: connect.computers[0]!.key, environmentId: 'env-1', label: 'Morgan’s MacBook Pro', url: 'http://127.0.0.1:14807/', status: 'Connected', selected: true }]);
  expect(connect.ready).toBe(true);
  await welcomeLocal(client, native, 'select', connect.computers[0]!.key, '');
  expect((await welcomeView(client, native, { step: 'connect', now: 0 })).ready).toBe(false);
  await welcomeLocal(client, native, 'select', connect.computers[0]!.key, '');
  await welcomeLocal(client, native, 'setup', '', '');
  const agents = await welcomeView(client, native, { step: 'agents', now: 0 });
  expect(agents.machines.map(machine => [machine.label, machine.agents.length, machine.chatgpt])).toEqual([['Morgan’s MacBook Pro', 3, true]]);
  const projects = await welcomeView(client, native, { step: 'import', now: Date.now() });
  expect([projects.candidateCount, projects.selectedCount, projects.selectionLabel, projects.importLabel]).toEqual([1, 1, '1 of 1 selected', 'Import 1 project']);
  // One computer: its group has no legend.
  expect([projects.legend, projects.scans.map(scan => [scan.label, scan.empty, scan.candidates.length])]).toEqual([false, [['Morgan’s MacBook Pro', false, 1]]]);
  await importProjects(client, native);
  expect(calls.map(call => call.method)).toEqual(['agentSessions.scan', 'projects.mutate', 'agentSessions.import']);
  expect(calls[1]!.payload).toMatchObject({ type: 'project.create', projectId: 'project-1', title: 'app', workspaceRoot: '/work/app', createWorkspaceRootIfMissing: false });
  expect(client.projectId).toBe('project-1');
  expect(pagesPrefs(client).onboardingCompletedAt).toMatch(/^\d{4}-\d{2}-\d{2}T/);
  expect((await welcomeView(client, native, { step: 'import', now: 0 })).show).toBe(false);
});

test('a client that already has an environment never sees the wizard and records setup as done', async () => {
  const client = { local: {}, origin: '', environmentId: '', connection: 'disconnected', statusMessage: '', scopes: [], config: {}, shell: { projects: [], threads: [], sequence: 0 } } as unknown as T3Client;
  const view = await welcomeView(client, fakeNative([{ origin: 'http://127.0.0.1:1', environmentId: 'e' }]), { step: 'connect', now: Date.parse('2026-10-04T00:00:00.000Z') });
  expect(view.show).toBe(false);
  expect(pagesPrefs(client).onboardingCompletedAt).toBe('2026-10-04T00:00:00.000Z');
});

test('agent setup buttons open a terminal without Enter and leaving the agents step cleans it up', async () => {
  const calls: { method: string; payload: Obj }[] = [];
  const client = {
    local: { deviceSettings: { appearanceMode: 'system' } }, origin: 'http://127.0.0.1:14807', environmentId: 'setup-env', connection: 'connected', statusMessage: '', scopes: ['terminal:operate'],
    config: { cwd: '/tmp', environment: { platform: { os: 'darwin' } }, settings: { providerInstances: { codex: { config: { setupMode: 'existing' } } } }, providers: [{ driver: 'codex', instanceId: 'codex', installed: false, status: 'error', auth: { status: 'unauthenticated' } }] },
    shell: { projects: [], threads: [], sequence: 0 }, ids: async () => ['fixture-id'],
    rpc: async (_native: Native, method: string, payload: Obj) => { calls.push({ method, payload }); return {}; },
  } as unknown as T3Client;
  const native = fakeNative([]);
  const connect = await welcomeView(client, native, { step: 'connect', now: Date.now() });
  await welcomeLocal(client, native, 'setup', '', '');
  const agents = await welcomeView(client, native, { step: 'agents', now: 0 });
  const card = agents.machines[0]!.agents[0]!;
  expect(card.kind).toBe('card'); expect(card.terminalAvailable).toBe(true);
  await welcomeLocal(client, native, 'terminal-open', connect.computers[0]!.key, card.key);
  const opened = await welcomeView(client, native, { step: 'agents', now: 0 });
  expect(opened.machines[0]!.terminal).toMatchObject({ ready: true, terminalId: 'onboarding-codex-fixture-id' });
  expect(calls[1]!.payload.data).toBe('curl -fsSL https://chatgpt.com/codex/install.sh | sh');
  await welcomeLocal(client, native, 'terminal-page', 'setup-env', JSON.stringify({ type: 'exited', terminalId: 'old-session' }));
  expect(calls).toHaveLength(2);
  await welcomeView(client, native, { step: 'connect', now: 0 });
  expect(calls[2]).toEqual({ method: 'terminal.close', payload: { threadId: 'onboarding-agent-setup', terminalId: 'onboarding-codex-fixture-id', deleteHistory: true } });
});

// #298: after a ChatGPT sign-in on the welcome, the projects step re-subscribed
// the Codex row's sign-in streams on every answer (~224 provider.auth.subscribe in 20 s). The reference mounts
// OnboardingCodexSetup on the agents step only (WelcomeWizard.tsx:269-281).
test('the agents step holds one sign-in subscription per stream, and the projects step holds none', async () => {
  const ops: Obj[] = [];
  const client = {
    local: { deviceSettings: { appearanceMode: 'system' } }, origin: 'http://127.0.0.1:16250', environmentId: 'env-lane', connection: 'connected', statusMessage: '', scopes: [],
    ready: true, writable: true, generation: 1,
    config: { environment: { label: 'Lane Mac' }, settings: { providers: {}, providerInstances: { codex: { driver: 'codex', enabled: true, config: { setupMode: 'managed' } } } },
      providers: [{ instanceId: 'codex', driver: 'codex', displayName: 'Codex', enabled: true, installed: true, status: 'ready', auth: { status: 'authenticated', type: 'chatgpt' }, setup: { canInstall: true, canAuthenticate: true } }] },
    shell: { projects: [], threads: [], sequence: 0 }, threadId: '', projectId: '',
    rpc: async () => ({ candidates: [], scannedAt: new Date().toISOString() }),
  } as unknown as T3Client;
  let serial = 0;
  const native = { available: true, watch: () => {}, later: async (request: Obj) => {
    ops.push(request);
    if (request.op === 'environments') return { ok: true, value: { saved: [] }, generation: 1 };
    if (request.op === 'subscribe') return { ok: true, value: { id: `sub-${++serial}` }, generation: 1 };
    return { ok: true, value: {}, generation: 1 };
  } } as unknown as Native;
  const keys = (op: string) => ops.filter(request => request.op === op).map(request => String(request.key));
  await welcomeView(client, native, { step: 'connect', now: Date.now() });
  await welcomeLocal(client, native, 'setup', '', '');
  for (let answer = 0; answer < 3; answer++) await welcomeView(client, native, { step: 'agents', now: 0 });
  expect(keys('subscribe')).toEqual(['provider-auth:codex', 'provider-install:codex']);
  // Continue to Projects: the rows unmount once; every later answer (each stream event asks again) subscribes nothing.
  for (let answer = 0; answer < 3; answer++) await welcomeView(client, native, { step: 'import', now: Date.now() });
  expect(keys('subscribe')).toEqual(['provider-auth:codex', 'provider-install:codex']);
  expect(keys('unsubscribe')).toEqual(['provider-auth:codex', 'provider-install:codex']);
});

test('project paths keep sibling checkout names visible on Unix and Windows', () => {
  expect(splitOnboardingProjectPath('/Users/me/Projects/app/')).toEqual({ name: 'app', parent: '/Users/me/Projects/' });
  expect(splitOnboardingProjectPath('C:\\Users\\me\\app\\')).toEqual({ name: 'app', parent: 'C:\\Users\\me\\' });
  for (const path of ['/', 'C:\\', '//server/share/']) expect(splitOnboardingProjectPath(path)).toEqual({ name: path, parent: '' });
});

// ImportStep: one fieldset per computer set up, each with its own empty or error state, and a legend only when
// there are several (WelcomeWizard.tsx:1444-1490); a candidate is imported on its own computer.
describe('the projects step with two computers', () => {
  const SECOND = environmentKey('https://second.example', 'env-b');
  afterEach(() => { fleet.entries.delete(SECOND); fleet.saved = []; });

  function twoComputers(secondScan: { ok: boolean; value?: Obj; message?: string }) {
    const focusedCalls: string[] = [], secondCalls: Obj[] = [];
    fleet.entries.set(SECOND, { key: SECOND, origin: 'https://second.example', environmentId: 'env-b', phase: 'connected', message: '', traceId: '',
      generation: 2, synchronized: 2, lastEvent: 0, subscriptions: {}, config: { environment: { environmentId: 'env-b', label: 'Lane Second' } },
      shell: initialShell(), scopes: [], error: '', requested: true });
    const client = {
      local: { deviceSettings: { appearanceMode: 'system' } }, origin: 'http://127.0.0.1:14807', environmentId: 'env-1', connection: 'connected', statusMessage: '', scopes: [],
      config: { environment: { label: 'This machine' } }, shell: { projects: [] as Obj[], threads: [], sequence: 0 }, threadId: '', projectId: '',
      rpc: async (_native: Native, method: string) => { focusedCalls.push(method); return method === 'agentSessions.scan' ? { candidates: [] } : {}; },
      restAccess: () => ({ http: async () => ({ snapshotSequence: 2, projects: [], threads: [] }) }),
    } as unknown as T3Client;
    const saved = [{ origin: 'https://second.example', environmentId: 'env-b', label: 'Lane Second', enabled: true }];
    let listed: Obj[] = [];
    const native = { available: true, watch: () => {}, later: async (request: Obj) => {
      if (request.op === 'environments') return { ok: true, value: { saved: listed }, generation: 1 };
      if (request.op === 'ids') return { ok: true, value: ['command-1', 'project-b'], generation: 1 };
      if (request.op === 'request' && request.fleet === SECOND) {
        secondCalls.push({ method: request.method, payload: request.payload });
        if (request.method === 'agentSessions.scan') return secondScan.ok ? { ok: true, value: secondScan.value, generation: 2 } : { ok: false, error: { message: secondScan.message }, generation: 2 };
        if (request.method === 'agentSessions.import') return { ok: true, value: { importedCount: 2, skippedCount: 0 }, generation: 2 };
        return { ok: true, value: {}, generation: 2 };
      }
      return { ok: true, value: {}, generation: 1 };
    } } as unknown as Native;
    // The setup reads the fleet's copy of the saved list, which a sync keeps.
    return { client, native, focusedCalls, secondCalls, pair: () => { listed = saved; fleet.saved = saved; } };
  }

  test('each computer is its own labelled group with its own empty or error state', async () => {
    const { client, native, pair } = twoComputers({ ok: false, message: 'The environment is not connected.' });
    expect((await welcomeView(client, native, { step: 'connect', now: Date.now() })).show).toBe(true);
    pair();
    const connect = await welcomeView(client, native, { step: 'connect', now: Date.now() });
    expect(connect.computers.map(computer => [computer.label, computer.selected])).toEqual([['This machine', true], ['Lane Second', true]]);
    await welcomeLocal(client, native, 'setup', '', '');
    const view = await welcomeView(client, native, { step: 'import', now: Date.now() });
    expect(view.legend).toBe(true);
    expect(view.scans.map(scan => [scan.label, scan.empty, scan.error])).toEqual([['This machine', true, ''], ['Lane Second', false, 'The environment is not connected.']]);
  });

  test("a candidate from the second computer is created and imported there, not on this connection", async () => {
    const scanned = { candidates: [{ path: '/lane/app', title: 'app', sources: ['codex'], threadCount: 1, lastActiveAt: new Date(Date.now() - 86_400_000).toISOString(), alreadyImported: false, git: { root: '/lane/app' } }] };
    const { client, native, pair, focusedCalls, secondCalls } = twoComputers({ ok: true, value: scanned });
    await welcomeView(client, native, { step: 'connect', now: Date.now() });
    pair();
    await welcomeView(client, native, { step: 'connect', now: Date.now() });
    await welcomeLocal(client, native, 'setup', '', '');
    const view = await welcomeView(client, native, { step: 'import', now: Date.now() });
    const second = view.scans[1]!;
    expect([second.label, second.candidates.map(candidate => candidate.title), view.selectedCount]).toEqual(['Lane Second', ['app'], 0]);
    await welcomeLocal(client, native, 'pick', second.candidates[0]!.key, '');
    await importProjects(client, native);
    expect(focusedCalls).toEqual(['agentSessions.scan']);
    expect(secondCalls.map(call => call.method)).toEqual(['agentSessions.scan', 'projects.mutate', 'agentSessions.import']);
    expect(secondCalls[1]!.payload).toMatchObject({ type: 'project.create', projectId: 'project-b', workspaceRoot: '/lane/app' });
    expect(secondCalls[2]!.payload).toEqual({ projectId: 'project-b', expectedWorkspaceRoot: '/lane/app' });
  });
});
