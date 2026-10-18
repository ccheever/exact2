// Settings across environments (D15): the scope menu lists every environment, a write
// reaches each selected connected environment through its own transport, a failure names
// the environments that did not save, and disagreeing targets draw the mixed switch.
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import { applyShell, initialShell, type Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { applyCoreSetting, decodeClientPrefs, generalSections, inheritanceGroups, resolveScope, serverContext } from '../../src/settings/settings-core';
import { settingsCore } from '../../src/settings/settings-core-view';
import { fleet, type FleetEntry } from '../../src/connections/settings-b-fleet';
import { scopeEnvironments, scopeProjectGroups } from '../../src/settings/settings-scope-sources';
import { restCommand } from '../../src/settings/settings-rest-commands';
import { archivedSettings, lastRunSummary, storageSettings } from '../../src/settings/settings-data';
import { groupDeviceHosts, scopedDeviceHostsView, deviceHostsCommand, hostChecks } from '../../src/device/settings-a-hosts';
import { taskEnvironmentSummaries } from '../../src/settings/scheduled-view';
import { toasts } from '../../src/shell/toast';

const capabilities = { storageCleanup: true, storageCleanupRun: true, projectWorktreeCleanup: true, settledWorktreeCleanup: true, worktreesDirectory: true, projectSettingsOverrides: true, threadAutoSettlement: true, threadRestartContinuation: true };
function focused(settings: Obj = {}) {
  const client = {
    local: { deviceSettings: { composerCollapseOnScroll: true, planModeEnabled: false, timestampFormat: 'locale', appearanceMode: 'system', sendShortcut: 'enter' },
      clientSettings: decodeClientPrefs({}), groupingMode: 'repository', groupingOverrides: {} },
    ready: true, environmentId: 'env-a', origin: 'http://127.0.0.1:16100', writes: [] as Obj[],
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
    config: { environment: { environmentId, label, capabilities, platform: { machine: 'server' } }, settings: { projectSettingsOverrides: {}, ...settings } }, shell, scopes: [], error: '', requested: true };
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
    expect(page.cleanupDisabled).toBe(true);
    expect(page.cleanupDescription).not.toContain('Turn on a cleanup rule');
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
    const core = await settingsCore(as(client), nativeFor(), '', 'github.com/acme/app', '', '', 'general', '', true);
    const start = core.sections.flatMap(section => section.rows).find(item => item.id === 'start-from-origin')!;
    expect([start.mixed, start.inheritance]).toEqual([true, 'mixed']);
    const calls: Obj[] = [];
    await applyCoreSetting(as(client), nativeFor(new Set(), calls), 'start-from-origin:||github.com/acme/app|', 'true');
    expect(client.writes).toEqual([{ patch: { projectSettingsOverrides: { pa: { newWorktreesStartFromOrigin: true } } } }]);
    expect(calls.filter(call => call.method === 'server.updateSettings').map(call => (call.payload as Obj).patch)).toEqual([{ projectSettingsOverrides: { 'p-env-b': { newWorktreesStartFromOrigin: true } } }]);
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
  });
});
