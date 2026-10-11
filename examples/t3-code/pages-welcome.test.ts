import { afterEach, beforeEach, expect, test } from 'bun:test';
import { noPrimary, resetPrimary } from './local-primary-fixture';
// These cases are the hosted rules (resolveHostedFirstRunDecision): no embedded server runs on this Mac.
beforeEach(noPrimary);
afterEach(resetPrimary);
import { hostedDecision, transition, agentRows, providerState, decodeCandidates, recentCandidates, welcomeView, welcomeLocal, importProjects, welcomeShowing } from './pages-welcome';
import { pagesPrefs } from './pages-prefs';
import type { T3Client } from './client';
import type { Native } from './protocol';
import type { Obj } from './domain';

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
    config: { ...fixtureConfig, environment: { label: 'Daehyeon’s MacBook Pro' } }, shell: { projects: [] as Obj[], threads: [], sequence: 0 }, threadId: '', projectId: '',
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
  expect(welcomeShowing(client)).toBe(true);
  expect(connect.computers).toEqual([{ key: connect.computers[0]!.key, environmentId: 'env-1', label: 'Daehyeon’s MacBook Pro', url: 'http://127.0.0.1:14807/', status: 'Connected', selected: true }]);
  expect(connect.ready).toBe(true);
  await welcomeLocal(client, native, 'select', connect.computers[0]!.key, '');
  expect((await welcomeView(client, native, { step: 'connect', now: 0 })).ready).toBe(false);
  await welcomeLocal(client, native, 'select', connect.computers[0]!.key, '');
  await welcomeLocal(client, native, 'setup', '', '');
  const agents = await welcomeView(client, native, { step: 'agents', now: 0 });
  expect(agents.machines.map(machine => [machine.label, machine.agents.length, machine.chatgpt])).toEqual([['Daehyeon’s MacBook Pro', 3, true]]);
  const projects = await welcomeView(client, native, { step: 'import', now: Date.now() });
  expect([projects.candidateCount, projects.selectedCount, projects.selectionLabel, projects.importLabel]).toEqual([1, 1, '1 of 1 selected', 'Import 1 project']);
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
  await welcomeLocal(client, native, 'terminal-exited', 'setup-env', JSON.stringify({ type: 'exited', terminalId: 'old-session' }));
  expect(calls).toHaveLength(2);
  await welcomeView(client, native, { step: 'connect', now: 0 });
  expect(calls[2]).toEqual({ method: 'terminal.close', payload: { threadId: 'onboarding-agent-setup', terminalId: 'onboarding-codex-fixture-id', deleteHistory: true } });
});

// fix-provider-auth-state, bug 18 (#298): after a ChatGPT sign-in on the welcome, the projects step re-subscribed
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
