// T3 Code 1e2ecbd975 (MIT; see LICENSE-T3): a sidebar marks what is not on the primary environment as
// remote, whichever environment is focused. LegacySidebar.tsx:428 and Sidebar.tsx:1338 (`isRemote`,
// with `currentEnvironmentId={primaryEnvironmentId}` at :5173) mark a thread; sidebarProjectGrouping.ts:98-107
// marks a legacy project. Opening a background environment's thread focuses that environment here, so the
// focused environment's rows are remote then and the primary's are not.
import { afterEach, describe, expect, test } from 'bun:test';
import { T3Client } from './client';
import type { Obj } from './domain';
import { fleet, remoteThread, type FleetEntry } from './settings-b-fleet';
import { primary } from './local-primary';
import { parseLocalBackendStatus, unknownLocalBackend } from './local-backend';
import { sidebarSnapshot } from './sidebar-view';
import { legacySidebarSnapshot } from './legacy-sidebar-view';

const at = '2026-10-15T09:00:00.000Z', NOW = Date.parse('2026-10-15T10:00:00.000Z');
const identity = { projectIdentity: (name: string) => ({ projectMark: name.slice(0, 2), projectInk: 'ink', projectSurface: 'surface' }),
  providerBadge: () => ({ providerBadge: '', providerBadgeColor: '' }) };
const thread = (id: string, projectId: string): Obj => ({ id, projectId, title: `Thread ${id}`, status: 'idle', latestRunId: null, activeProviderThreadId: null,
  pendingRuntimeRequest: null, createdAt: at, updatedAt: at, archivedAt: null, settledOverride: null, lineage: { relationshipToParent: null } });

/** The primary ("This Mac", env-local on 16101) when `on`, else none (the Local environment switch off). */
function setPrimary(on: boolean) {
  primary.update(on ? parseLocalBackendStatus({ state: 'ready', enabled: true, port: 16101, httpBaseUrl: 'http://127.0.0.1:16101', wsBaseUrl: 'ws://127.0.0.1:16101',
    bearerReady: true, environmentId: 'env-local', label: 'This Mac' }) : unknownLocalBackend(), on);
}
/** A client focused on `focus`; the other environment is the fleet's synchronized background entry. */
function rig(focus: 'local' | 'second') {
  const local = { id: 'env-local', label: 'This Mac', origin: 'http://127.0.0.1:16101', project: { id: 'p1', title: 'mine', workspaceRoot: '/mac/mine', createdAt: at, updatedAt: at }, thread: thread('t1', 'p1'), machine: 'laptop' };
  const second = { id: 'env-second', label: 'Lane Second', origin: 'http://127.0.0.1:16710', project: { id: 'p9', title: 'background-only', workspaceRoot: '/lane/background-only', createdAt: at, updatedAt: at }, thread: thread('t9', 'p9'), machine: 'mac-studio' };
  const [focused, away] = focus === 'local' ? [local, second] : [second, local];
  const client = new T3Client();
  Object.assign(client, { loaded: true, connection: 'connected', origin: focused.origin, configLive: true, shellLive: true, threadLive: true, presentation: {} });
  client.environmentId = focused.id; client.threadId = focused.thread.id as string; client.projectId = focused.project.id;
  client.shell = { projects: [focused.project], threads: [focused.thread], sequence: 1 };
  client.config = { environment: { label: focused.label, platform: { machine: focused.machine }, capabilities: {} }, settings: {}, providers: [], keybindings: [] };
  client.local.clientSettings = { ...client.local.clientSettings, legacySidebarEnabled: true } as typeof client.local.clientSettings;
  fleet.entries.set('away', { key: 'away', origin: away.origin, environmentId: away.id, phase: 'connected', message: '', traceId: '', generation: 1, synchronized: 1, lastEvent: 0,
    subscriptions: {}, config: { environment: { label: away.label, platform: { machine: away.machine } }, settings: {} },
    shell: { projects: [away.project], threads: [away.thread], sequence: 1 }, scopes: [], error: '', requested: false } as unknown as FleetEntry);
  return client;
}
const rowId = (client: T3Client, environmentId: string, id: string) => client.environmentId === environmentId ? id : `fleet:${environmentId}:${id}`;
const flat = (client: T3Client) => Object.fromEntries(sidebarSnapshot(client, NOW, identity).threads.map(row => [row.id, row.remoteMachine]));
const legacy = (client: T3Client) => {
  const view = legacySidebarSnapshot(client, NOW, identity);
  return {
    projects: Object.fromEntries(view.projects.map(project => [project.name, [project.remoteMachine, project.remoteLabel, project.remoteTooltip]])),
    threads: Object.fromEntries(view.projects.flatMap(project => project.threads).map(row => [row.id, [row.remoteMachine, row.remoteLabel]])),
  };
};
afterEach(() => { fleet.entries.clear(); primary.update(unknownLocalBackend(), true); });

describe('remote is "not the primary environment"', () => {
  test('focused on the primary, the background environment\'s rows are remote', () => {
    setPrimary(true);
    const client = rig('local'), away = rowId(client, 'env-second', 't9');
    expect(flat(client)).toEqual({ t1: '', [away]: 'mac-studio' });
    expect(legacy(client)).toEqual({
      projects: { mine: ['', '', ''], 'background-only': ['cloud', 'Remote project', 'Remote environment: Lane Second'] },
      threads: { t1: ['', ''], [away]: ['mac-studio', 'Lane Second'] },
    });
  });
  test('after a background thread focuses its environment, its rows stay remote and the primary\'s stay local', () => {
    setPrimary(true);
    const client = rig('second'), mine = rowId(client, 'env-local', 't1');
    expect(remoteThread(client, client.shell.threads[0]!)).toEqual({ machine: 'mac-studio', label: 'Lane Second' });
    expect(flat(client)).toEqual({ t9: 'mac-studio', [mine]: '' });
    expect(legacy(client)).toEqual({
      projects: { mine: ['', '', ''], 'background-only': ['cloud', 'Remote project', 'Remote environment: Lane Second'] },
      threads: { t9: ['mac-studio', 'Lane Second'], [mine]: ['', ''] },
    });
  });
  test('with no primary every thread is remote and no project is', () => {
    setPrimary(false);
    const client = rig('second'), mine = rowId(client, 'env-local', 't1');
    expect(flat(client)).toEqual({ t9: 'mac-studio', [mine]: 'laptop' });
    expect(legacy(client)).toEqual({
      projects: { mine: ['', '', ''], 'background-only': ['', '', ''] },
      threads: { t9: ['mac-studio', 'Lane Second'], [mine]: ['laptop', 'This Mac'] },
    });
  });
});
