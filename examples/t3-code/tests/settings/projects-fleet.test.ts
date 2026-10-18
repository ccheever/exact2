import { AUTH_STANDARD_CLIENT_SCOPES } from '../../src/connections/remote-scopes';
// Settings › Projects over every environment (ProjectsSettings.tsx, ProjectSettingsPanel.tsx and
// useSettingsProjectGroups at T3 Code 1e2ecbd975): a project only a background environment has opens its page without
// changing the focused environment, reads that environment's settings and writes through its transport.
import { afterEach, describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import type { Files, Native } from '../../src/client/protocol';
import { fleet, type FleetEntry } from '../../src/connections/settings-b-fleet';
import { byEnvironment, groupOfMember, settingsGroups, settingsPageClient } from '../../src/settings/projects-fleet';
import { archiveCommand } from '../../src/settings/settings-a-archive';
import { projectsPage } from '../../src/settings/projects-view';
import { sidebarWrites } from '../../src/client/client-ops-sidebar';
import { restCommand } from '../../src/settings/settings-rest-commands';
import { runIconOp } from '../../src/settings/settings-b-icons';
import type { OpOut } from '../../src/client/client-ops';

const at = '2026-10-01T00:00:00.000Z';
const files: Files = { fs: { async mkdir() {}, async atomicWriteFile() {}, async readFile() { return new ArrayBuffer(0); } } };
const obj = (value: unknown): Obj => (value && typeof value === 'object' ? value as Obj : {});
const repo = { provider: 'github', canonicalKey: 'github.com/acme/app', displayName: 'acme/app' };

function focused(): T3Client {
  const client = new T3Client();
  Object.assign(client, { loaded: true, connection: 'connected', origin: 'http://127.0.0.1:16410', configLive: true, shellLive: true, threadLive: true, scopes: [...AUTH_STANDARD_CLIENT_SCOPES] });
  client.environmentId = 'local'; client.threadId = ''; client.projectId = 'p1';
  client.shell = { projects: [{ id: 'p1', title: 'App', workspaceRoot: '/work/app', repositoryIdentity: repo, createdAt: at, updatedAt: at }], threads: [], sequence: 1 };
  client.config = { environment: { label: 'This Mac', capabilities: { serverResolvedCommandContext: true, projectSettingsOverrides: true } }, settings: {}, providers: [], keybindings: [] };
  return client;
}
/** Studio, a background environment: a solo project with one thread and a worktree override, and the shared repository's checkout. */
function studio(writable = true): FleetEntry {
  const entry = { key: 'remote', origin: 'http://studio.local:3773', environmentId: 'env-remote', phase: 'connected', message: '', traceId: '', generation: 3, synchronized: 3, lastEvent: 0, subscriptions: {},
    config: { environment: { label: 'Studio', capabilities: { serverResolvedCommandContext: true, projectSettingsOverrides: true } }, settings: { projectSettingsOverrides: { r1: { defaultThreadEnvMode: 'worktree' } } } },
    shell: { projects: [{ id: 'r1', title: 'solo', workspaceRoot: '/srv/solo', createdAt: at, updatedAt: at },
      { id: 'r9', title: 'App', workspaceRoot: '/srv/app', repositoryIdentity: repo, createdAt: at, updatedAt: at }],
    threads: [{ id: 't1', projectId: 'r1', title: 'One', createdAt: at, updatedAt: at }], sequence: 1 },
    scopes: writable ? [...AUTH_STANDARD_CLIENT_SCOPES] : [], error: '', requested: true } as unknown as FleetEntry;
  fleet.entries.set('remote', entry);
  return entry;
}
afterEach(() => { fleet.entries.clear(); });

/** The module: ids, and the background transport answering as Studio's server (its shell, config and settings). */
function nativeOf(entry: FleetEntry, ops: Obj[]): Native {
  let ids = 0;
  return { available: true, watch() {}, async later(request) {
    const op = obj(request); ops.push(op);
    if (op.op === 'ids') return { ok: true, generation: 0, value: Array.from({ length: Number(op.count) }, () => `00000000-0000-4000-8000-${String(ids++).padStart(12, '0')}`) };
    if (op.fleet !== 'remote') return { ok: true, generation: 0, value: {} };
    const value = op.op === 'http' ? { projects: entry.shell.projects, threads: entry.shell.threads, snapshotSequence: 1 }
      : op.method === 'server.getConfig' ? entry.config : op.method === 'server.getSettings' ? entry.config.settings
        : op.method === 'server.updateSettings' ? { ...obj(entry.config.settings), ...obj(obj(op.payload).patch) } : {};
    return { ok: true, generation: 3, value };
  } };
}
const fleetRequests = (ops: Obj[]) => ops.filter(op => op.fleet === 'remote' && op.op === 'request').map(op => ({ method: op.method, payload: obj(op.payload) }));

describe('useSettingsProjectGroups over every environment', () => {
  test('a background environment\'s projects join the groups under their `fleet:` ids; a shared repository is one group', () => {
    const client = focused(); studio();
    expect(settingsGroups(client).map(group => [group.key, group.members.map(member => member.id)])).toEqual([
      ['github.com/acme/app', ['p1', 'fleet:env-remote:r9']], ['env-remote:/srv/solo', ['fleet:env-remote:r1']]]);
    expect(groupOfMember(client, 'fleet:env-remote:r1')?.key).toBe('env-remote:/srv/solo');
    expect(groupOfMember(client, 'p1')?.key).toBe('github.com/acme/app');
    expect([...byEnvironment(client, ['p1', 'fleet:env-remote:r9', 'fleet:env-remote:r1'])]).toEqual([['local', ['p1']], ['env-remote', ['r9', 'r1']]]);
  });
});

describe('Settings › Projects on a project only a background environment has', () => {
  test('the page reads that environment: its name, threads, environment and Workspace override, writable there', () => {
    const client = focused(); studio();
    const page = projectsPage(client, 'env-remote:/srv/solo', '', true);
    expect(page).toMatchObject({ ready: true, selected: true, name: 'solo', threads: 1, hasOther: false, removeTitle: 'Remove project' });
    expect(page.workspace).toMatchObject({ value: 'worktree', valueLabel: 'New worktree', disabled: false });
    expect(page.confirm).toContain('Environment: Studio');
    expect(page.confirm).toContain('Path: /srv/solo');
    // The focused environment's selection is untouched.
    expect(client).toMatchObject({ environmentId: 'local', projectId: 'p1' });
  });
  test('a read-only background connection disables its rows; a group across environments names each member\'s', () => {
    const client = focused(); studio(false);
    expect(projectsPage(client, 'env-remote:/srv/solo', '', true).workspace.disabled).toBe(true);
    const shared = projectsPage(client, 'github.com/acme/app', '', true);
    expect(shared.members.map(member => [member.id, member.environment])).toEqual([['p1', 'This Mac'], ['fleet:env-remote:r9', 'Studio']]);
    expect(shared.removeLabel).toBe('Remove all entries');
  });
  test('a settings page keyed by environment and project reads the background environment under its own project id', () => {
    const client = focused(); studio();
    const scoped = settingsPageClient(client, null, ['local', 'fleet:env-remote:r1', true]);
    expect(scoped.args).toEqual(['env-remote', 'r1', true]);
    expect(scoped.client).not.toBe(client);
    expect(scoped.client.environmentId).toBe('env-remote');
    expect(obj(obj(scoped.client.config.settings).projectSettingsOverrides).r1).toEqual({ defaultThreadEnvMode: 'worktree' });
    expect(settingsPageClient(client, null, ['local', 'p1']).client).toBe(client);
  });
});

describe('its writes go through the member\'s own environment', () => {
  test('renaming the group sends project.update to Studio for its project id, and nothing to the focused server', async () => {
    const client = focused(), entry = studio(), ops: Obj[] = [], local: string[] = [];
    client.request = async (_native, method) => { local.push(method); return {}; };
    client.http = async () => ({ projects: client.shell.projects, threads: [], snapshotSequence: 1 });
    await sidebarWrites.call(client, 'rename-group', 'env-remote:/srv/solo', 'Solo two', 0, nativeOf(entry, ops), files, { message: '', id: '', value: '' } as OpOut);
    expect(fleetRequests(ops)).toEqual([{ method: 'projects.mutate', payload: { type: 'project.update', title: 'Solo two', projectId: 'r1', commandId: '00000000-0000-4000-8000-000000000000' } }]);
    expect(local).toEqual([]);
    expect(opsOf(ops, 'connect')).toEqual([]);
  });
  test('the Workspace row writes Studio\'s override for its own project id', async () => {
    const client = focused(), entry = studio(), ops: Obj[] = [];
    await restCommand(client, nativeOf(entry, ops), files, 'scoped', 'local:fleet:env-remote:r1', 'key=defaultThreadEnvMode&value=local');
    const update = fleetRequests(ops).find(request => request.method === 'server.updateSettings');
    expect(obj(obj(update?.payload.patch).projectSettingsOverrides).r1).toMatchObject({ defaultThreadEnvMode: 'local' });
    expect(obj(obj(entry.config.settings).projectSettingsOverrides).r1).toMatchObject({ defaultThreadEnvMode: 'local' });
    expect(client.config.settings).toEqual({});
  });
  test('an icon reaches each member on its own environment', async () => {
    const client = focused(), entry = studio(), ops: Obj[] = [];
    await runIconOp(client, nativeOf(entry, ops), files, 'project-icon-set', 'fleet:env-remote:r1', 'kind=emoji&emoji=%F0%9F%9A%80');
    expect(fleetRequests(ops).map(request => [request.method, request.payload.projectId, request.payload.projectIcon])).toEqual([['projects.mutate', 'r1', { kind: 'emoji', emoji: '🚀' }]]);
  });
});
const opsOf = (ops: Obj[], name: string) => ops.filter(op => op.op === name);


test('background archive actions use the row owner without changing the focused thread', async () => {
  const client = focused(), entry = studio(), ops: Obj[] = [];
  client.threadId = 'focused-thread';
  client.local.drafts['env-remote:t1'] = 'background draft';
  client.local.drafts['local:focused-thread'] = 'focused draft';
  Object.assign(client.local.clientSettings!, { confirmThreadDelete: false });
  const native = nativeOf(entry, ops), later = native.later;
  native.later = async request => {
    const op = obj(request);
    if (op.method === 'orchestration.getArchivedShellSnapshot') {
      ops.push(op);
      return { ok: true, generation: 3, value: { snapshotSequence: 1, projects: entry.shell.projects, threads: entry.shell.threads.map(thread => ({ ...thread, archivedAt: at })) } };
    }
    return later(request);
  };
  await archiveCommand(client, native, files, 'archive-pick', 'env-remote:r1:t1', 'unarchive');
  await archiveCommand(client, native, files, 'archive-pick', 'env-remote:r1:t1', 'delete');
  expect(fleetRequests(ops).filter(request => request.method === 'orchestration.dispatchCommand').map(request => [request.payload.type, request.payload.threadId])).toEqual([
    ['thread.unarchive', 't1'], ['thread.delete', 't1']]);
  expect(client.threadId).toBe('focused-thread');
  expect(client.local.drafts['env-remote:t1']).toBeUndefined();
  expect(client.local.drafts['local:focused-thread']).toBe('focused draft');
});
