import { AUTH_STANDARD_CLIENT_SCOPES } from '../../src/connections/remote-scopes';
// Settings across environments (D15): the scope menu lists every environment, a write
// reaches each selected connected environment through its own transport, a failure names
// the environments that did not save, and disagreeing targets draw the mixed switch.
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { appText } from '../support/app-files';
import type { T3Client } from '../../src/client/client';
import { applyShell, initialShell, type Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { applyCoreSetting, decodeClientPrefs, generalSections, inheritanceGroups, resolveScope, serverContext } from '../../src/settings/settings-core';
import { settingsCore } from '../../src/settings/settings-core-view';
import { fleet, type FleetEntry } from '../../src/connections/settings-b-fleet';
import { scopeEnvironments, scopeProjectGroups } from '../../src/settings/settings-scope-sources';
import { restCommand } from '../../src/settings/settings-rest-commands';
import { archivedSettings, lastRunSummary, storageSettings, worktreeRows } from '../../src/settings/settings-data';
import { groupDeviceHosts, scopedDeviceHostsView, deviceHostsCommand, hostChecks } from '../../src/device/settings-a-hosts';
import { taskEnvironmentSummaries } from '../../src/settings/scheduled-view';
import { toasts } from '../../src/shell/toast';
import { runCleanup } from '../../src/settings/storage-maintenance';
import { primaryAt, resetPrimary } from '../support/local-primary-fixture';

const capabilities = { storageCleanup: true, storageCleanupRun: true, projectWorktreeCleanup: true, settledWorktreeCleanup: true, worktreesDirectory: true, projectSettingsOverrides: true, threadAutoSettlement: true, threadRestartContinuation: true };
function focused(settings: Obj = {}) {
  const client = {
    local: { deviceSettings: { composerCollapseOnScroll: true, planModeEnabled: false, timestampFormat: 'locale', appearanceMode: 'system', sendShortcut: 'enter' },
      clientSettings: decodeClientPrefs({}), groupingMode: 'repository', groupingOverrides: {} },
    scopes: AUTH_STANDARD_CLIENT_SCOPES, ready: true, environmentId: 'env-a', origin: 'http://127.0.0.1:16100', writes: [] as Obj[],
    config: { environment: { environmentId: 'env-a', label: 'Studio', capabilities, platform: { machine: 'laptop' } }, providers: [], settings: { projectSettingsOverrides: {}, ...settings } } as Obj,
    projectGroups: () => [{ key: 'github.com/acme/app', name: 'App', members: [{ id: 'pa', title: 'App', workspaceRoot: '/a/app' }] }],
    async settingsCoreRequest(_native: Native, method: string, payload: Obj) {
      if (method === 'server.getSettings') return client.config.settings as Obj;
      if (method === 'projects.readFile') throw new Error('missing');
      client.writes.push(payload);
      client.config.settings = { ...(client.config.settings as Obj), ...(payload.patch as Obj) };
      return client.config.settings as Obj;
    },
  };
  return client;
}
const as = (client: ReturnType<typeof focused>) => client as unknown as T3Client;

function entry(key: string, environmentId: string, label: string, settings: Obj, phase: FleetEntry['phase'] = 'connected'): FleetEntry {
  const shell = applyShell(initialShell(), { snapshotSequence: 1, projects: [{ id: `p-${environmentId}`, title: 'App', workspaceRoot: `/b/${environmentId}`,
    repositoryIdentity: { canonicalKey: 'github.com/acme/app' } }], threads: [] });
  return { key, origin: key.split('\n')[0]!, environmentId, phase, message: '', traceId: '', generation: 3, synchronized: 3, lastEvent: 0, subscriptions: {},
    config: { environment: { environmentId, label, capabilities, platform: { machine: 'server' } }, settings: { projectSettingsOverrides: {}, ...settings } }, shell, scopes: [...AUTH_STANDARD_CLIENT_SCOPES], error: '', requested: true };
}
/** The fleet's transports: each request names its environment with `fleet`; `fail` holds the keys whose write is refused. */
function nativeFor(fail = new Set<string>(), calls: Obj[] = []): Native {
  return { available: true, watch() {}, async later(request) {
    const call = request as Obj; calls.push(call);
    const target = fleet.entries.get(String(call.fleet));
    if (!target) return { ok: false, generation: 0, error: { message: 'No such environment.' } };
    if (fail.has(target.key)) return { ok: false, generation: target.generation, error: { message: 'Disconnected during save' } };
    if (call.method === 'projects.readFile') return { ok: false, generation: target.generation, error: { message: 'missing' } };
    const patch = (call.payload as Obj).patch as Obj;
    return { ok: true, generation: target.generation, value: { ...(target.config.settings as Obj), ...patch } };
  } };
}
const B = 'http://127.0.0.1:16101\nenv-b', C = 'http://127.0.0.1:16102\nenv-c';
beforeEach(() => { fleet.entries.clear(); fleet.saved = []; });
afterEach(() => { fleet.entries.clear(); fleet.saved = []; });

describe('settings scope across environments', () => {
  test('the menu lists every environment, disambiguates same names by address and marks an offline one', () => {
    const client = focused();
    fleet.entries.set(B, entry(B, 'env-b', 'Studio', {}));
    fleet.entries.set(C, entry(C, 'env-c', 'Build box', {}, 'reconnecting'));
    const scope = resolveScope(as(client), '', '', '');
    expect(scope.environmentChoices.map(choice => [choice.label, choice.secondary, choice.mark, choice.offline])).toEqual([
      ['All environments', '', '', false], ['Studio', '127.0.0.1:16100', 'laptop', false], ['Studio', '127.0.0.1:16101', 'server', false], ['Build box', '127.0.0.1:16102', 'server', true]]);
    expect(scope.resolved.environmentIds).toEqual(['env-a', 'env-b', 'env-c']);
    expect(resolveScope(as(client), 'env-c', '', '')).toMatchObject({ kind: 'environment', connected: false, environmentLabel: 'Build box', connective: 'on', environmentIcon: 'server' });
    expect(scopeEnvironments(as(client)).map(environment => environment.connection.phase)).toEqual(['connected', 'connected', 'reconnecting']);
  });
  test('a project groups the same repository across environments', () => {
    fleet.entries.set(B, entry(B, 'env-b', 'Server', {}));
    const groups = scopeProjectGroups(as(focused()));
    expect(groups.map(group => [group.projectKey, group.displayName, group.memberProjects.map(member => `${member.environmentId}:${member.id}`)]))
      .toEqual([['github.com/acme/app', 'App', ['env-a:pa', 'env-b:p-env-b']]]);
  });
  test('All environments writes once per connected environment through its own transport', async () => {
    const client = focused();
    fleet.entries.set(B, entry(B, 'env-b', 'Server', {}));
    fleet.entries.set(C, entry(C, 'env-c', 'Offline box', {}, 'error'));
    const calls: Obj[] = [];
    await applyCoreSetting(as(client), nativeFor(new Set(), calls), 'provider-update-checks:|||', 'false');
    expect(client.writes).toEqual([{ patch: { enableProviderUpdateChecks: false } }]);
    expect(calls.map(call => [call.fleet, call.method, call.payload, call.generation])).toEqual([[B, 'server.updateSettings', { patch: { enableProviderUpdateChecks: false } }, 3]]);
    expect((fleet.entries.get(B)!.config.settings as Obj).enableProviderUpdateChecks).toBe(false);
    expect(toasts(as(client))).toEqual([]);
  });
  test('Storage edits fan out while project custom rules retain each checkout inheritance', async () => {
    const client = focused({ storageCleanup: { worktreeAfterDays: 2 }, projectSettingsOverrides: {} });
    fleet.entries.set(B, entry(B, 'env-b', 'Server', { storageCleanup: { worktreeAfterDays: 9 } }));
    const calls: Obj[] = [], native = nativeFor(new Set(), calls);
    await restCommand(as(client), native, { read: async () => null, write: async () => {} }, 'storage', '', 'scope=%7C%7C&key=worktreeSettledAfterDays&value=0');
    expect(client.writes.at(-1)).toEqual({ patch: { storageCleanup: { worktreeSettledAfterDays: 0 } } });
    expect(calls.at(-1)?.payload).toEqual({ patch: { storageCleanup: { worktreeSettledAfterDays: 0 } } });
    // The server merges partial nested storage patches; preserve its distinct rules in this transport fixture.
    client.config.settings = { storageCleanup: { worktreeAfterDays: 2, worktreeSettledAfterDays: 0 }, projectSettingsOverrides: {} };
    fleet.entries.get(B)!.config.settings = { storageCleanup: { worktreeAfterDays: 9, worktreeSettledAfterDays: 0 }, projectSettingsOverrides: {} };
    await restCommand(as(client), native, { read: async () => null, write: async () => {} }, 'storage', '', `scope=${encodeURIComponent('|github.com/acme/app|')}&key=mode&value=custom`);
    const local = client.writes.at(-1)!;
    expect(((local.patch as Obj).projectSettingsOverrides as Obj).pa).toMatchObject({ worktreeCleanup: { mode: 'custom', rules: { worktreeAfterDays: 2 } } });
    const remote = calls.at(-1)!.payload as Obj;
    expect(((remote.patch as Obj).projectSettingsOverrides as Obj)['p-env-b']).toMatchObject({ worktreeCleanup: { mode: 'custom', rules: { worktreeAfterDays: 9 } } });
    const count = client.writes.length + calls.length;
    await expect(restCommand(as(client), native, { read: async () => null, write: async () => {} }, 'storage', '', `scope=${encodeURIComponent('gone||')}&key=worktreeSettledAfterDays&value=0`)).rejects.toThrow('no longer available');
    expect(client.writes.length + calls.length).toBe(count);
    const stale = await storageSettings(as(client), native, 'env-a', 'gone', true);
    expect(stale.available).toBe(false);
    expect(stale.error).toContain('checkout is no longer available');
  });
  test('Worktrees and Maintenance group their rows as WorktreesSettings and MaintenanceSettingsPanel do', async () => {
    const client = focused();
    Object.assign(client, { generation: 1, shell: initialShell() });
    const key = 'http://127.0.0.1:16104\nenv-storage-groups';
    fleet.entries.set(key, entry(key, 'env-storage-groups', 'Server', { storageCleanup: { worktreeOnDelete: true } }));
    const page = await storageSettings(as(client), nativeFor(), '', '', true, 'env-storage-groups', '', '');
    // Creation: the location, Start from remote, Submodules under Advanced; Branches; then Cleanup's three rules with
    // settled and unchanged worktrees and Keep worktrees with local changes under its Advanced fold.
    expect([page.directorySupported, page.creation.map(row => [row.id, row.divider]), page.creationAdvanced.map(row => [row.id, row.divider])])
      .toEqual([true, [['start-from-origin', true]], [['worktree-submodules', false]]]);
    expect(page.branches.map(row => row.key)).toEqual(['branchNamingMode', 'branchNamePrefix']);
    expect(page.worktrees.map(rule => [rule.key, rule.first])).toEqual([['worktreeOnDelete', true], ['worktreeOnMerge', false], ['worktreeAfterDays', false]]);
    expect([page.worktreesAdvanced.map(rule => [rule.key, rule.first]), page.keepSupported]).toEqual([[['worktreeSettledAfterDays', true], ['worktreeUnchanged', false]], true]);
    // Maintenance ends with Diagnostics, on the one selected environment.
    expect(page.diagnostics.map(row => [row.id, row.kind, row.target, row.description])).toEqual([['diagnostics', 'link', 'diagnostics', 'Inspect processes, resource use, and logs on this environment.']]);
    // A machine without settled cleanup keeps only unchanged worktrees under Advanced.
    fleet.entries.set(key, { ...fleet.entries.get(key)!, config: { ...fleet.entries.get(key)!.config, environment: { environmentId: 'env-storage-groups', label: 'Server', capabilities: { ...capabilities, settledWorktreeCleanup: false } } } });
    expect((await storageSettings(as(client), nativeFor(), '', '', true, 'env-storage-groups', '', '')).worktreesAdvanced.map(rule => rule.key)).toEqual(['worktreeUnchanged']);
    // Worktree location shows wherever every selected machine has a worktrees directory, storage cleanup or not
    // (WorktreesDirectoryRow), with Start from remote divided under it; a machine without one starts the card at Start
    // from remote, undivided.
    const only = async (extra: Obj) => {
      fleet.entries.set(key, { ...fleet.entries.get(key)!, config: { ...fleet.entries.get(key)!.config, environment: { environmentId: 'env-storage-groups', label: 'Server', capabilities: { ...capabilities, ...extra } } } });
      const page = await storageSettings(as(client), nativeFor(), '', '', true, 'env-storage-groups', '', '');
      return [page.available, page.directorySupported, page.creation.map(row => [row.id, row.divider])];
    };
    expect(await only({ storageCleanup: false })).toEqual([false, true, [['start-from-origin', true]]]);
    // Its write asks for the directory alone, as useUpdateScopedSettings does.
    const calls: Obj[] = [];
    await restCommand(as(client), nativeFor(new Set(), calls), { read: async () => null, write: async () => {} }, 'storage', '', `scope=${encodeURIComponent('env-storage-groups||')}&key=worktreesDirectory&value=~/wt`);
    expect(calls.at(-1)?.payload).toEqual({ patch: { worktreesDirectory: '~/wt' } });
    expect(await only({ worktreesDirectory: false })).toEqual([true, false, [['start-from-origin', false]]]);
    const storage = appText('settings-storage.contract');
    expect(storage).toContain('          if data.directorySupported\n            column width="100%" id="storage-worktrees-location" tabindex=-1');
  });
  test('Storage keeps maintenance available on a supported machine while an older machine hides configuration', async () => {
    const client = focused();
    Object.assign(client, { scopes: [], generation: 1, shell: initialShell() });
    client.config.environment = { ...(client.config.environment as Obj), capabilities: { ...capabilities, storageCleanup: false, storageCleanupRun: false } };
    const key = 'http://127.0.0.1:16103\nenv-storage-maintenance';
    fleet.entries.set(key, entry(key, 'env-storage-maintenance', 'Server', { storageCleanup: { logsAfterDays: 7 } }));
    const page = await storageSettings(as(client), nativeFor(), '', '', true, '', '', '');
    expect(page.available).toBe(false);
    expect(page.notice).toContain('Update');
    expect(page.cleanupAvailable).toBe(true);
    expect(page.cleanupDisabled).toBe(false);
    expect(page.cleanupDescription).not.toContain('Turn on a cleanup rule');
  });
  test('cleanup enables and runs only targets with settings permission, independently of the focused grant', async () => {
    const client = focused();
    Object.assign(client, { scopes: ['environment:maintain'], generation: 1, shell: initialShell() });
    const settingsOnly = entry(B, 'env-b', 'Settings only', { storageCleanup: { logsAfterDays: 7 } });
    const maintainOnly = entry(C, 'env-c', 'Maintain only', { storageCleanup: { logsAfterDays: 7 } });
    settingsOnly.scopes = ['settings:write']; maintainOnly.scopes = ['environment:maintain'];
    fleet.entries.set(B, settingsOnly); fleet.entries.set(C, maintainOnly);
    const calls: Obj[] = [], native = nativeFor(new Set(), calls);
    expect((await storageSettings(as(client), native, '', '', true, 'env-b', '', '')).cleanupDisabled).toBe(false);
    expect((await storageSettings(as(client), native, '', '', true, 'env-c', '', '')).cleanupDisabled).toBe(true);
    await runCleanup(as(client), native, ['env-b', 'env-c']);
    expect(calls.filter(call => call.method === 'server.runStorageCleanup').map(call => call.fleet)).toEqual([B]);
    await expect(runCleanup(as(client), native, ['env-c'])).rejects.toThrow('permission');
  });
  test('a single environment writes only there', async () => {
    const client = focused();
    fleet.entries.set(B, entry(B, 'env-b', 'Server', {}));
    const calls: Obj[] = [];
    await applyCoreSetting(as(client), nativeFor(new Set(), calls), 'snooze-limited-threads:|env-b||', 'true');
    expect(client.writes).toEqual([]);
    expect(calls.map(call => call.fleet)).toEqual([B]);
  });
  test('a failed environment is named; the others keep the change', async () => {
    const client = focused();
    fleet.entries.set(B, entry(B, 'env-b', 'Server', {}));
    await applyCoreSetting(as(client), nativeFor(new Set([B])), 'provider-update-checks:|||', 'false');
    expect(client.writes.length).toBe(1);
    expect(toasts(as(client)).at(-1)).toMatchObject({ kind: 'error', title: 'Setting saved on some environments',
      description: 'Could not save on Server: Disconnected during save\nSaved on Studio.' });
    // An offline named environment refuses before any request.
    fleet.entries.set(C, entry(C, 'env-c', 'Offline box', {}, 'error'));
    await applyCoreSetting(as(client), nativeFor(), 'provider-update-checks:|env-c||', 'true');
    expect(toasts(as(client)).at(-1)).toMatchObject({ kind: 'warning', title: 'Setting not saved', description: 'Connect Offline box to save this setting.' });
  });
  test('disagreeing environments draw mixed; one press turns every target on', async () => {
    const client = focused({ enableProviderUpdateChecks: false });
    fleet.entries.set(B, entry(B, 'env-b', 'Server', { enableProviderUpdateChecks: true }));
    const row = () => generalSections(as(client), serverContext(as(client), resolveScope(as(client), '', '', ''), new Map()))
      .flatMap(section => section.rows).find(item => item.id === 'provider-update-checks')!;
    expect([row().mixed, row().checked, row().inheritance]).toEqual([true, false, 'mixed']);
    await applyCoreSetting(as(client), nativeFor(), 'provider-update-checks:|||', 'true');
    expect([row().mixed, row().checked]).toEqual([false, true]);
    // Equal values: the switch is plain on/off as before.
    expect(generalSections(as(client), serverContext(as(client), resolveScope(as(client), 'env-a', '', ''), new Map())).flatMap(section => section.rows)
      .filter(item => item.kind === 'switch' && item.mixed)).toEqual([]);
  });
  test('checkouts of one project that disagree draw mixed for start-from-origin', async () => {
    const client = focused({ projectSettingsOverrides: { pa: { newWorktreesStartFromOrigin: false } } });
    fleet.entries.set(B, entry(B, 'env-b', 'Server', {}));
    // Worktrees' Creation rows are the storage panel's, written as the core pages' are (`rest:core`).
    const { creation } = await worktreeRows(as(client), nativeFor(), resolveScope(as(client), '', 'github.com/acme/app', ''), false);
    const start = creation.find(item => item.id === 'start-from-origin')!;
    expect([start.mixed, start.inheritance]).toEqual([true, 'mixed']);
    const calls: Obj[] = [];
    await restCommand(as(client), nativeFor(new Set(), calls), { read: async () => null, write: async () => {} }, 'core', '', `id=${encodeURIComponent('start-from-origin:||github.com/acme/app|')}&value=true`);
    expect(client.writes).toEqual([{ patch: { projectSettingsOverrides: { pa: { newWorktreesStartFromOrigin: true } } } }]);
    expect(calls.filter(call => call.method === 'server.updateSettings').map(call => (call.payload as Obj).patch)).toEqual([{ projectSettingsOverrides: { 'p-env-b': { newWorktreesStartFromOrigin: true } } }]);
  });
});

describe('the primary represents a scope that includes it', () => {
  const P = 'http://127.0.0.1:16109\nenv-p';
  const ready = { instanceId: 'acp', driver: 'acp', enabled: true, installed: true, status: 'ready', auth: { status: 'authenticated' }, models: [{ slug: 'stand-in', name: 'Stand-in', isDefault: true }] };
  afterEach(() => resetPrimary());
  test('the primary lists first and supplies General’s values, even while another environment is focused', () => {
    const client = focused();
    client.config.environment = { ...(client.config.environment as Obj), label: 'Lane Second' };
    client.config.providers = [ready];
    // The fleet holds the background environments, the primary connected first; the saved catalog keeps pairing order.
    fleet.entries.set(P, entry(P, 'env-p', "Daehyeon's MacBook Pro", {}));
    fleet.entries.set(C, entry(C, 'env-c', 'Lane Third', {}));
    fleet.saved = [{ environmentId: 'env-a', origin: 'http://127.0.0.1:16100' }, { environmentId: 'env-c', origin: 'http://127.0.0.1:16102' }];
    primaryAt('http://127.0.0.1:16109', 'env-p', "Daehyeon's MacBook Pro");
    const scope = resolveScope(as(client), '', '', '');
    expect(scope.environmentChoices.map(choice => choice.label)).toEqual(['All environments', "Daehyeon's MacBook Pro", 'Lane Second', 'Lane Third']);
    const model = generalSections(as(client), serverContext(as(client), scope, new Map())).flatMap(section => section.rows).find(row => row.id === 'default-model')!;
    expect([model.status, model.kind, model.label, model.description]).toEqual(['', 'text-only', 'No providers available', 'Default model for new threads, selected automatically. Projects can override it.']);
    // Without a primary the focused environment represents the scope, as before.
    resetPrimary();
    const focusedModel = generalSections(as(client), serverContext(as(client), resolveScope(as(client), '', '', ''), new Map())).flatMap(section => section.rows).find(row => row.id === 'default-model')!;
    expect(focusedModel.kind).not.toBe('text-only');
  });
  test('the project menu lists groups by their cross-environment name, alphabetically, marked by the project title', () => {
    const notes = (id: string) => ({ id, title: 'shared-notes', workspaceRoot: `/w/${id}`, repositoryIdentity: { canonicalKey: 'github.com/t3-fixture/shared-notes', displayName: 't3-fixture/shared-notes', name: 'shared-notes' } });
    const client = { ...focused(), projectGroups: () => [
      { key: 'github.com/t3-fixture/shared-notes', name: 'shared-notes', members: [notes('pn')] },
      { key: 'env-a:/w/background-only', name: 'background-only', members: [{ id: 'pb', title: 'background-only', workspaceRoot: '/w/background-only' }] },
      { key: 'env-a:/w/agent-edits', name: 'agent-edits', members: [{ id: 'pe', title: 'agent-edits', workspaceRoot: '/w/agent-edits' }] }] };
    const remote = entry(C, 'env-c', 'Lane Third', {});
    remote.shell = applyShell(initialShell(), { snapshotSequence: 1, projects: [notes('pn2')], threads: [] });
    fleet.entries.set(C, remote);
    const choices = resolveScope(client as unknown as T3Client, '', '', '').projectChoices;
    expect(choices.map(choice => [choice.label, choice.mark])).toEqual([['All projects', ''], ['agent-edits', 'AE'], ['background-only', 'BO'], ['t3-fixture/shared-notes', 'SN']]);
    // 74541bab17: a chosen project lists only the environments it has a checkout on, an unknown project every environment
    // (so it stays recoverable), and a chosen environment lists only its projects.
    const environmentIds = (projectKey: string) => resolveScope(client as unknown as T3Client, '', projectKey, '').environmentChoices.map(choice => choice.id);
    expect(environmentIds('github.com/t3-fixture/shared-notes')).toEqual(['', 'env-a', 'env-c']);
    expect(environmentIds('env-a:/w/agent-edits')).toEqual(['', 'env-a']);
    expect(environmentIds('removed-project')).toEqual(environmentIds(''));
    expect(resolveScope(client as unknown as T3Client, 'env-c', '', '').projectChoices.map(choice => choice.label)).toEqual(['All projects', 't3-fixture/shared-notes']);
  });
});

describe('eight-machine settings', () => {
  test('the scope keeps offline choices and groups equal raw inheritance resolutions', () => {
    const client = focused({ enableProviderUpdateChecks: true });
    for (let index = 1; index < 8; index++) {
      const id = `env-${index}`, key = `http://127.0.0.1:${16500 + index}\n${id}`;
      fleet.entries.set(key, entry(key, id, 'Studio', { enableProviderUpdateChecks: index < 4 }, index === 7 ? 'reconnecting' : 'connected'));
    }
    const scope = resolveScope(as(client), '', '', '');
    expect(scope.environmentChoices).toHaveLength(9);
    expect(scope.environmentChoices.filter(choice => choice.offline).map(choice => choice.id)).toEqual(['env-7']);
    const groups = inheritanceGroups(serverContext(as(client), scope, new Map()), 'enableProviderUpdateChecks');
    expect(groups.map(group => [group.value, group.labels.length, group.folded, group.sameEverywhere])).toEqual([
      ['On', 4, true, false], ['Off', 3, true, false]]);
    fleet.entries.clear();
    for (let index = 1; index < 8; index++) {
      const id = `env-${index}`, key = `http://127.0.0.1:${16500 + index}\n${id}`;
      fleet.entries.set(key, entry(key, id, 'Studio', { enableProviderUpdateChecks: true }));
    }
    const equal = inheritanceGroups(serverContext(as(client), resolveScope(as(client), '', '', ''), new Map()), 'enableProviderUpdateChecks');
    expect(equal).toHaveLength(1);
    expect(equal[0]).toMatchObject({ sameEverywhere: true, folded: false });
    expect(equal[0]!.labels).toHaveLength(8);
  });
  test('resetting a folded override group keeps unrelated project overrides', async () => {
    const client = focused({ newWorktreesStartFromOrigin: true, projectSettingsOverrides: { pa: { newWorktreesStartFromOrigin: false, defaultRuntimeMode: 'full-access' } } });
    fleet.entries.set(B, entry(B, 'env-b', 'Studio', { newWorktreesStartFromOrigin: true, projectSettingsOverrides: { 'p-env-b': { newWorktreesStartFromOrigin: false } } }));
    const scope = resolveScope(as(client), '', '', ''), context = serverContext(as(client), scope, new Map());
    const group = inheritanceGroups(context, 'newWorktreesStartFromOrigin')[0]!;
    expect(group.overrides.map(override => override.label)).toEqual(['App', 'App']);
    const calls: Obj[] = [];
    await applyCoreSetting(as(client), nativeFor(new Set(), calls), 'start-from-origin:clear-overrides|||', group.overrideTargets);
    expect(client.writes).toEqual([{ patch: { projectSettingsOverrides: { pa: { defaultRuntimeMode: 'full-access' } } } }]);
    expect(calls.map(call => call.payload)).toEqual([{ patch: { projectSettingsOverrides: { 'p-env-b': null } } }]);
  });
  test('host identity conflicts keep lists separate; checks address every selected transport', async () => {
    const host = { id: 'h-a', label: 'Build host', target: 'builder' };
    const client = focused({ deviceHosts: [host] });
    Object.assign(client, { shell: initialShell(), restAccess: () => ({ request: async (method: string) => method === 'device.testHost' ? { kind: 'local' } : client.config.settings }) });
    for (let index = 1; index < 8; index++) {
      const id = `env-${index}`, key = `http://127.0.0.1:${16500 + index}\n${id}`;
      fleet.entries.set(key, entry(key, id, 'Studio', { deviceHosts: [{ ...host, id: `host-${index}` }] }));
    }
    const native: Native = { available: true, watch() {}, async later(request) {
      const call = request as Obj, environment = fleet.entries.get(String(call.fleet))!;
      const value = call.method === 'device.testHost' ? { kind: 'local' } : environment.config.settings;
      return { ok: true, generation: environment.generation, value };
    } };
    const scope = resolveScope(as(client), '', '', '');
    const view = scopedDeviceHostsView(as(client), native, scope.selected, scope.environments, null, false);
    expect(view.groups).toEqual([]);
    expect(view.hosts).toHaveLength(1);
    await deviceHostsCommand(as(client), native, 'hosts-test-row', { id: 'h-a' }, false);
    expect(hostChecks(as(client))[0]!.results).toHaveLength(8);
    expect(hostChecks(as(client))[0]!.summary).toBe('Connection checks passed');
    expect(groupDeviceHosts([{ environmentId: 'a', label: 'A', hosts: [host] }, { environmentId: 'b', label: 'B', hosts: [{ ...host, target: 'other' }] }])).toBeNull();
    const changed = fleet.entries.values().next().value!;
    changed.config.settings = { deviceHosts: [{ ...host, label: 'Different label' }] };
    expect(scopedDeviceHostsView(as(client), native, scope.selected, scope.environments, null, false).groups).toHaveLength(8);
  });
  test('folded device hosts identify configured hosts and summarize more than two names', () => {
    const host = { id: 'shared', label: 'Build host', target: 'builder' };
    const client = focused({ deviceHosts: [host] });
    Object.assign(client, { shell: initialShell() });
    fleet.entries.set(B, entry(B, 'env-b', 'Studio', { deviceHosts: [
      { ...host, label: 'Mac mini' }, { id: 'sim', label: 'Simulator', target: 'simulator' }, { id: 'lab', label: 'Device lab', target: 'lab' },
    ] }));
    fleet.entries.set(C, entry(C, 'env-c', 'Empty machine', {}));
    const scope = resolveScope(as(client), '', '', '');
    const view = scopedDeviceHostsView(as(client), nativeFor(), scope.selected, scope.environments, null, false);
    expect(view.groups.map(group => [group.id, group.summary])).toEqual([
      ['env-a', 'Build host'], ['env-b', 'Mac mini, Simulator +1'], ['env-c', 'No device hosts'],
    ]);
  });
  test('archives join logical projects, preserve colliding thread ids and cache reads per machine', async () => {
    const client = focused();
    const snapshot = (projectId: string, index: number) => ({ snapshotSequence: 1, projects: [{ id: projectId, title: 'App' }], threads: [{ id: 'same-thread', projectId, title: `Thread ${index}`, createdAt: '2026-10-01T00:00:00Z', archivedAt: `2026-10-${String(index + 1).padStart(2, '0')}T00:00:00Z` }] });
    let reads = 0;
    Object.assign(client, { shell: initialShell(), restAccess: () => ({ read: async () => { reads++; return snapshot('pa', 0); } }) });
    for (let index = 1; index < 8; index++) {
      const id = `env-${index}`, key = `http://127.0.0.1:${16500 + index}\n${id}`;
      fleet.entries.set(key, entry(key, id, 'Studio', {}));
    }
    const native: Native = { available: true, watch() {}, async later(request) {
      reads++;
      const environment = fleet.entries.get(String((request as Obj).fleet))!, index = Number(environment.environmentId.slice(4));
      return { ok: true, generation: environment.generation, value: snapshot(`p-${environment.environmentId}`, index) };
    } };
    const view = await archivedSettings(as(client), native, '', '', true, Date.parse('2026-10-10T00:00:00Z'));
    expect(view.error).toBe('');
    expect(view.groups).toHaveLength(1);
    expect(view.groups[0]!.threads.map(thread => thread.title)).toEqual(['Thread 7', 'Thread 6', 'Thread 5', 'Thread 4', 'Thread 3', 'Thread 2', 'Thread 1', 'Thread 0']);
    expect(new Set(view.groups[0]!.threads.map(thread => thread.id)).size).toBe(8);
    expect(view.groups[0]!.threads.every(thread => thread.description.includes('127.0.0.1:'))).toBe(true);
    await archivedSettings(as(client), native, '', '', true, Date.parse('2026-10-11T00:00:00Z'));
    expect(reads).toBe(8);
  });
  test('storage and scheduled status summaries collapse only matching outcomes', () => {
    const reports = Array.from({ length: 8 }, (_, index) => ({ label: `Machine ${index}`, counts: '2 removed, 1 kept' }));
    expect(lastRunSummary(reports, 8)).toBe('2 removed, 1 kept on every machine');
    expect(lastRunSummary(reports.slice(0, 1), 8)).toBe('Machine 0: 2 removed, 1 kept');
    const section = (id: string, state: string, tasks: never[] = []) => ({ id, label: id, secondary: '', kind: 'laptop', transport: '', heading: true, state, title: '', description: '', linkMissing: false, tasks });
    expect(taskEnvironmentSummaries([section('Offline A', 'disconnected'), section('Offline B', 'disconnected'), section('Empty', 'ready')]).map(summary => summary.text)).toEqual(['Not connected: Offline A (Disconnected), Offline B (Disconnected)']);
    expect(taskEnvironmentSummaries([
      { ...section('a', 'disconnected'), label: 'Studio', secondary: '127.0.0.1:16501' },
      { ...section('b', 'disconnected'), label: 'Studio', secondary: '127.0.0.1:16502' },
      { ...section('c', 'loading'), label: 'Studio', secondary: '127.0.0.1:16503' },
      { ...section('d', 'error'), label: 'Studio', secondary: '127.0.0.1:16504', description: 'Read failed' },
    ]).map(summary => summary.text)).toEqual([
      'Not connected: Studio · 127.0.0.1:16501 (Disconnected), Studio · 127.0.0.1:16502 (Disconnected)',
      'Loading scheduled tasks on Studio · 127.0.0.1:16503',
      'Could not load scheduled tasks: Studio · 127.0.0.1:16504: Read failed',
    ]);
  });
});
