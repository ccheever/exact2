// The non-Git composer strip and a No project draft switching machine.
// Ported from T3 Code (MIT, see LICENSE-T3): apps/web/src/components/BranchToolbar.logic.test.ts
// (shouldShowEnvironmentIndicator, shouldShowComposerContextStrip; original names) and
// packages/client-runtime/src/state/projectCommands.test.ts (openScratch; the store is this
// client's shell, read again until the project is in it).
import { afterEach, describe, expect, test } from 'bun:test';
import { primaryAt, resetPrimary } from '../support/local-primary-fixture';
afterEach(resetPrimary);
import type { T3Client } from '../../src/client/client';
import { obj, type Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { EnvironmentFleet, fleet, type FleetEntry } from '../../src/connections/settings-b-fleet';
import { shouldShowComposerContextStrip, shouldShowEnvironmentIndicator, gitlessStrip, noteStripLayout, resolveContextStripLabelsCompact, stripLabelsCompact, stripLabelTexts } from '../../src/composer/context-strip';
import { probeKey } from '../../src/composer/composer-menus';
import { composerBranches } from '../../src/composer/composer-controls-branch';
import { NO_RUN_ON } from '../../src/details/git-env';
import { connected } from '../support/composer-controls-fixture';
import { awaitScratchProject, machineChanging, openRemoteScratch, SCRATCH_NOT_LOADED } from '../../src/composer/scratch-machine';
import { setLayoutFrames } from '../support/layout-frames';
import { withTimers } from '../support/timers';

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

describe('shouldShowComposerContextStrip (v0.1.5 BranchToolbar.logic.test.ts)', () => {
  test('honors the active-thread preference', () => {
    const input = { isDraftHeroState: false, hasActiveProject: true, isGitRepo: true, showEnvironmentIndicator: true };
    expect(shouldShowComposerContextStrip({ ...input, persistInActiveThreads: false })).toBe(false);
    expect(shouldShowComposerContextStrip({ ...input, persistInActiveThreads: true })).toBe(true);
  });
  test('an active thread of a non-Git project needs the environment indicator', () => {
    const input = { isDraftHeroState: false, persistInActiveThreads: true, hasActiveProject: true, isGitRepo: false };
    expect(shouldShowComposerContextStrip({ ...input, showEnvironmentIndicator: true })).toBe(true);
    expect(shouldShowComposerContextStrip({ ...input, showEnvironmentIndicator: false })).toBe(false);
  });
  test('a new thread always shows the strip, which names its permissions', () => {
    for (const isGitRepo of [false, true]) for (const showEnvironmentIndicator of [false, true]) {
      expect(shouldShowComposerContextStrip({ isDraftHeroState: true, persistInActiveThreads: false, hasActiveProject: true, isGitRepo, showEnvironmentIndicator })).toBe(true);
    }
    expect(shouldShowComposerContextStrip({ isDraftHeroState: true, persistInActiveThreads: false, hasActiveProject: false, isGitRepo: true, showEnvironmentIndicator: true })).toBe(false);
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
  test('a draft on this device alone shows the strip without the machine selector (its Permissions only)', () => {
    primaryAt('http://127.0.0.1:1', 'a'); // this device: the primary environment
    const bare = { ...hidden, show: true, gitless: true, envShow: false };
    expect(gitlessStrip(plain('http://127.0.0.1:1'), hidden, false, new EnvironmentFleet())).toEqual(bare);
    const source = new EnvironmentFleet(), b = entry('b', { shell: { projects: [{ id: 'pb', workspaceRoot: '/notes' }], threads: [], sequence: 0 } as unknown as FleetEntry['shell'] });
    source.entries.set(b.key, b);
    // A path without a repository identity stays on its machine (deriveLogicalProjectKey): '/notes' on b is another project.
    expect(gitlessStrip(plain('http://127.0.0.1:1'), hidden, false, source)).toEqual(bare);
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
    expect(await composerBranches(client, native, false, '')).toMatchObject({ show: true, gitless: true, envShow: false }); // this device alone
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
      if (request.op === 'wake') { wakes.push(machineChanging(client)); return { ok: true, value: {}, generation: 0 }; }
      return { ok: true, value: { projectId: 'scratch' }, generation: 1 };
    } } as Native;
    expect(await openRemoteScratch(client, native, c)).toBe('scratch');
    expect(wakes).toEqual([true, false]);
    expect(machineChanging(client)).toBe(false);
  });
  test('fails with ScratchProjectNotLoadedError after 10 seconds without the project', async () => {
    const [error, waited] = await withTimers(async () => {
      const start = Date.now();
      return [await awaitScratchProject(() => false, async () => {}).catch((failure: unknown) => failure), Date.now() - start] as const;
    });
    expect((error as Error).message).toBe(SCRATCH_NOT_LOADED);
    expect(waited).toBe(10_000);
  });
});

describe('the strip labels give way to icons when they do not fit (useLabelsOverflow)', () => {
  test('resolveContextStripLabelsCompact expands again only with 16pt to spare', () => {
    expect(resolveContextStripLabelsCompact({ compact: false, neededWidth: 300, availableWidth: 300 })).toBe(false);
    expect(resolveContextStripLabelsCompact({ compact: false, neededWidth: 301, availableWidth: 300 })).toBe(true);
    expect(resolveContextStripLabelsCompact({ compact: true, neededWidth: 290, availableWidth: 300 })).toBe(true);
    expect(resolveContextStripLabelsCompact({ compact: true, neededWidth: 284, availableWidth: 300 })).toBe(false);
  });
  test('the strip\'s padding is not free space; a squeezed label counts at its natural width', () => {
    const client = { presentation: { anchors: {} } } as unknown as T3Client;
    const view = { show: true, gitless: false, envLocked: false, envShow: false, envLabel: 'Current checkout', envMachineLabel: '', branchLabel: 'feature/a-long-branch-name', prShow: false };
    noteStripLayout(client, view);
    expect(stripLabelTexts(client)).toEqual(['Current checkout', 'feature/a-long-branch-name']);
    // Unmeasured: labels stay.
    expect(stripLabelsCompact(client, 16)).toBe(false);
    // A control is its icons and padding (44pt and 60pt here) plus its label as laid out.
    const lay = (stripWidth: number, workspace: number, branch: number) => {
      setLayoutFrames(client, { strip: [0, 0, stripWidth, 32] });
      client.presentation = { anchors: {
        'sc:workspace': [8, 44 + workspace], 'sl:workspace': [40, workspace], [probeKey('Current checkout', 12, 400)]: [0, 96],
        'sc:branch': [200, 60 + branch], 'sl:branch': [220, branch], [probeKey('feature/a-long-branch-name', 12, 400)]: [0, 160] } };
      return stripLabelsCompact(client, 16);
    };
    // Controls 140 + 220 and a 4pt gap need 364; a 380pt strip's padding and border leave exactly that.
    expect(lay(380, 96, 160)).toBe(false);
    // The branch label squeezed to 120 of its 160 still needs its 160.
    expect(lay(376, 96, 120)).toBe(true);
    // Compact, the labels are gone; they come back only with 16pt to spare.
    expect(lay(390, 0, 0)).toBe(true);
    expect(lay(396, 0, 0)).toBe(false);
  });
});
