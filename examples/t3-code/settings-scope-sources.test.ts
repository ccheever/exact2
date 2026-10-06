// Settings across environments (D15): the scope menu lists every environment, a write
// reaches each selected connected environment through its own transport, a failure names
// the environments that did not save, and disagreeing targets draw the mixed switch.
import { beforeEach, describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import { applyShell, initialShell, type Obj } from './domain';
import type { Native } from './protocol';
import { applyCoreSetting, decodeClientPrefs, generalSections, resolveScope, serverContext } from './settings-core';
import { settingsCore } from './settings-core-view';
import { fleet, type FleetEntry } from './settings-b-fleet';
import { scopeEnvironments, scopeProjectGroups } from './settings-scope-sources';
import { toasts } from './toast';

const capabilities = { projectSettingsOverrides: true, threadAutoSettlement: true, threadRestartContinuation: true };
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

describe('settings scope across environments', () => {
  test('the menu lists every environment, disambiguates same names by address and marks an offline one', () => {
    const client = focused();
    fleet.entries.set(B, entry(B, 'env-b', 'Studio', {}));
    fleet.entries.set(C, entry(C, 'env-c', 'Build box', {}, 'reconnecting'));
    const scope = resolveScope(as(client), '', '', '');
    expect(scope.environmentChoices.map(choice => [choice.label, choice.mark, choice.offline])).toEqual([
      ['All environments', '', false], ['Studio · http://127.0.0.1:16100', 'laptop', false], ['Studio · http://127.0.0.1:16101', 'server', false], ['Build box', 'server', true]]);
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
      description: 'Could not update Server. The other selected environments saved the change.' });
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
