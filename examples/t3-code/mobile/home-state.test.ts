import { expect, test } from 'bun:test';
import { mobileHomeView } from './home-state';
import { T3Client } from './shared/client';
import { EnvironmentFleet, type FleetEntry } from './shared/settings-b-fleet';
import { initialShell } from './shared/domain';

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
  expect(draft.swipe.primary).toBe(''); expect(first.arrangement.pinned).toEqual([]); expect(first.arrangement.active).toEqual([]);
});
