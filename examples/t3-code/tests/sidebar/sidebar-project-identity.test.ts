// T3 Code 9c161462c9 (MIT; see LICENSE-T3): the project filter draws each group's
// ProjectFavicon over the group snapshot, whose title is its representative member's
// (sidebarProjectGrouping.ts buildSidebarProjectSnapshots), so "t3-fixture/shared-notes"
// is "SN"; a thread on an ACP Registry instance draws that agent's icon
// (ProviderInstanceIcon's acpRegistryAgentId / acpRegistryIconUrl).
import { afterEach, describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import { initialShell, type Obj } from '../../src/client/domain';
import type { Files, Native } from '../../src/client/protocol';
import { fleet, environmentKey } from '../../src/connections/settings-b-fleet';
import { sidebarCommand } from '../../src/sidebar/sidebar-commands';
import { projectIdentity, providerBadge } from '../../src/client/presentation';
import { sidebarSnapshot } from '../../src/sidebar/sidebar-view';
import { sidebarPrefs } from '../../src/sidebar/sidebar-state';

const SECOND = environmentKey('http://127.0.0.1:16580', 'env-b');
afterEach(() => { fleet.entries.clear(); });
const NOW = Date.parse('2026-10-20T12:00:00.000Z');
const helpers = { projectIdentity, providerBadge };
const thread = (id: string, projectId: string, instanceId: string): Obj => ({ id, projectId, title: `Thread ${id}`, status: 'idle', latestRunId: null,
  createdAt: '2026-10-20T11:00:00.000Z', updatedAt: '2026-10-20T11:00:00.000Z', archivedAt: null, settledOverride: null, settledAt: null,
  modelSelection: { instanceId, model: 'm' }, lineage: { relationshipToParent: null } });

function client(): T3Client {
  const repositoryIdentity = { canonicalKey: 'github.com/t3-fixture/shared-notes', displayName: 't3-fixture/shared-notes', name: 'shared-notes' };
  const members = [{ id: 'p1', title: 'shared-notes', workspaceRoot: '/a/shared-notes', repositoryIdentity }, { id: 'p3', title: 'shared-notes-review', workspaceRoot: '/a/review', repositoryIdentity }];
  return {
    shell: { projects: [...members, { id: 'p2', title: 'agent-edits', workspaceRoot: '/a/agent-edits' }], sequence: 1,
      threads: [thread('a', 'p2', 'acpRegistry_fixture-acp-standin'), thread('b', 'p1', 'codex')] },
    config: { environment: { capabilities: {} }, keybindings: [], settings: { providerInstances: { 'acpRegistry_fixture-acp-standin': { driver: 'acpRegistry', config: { agentId: 'fixture-acp-standin' } } } }, providers: [
      { instanceId: 'acpRegistry_fixture-acp-standin', driver: 'acpRegistry', displayName: 'ACP Stand-in' },
      { instanceId: 'codex', driver: 'codex', displayName: 'Codex' }] },
    environmentId: 'env-a', threadId: '', projectId: '', query: '', connection: 'connected', writable: true, ready: true, presentation: {},
    local: { drafts: {}, selections: {}, deviceSettings: { timestampFormat: '24-hour' }, clientSettings: {}, sidebarWidth: 256 },
    projectGroups() { return [{ key: 'g1', name: 't3-fixture/shared-notes', members }, { key: 'g2', name: 'agent-edits', members: [{ id: 'p2', title: 'agent-edits' }] }]; },
  } as unknown as T3Client;
}

describe('the project filter', () => {
  test('a grouped project draws its member\'s monogram, in the menu and on the button', () => {
    const c = client();
    const menu = sidebarSnapshot(c, NOW, helpers).sidebar.scopes;
    expect(menu.map(scope => [scope.label, scope.mark])).toEqual([['All projects', ''], ['agent-edits', 'AE'], ['t3-fixture/shared-notes', 'SN']]);
    expect(menu.find(scope => scope.key === 'g1')!.glyph.text).toBe('SN');
    sidebarPrefs(c).scope = 'g1';
    const sidebar = sidebarSnapshot(c, NOW, helpers).sidebar;
    expect([sidebar.scopeLabel, sidebar.scopeMark, sidebar.scopeGlyph.text]).toEqual(['t3-fixture/shared-notes', 'SN', 'SN']);
  });
});

describe('a thread row\'s provider mark', () => {
  test('an ACP Registry instance carries its agent\'s icon; other drivers none', () => {
    const rows = sidebarSnapshot(client(), NOW, helpers).threads;
    expect(rows.find(row => row.id === 'a')).toMatchObject({ providerDriver: 'acpRegistry', providerIcon: 'https://cdn.agentclientprotocol.com/registry/v1/latest/fixture-acp-standin.svg' });
    expect(rows.find(row => row.id === 'b')).toMatchObject({ providerDriver: 'codex', providerIcon: '' });
    // A local ACP agent has no registry icon: the ACP glyph alone.
    const local = client();
    local.config = { ...local.config, settings: { providerInstances: { 'acpRegistry_fixture-acp-standin': { driver: 'acpRegistry', config: { source: 'local', agentId: 'fixture-acp-standin' } } } } };
    expect(sidebarSnapshot(local, NOW, helpers).threads.find(row => row.id === 'a')).toMatchObject({ providerDriver: 'acpRegistry', providerIcon: '' });
  });
});

describe('a background environment\'s threads', () => {
  function withBackground(): T3Client {
    const c = client();
    c.config = { ...c.config, providers: [], settings: {} }; // the focused environment has no ACP instance
    fleet.entries.set(SECOND, { key: SECOND, origin: 'http://127.0.0.1:16580', environmentId: 'env-b', phase: 'connected', message: '', traceId: '', generation: 2, synchronized: 2,
      lastEvent: 0, subscriptions: {}, error: '', requested: true, scopes: [],
      config: { environment: { environmentId: 'env-b', label: 'Lane Second', capabilities: {} }, providers: [{ instanceId: 'acpRegistry_fixture-acp-standin', driver: 'acpRegistry' }],
        settings: { providerInstances: { 'acpRegistry_fixture-acp-standin': { driver: 'acpRegistry', config: { agentId: 'fixture-acp-standin' } } } } },
      shell: { ...initialShell(), projects: [{ id: 'pb', title: 'background-only', workspaceRoot: '/b' }], threads: [thread('tb', 'pb', 'acpRegistry_fixture-acp-standin')] } });
    return c;
  }
  test('read their provider from their own environment', () => {
    const row = sidebarSnapshot(withBackground(), NOW, helpers).threads.find(entry => entry.id === 'fleet:env-b:tb');
    expect(row).toMatchObject({ providerDriver: 'acpRegistry', providerIcon: 'https://cdn.agentclientprotocol.com/registry/v1/latest/fixture-acp-standin.svg' });
  });
  test('New thread opens the Projects picker when the projects span environments', async () => {
    const c = withBackground();
    c.projectGroups = () => [{ key: 'g2', name: 'agent-edits', members: [{ id: 'p2', title: 'agent-edits' }] }];
    const native = { available: true, watch() {}, later: async () => ({ ok: true, generation: 1, value: {} }) } as unknown as Native;
    const files = { fs: { async mkdir() {}, async readFile() { throw new Error('none'); }, async atomicWriteFile() {} } } as Files;
    expect(await sidebarCommand(c, native, files, 'new-thread-click', '', '')).toBe('sidebar:palette-new-thread');
  });
});
