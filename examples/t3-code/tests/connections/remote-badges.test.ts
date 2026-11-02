// T3 Code 1e2ecbd975 (MIT; see LICENSE-T3): the sidebar marks what is not on the primary environment as
// remote, whichever environment is focused. Sidebar.tsx:1338 (`isRemote`, with
// `currentEnvironmentId={primaryEnvironmentId}` at :5173) marks a thread. Opening a background environment's
// thread focuses that environment here, so the focused environment's rows are remote then and the primary's are not.
import { afterEach, describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import { fleet, remoteThread, type FleetEntry } from '../../src/connections/settings-b-fleet';
import { primary } from '../../src/connections/local-primary';
import { parseLocalBackendStatus, unknownLocalBackend } from '../../src/connections/local-backend';
import { sidebarSnapshot } from '../../src/sidebar/sidebar-view';

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
  fleet.entries.set('away', { key: 'away', origin: away.origin, environmentId: away.id, phase: 'connected', message: '', traceId: '', generation: 1, synchronized: 1, lastEvent: 0,
    subscriptions: {}, config: { environment: { label: away.label, platform: { machine: away.machine } }, settings: {} },
    shell: { projects: [away.project], threads: [away.thread], sequence: 1 }, scopes: [], error: '', requested: false } as unknown as FleetEntry);
  return client;
}
const rowId = (client: T3Client, environmentId: string, id: string) => client.environmentId === environmentId ? id : `fleet:${environmentId}:${id}`;
const flat = (client: T3Client) => Object.fromEntries(sidebarSnapshot(client, NOW, identity).threads.map(row => [row.id, row.remoteMachine]));
afterEach(() => { fleet.entries.clear(); primary.update(unknownLocalBackend(), true); });

describe('remote is "not the primary environment"', () => {
  test('focused on the primary, the background environment\'s rows are remote', () => {
    setPrimary(true);
    const client = rig('local'), away = rowId(client, 'env-second', 't9');
    expect(flat(client)).toEqual({ t1: '', [away]: 'mac-studio' });
  });
  test('after a background thread focuses its environment, its rows stay remote and the primary\'s stay local', () => {
    setPrimary(true);
    const client = rig('second'), mine = rowId(client, 'env-local', 't1');
    expect(remoteThread(client, client.shell.threads[0]!)).toEqual({ machine: 'mac-studio', label: 'Lane Second' });
    expect(flat(client)).toEqual({ t9: 'mac-studio', [mine]: '' });
  });
  test('with no primary every thread is remote and no project is', () => {
    setPrimary(false);
    const client = rig('second'), mine = rowId(client, 'env-local', 't1');
    expect(flat(client)).toEqual({ t9: 'mac-studio', [mine]: 'laptop' });
  });
});

describe('a remote card with nothing else for its third line', () => {
  // SidebarThreadRow at 2359643cff: the machine glyph no longer keeps the meta line; it joins the title line.
  test('is two lines, its machine glyph beside the provider', () => {
    setPrimary(true);
    const client = rig('local'), away = rowId(client, 'env-second', 't9');
    const rows = Object.fromEntries(sidebarSnapshot(client, NOW, identity).threads.map(row => [row.id, row]));
    expect(rows[away]).toMatchObject({ remoteMachine: 'mac-studio', metaEmpty: true });
    expect(rows.t1).toMatchObject({ remoteMachine: '', metaEmpty: true });
  });
});
