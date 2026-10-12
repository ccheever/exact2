// Ported from T3 Code 1e2ecbd975 settingsScope.test.ts and settingsScopeAxis.test.ts (MIT, LICENSE-T3).
// Original describe/it names; EnvironmentId/ProjectId.make become plain strings.
import { describe, expect, it } from 'bun:test';
import { environmentAxisValue, projectAxisValue, resolveSettingsScope, selectEnvironmentAxis, selectProjectAxis, settingsScopeEnvironmentLabel,
  validateSettingsScopeSearch, type ScopeGroup, type ScopeMember } from './settings-scope';

const laptopId = 'laptop';
const serverId = 'server';
const environments = [{ environmentId: laptopId, label: 'Laptop' }, { environmentId: serverId, label: 'Server' }];
function member(id: string, environmentId: string): ScopeMember {
  return { id, environmentId, title: 'T3 Code', workspaceRoot: `/repos/${id}`, physicalProjectKey: `${environmentId}:/repos/${id}`,
    environmentLabel: environments.find(environment => environment.environmentId === environmentId)?.label ?? null };
}
const first = member('first', laptopId), second = member('second', laptopId), third = member('third', serverId), other = member('other', serverId);
const group = (projectKey: string, members: readonly ScopeMember[]): ScopeGroup => ({ ...members[0]!, projectKey, displayName: projectKey, memberProjects: members });
const groups = [group('t3code', [first, second, third]), group('other', [other])];

describe('settings scope search', () => {
  it('ignores the retired scope key from older links', () => {
    expect(validateSettingsScopeSearch({ scope: 'device', project: 't3code' })).toEqual({ project: 't3code' });
    expect(validateSettingsScopeSearch({ scope: 'all' })).toEqual({});
  });
  it('retains legacy project and machine links without inventing an explicit broad scope', () => {
    expect(validateSettingsScopeSearch({ project: 't3code', machine: laptopId, unused: true })).toEqual({ project: 't3code', machine: laptopId });
  });
  it('retains an orphan checkout so it cannot turn into all environments', () => {
    const search = validateSettingsScopeSearch({ checkout: first.physicalProjectKey });
    expect(search).toEqual({ checkout: first.physicalProjectKey });
    expect(resolveSettingsScope(search, groups, environments)).toMatchObject({ kind: 'unavailable', reason: 'project-required', members: [], environmentIds: [] });
  });
});

describe('settings scope resolution', () => {
  it('defaults to every environment with no project', () => {
    expect(resolveSettingsScope({}, groups, environments)).toMatchObject({ kind: 'all', members: [], environmentIds: [laptopId, serverId] });
  });
  it('resolves one environment without targeting its project overrides', () => {
    expect(resolveSettingsScope({ machine: serverId }, groups, environments)).toMatchObject({ kind: 'environment', environmentId: serverId, environmentIds: [serverId], members: [] });
  });
  it('keeps all physical members in a project aggregate, including several on one environment', () => {
    expect(resolveSettingsScope({ project: 't3code' }, groups, environments)).toMatchObject({ kind: 'project', environmentId: null, members: [first, second, third], environmentIds: [laptopId, serverId] });
  });
  it('preserves legacy project plus machine aggregates with multiple checkouts', () => {
    expect(resolveSettingsScope({ project: 't3code', machine: laptopId }, groups, environments))
      .toMatchObject({ kind: 'project', environmentId: laptopId, label: 't3code / Laptop', members: [first, second], environmentIds: [laptopId] });
  });
  it('narrows a checkout target to exactly one member, deriving its environment when omitted', () => {
    expect(resolveSettingsScope({ project: 't3code', checkout: second.physicalProjectKey }, groups, environments))
      .toMatchObject({ kind: 'checkout', checkout: second, environmentId: laptopId, label: 't3code / Laptop · /repos/second', members: [second], environmentIds: [laptopId] });
  });
  it.each([
    { project: 'missing' }, { machine: 'removed' }, { project: 't3code', machine: 'removed' }, { project: 't3code', checkout: 'deleted' },
    { project: 'other', machine: laptopId }, { project: 'other', checkout: first.physicalProjectKey }, { project: 't3code', machine: serverId, checkout: first.physicalProjectKey },
  ])('never widens an invalid or stale target: %j', search => {
    expect(resolveSettingsScope(search, groups, environments)).toMatchObject({ kind: 'unavailable', members: [], environmentIds: [] });
  });
  it('leaves a removed checkout unavailable while sibling checkouts remain', () => {
    const search = { project: 't3code', machine: laptopId, checkout: first.physicalProjectKey };
    expect(resolveSettingsScope(search, groups, environments)).toMatchObject({ kind: 'checkout', members: [first] });
    expect(resolveSettingsScope(search, [group('t3code', [second, third])], environments)).toMatchObject({ kind: 'unavailable', members: [], environmentIds: [] });
  });
  it("does not select another environment after removing a project's last local checkout", () => {
    const search = { project: 't3code', machine: laptopId };
    expect(resolveSettingsScope(search, groups, environments)).toMatchObject({ kind: 'project', members: [first, second] });
    expect(resolveSettingsScope(search, [group('t3code', [third])], environments)).toMatchObject({ kind: 'unavailable', members: [], environmentIds: [] });
  });
  it('rejects a cached checkout whose environment was removed', () => {
    expect(resolveSettingsScope({ project: 't3code', checkout: third.physicalProjectKey }, groups, environments.slice(0, 1)))
      .toMatchObject({ kind: 'unavailable', reason: 'environment-missing', members: [], environmentIds: [] });
  });
});

const firstEnv = { environmentId: 'first', label: 'Development', displayUrl: 'https://first.example.com' as string | null };
const secondEnv = { environmentId: 'second', label: 'Development', displayUrl: 'https://second.example.com' as string | null };

describe('settings scope environment labels', () => {
  it('distinguishes same-name environments by address', () => {
    const list = [firstEnv, secondEnv];
    expect(list.map(environment => settingsScopeEnvironmentLabel(environment, list))).toEqual(['Development · https://first.example.com', 'Development · https://second.example.com']);
  });
  it('falls back to environment IDs when duplicate names have no display URL', () => {
    const list = [firstEnv, secondEnv].map(environment => ({ ...environment, displayUrl: null }));
    expect(list.map(environment => settingsScopeEnvironmentLabel(environment, list))).toEqual(['Development · first', 'Development · second']);
  });
  it('keeps unique names compact and removes disambiguation after a rename', () => {
    expect(settingsScopeEnvironmentLabel(firstEnv, [firstEnv])).toBe('Development');
    expect(settingsScopeEnvironmentLabel(firstEnv, [firstEnv, { ...secondEnv, label: 'Production' }])).toBe('Development');
  });
});

describe('settings scope axes', () => {
  it('maps each axis to its search key and back', () => {
    expect(projectAxisValue({})).toBe('all');
    expect(projectAxisValue({ project: 'app' })).toBe('app');
    expect(selectProjectAxis({ machine: 'second' }, 'app')).toEqual({ project: 'app', machine: 'second' });
    expect(selectProjectAxis({ machine: 'second', project: 'app' }, 'all')).toEqual({ machine: 'second' });
    expect(selectEnvironmentAxis({ project: 'app' }, 'first')).toEqual({ project: 'app', machine: 'first' });
    expect(selectEnvironmentAxis({ project: 'app', machine: 'first' }, 'all')).toEqual({ project: 'app' });
  });
  it('drops a checkout narrowing from older links when either axis changes', () => {
    const checkout = { project: 'app', checkout: 'app@first', machine: 'first' };
    expect(selectEnvironmentAxis(checkout, 'second')).toEqual({ project: 'app', machine: 'second' });
    expect(selectProjectAxis(checkout, 'app')).toEqual({ project: 'app', machine: 'first' });
  });
});

describe('environmentAxisValue', () => {
  it("shows the checkout's environment for a legacy checkout link", () => {
    expect(environmentAxisValue({ project: 'p', checkout: 'c' }, 'laptop')).toBe('laptop');
    expect(environmentAxisValue({ project: 'p' }, null)).toBe('all');
    expect(environmentAxisValue({ machine: 'desk' }, 'laptop')).toBe('desk');
  });
});
