import { afterEach, describe, expect, test } from 'bun:test';
import { mobileClient } from './client';
import { environmentSources } from './shared/connections';
import { obj, type Obj } from './shared/domain';
import { fleet, environmentKey, type FleetEntry } from './shared/settings-b-fleet';
import type { Native } from './shared/protocol';
import { decodeMobileServerScope, mobileServerPlan, mobileServerProjection, mobileServerSettingsCommand, mobileServerTargets, mobileServerValue } from './settings-server';
import { settingsGrants } from './settings-server-source';

function source(settings: Obj = {}, id = 'a') {
  return environmentSources({ connection: 'connected', origin: `http://${id}.test`, environmentId: id, scopes: [], statusMessage: '',
    config: { settings: { defaultThreadEnvMode: null, worktreeSubmodules: null, branchNamingMode: 'static', defaultRuntimeMode: 'approval-required', ...settings }, environment: { capabilities: { projectSettingsOverrides: true, threadRestartContinuation: true } } } },
  [{ origin: `http://${id}.test`, environmentId: id }], new Map())[0]!;
}
const all = { environmentIds: ['a', 'b'], members: null, projectLabel: '' };
const project = { environmentIds: ['a'], members: [{ environmentId: 'a', id: 'p1' }, { environmentId: 'a', id: 'p2' }], projectLabel: 'Project' };
describe('mobile server scopes and permissions', () => {
  test('real null inherits without Mixed; a different second target becomes Mixed', () => {
    const one = mobileServerProjection('new-threads', all, [source(), source({}, 'b')], new Set(['a', 'b']));
    expect(one.sections[0]!.mixed).toBe(false); expect(one.sections[0]!.rows[0]!.selected).toBe(true);
    const mixed = mobileServerProjection('new-threads', all, [source(), source({ defaultThreadEnvMode: 'local' }, 'b')], new Set(['a', 'b']));
    expect(mixed.sections[0]!.mixed).toBe(true); expect(mixed.sections[0]!.rows.every(row => !row.selected)).toBe(true);
  });
  test('selected unavailable project never broadens into environment writes', () => {
    const scope = decodeMobileServerScope(JSON.stringify({ ...project, members: [] }));
    expect(mobileServerTargets(scope, [source()])).toEqual([]);
    expect(mobileServerProjection('maintenance', scope, [source()], new Set(['a'])).emptyMessage).toContain('Select a project');
  });
  test('one environment write combines physical project members and preserves unrelated overrides', () => {
    const targets = mobileServerTargets(project, [source({ projectSettingsOverrides: { p1: { defaultRuntimeMode: 'full-access', branchNamePrefix: 'human' }, p2: { branchNamingMode: 'semantic' } } })]);
    const planned = mobileServerPlan(targets, { responseStreamingMode: 'paragraph' });
    expect(planned).toEqual([{ environmentId: 'a', patch: { projectSettingsOverrides: { p1: { defaultRuntimeMode: 'full-access', branchNamePrefix: 'human', responseStreamingMode: 'paragraph' }, p2: { branchNamingMode: 'semantic', responseStreamingMode: 'paragraph' } } } }]);
    expect(mobileServerPlan(targets, {}, ['defaultRuntimeMode'])[0]!.patch).toEqual({ projectSettingsOverrides: { p1: { branchNamePrefix: 'human' }, p2: { branchNamingMode: 'semantic' } } });
  });
  test('mobile-only merge credits project override applies beyond the adopted shared key list', () => {
    const data = mobileServerProjection('source-control', { ...project, members: [project.members[0]!] },
      [source({ removeAgentCreditsOnMerge: false, projectSettingsOverrides: { p1: { removeAgentCreditsOnMerge: true } } })], new Set(['a']));
    expect(data.sections.find(section => section.title === 'Pull requests')!.rows[0]!.selected).toBe(true);
    expect(data.hasOverrides).toBe(true);
  });
  test('project choices omit inherit and environment-wide maintenance is disabled', () => {
    const data = mobileServerProjection('new-threads', project, [source()], new Set(['a']));
    expect(data.sections[0]!.rows.some(row => row.value === 'null')).toBe(false);
    const maintenance = mobileServerProjection('maintenance', project, [source()], new Set(['a']));
    expect(maintenance.sections).toHaveLength(1); expect(maintenance.sections[0]!.rows[0]!.disabled).toBe(true);
    expect(() => mobileServerValue('maintenance', 'enableProviderUpdateChecks', 'true', true)).toThrow('scope');
    expect(() => mobileServerValue('new-threads', 'defaultRuntimeMode', 'invented', false)).toThrow('supported');
  });
  test('explicit permissions deny legacy fallbacks; old scope fallback remains source-faithful', () => {
    expect(settingsGrants({ authenticated: true, permissions: [], scopes: ['orchestration:operate'] }, 'settings:write')).toBe(false);
    expect(settingsGrants({ authenticated: true, scopes: ['orchestration:operate'] }, 'settings:write')).toBe(true);
    expect(settingsGrants({ authenticated: true, scopes: ['orchestration:operate'], auth: { serverUpdateScope: 'environment:maintain' } }, 'settings:write')).toBe(false);
    expect(settingsGrants({ authenticated: false, permissions: ['providers:manage'] }, 'providers:manage')).toBe(false);
  });
});
const original = { origin: mobileClient.origin, environmentId: mobileClient.environmentId, connection: mobileClient.connection, config: mobileClient.config, generation: mobileClient.generation };
afterEach(() => { Object.assign(mobileClient, original); fleet.entries.clear(); });
function transport(failB = false, permissions = ['settings:write']) {
  const a = source(), b = source({}, 'b'), requests: Obj[] = [];
  Object.assign(mobileClient, { origin: a.origin, environmentId: 'a', connection: 'connected', generation: 19, config: a.config });
  const background: FleetEntry = { key: environmentKey(b.origin, 'b'), origin: b.origin, environmentId: 'b', phase: 'connected', generation: 20, synchronized: 20,
    config: b.config, shell: { projects: [], threads: [], sequence: 0 }, message: '', traceId: '', lastEvent: 0, subscriptions: {}, scopes: [], error: '', requested: true };
  fleet.entries.set(background.key, background);
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); requests.push(request);
    const isB = typeof request.fleet === 'string';
    if (request.method === 'server.updateSettings' && isB && failB) return { ok: false, generation: 20, error: { kind: 'server', message: 'Target B refused' } };
    const value = request.op === 'environments' ? { saved: [a, b].map(item => ({ origin: item.origin, environmentId: item.environmentId })) }
      : request.op === 'http' ? { authenticated: true, permissions }
      : request.method === 'server.getConfig' ? isB ? b.config : a.config
      : request.method === 'server.updateSettings' ? { ...obj((isB ? b : a).config.settings), ...obj(obj(request.payload).patch) } : {};
    return { ok: true, generation: isB ? 20 : 19, value };
  } };
  return { native, requests };
}
test('all target writes settle and partial failure names failure without hiding success', async () => {
  const { native, requests } = transport(true);
  const result = await mobileServerSettingsCommand('agent-behavior', JSON.stringify(all), 'responseStreamingMode', 'paragraph', native);
  expect(requests.filter(request => request.method === 'server.updateSettings')).toHaveLength(2);
  expect(result.message).toContain('saved on some environments'); expect(result.message).toContain('Target B refused');
  expect(obj(mobileClient.config.settings).responseStreamingMode).toBe('paragraph');
});
test('permission denial and disconnected captured member both prevent every write', async () => {
  const first = transport(false, []);
  expect((await mobileServerSettingsCommand('agent-behavior', JSON.stringify(all), 'responseStreamingMode', 'turn', first.native)).message).toContain('do not allow');
  expect(first.requests.some(request => request.method === 'server.updateSettings')).toBe(false);
  const second = transport(); fleet.entries.clear();
  expect((await mobileServerSettingsCommand('agent-behavior', JSON.stringify(all), 'responseStreamingMode', 'turn', second.native)).message).toContain('scope changed');
  expect(second.requests.some(request => request.method === 'server.updateSettings')).toBe(false);
});
