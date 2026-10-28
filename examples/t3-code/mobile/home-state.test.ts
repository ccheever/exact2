import { expect, test } from 'bun:test';
import { mobileHomeView, mobileHomeArrangeView } from './home-state';
import { mobileHomeAction, mobileHomeSnoozePresets } from './home-actions';
import { homeOrderState } from './home-order';
import { mobileNewTaskDraftList } from './mobile-new-task-drafts';
import { T3Client } from './shared/client';
import { EnvironmentFleet, type FleetEntry } from './shared/settings-b-fleet';
import { initialShell, obj, type Obj } from './shared/domain';
import { runThreadSearch } from './shared/sidebar-presentation';
import type { Native } from './shared/protocol';

const now = Date.parse('2026-10-07T12:00:00Z');
const config = { environment: { capabilities: { threadSettlement: true, threadSnooze: true } } };
const thread = { id: 'same', title: 'Thread', projectId: 'p', archivedAt: null, deletedAt: null, settledOverride: null, status: 'idle', latestRunId: null };
function fixture() {
  const client = new T3Client(), background = new EnvironmentFleet();
  Object.assign(client, { environmentId: 'one', origin: 'https://one.test', generation: 3, connection: 'connected', configLive: true, shellLive: true, shellLoaded: true, scopes: ['orchestration:operate'] });
  client.config = config; client.shell = { ...initialShell(), threads: [{ ...thread }] };
  const remote: FleetEntry = { key: 'remote', environmentId: 'two', origin: 'https://two.test', generation: 8, synchronized: 8,
    phase: 'connected', message: '', traceId: '', lastEvent: 0, subscriptions: {}, config, shell: { ...initialShell(), threads: [{ ...thread }] }, scopes: ['orchestration:operate'], error: '', requested: true };
  background.entries.set('remote', remote);
  const view = (home = true, sidebar = false, route = 'visit') => mobileHomeView([0, now, '', 10, true, false, false, false, false, '', '', '', 'repository', route, home, sidebar], client, background);
  return { client, background, remote, view };
}

test('background transport failures stop an empty Home from displaying successful loading', () => {
  const f = fixture();
  Object.assign(f.client, { connection: 'disconnected', shellLoaded: false, shellLive: false });
  f.client.shell = initialShell();
  Object.assign(f.remote, { phase: 'reconnecting', message: 'The remote host is unreachable.', error: '', synchronized: -1 });
  const view = f.view();
  expect(view.loading).toBe(false); expect(view.emptyTitle).not.toBe('Connecting to environment');
});

test('Home context JSON carries the exact focused/background identity and the existing menu data', () => {
  const f = fixture(), rows = f.view().items.filter(item => item.kind === 'thread');
  expect(rows).toHaveLength(2);
  for (const item of rows) {
    const context = JSON.parse(item.nativeMenu), focused = item.environmentId === 'one';
    expect(context).toEqual({ identity: `${item.environmentId}:same`, requestRoute: 'visit', enabled: false, items: item.menuItems, swipeItems: item.swipe.snoozeItems, swipeSnoozable: item.swipe.snoozable, swipeResetKey: item.swipe.resetKey,
      environmentId: item.environmentId, threadId: 'same', origin: focused ? 'https://one.test' : 'https://two.test', generation: focused ? 3 : 8,
      connected: true, homeVisible: true, sidebarVisible: false });
    expect(context.items.find((entry: { id: string }) => entry.id === 'snooze:custom').disabled).toBe(false);
  }
});

test('Home context follows root route/visibility and endpoint replacement without creating a second owner', () => {
  const f = fixture();
  expect(JSON.parse(f.view(false, true, 'next').items.find(item => item.key === 'two:same')!.nativeMenu)).toMatchObject({ requestRoute: 'next', enabled: true, homeVisible: false, sidebarVisible: true });
  f.client.origin = 'https://new-one.test'; f.client.generation = 4; f.client.connection = 'reconnecting';
  const context = JSON.parse(f.view().items.find(item => item.key === 'one:same')!.nativeMenu);
  expect(context).toMatchObject({ origin: 'https://new-one.test', generation: 4, connected: false, items: [] });
  f.remote.scopes = [];
  expect(JSON.parse(f.view().items.find(item => item.key === 'two:same')!.nativeMenu).items).toEqual([]);
  f.remote.phase = 'reconnecting'; expect(f.view().items.some(item => item.key === 'two:same')).toBe(false);
});

test('swipe Custom context uses explicit admission while settled and legacy menus remain unchanged', () => {
  const f = fixture(); f.client.shell.threads[0]!.settledOverride = 'settled';
  f.remote.config = { environment: { capabilities: { threadSettlement: false, threadSnooze: true } } };
  const args = [0, now, '', 10, true, false, true, true, true, '', '', '', 'repository', 'visit', true, true];
  const rows = mobileHomeView(args, f.client, f.background).items.filter(item => item.kind === 'thread');
  expect(rows).toHaveLength(2);
  for (const row of rows) {
    expect(row.menuItems.some(item => item.id === 'snooze:custom')).toBe(false);
    expect(row.swipe.snoozable).toBe(true); expect(row.swipe.snoozeItems.at(-1)?.operation).toBe('swipe:snooze:custom');
    expect(JSON.parse(row.nativeMenu)).toMatchObject({ swipeSnoozable: true, swipeResetKey: row.swipe.resetKey });
  }
  expect(rows.find(item => item.environmentId === 'one')?.swipe.primary).toBe('unsettle');
  expect(rows.find(item => item.environmentId === 'two')?.swipe.primary).toBe('archive');
});

test('swipe refresh projects one earliest future guard deadline and excludes past preparing expiries', () => {
  const f = fixture(); f.client.shell.threads[0]!.latestUserMessageAt = new Date(now).toISOString();
  f.remote.shell.threads[0]!.latestUserMessageAt = new Date(now + 1000).toISOString();
  expect(f.view().nextSwipeRefreshAt).toBe(now + 120050);
  f.client.shell.threads[0]!.pendingRuntimeRequest = { kind: 'approval' };
  expect(f.view().nextSwipeRefreshAt).toBe(now + 121050);
  Object.assign(f.remote.shell.threads[0]!, { status: 'preparing', latestRunId: 'r', latestUserMessageAt: new Date(now - 120001).toISOString() });
  expect(f.view().nextSwipeRefreshAt).toBe(now + 49);
  f.remote.shell.threads[0]!.latestUserMessageAt = new Date(now - 120050).toISOString();
  expect(f.view().nextSwipeRefreshAt).toBe(0);
  f.remote.shell.threads[0]!.latestUserMessageAt = null; expect(f.view().nextSwipeRefreshAt).toBe(0);
});

test('collapsed and filtered-out rows do not create swipe refresh deadlines', () => {
  const f = fixture(); f.client.shell.threads[0]!.latestUserMessageAt = new Date(now).toISOString();
  f.client.shell.threads[0]!.settledOverride = 'settled';
  expect(f.view().nextSwipeRefreshAt).toBe(0);
  const args = [0, now, 'no matching title', 10, true, false, true, true, true, '', '', '', 'repository', 'visit', true, true];
  expect(mobileHomeView(args, f.client, f.background).nextSwipeRefreshAt).toBe(0);
});

test('two Home projections read local drafts without mutation and sidebar menu needs no live endpoint', () => {
  const f = fixture(); f.client.shell.threads = []; f.remote.shell.threads = [];
  Object.assign(f.client.local, { mobileNewTaskDrafts: { version: 1, records: { 'new-task:A': { key: 'new-task:A', environmentId: 'offline', projectId: 'gone', origin: 'https://old.test', createdAt: '2026-10-08T00:00:00Z', revision: 4, choices: null } }, receipts: {}, claims: {}, fileReleases: [] } });
  f.client.local.drafts['new-task:A'] = 'Offline note';
  const args = [0, now, '', 10, true, false, false, false, false, '', '', '', 'repository', 'visit', true, true,
    [{ environmentId: 'offline', label: 'Saved Mac', machineSymbol: 'laptopcomputer' }, { environmentId: 'one', label: 'Other', machineSymbol: 'server.rack' }]];
  const before = JSON.stringify(f.client.local), first = mobileHomeView(args, f.client, f.background), second = mobileHomeView(args, f.client, f.background);
  expect(first.items).toEqual(second.items); expect(JSON.stringify(f.client.local)).toBe(before);
  const draft = first.items[0]!; expect(draft.environmentLabel).toBe('Saved Mac'); expect(first.nextSwipeRefreshAt).toBe(0);
  expect(JSON.parse(draft.nativeMenu)).toEqual({ identity: 'draft-task:new-task:A', requestRoute: 'visit', enabled: true, items: draft.menuItems });
  expect(draft.swipe.primary).toBe('');
  expect(mobileHomeArrangeView(true, now, false, f.client, f.background)).toMatchObject({ pinned: [], active: [] });
  expect(first).not.toHaveProperty('arrangement');
});

function emptyView(client: T3Client, background: EnvironmentFleet, input: {
  surface?: string; query?: string; environment?: string; project?: string; catalogLoaded?: boolean;
} = {}) {
  return mobileHomeView([0, now, input.query ?? '', 10, true, false, false, false, false,
    input.environment ?? '', input.project ?? '', '', 'repository', 'empty-visit', true, true, [],
    input.surface ?? 'sidebar', input.catalogLoaded ?? true], client, background);
}

test('unpaired iPad sidebar uses a list label while compact Home keeps the environment card', () => {
  const client = new T3Client(), background = new EnvironmentFleet();
  expect(emptyView(client, background)).toMatchObject({ items: [], emptyTitle: 'No threads yet', emptyDetail: '', addEnvironment: false, loading: false });
  expect(emptyView(client, background, { surface: 'compact' })).toMatchObject({
    emptyTitle: 'No environments connected', emptyDetail: 'Add an environment to load projects and start coding sessions.', addEnvironment: true,
  });
  expect(emptyView(client, background, { catalogLoaded: false, query: 'needle', project: 'gone' })).toMatchObject({
    emptyTitle: 'Loading threads…', emptyDetail: '', addEnvironment: false, loading: false,
  });
  expect(emptyView(client, background, { query: 'needle' }).emptyTitle).toBe('No matching threads');
});

test('compact Home checks the unknown catalog before showing a settled empty catalog', () => {
  const client = new T3Client(), background = new EnvironmentFleet();
  expect(emptyView(client, background, { surface: 'compact', catalogLoaded: false })).toMatchObject({
    items: [], emptyTitle: 'Loading environments', emptyDetail: 'Checking saved environments on this device.',
    loading: true, addEnvironment: true, fullPageEmpty: true,
  });
  expect(emptyView(client, background, { surface: 'compact', catalogLoaded: true })).toMatchObject({
    items: [], emptyTitle: 'No environments connected', emptyDetail: 'Add an environment to load projects and start coding sessions.',
    loading: false, addEnvironment: true, fullPageEmpty: true,
  });
});

test('only an unfiltered empty catalogue is full-page Home; filtered and sidebar empties stay in the list', () => {
  const client = new T3Client(), background = new EnvironmentFleet();
  expect(emptyView(client, background, { surface: 'compact' }).fullPageEmpty).toBe(true);
  expect(emptyView(client, background, { surface: 'compact', catalogLoaded: false }).fullPageEmpty).toBe(true);
  expect(emptyView(client, background).fullPageEmpty).toBe(false);
  const f = fixture();
  expect(emptyView(f.client, f.background, { surface: 'compact', query: 'absent' })).toMatchObject({ items: [], emptyTitle: 'No results', fullPageEmpty: false });
  f.remote.shell.threads = [];
  expect(emptyView(f.client, f.background, { surface: 'compact', environment: 'two' })).toMatchObject({ items: [], emptyTitle: 'No threads in this environment', fullPageEmpty: false });
  expect(emptyView(f.client, f.background, { surface: 'compact' }).fullPageEmpty).toBe(false);
});

test('sidebar empty text uses selected logical project and query before environment scope', () => {
  const f = fixture(); f.client.shell.threads = []; f.remote.shell.threads = [];
  f.client.shell.projects = [{ id: 'p', title: 'Local', workspaceRoot: '/one', repositoryIdentity: { canonicalKey: 'shared-repo', displayName: 'team/repo' } }];
  f.remote.shell.projects = [{ id: 'p', title: 'Remote', workspaceRoot: '/two', repositoryIdentity: { canonicalKey: 'shared-repo', displayName: 'team/repo' } }];
  expect(emptyView(f.client, f.background, { project: 'two:p' }).emptyTitle).toBe('No threads in team/repo');
  expect(emptyView(f.client, f.background, { environment: 'two' }).emptyTitle).toBe('No threads yet');
  expect(emptyView(f.client, f.background, { project: 'removed' }).emptyTitle).toBe('No threads yet');
  expect(emptyView(f.client, f.background, { query: 'x', project: 'two:p' }).emptyTitle).toBe('No matching threads');
  expect(emptyView(f.client, f.background, { surface: 'compact' })).toMatchObject({
    emptyTitle: 'No threads yet', emptyDetail: 'Create a task to start a new coding runtime in one of your connected projects.',
  });
});

test('sidebar uses the existing pending search only for the displayed query and a connected scope', async () => {
  const f = fixture(); f.client.shell.threads = []; f.remote.shell.threads = []; f.client.query = 'needle';
  let finish!: (value: unknown) => void;
  const native: Native = { available: true, watch() {}, later: async () => await new Promise(resolve => { finish = resolve; }) };
  const pending = runThreadSearch(f.client, native, 'needle');
  expect(emptyView(f.client, f.background, { query: 'needle' }).emptyTitle).toBe('Searching thread messages…');
  expect(emptyView(f.client, f.background, { query: 'x' }).emptyTitle).toBe('No matching threads');
  f.remote.phase = 'offline';
  expect(emptyView(f.client, f.background, { query: 'needle', environment: 'two' }).emptyTitle).toBe('No matching threads');
  finish({ ok: true, generation: f.client.generation, value: { matches: [] } }); await pending;
  expect(emptyView(f.client, f.background, { query: 'needle' }).emptyTitle).toBe('No matching threads');
});

test('sidebar empty presentation does not replace visible draft or thread rows', () => {
  const f = fixture(), before = JSON.stringify([f.client.shell, f.remote.shell]);
  const compact = emptyView(f.client, f.background, { surface: 'compact' }), sidebar = emptyView(f.client, f.background);
  expect(sidebar.items).toEqual(compact.items); expect(sidebar.emptyTitle).toBe('');
  expect(sidebar.emptyDetail).toBe(''); expect(sidebar.addEnvironment).toBe(false);
  expect(JSON.stringify([f.client.shell, f.remote.shell])).toBe(before);
});

test('failed dismissal recovery survives filtered projections and changes only the captured logical row in both lists', async () => {
  const f = fixture(); f.client.shell.threads.push({ ...thread, id: 'unrelated' });
  const project = (surface: string, query = '') => mobileHomeView([0, now, query, 10, true, false, false, true, true,
    '', '', '', 'repository', 'visit', true, true, [], surface, true], f.client, f.background);
  const before = new Map(['compact', 'sidebar'].map(surface => [surface, project(surface)]));
  const first = before.get('compact')!.items.find(row => row.key === 'one:same')!;
  expect(first.dismissalIdentity).toBe(encodeURIComponent(JSON.stringify(['visit', 'one', 'same', 'https://one.test', 3, first.swipe.resetKey])));
  let release!: () => void, started!: () => void;
  const begun = new Promise<void>(resolve => { started = resolve; });
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input);
    if (request.op === 'mobileHomeDismissal' && request.phase === 'begin') {
      started(); await new Promise<void>(resolve => { release = resolve; });
      return { ok: true, generation: 3, value: { token: 'recovery-capture', completed: true } };
    }
    return request.method === 'orchestration.dispatchCommand'
      ? { ok: false, generation: 3, error: { kind: 'transport', message: 'Synthetic refusal' } }
      : { ok: true, generation: 3, value: request.path === '/api/auth/session' ? { authenticated: true, permissions: ['orchestration:operate'] }
        : request.op === 'ids' ? ['recovery-command'] : request.op === 'mobileHomeDismissal' && request.phase === 'prepare' ? { token: 'recovery-capture' } : {} };
  } };
  const snapshot = JSON.stringify([f.client.shell, f.remote.shell, f.client.local]);
  const pending = mobileHomeAction('visit', 'one', 'same', 'settle', '', now, native, f.client, f.background);
  try {
    await begun;
    for (const surface of ['compact', 'sidebar']) expect(project(surface, 'no matching title').items).toEqual([]);
    release(); expect((await pending).message).toBe('Synthetic refusal');
    for (const surface of ['compact', 'sidebar']) {
      const after = project(surface), original = before.get(surface)!;
      expect(after.items.map(({ dismissalIdentity, ...row }) => row)).toEqual(original.items.map(({ dismissalIdentity, ...row }) => row));
      for (const row of after.items) {
        const previous = original.items.find(value => value.key === row.key)!;
        if (row.key === 'one:same') expect(row.dismissalIdentity).not.toBe(previous.dismissalIdentity);
        else expect(row.dismissalIdentity).toBe(previous.dismissalIdentity);
      }
      for (let count = 0; count < 3; count++) expect(project(surface)).toEqual(after);
    }
    expect(JSON.stringify([f.client.shell, f.remote.shell, f.client.local])).toBe(snapshot);
  } finally { release?.(); await pending.catch(() => undefined); }
});

test('Home row action projection resolves catalogue entries in linear work', () => {
  const measurements: { threads: number; operable: boolean; workingEnabled: boolean; identityReads: number;
    timeFormats: number; dateFormats: number; plainMs: number; countedMs: number; outputSHA256: string }[] = [];
  let measuring = false, identityReads = 0, timeFormats = 0, dateFormats = 0;
  const timeFormat = Date.prototype.toLocaleTimeString, dateFormat = Date.prototype.toLocaleDateString;
  Date.prototype.toLocaleTimeString = function (...args) { if (measuring) timeFormats++; return timeFormat.apply(this, args); };
  Date.prototype.toLocaleDateString = function (...args) { if (measuring) dateFormats++; return dateFormat.apply(this, args); };
  try {
    measuring = true; mobileHomeSnoozePresets(now); measuring = false;
    const onePresetFormatting = { timeFormats, dateFormats };
    for (const operable of [false, true]) for (const workingEnabled of [false, true]) for (const count of [40, 200, 400, 741]) {
      const catalogue = () => {
        const f = fixture(); mobileNewTaskDraftList(f.client);
        const scopes = ['orchestration:read', 'diagnostics:read', 'terminal:read', 'filesystem:read'];
        if (operable) scopes.push('orchestration:operate');
        f.client.scopes = [...scopes]; f.remote.scopes = [...scopes];
        f.client.config = f.remote.config = { environment: { capabilities: {
          threadSettlement: true, threadSnooze: true, threadPinning: true, threadPinReorder: true, threadActiveReorder: true,
        } } };
        f.client.shell.threads = []; f.remote.shell.threads = [];
        for (const [environment, shell] of [['one', f.client.shell], ['two', f.remote.shell]] as const) {
          shell.projects = Array.from({ length: 20 }, (_, index) => ({
            id: `project-${index}`, title: `Project ${index}`, workspaceRoot: `/synthetic/${environment}/project-${index}`,
          }));
        }
        for (let index = 0; index < count; index++) {
          const state: Obj = index % 8 === 0 ? { pinnedAt: '2026-10-01T00:00:00Z', pinOrderKey: 'm' }
            : index % 8 === 1 ? { latestRunId: 'run', activeRunId: 'run', status: 'running' }
            : index % 8 === 2 ? { snoozedUntil: '2026-10-08T12:00:00Z' }
            : index % 8 === 3 ? { settledOverride: 'settled' }
            : index % 8 === 4 ? { pendingRuntimeRequest: { kind: 'approval' } }
            : index % 8 === 5 ? { pendingRuntimeRequest: { kind: 'user_input' } }
            : index % 8 === 6 ? { latestRunId: 'run', status: 'preparing', latestUserMessageAt: new Date(now).toISOString() } : {};
          const shell = index % 2 === 0 ? f.client.shell : f.remote.shell;
          shell.threads.push({ ...thread, id: `thread-${String(Math.floor(index / 2)).padStart(4, '0')}`,
            title: `Thread ${index}`, projectId: `project-${Math.floor(index / 2) % 20}`,
            createdAt: '2026-10-01T00:00:00Z', updatedAt: '2026-10-01T00:00:00Z', settledOverride: 'active', ...state });
        }
        return f;
      };
      const plain = catalogue(), counted = catalogue();
      const snapshot = (f: ReturnType<typeof fixture>) => JSON.stringify([f.client.shell, f.remote.shell, f.client.local,
        f.client.environmentId, f.client.projectId, f.client.threadId, f.client.origin, f.client.generation]);
      const before = snapshot(counted);
      for (const shell of [counted.client.shell, counted.remote.shell]) shell.threads = shell.threads.map(raw => new Proxy(raw, {
        get(target, property, receiver) {
          if (measuring && property === 'id') identityReads++;
          return Reflect.get(target, property, receiver);
        },
      }));
      const view = (f: ReturnType<typeof fixture>) => mobileHomeView([0, now, '', count, true, workingEnabled, true, true, true,
        '', '', '', 'repository', 'workload-visit', true, true], f.client, f.background);
      const start = performance.now(), expected = view(plain), plainMs = performance.now() - start;
      identityReads = timeFormats = dateFormats = 0; measuring = true;
      const countedStart = performance.now(), actual = view(counted), countedMs = performance.now() - countedStart;
      measuring = false;
      const rows = actual.items.filter(item => item.kind === 'thread');
      expect(actual).toEqual(expected);
      expect(new Set(rows.map(row => row.key))).toEqual(new Set([
        ...plain.client.shell.threads.map(row => `one:${row.id}`), ...plain.remote.shell.threads.map(row => `two:${row.id}`),
      ]));
      expect(rows).toHaveLength(count);
      const arrangement = mobileHomeArrangeView(true, now, workingEnabled, counted.client, counted.background);
      expect((['pinned', 'active', 'snoozed', 'settled'] as const).flatMap(section => arrangement[section]).length).toBe(count);
      expect(arrangement.workingEnabled).toBe(workingEnabled);
      const rank = { pinned: 0, active: 1, working: 2, snoozed: 3, settled: 4 };
      expect(rows.every((row, index) => index === 0 || rank[rows[index - 1]!.section] <= rank[row.section])).toBe(true);
      expect(rows.some(row => row.section === 'working')).toBe(workingEnabled);
      for (const row of rows) {
        const focused = row.environmentId === 'one';
        expect(JSON.parse(row.nativeMenu)).toEqual({ identity: row.key, requestRoute: 'workload-visit', enabled: true,
          items: row.menuItems, swipeItems: row.swipe.snoozeItems, swipeSnoozable: row.swipe.snoozable, swipeResetKey: row.swipe.resetKey,
          environmentId: row.environmentId, threadId: row.threadId, origin: focused ? 'https://one.test' : 'https://two.test',
          generation: focused ? 3 : 8, connected: true, homeVisible: true, sidebarVisible: true });
      }
      expect(rows.every(row => operable ? row.menuItems.some(item => item.id === 'copy-thread-id') && !!row.swipe.primary
        : row.menuItems.length === 0 && row.swipe.primary === '' && !row.swipe.snoozable)).toBe(true);
      expect(rows.some(row => row.swipe.snoozable)).toBe(operable);
      expect(actual.nextSwipeRefreshAt).toBe(operable ? now + 120050 : 0);
      expect(snapshot(counted)).toBe(before); expect(homeOrderState(counted.client)).toEqual(homeOrderState(plain.client));
      const outputSHA256 = new Bun.CryptoHasher('sha256').update(JSON.stringify(actual)).digest('hex');
      measurements.push({ threads: count, operable, workingEnabled, identityReads, timeFormats, dateFormats, plainMs, countedMs, outputSHA256 });
    }
    console.info('Home synthetic catalogue workload', JSON.stringify(measurements));
    // Count raw identity reads across every traversal method, including spreads.
    // Independent order/row/Arrange passes have a linear allowance, never a timer.
    for (const measurement of measurements) {
      expect(measurement.identityReads).toBeLessThanOrEqual(32 * measurement.threads);
      expect(measurement.timeFormats).toBeLessThanOrEqual(onePresetFormatting.timeFormats);
      expect(measurement.dateFormats).toBeLessThanOrEqual(onePresetFormatting.dateFormats);
    }
  } finally {
    measuring = false; Date.prototype.toLocaleTimeString = timeFormat; Date.prototype.toLocaleDateString = dateFormat;
  }
}, 45_000);


test('closed Arrange reads no catalogue and reopens with current complete rows', () => {
  const unavailable = new Proxy(new T3Client(), { get() { throw new Error('Inactive Arrange read its client'); } });
  expect(mobileHomeArrangeView(false, now, true, unavailable)).toEqual({ version: '', blocked: true,
    workingEnabled: true, pinned: [], active: [], snoozed: [], settled: [] });
  const f = fixture();
  const first = mobileHomeArrangeView(true, now, false, f.client, f.background);
  expect(first.active.map(row => row.key)).toEqual(['one:same', 'two:same']);
  mobileHomeArrangeView(false, now + 1, false, f.client, f.background);
  f.client.shell.threads[0]!.settledOverride = 'settled';
  const reopened = mobileHomeArrangeView(true, now + 2, false, f.client, f.background);
  expect(reopened.active.map(row => row.key)).toEqual(['two:same']);
  expect(reopened.settled.map(row => row.key)).toEqual(['one:same']);
  expect(reopened.version).not.toBe(first.version);
});
