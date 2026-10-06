// Ported from T3 Code 1e2ecbd975 scopedSettings.test.ts (MIT, LICENSE-T3), original names.
// DEFAULT_SERVER_SETTINGS is the subset these cases read; applyServerSettingsPatch is the
// one-level merge the cleanup case needs (storageCleanup merged, worktreeCleanup replaced).
import { describe, expect, it, mock } from 'bun:test';
import type { Obj } from './domain';
import { listProjectOverrides, persistScopedSettingsPatch, planProjectOverridesClear, planScopedSettingsClear, planScopedSettingsPatch, resolveScopedSettingsTargets,
  resolveWorktreeCleanup, scopedPlanNotice, scopedSettingsAreMixed, scopedSettingsSource, selectScopedSettingsEnvironments } from './scoped-settings-plan';
import { resolveSettingsScope, type ResolvedSettingsScope, type ScopeGroup } from './settings-scope';

const DEFAULT_SERVER_SETTINGS: Obj = {
  worktreeCleanup: null, storageCleanup: { worktreeAfterDays: null, worktreeOnMerge: false, worktreeOnDelete: false, worktreeUnchanged: false, browserArtifactsAfterDays: null },
  enableProviderUpdateChecks: true, enableAgentBrowserAccess: true, enableAgentDeviceAccess: false, enableDeviceSupport: false, defaultAutoPull: false, deviceHosts: [],
  sidebarAutoSettleOnMerge: true, defaultModelSelection: null, defaultThreadEnvMode: null, projectSettingsOverrides: {},
  sourceControlWritingStyle: { mode: 'repo_conventions', customInstructions: '', followChangeRequestTemplates: true },
};
const applyServerSettingsPatch = (current: Obj, patch: Obj): Obj => ({ ...current, ...patch,
  ...(patch.storageCleanup ? { storageCleanup: { ...(current.storageCleanup as Obj), ...(patch.storageCleanup as Obj) } } : {}) });

function environment(id: string, options: { connected?: boolean; loaded?: boolean; settings?: Obj; projectOverrides?: boolean } = {}) {
  return { environmentId: id, label: id, connection: { phase: options.connected === false ? 'offline' : 'connected' },
    serverConfig: options.loaded === false ? null : { settings: { ...DEFAULT_SERVER_SETTINGS, ...options.settings }, environment: { capabilities: { projectSettingsOverrides: options.projectOverrides !== false } } } };
}
const laptop = environment('Laptop');
const server = environment('Server');
const offline = environment('Offline', { connected: false });
const loading = environment('Loading', { loaded: false });
const environments = [laptop, server, offline, loading];
const all = resolveSettingsScope({}, [], environments);
const named = resolveSettingsScope({ machine: server.environmentId }, [], environments);
const projectId = 'project';
const laptopProjectId = 'laptop-project';
const member = { id: projectId, environmentId: server.environmentId, title: 'Project', workspaceRoot: '/repo', physicalProjectKey: `${server.environmentId}:/repo`, environmentLabel: server.label };
const laptopMember = { ...member, id: laptopProjectId, environmentId: laptop.environmentId, physicalProjectKey: `${laptop.environmentId}:/repo`, environmentLabel: laptop.label };
const group: ScopeGroup = { ...member, projectKey: 'project-group', displayName: 'Project', memberProjects: [member, laptopMember] };
const project = resolveSettingsScope({ project: group.projectKey }, [group], environments);
const checkout = resolveSettingsScope({ project: group.projectKey, machine: server.environmentId, checkout: member.physicalProjectKey }, [group], environments);
const success = () => mock(async () => ({ _tag: 'Success' as const }));

describe('scoped settings targets', () => {
  it('uses the named environment even when a different primary is available', () => {
    const selected = selectScopedSettingsEnvironments(named, environments, laptop.environmentId);
    expect(selected.environments).toEqual([server]);
    expect(selected.environment).toBe(server);
  });
  it('keeps an offline named environment selected without falling back to primary', () => {
    const scope = resolveSettingsScope({ machine: offline.environmentId }, [], environments);
    const selected = selectScopedSettingsEnvironments(scope, environments, laptop.environmentId);
    expect(selected.environments).toEqual([offline]);
    expect(selected.connectedEnvironments).toEqual([]);
    expect(selected.environment).toBeNull();
    expect(planScopedSettingsPatch(scope, environments, { enableProviderUpdateChecks: false })).toMatchObject({ serverWrites: [], unavailableReason: 'Connect Offline to save this setting.' });
  });
  it('prefers the selected primary as the aggregate representative without including disconnected targets', () => {
    const selected = selectScopedSettingsEnvironments(all, environments, server.environmentId);
    expect(selected.environment).toBe(server);
    expect(selected.environments).toEqual(environments);
    expect(selected.connectedEnvironments).toEqual([laptop, server]);
  });
  it("resolves each member's effective settings and source at project scope", () => {
    const overridden = environment('Server', { settings: { defaultAutoPull: false, projectSettingsOverrides: { [projectId]: { defaultAutoPull: true } } } });
    const targets = resolveScopedSettingsTargets(project, [laptop, overridden]);
    expect(targets.map(target => [target.projectId, target.settings.defaultAutoPull])).toEqual([[projectId, true], [laptopProjectId, false]]);
    expect(scopedSettingsSource(targets, ['defaultAutoPull'])).toBe('mixed');
    expect(scopedSettingsSource([targets[0]!], ['defaultAutoPull'])).toBe('project');
    expect(scopedSettingsSource(targets, ['enableProviderUpdateChecks'])).toBe('environment');
    expect(scopedSettingsAreMixed(targets, ['defaultAutoPull'])).toBe(true);
  });
});

describe('scoped settings writes', () => {
  it("edits the effective machine policy without changing other machines' rules", () => {
    const custom = environment('Laptop', { settings: { worktreeCleanup: { mode: 'custom', rules: { worktreeAfterDays: 12, worktreeOnDelete: true, worktreeOnMerge: true, worktreeUnchanged: false } } } });
    const disabled = environment('Server', { settings: { worktreeCleanup: { mode: 'off' } } });
    const targets = [custom, disabled];
    const plan = planScopedSettingsPatch(all, targets, { storageCleanup: { worktreeOnDelete: false } });
    const policies = plan.serverWrites.map((write, index) => resolveWorktreeCleanup(applyServerSettingsPatch(targets[index]!.serverConfig!.settings, write.patch), null));
    expect(policies).toEqual([{ worktreeAfterDays: 12, worktreeOnDelete: false, worktreeOnMerge: true, worktreeUnchanged: false },
      { worktreeAfterDays: null, worktreeOnDelete: false, worktreeOnMerge: false, worktreeUnchanged: false }]);
  });
  it('isolates a formerly shared server preference to the named environment', async () => {
    const persistServer = success(), persistClient = mock(() => {});
    await persistScopedSettingsPatch(planScopedSettingsPatch(named, environments, { sidebarAutoSettleOnMerge: false }), persistServer, persistClient);
    expect(persistServer.mock.calls as unknown[]).toEqual([[{ environmentId: server.environmentId, input: { patch: { sidebarAutoSettleOnMerge: false } } }]]);
    expect(persistClient).not.toHaveBeenCalled();
  });
  it('writes an aggregate preference only to connected environments with loaded configuration', async () => {
    const persistServer = success(), persistClient = mock(() => {});
    await persistScopedSettingsPatch(planScopedSettingsPatch(all, environments, { enableProviderUpdateChecks: false }), persistServer, persistClient);
    expect((persistServer.mock.calls as unknown as [{ environmentId: string }][]).map(([input]) => input.environmentId)).toEqual([laptop.environmentId, server.environmentId]);
    expect(persistClient).not.toHaveBeenCalled();
  });
  it('persists client keys locally at any scope alongside server keys', async () => {
    const persistServer = success(), persistClient = mock((_patch: Obj) => {});
    await persistScopedSettingsPatch(planScopedSettingsPatch(named, environments, { diffIgnoreWhitespace: false, enableProviderUpdateChecks: false }), persistServer, persistClient);
    expect(persistClient.mock.calls as unknown[]).toEqual([[{ diffIgnoreWhitespace: false }]]);
    expect(persistServer.mock.calls as unknown[]).toEqual([[{ environmentId: server.environmentId, input: { patch: { enableProviderUpdateChecks: false } } }]]);
  });
  it("writes project overrides into each member's entry on its environment", () => {
    const withExisting = environment('Server', { settings: { projectSettingsOverrides: { [projectId]: { enableAgentBrowserAccess: false } } } });
    const plan = planScopedSettingsPatch(project, [laptop, withExisting], { defaultAutoPull: true });
    expect(plan.unavailableReason).toBeNull();
    expect(plan.serverWrites).toEqual([
      { environmentId: server.environmentId, label: server.label, patch: { projectSettingsOverrides: { [projectId]: { enableAgentBrowserAccess: false, defaultAutoPull: true } } } },
      { environmentId: laptop.environmentId, label: laptop.label, patch: { projectSettingsOverrides: { [laptopProjectId]: { defaultAutoPull: true } } } },
    ]);
    expect(planScopedSettingsPatch(checkout, [laptop, server], { defaultAutoPull: true })).toMatchObject({ serverWrites: [{ environmentId: server.environmentId }] });
  });
  it("keeps each project's other cleanup rules when changing one rule across machines", () => {
    const machine = environment('Laptop', { settings: { storageCleanup: { ...(DEFAULT_SERVER_SETTINGS.storageCleanup as Obj), worktreeAfterDays: 30 } } });
    const customized = environment('Server', { settings: { projectSettingsOverrides: { [projectId]: { defaultAutoPull: true,
      worktreeCleanup: { mode: 'custom', rules: { worktreeAfterDays: 8, worktreeOnDelete: false, worktreeOnMerge: true, worktreeUnchanged: false } } } } } });
    const plan = planScopedSettingsPatch(project, [machine, customized], { worktreeCleanup: { mode: 'custom', rules: { worktreeOnDelete: true } } });
    expect(plan.serverWrites.map(write => write.patch.projectSettingsOverrides)).toEqual([
      { [projectId]: { defaultAutoPull: true, worktreeCleanup: { mode: 'custom', rules: { worktreeAfterDays: 8, worktreeOnDelete: true, worktreeOnMerge: true, worktreeUnchanged: false } } } },
      { [laptopProjectId]: { worktreeCleanup: { mode: 'custom', rules: { worktreeAfterDays: 30, worktreeOnDelete: true, worktreeOnMerge: false, worktreeUnchanged: false } } } },
    ]);
    expect(planScopedSettingsClear(checkout, [customized], ['worktreeCleanup']).serverWrites[0]?.patch).toEqual({ projectSettingsOverrides: { [projectId]: { defaultAutoPull: true } } });
    expect(planScopedSettingsPatch(project, [machine, customized], { storageCleanup: { browserArtifactsAfterDays: 8 } }).serverWrites).toEqual([]);
  });
  it('scopes agent device access to projects while keeping hub and hosts environment-wide', () => {
    const plan = planScopedSettingsPatch(project, [laptop, server], { enableAgentDeviceAccess: true });
    expect(plan.serverWrites.map(write => write.patch)).toEqual([
      { projectSettingsOverrides: { [projectId]: { enableAgentDeviceAccess: true } } }, { projectSettingsOverrides: { [laptopProjectId]: { enableAgentDeviceAccess: true } } }]);
    for (const patch of [{ enableDeviceSupport: true }, { deviceHosts: [] }]) expect(planScopedSettingsPatch(project, environments, patch).serverWrites).toEqual([]);
  });
  it('refuses environment-wide keys and older servers at project scope', () => {
    expect(planScopedSettingsPatch(project, environments, { enableProviderUpdateChecks: false }))
      .toMatchObject({ serverWrites: [], unavailableReason: 'This setting is environment-wide and cannot be overridden by a project.' });
    const legacy = environment('Server', { projectOverrides: false });
    expect(planScopedSettingsPatch(checkout, [laptop, legacy], { defaultAutoPull: true })).toMatchObject({ serverWrites: [], unavailableReason: expect.stringContaining('update') });
  });
  it('clears overrides per member and removes an emptied entry', () => {
    const withOverrides = environment('Server', { settings: { projectSettingsOverrides: { [projectId]: { defaultAutoPull: true, enableAgentBrowserAccess: false } } } });
    const plan = planScopedSettingsClear(checkout, [laptop, withOverrides], ['defaultAutoPull']);
    expect(plan.serverWrites).toEqual([{ environmentId: server.environmentId, label: server.label, patch: { projectSettingsOverrides: { [projectId]: { enableAgentBrowserAccess: false } } } }]);
    expect(planScopedSettingsClear(checkout, [laptop, withOverrides], ['defaultAutoPull', 'enableAgentBrowserAccess']).serverWrites[0]?.patch).toEqual({ projectSettingsOverrides: { [projectId]: null } });
  });
  it('never substitutes an environment-default write for an invalid scope', () => {
    const scope = resolveSettingsScope({ machine: 'removed' }, [group], environments);
    const plan = planScopedSettingsPatch(scope, environments, { enableAgentBrowserAccess: false });
    expect(plan.serverWrites).toEqual([]);
    expect(plan.hasClientWrite).toBe(false);
    expect(plan.unavailableReason).not.toBeNull();
  });
  it('waits for every target and identifies both RPC failures and rejected writes', async () => {
    const third = environment('Third'), fourth = environment('Fourth');
    const selected = [...environments, third, fourth];
    const scope = resolveSettingsScope({}, [], selected);
    const answers: (() => Promise<{ _tag: 'Success' | 'Failure' }>)[] = [async () => ({ _tag: 'Success' }), async () => ({ _tag: 'Failure' }),
      async () => { throw new Error('Disconnected during save'); }, async () => ({ _tag: 'Success' })];
    const persistServer = mock(() => answers.shift()!());
    const result = await persistScopedSettingsPatch(planScopedSettingsPatch(scope, selected, { enableAgentBrowserAccess: false }), persistServer, () => {});
    expect(result.savedEnvironmentCount).toBe(2);
    expect(result.failedEnvironments.map(({ label }) => label)).toEqual([server.label, third.label]);
    expect(persistServer).toHaveBeenCalledTimes(4);
    // useRunScopedPlan's toast for this result.
    expect(scopedPlanNotice({ unavailableReason: null }, result)).toEqual({ kind: 'error', title: 'Setting saved on some environments',
      description: 'Could not update Server, Third. The other selected environments saved the change.' });
  });
});

describe('scoped settings mixed values', () => {
  it('compares only requested settings across connected targets', () => {
    const changed = environment('Changed', { settings: { enableAgentBrowserAccess: false } });
    const targets = resolveScopedSettingsTargets(all, [laptop, changed]);
    expect(scopedSettingsAreMixed(targets, ['enableAgentBrowserAccess'])).toBe(true);
    expect(scopedSettingsAreMixed(targets, ['enableProviderUpdateChecks'])).toBe(false);
    expect(scopedSettingsAreMixed([], ['enableAgentBrowserAccess'])).toBe(false);
  });
  it('treats independently decoded equal nested settings as the same value', () => {
    const style = { ...(DEFAULT_SERVER_SETTINGS.sourceControlWritingStyle as Obj), mode: 'custom', customInstructions: 'Use plain language' };
    const first = environment('First', { settings: { sourceControlWritingStyle: { ...style } } });
    const second = environment('Second', { settings: { sourceControlWritingStyle: { customInstructions: 'Use plain language', followChangeRequestTemplates: true, mode: 'custom' } } });
    const targets = resolveScopedSettingsTargets(all, [first, second]);
    expect(scopedSettingsAreMixed(targets, ['sourceControlWritingStyle'])).toBe(false);
  });
});

describe('project overrides at environment scope', () => {
  const laptopEnv = 'laptop', desk = 'desk', fleet = 'fleet', t3 = 't3';
  const env = (environmentId: string, overrides: Obj) => ({ environmentId, label: environmentId, connection: { phase: 'connected' },
    serverConfig: { settings: { ...DEFAULT_SERVER_SETTINGS, projectSettingsOverrides: overrides }, environment: { capabilities: { projectSettingsOverrides: true } } } });
  it('lists only the projects that override the keys', () => {
    const entries = listProjectOverrides([env(laptopEnv, { [fleet]: { defaultAutoPull: true, defaultThreadEnvMode: 'local' }, [t3]: { defaultThreadEnvMode: 'local' } }),
      env(desk, { [fleet]: { defaultAutoPull: false } })], ['defaultAutoPull']);
    expect(entries).toEqual([{ environmentId: laptopEnv, projectId: fleet }, { environmentId: desk, projectId: fleet }]);
  });
  it('clears only those keys and drops entries that become empty', () => {
    const plan = planProjectOverridesClear([env(laptopEnv, { [fleet]: { defaultAutoPull: true, defaultThreadEnvMode: 'local' }, [t3]: { defaultAutoPull: true } })],
      [{ environmentId: laptopEnv, projectId: fleet }, { environmentId: laptopEnv, projectId: t3 }], ['defaultAutoPull']);
    expect(plan.serverWrites).toEqual([{ environmentId: laptopEnv, label: laptopEnv, patch: { projectSettingsOverrides: { [fleet]: { defaultThreadEnvMode: 'local' }, [t3]: null } } }]);
  });
});

const projectScope = (environmentId: string, member: Obj): ResolvedSettingsScope => ({ kind: 'project', group: {} as never, environmentId: null, label: 'fleet', members: [member as never], environmentIds: [environmentId] });

describe('null patches at project scope', () => {
  it('removes the override for keys that cannot store null and keeps it for keys that can', () => {
    const environmentId = 'laptop', id = 'fleet';
    const scope = projectScope(environmentId, { id, environmentId });
    const list = [{ environmentId, label: 'Laptop', connection: { phase: 'connected' }, serverConfig: { settings: { ...DEFAULT_SERVER_SETTINGS,
      projectSettingsOverrides: { [id]: { defaultThreadEnvMode: 'worktree', defaultAutoPull: true } } }, environment: { capabilities: { projectSettingsOverrides: true } } } }];
    expect(planScopedSettingsPatch(scope, list, { defaultThreadEnvMode: null }).serverWrites[0]?.patch).toEqual({ projectSettingsOverrides: { [id]: { defaultAutoPull: true } } });
    expect(planScopedSettingsPatch(scope, list, { defaultModelSelection: null }).serverWrites[0]?.patch)
      .toEqual({ projectSettingsOverrides: { [id]: { defaultThreadEnvMode: 'worktree', defaultAutoPull: true, defaultModelSelection: null } } });
  });
});

describe('partial object patches at project scope', () => {
  it("completes a writing style field patch from the target's effective value", () => {
    const environmentId = 'laptop', id = 'fleet';
    const settings = { ...DEFAULT_SERVER_SETTINGS, projectSettingsOverrides: { [id]: { sourceControlWritingStyle: { mode: 'custom', customInstructions: 'Keep it short.', followChangeRequestTemplates: false } } } };
    const plan = planScopedSettingsPatch(projectScope(environmentId, { id, environmentId, physicalProjectKey: 'laptop:/repo', workspaceRoot: '/repo' }),
      [{ environmentId, label: 'Laptop', connection: { phase: 'connected' }, serverConfig: { settings, environment: { capabilities: { projectSettingsOverrides: true } } } }],
      { sourceControlWritingStyle: { customInstructions: 'Be terse.' } });
    expect(plan.serverWrites[0]?.patch).toEqual({ projectSettingsOverrides: { [id]: { sourceControlWritingStyle: { mode: 'custom', customInstructions: 'Be terse.', followChangeRequestTemplates: false } } } });
  });
});
