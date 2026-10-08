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
    expect(context).toEqual({ identity: `${item.environmentId}:same`, requestRoute: 'visit', enabled: false, items: item.menuItems,
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
