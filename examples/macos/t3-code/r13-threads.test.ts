// Lane r13-threads: the non-Git composer strip (T5) and a No project draft switching machine (T1).
// Ported from T3 Code (MIT, see LICENSE-T3): apps/web/src/components/BranchToolbar.logic.test.ts
// (shouldShowEnvironmentIndicator, shouldShowComposerContextStrip; original names) and
// packages/client-runtime/src/state/projectCommands.test.ts (openScratch; the store is this
// client's shell, read again until the project is in it).
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import { obj, type Obj } from './domain';
import type { Native } from './protocol';
import { EnvironmentFleet, fleet, type FleetEntry } from './settings-b-fleet';
import { shouldShowComposerContextStrip, shouldShowEnvironmentIndicator, gitlessStrip } from './r12-threads-strip';
import { composerBranches } from './composer-controls-branch';
import { NO_RUN_ON } from './r4-git-env';
import { connected } from './composer-controls-fixture';
import { awaitScratchProject, machineChanging, openRemoteScratch, SCRATCH_NOT_LOADED } from './r12-threads-scratch';

describe('shouldShowEnvironmentIndicator', () => {
  test('shows the indicator whenever multiple environments are pickable', () => {
    expect(shouldShowEnvironmentIndicator({ activeEnvironment: { isPrimary: true }, canPickEnvironment: true })).toBe(true);
  });
  test('shows a sole remote environment so the user knows where the project runs', () => {
    expect(shouldShowEnvironmentIndicator({ activeEnvironment: { isPrimary: false }, canPickEnvironment: false })).toBe(true);
  });
  test('hides a sole primary (this-device) environment', () => {
    expect(shouldShowEnvironmentIndicator({ activeEnvironment: { isPrimary: true }, canPickEnvironment: false })).toBe(false);
  });
  test('hides the indicator when the active environment is unknown', () => {
    expect(shouldShowEnvironmentIndicator({ activeEnvironment: null, canPickEnvironment: false })).toBe(false);
  });
});

describe('shouldShowComposerContextStrip', () => {
  test.each([false, true])('honors the active-thread preference with resting controls %s', hostsRestingComposerControls => {
    const input = { isDraftHeroState: false, hasActiveProject: true, isGitRepo: true, showEnvironmentIndicator: true, hostsRestingComposerControls };
    expect(shouldShowComposerContextStrip({ ...input, persistInActiveThreads: false })).toBe(false);
    expect(shouldShowComposerContextStrip({ ...input, persistInActiveThreads: true })).toBe(true);
  });
  test('keeps the environment indicator visible for a non-Git project', () => {
    expect(shouldShowComposerContextStrip({ isDraftHeroState: true, persistInActiveThreads: false, hasActiveProject: true, isGitRepo: false,
      showEnvironmentIndicator: true, hostsRestingComposerControls: false })).toBe(true);
  });
  test('hides the strip when a non-Git project has nothing to show', () => {
    expect(shouldShowComposerContextStrip({ isDraftHeroState: true, persistInActiveThreads: false, hasActiveProject: true, isGitRepo: false,
      showEnvironmentIndicator: false, hostsRestingComposerControls: false })).toBe(false);
  });
  test('keeps the strip for visible resting composer controls in a non-Git thread', () => {
    expect(shouldShowComposerContextStrip({ isDraftHeroState: true, persistInActiveThreads: false, hasActiveProject: true, isGitRepo: false,
      showEnvironmentIndicator: false, hostsRestingComposerControls: true })).toBe(true);
  });
  test('shows Git controls without requiring an environment indicator', () => {
    expect(shouldShowComposerContextStrip({ isDraftHeroState: true, persistInActiveThreads: false, hasActiveProject: true, isGitRepo: true,
      showEnvironmentIndicator: false, hostsRestingComposerControls: false })).toBe(true);
  });
});

const scratchRoot = (machine: string) => `/Users/${machine}/.t3/scratch`;
const entry = (id: string, extra: Partial<FleetEntry> = {}): FleetEntry => ({
  key: `https://${id}.example.invalid\n${id}`, origin: `https://${id}.example.invalid`, environmentId: id, phase: 'connected', message: '', traceId: '',
  generation: 1, synchronized: 1, lastEvent: 0, subscriptions: {}, config: { environment: { label: `Box ${id}` }, scratchWorkspaceRoot: scratchRoot(id) },
  shell: { projects: [], threads: [], sequence: 0 } as unknown as FleetEntry['shell'], scopes: [], error: '', requested: true, ...extra });
const hidden = { show: false, gitless: false, envLocked: false, ...NO_RUN_ON };
function plain(origin: string, threadId = ''): T3Client {
  return { environmentId: 'a', origin, connection: 'connected', projectId: 'pa', threadId, draftKey: threadId ? `a:${threadId}` : 'a:new:pa', presentation: {},
    config: { environment: { label: 'Local' }, scratchWorkspaceRoot: scratchRoot('a') },
    shell: { projects: [{ id: 'pa', title: 'notes', workspaceRoot: '/notes' }], threads: [] },
    local: { groupingMode: 'repository', groupingOverrides: {}, drafts: {}, selections: {}, composerControls: { contexts: {} } } } as unknown as T3Client;
}

describe('the non-Git strip shows only where it runs (BranchToolbar.tsx, BranchToolbarEnvironmentSelector)', () => {
  test('a draft on its only, remote machine shows the Select with that machine', () => {
    const strip = gitlessStrip(plain('https://far.example.invalid'), hidden, false, new EnvironmentFleet());
    expect(strip).toMatchObject({ show: true, gitless: true, envShow: true, envLocked: false, envMachineLabel: 'Local' });
    expect(strip.envOptions.map(option => [option.id, option.selected])).toEqual([['a', true]]);
  });
  test('a draft on this device alone has nothing to show; a non-Git project on two machines does', () => {
    expect(gitlessStrip(plain('http://127.0.0.1:1'), hidden, false, new EnvironmentFleet())).toBe(hidden);
    const source = new EnvironmentFleet(), b = entry('b', { shell: { projects: [{ id: 'pb', workspaceRoot: '/notes' }], threads: [], sequence: 0 } as unknown as FleetEntry['shell'] });
    source.entries.set(b.key, b);
    // A path without a repository identity stays on its machine (deriveLogicalProjectKey): '/notes' on b is another project.
    expect(gitlessStrip(plain('http://127.0.0.1:1'), hidden, false, source)).toBe(hidden);
  });
  test('a started thread keeps the static machine row, and only when its strip persists', () => {
    expect(gitlessStrip(plain('https://far.example.invalid', 't1'), hidden, true, new EnvironmentFleet())).toMatchObject({ show: true, gitless: true, envLocked: true, envOptions: [] });
    expect(gitlessStrip(plain('https://far.example.invalid', 't1'), hidden, false, new EnvironmentFleet())).toBe(hidden);
  });

  test('a No project draft with another connected machine: the strip carries only "Run on"', async () => {
    const { client, native } = await connected();
    const later = native.later.bind(native);
    native.later = async (input: unknown) => {
      const request = obj(input);
      if (request.op === 'request' && request.method === 'vcs.refreshStatus') return native.ok({ isRepo: false });
      return later(input);
    };
    client.config.scratchWorkspaceRoot = '/repo'; // the fixture's project is this machine's "No project" folder
    expect(await composerBranches(client, native, false, '')).toMatchObject({ show: false, gitless: false }); // this device alone
    const b = entry('b');
    fleet.entries.set(b.key, b);
    try {
      const strip = await composerBranches(client, native, false, '');
      expect(strip).toMatchObject({ show: true, gitless: true, envShow: true, envLocked: false, branchLabel: '', prShow: false });
      expect(strip.envOptions.map(option => [option.id, option.label, option.selected, option.projectId])).toEqual([['env1', 'This device', true, 'p1'], ['b', 'Box b', false, '']]);
    } finally { fleet.entries.delete(b.key); }
  });
});

describe('openScratch', () => {
  test('resolves once the created project reaches the client store', async () => {
    const source = new EnvironmentFleet(), c = entry('c');
    source.entries.set(c.key, c);
    let reads = 0;
    const native = { available: true, watch() {}, later: async (input: unknown) => {
      const request = input as Obj;
      if (request.op === 'request' && request.method === 'projects.ensureScratch') return { ok: true, value: { projectId: 'scratch' }, generation: 1 };
      // The created project's event arrives on the second read of the store.
      if (request.op === 'http') return { ok: true, value: { snapshotSequence: ++reads, projects: reads < 2 ? [] : [{ id: 'scratch', workspaceRoot: '/scratch' }], threads: [] }, generation: 1 };
      return { ok: true, value: {}, generation: 1 };
    } } as Native;
    expect(await openRemoteScratch({} as T3Client, native, c)).toBe('scratch');
    expect(c.shell.projects.map(project => project.id)).toEqual(['scratch']);
  });
  test('Send reads "Preparing machine" while the machine is prepared: the readers wake at the start and the end', async () => {
    const source = new EnvironmentFleet(), c = entry('c', { shell: { projects: [{ id: 'scratch', workspaceRoot: scratchRoot('c') }], threads: [], sequence: 0 } as unknown as FleetEntry['shell'] });
    source.entries.set(c.key, c);
    const client = {} as T3Client, wakes: boolean[] = [];
    const native = { available: true, watch() {}, later: async (input: unknown) => {
      const request = input as Obj;
      if (request.op === 'r10Wake') { wakes.push(machineChanging(client)); return { ok: true, value: {}, generation: 0 }; }
      return { ok: true, value: { projectId: 'scratch' }, generation: 1 };
    } } as Native;
    expect(await openRemoteScratch(client, native, c)).toBe('scratch');
    expect(wakes).toEqual([true, false]);
    expect(machineChanging(client)).toBe(false);
  });
  test('fails with ScratchProjectNotLoadedError after 10 seconds without the project', async () => {
    const slept: number[] = [];
    const native = { available: true, watch() {}, later: async (input: unknown) => { slept.push(Number((input as Obj).ms)); return { ok: true, value: {}, generation: 0 }; } } as Native;
    await expect(awaitScratchProject(native, () => false, async () => {})).rejects.toThrow(SCRATCH_NOT_LOADED);
    expect(slept.reduce((sum, ms) => sum + ms, 0)).toBe(10_000);
  });
});
