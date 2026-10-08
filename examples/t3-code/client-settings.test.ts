import { keyboardSettings, validShortcut, validWhen } from './keybinding-settings';
import { scheduledSettings, taskDraft, taskFromArguments, validateTaskInput } from './scheduled-settings';
import { scopedSettingPatch, scopedControls, scopedSearchControls } from './scoped-settings';
import { beforeAll, describe, test, expect } from 'bun:test';
import { T3Client } from './client';
import { snapshot } from './presentation';
import { obj, arr, str, type Obj } from './domain';
import { archivedSettings, decodeNotices, licenseSettings, diagnosticsSettings, storageSettings } from './settings-data';
import { timestamp, thread, storage, connected, opened } from './client-fixture';

// The sidebar asks each project's favicon once per process (r3-sidebar-glyph.ts syncFavicons), and the
// fixture's Backend records that ask in `committed`. These tests ran after client.test.ts's first ones, which
// had asked it already; one connection first gives them the same start.
beforeAll(async () => { await connected(); });

describe('project management and scoped defaults', () => {
  test('rename validates title and updates canonical project without another entry', async () => {
    const { client, native, command } = await connected();
    expect((await command('rename-project', 'p1', '  ')).message).toBe('Project title cannot be empty');
    expect(client.error).toBe('');
    expect(native.committed).toHaveLength(0);
    expect((await command('rename-project', 'p1', '  Renamed fixture  ')).message).toBe('');
    expect(client.shell.projects).toHaveLength(1);
    expect(client.shell.projects[0]).toMatchObject({ id: 'p1', title: 'Renamed fixture', workspaceRoot: '/repo' });
    expect(native.committed.at(-1)).toMatchObject({ type: 'project.update', projectId: 'p1', title: 'Renamed fixture' });
  });
  test('remove clears owner/drafts, keeps unrelated project and does not issue filesystem operations', async () => {
    const { client, native, command } = await opened();
    native.shell.projects = [...arr(native.shell.projects), { id: 'p2', title: 'Other', workspaceRoot: '/other' }];
    client.local.drafts['env1:t1'] = 'removed draft'; client.local.drafts['env1:new:p1'] = 'removed new draft';
    client.local.drafts['env1:new:p2'] = 'keep';
    expect((await command('remove-project', 'p1')).message).toBe('');
    expect(client.projectId).toBe('p2'); expect(client.threadId).toBe('');
    expect(client.local.drafts['env1:t1']).toBeUndefined(); expect(client.local.drafts['env1:new:p1']).toBeUndefined();
    expect(client.local.drafts['env1:new:p2']).toBe('keep');
    expect(native.committed.at(-1)).toMatchObject({ type: 'project.delete', projectId: 'p1', force: true });
    expect(native.calls.filter(call => String(call.op).includes('deleteFile'))).toHaveLength(0);
  });
  test('scoped permissions preserve unrelated overrides; inheritance changes only new-thread default', async () => {
    const { client, native, command } = await connected();
    obj(native.config.settings).projectSettingsOverrides = { p1: { defaultRuntimeMode: 'full-access', worktreeSubmodules: 'recursive' } };
    expect((await command('setting-permissions', 'env1:p1', 'approval-required')).message).toBe('');
    expect(obj(obj(obj(client.config.settings).projectSettingsOverrides).p1)).toEqual({ defaultRuntimeMode: 'approval-required', worktreeSubmodules: 'recursive' });
    expect(client.runtimeMode).toBe('approval-required');
    expect((await command('setting-permissions', 'env1:p1', 'inherit')).message).toBe('');
    expect(obj(obj(obj(client.config.settings).projectSettingsOverrides).p1)).toEqual({ worktreeSubmodules: 'recursive' });
    expect(client.runtimeMode).toBe('full-access');
    await command('select-thread', 't1'); await client.refresh(native, storage().files);
    await command('setting-permissions', 'env1:', 'full-access');
    expect(client.runtimeMode).toBe('approval-required');
  });
});

test('stale settings project refuses write rather than expanding to environment', async () => {
  const { client, native, command } = await connected();
  const before = JSON.stringify(native.config.settings);
  expect((await command('setting-permissions', 'env1:removed-id', 'approval-required')).message).toBe('That project is no longer available.');
  expect(JSON.stringify(native.config.settings)).toBe(before);
  expect(native.calls.filter(call => call.method === 'server.updateSettings')).toHaveLength(0);
});

test('stale environment refuses scoped setting write', async () => {
  const { native, command } = await connected();
  expect((await command('setting-permissions', 'old-environment:p1', 'full-access')).message).toBe('That environment is no longer selected.');
  expect(native.calls.filter(call => call.method === 'server.updateSettings')).toHaveLength(0);
});

test('repository group management resolves current members and leaves unrelated checkout untouched', async () => {
  const { client, native, command } = await connected();
  native.shell.projects = [
    { id: 'p1', title: 'One', workspaceRoot: '/repo/one', repositoryIdentity: { canonicalKey: 'repo-fixture', displayName: 'Fixture' } },
    { id: 'p2', title: 'Two', workspaceRoot: '/repo/two', repositoryIdentity: { canonicalKey: 'repo-fixture', displayName: 'Fixture' } },
    { id: 'p3', title: 'Other', workspaceRoot: '/other' } ];
  client.shell = { ...client.shell, projects: arr(native.shell.projects) };
  expect((await command('rename-group', 'repo-fixture', 'Together')).message).toBe('');
  expect(client.shell.projects.map(project => project.title)).toEqual(['Other', 'Together', 'Together']);
  expect((await command('remove-group', 'repo-fixture')).message).toBe('');
  expect(client.shell.projects.map(project => project.id)).toEqual(['p3']);
  expect((await command('rename-group', 'missing', 'No')).message).toBe('That project group is no longer available.');
});

test('provider management preserves configuration and targets one current instance', async () => {
  const { client, native, command } = await connected();
  const original = { driver: 'codex', displayName: 'Fixture', enabled: true, accentColor: '#123456', environment: { FIXTURE: 'isolated' }, config: { cliPath: '/fixture/codex', codexHome: '/fixture/home' } };
  obj(native.config.settings).providerInstances = { fixture: original, sibling: { driver: 'claude', enabled: false } };
  expect((await command('provider-name', 'fixture', '  ')).message).toBe('Provider name cannot be empty.');
  expect(client.error).toBe('');
  expect(native.calls.filter(call => call.method === 'server.updateSettings')).toHaveLength(0);
  expect((await command('provider-name', 'fixture', '  Renamed  ')).message).toBe('');
  expect(obj(obj(client.config.settings).providerInstances).fixture).toEqual({ ...original, displayName: 'Renamed' });
  expect((await command('provider-enabled', 'fixture', 'false')).message).toBe('');
  expect(client.configuredProviders().find(provider => provider.id === 'fixture')?.enabled).toBe(false);
  expect((await command('provider-enabled', 'fixture', 'true')).message).toBe('');
  expect(obj(obj(client.config.settings).providerInstances).fixture).toEqual({ ...original, displayName: 'Renamed' });
  expect((await command('provider-remove', 'fixture')).message).toBe('');
  expect(obj(obj(client.config.settings).providerInstances)).toEqual({ sibling: { driver: 'claude', enabled: false } });
  const writes = native.calls.filter(call => call.method === 'server.updateSettings').length;
  expect((await command('provider-name', 'fixture', 'Stale')).message).toBe('That provider instance is no longer available.');
  expect(native.calls.filter(call => call.method === 'server.updateSettings')).toHaveLength(writes);
});

test('provider creation validates advertised driver/identity and uses atomic create without replacing siblings', async () => {
  const { client, native, command } = await connected();
  obj(native.config.settings).providerInstances = { keep: { driver: 'codex', displayName: 'Keep', config: { untouched: true } } };
  const draft = JSON.stringify({ driver: 'codex', name: 'Disposable "quoted" fixture', binaryPath: '/fixture/codex', homePath: '/fixture/home' });
  expect((await command('provider-create', '123-invalid', draft)).message).toContain('starting with a letter');
  expect((await command('provider-create', 'keep', draft)).message).toBe('That provider instance ID already exists.');
  expect((await command('provider-create', 'new', JSON.stringify({ driver: 'unadvertised', name: 'No' }))).message).toContain('advertised');
  expect(native.calls.filter(call => call.method === 'server.updateSettings')).toHaveLength(0);
  expect((await command('provider-create', 'disposable', draft)).message).toBe('');
  expect(obj(obj(client.config.settings).providerInstances).keep).toEqual({ driver: 'codex', displayName: 'Keep', config: { untouched: true } });
  expect(obj(obj(client.config.settings).providerInstances).disposable).toEqual({ driver: 'codex', displayName: 'Disposable "quoted" fixture', enabled: true, config: { setupMode: 'existing', binaryPath: '/fixture/codex', homePath: '/fixture/home' } });
  expect(obj(obj(native.calls.find(call => obj(obj(call.payload).providerInstanceMutation).instanceId === 'disposable')).payload).providerInstanceMutation).toMatchObject({ operation: 'create' });
  expect((await command('provider-remove', 'disposable')).message).toBe('');
  expect(client.configuredProviders().map(provider => provider.id)).toEqual(['keep']);
});

test('repository_path and physical overrides persist; guarded member removal retains sibling', async () => {
  const { client, native, command, disk } = await connected();
  native.shell.projects = [{ id: 'p1', title: 'One', workspaceRoot: '/repo/one/', repositoryIdentity: { canonicalKey: 'repo', rootPath: '/repo' } }, { id: 'p2', title: 'Two', workspaceRoot: '/repo/two', repositoryIdentity: { canonicalKey: 'repo', rootPath: '/repo' } }];
  client.shell = { ...client.shell, projects: arr(native.shell.projects) };
  await command('grouping-mode', '', 'repository_path');
  expect(client.projectGroups().map(group => group.key)).toEqual(['repo::one', 'repo::two']);
  await command('grouping-override', 'p1', 'repository');
  expect(client.projectGroups().map(group => group.key)).toEqual(['repo', 'repo::two']);
  await command('grouping-override', 'p1', 'inherit');
  expect(client.local.groupingOverrides).toEqual({});
  await command('grouping-mode', '', 'repository');
  expect((await command('remove-group-member', 'p1', 'stale-group')).message).toContain('membership changed');
  expect(native.committed).toHaveLength(0);
  expect((await command('remove-group-member', 'p1', 'repo')).message).toBe('');
  expect(client.shell.projects.map(project => project.id)).toEqual(['p2']);
  expect((await command('grouping-override', 'p1', 'separate')).message).toBe('That project is no longer available.');
  await command('grouping-mode', '', 'repository_path');
  await command('grouping-override', 'p2', 'separate');
  const restored = new T3Client(); await restored.refresh(native, disk.files);
  expect(restored.local.groupingMode).toBe('repository_path');
  expect(restored.local.groupingOverrides).toEqual({ 'env1:/repo/two': 'separate' });
});

test('device settings persist locally, affect projection, and do not write an unavailable server scope', async () => {
  const { client, native, command, disk } = await connected();
  const before = JSON.stringify(native.config.settings);
  await command('device-setting', 'composerCollapseOnScroll', 'false');
  await command('device-setting', 'planModeEnabled', 'true');
  await command('device-setting', 'timestampFormat', '24-hour');
  expect(snapshot(client)).toMatchObject({ composerCollapseOnScroll: false, planModeEnabled: true });
  expect(JSON.stringify(native.config.settings)).toBe(before);
  expect(native.calls.filter(call => call.method === 'server.updateSettings')).toHaveLength(0);
  const restored = new T3Client(); await restored.refresh(native, disk.files);
  expect(restored.local.deviceSettings).toEqual({ composerCollapseOnScroll: false, planModeEnabled: true, timestampFormat: '24-hour', appearanceMode: 'system', sendShortcut: 'enter', snapShotShortcut: 'shift+shift', snapShotEnabled: false, snapShotIncludeAccessibility: true, snapShotPlaySound: true, snapShotSound: 'soft-pop', snapShotFlash: true, snapShotAnimations: true });
  expect((await command('device-setting', 'timestampFormat', 'invalid')).message).toBe('Unsupported device setting.');
  expect(client.local.deviceSettings.timestampFormat).toBe('24-hour');
});

 test('appearance and send shortcut validate and persist without server changes', async () => {
  const { client, native, command, disk } = await connected();
  for (const value of ['light', 'dark', 'system']) expect((await command('device-setting', 'appearanceMode', value)).message).toBe('');
  await command('device-setting', 'appearanceMode', 'dark');
  await command('device-setting', 'sendShortcut', 'mod-enter-multiline');
  const restored = new T3Client(); await restored.refresh(native, disk.files);
  expect(restored.local.deviceSettings.appearanceMode).toBe('dark');
  expect(restored.local.deviceSettings.sendShortcut).toBe('mod-enter-multiline');
  expect(native.calls.filter(call => call.op === 'devicePresentation').slice(-1)[0]).toMatchObject({ appearanceMode: 'dark', sendShortcut: 'mod-enter-multiline', rootFontSize: 16 });
  expect((await command('device-setting', 'appearanceMode', 'bad')).message).toBe('Unsupported device setting.');
  expect((await command('device-setting', 'sendShortcut', 'bad')).message).toBe('Unsupported device setting.');
  expect(native.calls.filter(call => call.method === 'server.updateSettings')).toHaveLength(0);
});

test('local appearance and send shortcut reset to reference defaults and survive reload', async () => {
  const { client, native, command, disk } = await connected();
  await command('device-setting', 'appearanceMode', 'light');
  await command('device-setting', 'sendShortcut', 'mod-enter');
  await command('device-setting', 'timestampFormat', '12-hour');
  await command('device-setting', 'appearanceMode', 'system');
  await command('device-setting', 'sendShortcut', 'enter');
  await command('device-setting', 'timestampFormat', 'locale');
  const restored = new T3Client(); await restored.refresh(native, disk.files);
  expect(restored.local.deviceSettings).toEqual(client.local.deviceSettings);
  expect(restored.local.deviceSettings).toMatchObject({ appearanceMode: 'system', sendShortcut: 'enter', timestampFormat: 'locale' });
});

test('scoped default model preserves options and other fields; unavailable scopes do not broaden writes', async () => {
  const { client, native, command } = await connected();
  client.providerId = 'codex-personal'; client.modelId = 'model-b'; client.modelOptions = [{ id: 'reasoningEffort', value: 'high' }];
  native.config.settings = { ...obj(native.config.settings), projectSettingsOverrides: { p1: { defaultRuntimeMode: 'approval-required' } } };
  expect((await command('setting-model', 'env1:p1', 'current')).message).toBe('');
  expect(obj(obj(native.config.settings).projectSettingsOverrides).p1).toEqual({ defaultRuntimeMode: 'approval-required', defaultModelSelection: { instanceId: 'codex-personal', model: 'model-b', options: [{ id: 'reasoningEffort', value: 'high' }] } });
  await command('new-thread', 'p1');
  expect(client.modelId).toBe('model-b');
  expect(client.runtimeMode).toBe('approval-required');
  expect((await command('setting-model', 'env1:p1', 'inherit')).message).toBe('');
  await command('new-thread', 'p1'); expect(client.modelId).toBe('model-a');
  const count = native.calls.filter(call => call.method === 'server.updateSettings').length;
  native.shell.projects = [];
  expect((await command('setting-model', 'env1:p1', 'current')).message).toBe('That project is no longer available.');
  expect((await command('setting-model', 'other:', 'current')).message).toBe('That environment is no longer selected.');
  expect(native.calls.filter(call => call.method === 'server.updateSettings')).toHaveLength(count);
});

test('Automatic model chooses ready instance default and scoped reset uses null', async () => {
  const { client, native, command } = await connected();
  native.config.settings = { defaultModelSelection: null };
  native.config.providers = [{ instanceId: 'disabled', enabled: false, installed: true, models: [{ slug: 'bad' }] }, ...arr(native.config.providers)];
  client.config = native.config;
  await command('new-thread', 'p1');
  expect(client.providerId).toBe('codex-personal'); expect(client.modelId).toBe('model-a');
  client.modelId = 'model-b';
  await command('setting-model', 'env1:', 'current');
  await command('setting-model', 'env1:', 'automatic');
  expect(obj(native.config.settings).defaultModelSelection).toBeNull();
  await command('new-thread', 'p1'); expect(client.modelId).toBe('model-a');
});

test('archived restore/delete target exact current archived member; reject stale scope without global writes', async () => {
  const { client, native, command } = await connected();
  native.shell.archivedThreads = [{ ...thread('a1'), createdAt: timestamp, archivedAt: timestamp }, { ...thread('a2'), createdAt: timestamp, archivedAt: timestamp }];
  const listed = await archivedSettings(client, native, 'env1', 'p1', true);
  expect(listed.groups).toHaveLength(1);
  expect((await command('unarchive-thread', 'env1:p1:a1')).message).toBe('');
  expect(client.shell.threads.map(thread => thread.id)).toContain('a1');
  expect(arr(native.shell.archivedThreads).map(thread => thread.id)).toEqual(['a2']);
  const count = native.committed.length;
  // Settings failures toast (shell-commands.ts settingsFailure); `toasted:` keeps the page's inline error empty.
  expect((await command('delete-archived-thread', 'env1:p1:a1')).message).toBe('toasted:That archived thread is no longer available.');
  expect((await command('unarchive-thread', 'other:p1:a2')).message).toContain('no longer selected');
  expect(native.committed).toHaveLength(count);
  expect((await command('delete-archived-thread', 'env1:p1:a2')).message).toBe('');
  expect(arr(native.shell.archivedThreads)).toEqual([]);
  const stale = await archivedSettings(client, native, 'env1', 'deleted', true);
  expect(stale.available).toBe(false); expect(stale.groups).toEqual([]);
});

test('license manifest validates provenance shape, all-term search, disclosure and retry', async () => {
  const { client, native } = await connected();
  const entry = { name: 'React', kind: 'package', version: '19.2', license: 'MIT', bundles: ['web'], sourceUrl: 'https://react.dev/', noticeText: 'Actual notice' };
  const manifest = { schemaVersion: 1, entries: [entry, { ...entry, name: 'Other', license: 'ISC' }] };
  const later = native.later.bind(native); let reads = 0;
  native.later = async request => { if (obj(request).path === '/third-party-licenses.json') { reads++; return native.good(manifest); } return later(request); };
  const filtered = await licenseSettings(client, native, 'react mit', '', 0, true);
  expect(filtered.total).toBe(2); expect(filtered.matched).toBe(1);
  expect(filtered.entries[0].noticeText).toBe('');
  const expanded = await licenseSettings(client, native, '', str(filtered.entries[0].id), 0, true);
  expect(expanded.entries[0].noticeText).toBe('Actual notice'); expect(reads).toBe(1);
  expect((await licenseSettings(client, native, 'missing', '', 0, true)).matched).toBe(0);
  await licenseSettings(client, native, '', '', 1, true); expect(reads).toBe(2);
  expect(() => decodeNotices({ schemaVersion: 2, entries: [] })).toThrow('unsupported format');
  expect(() => decodeNotices({ schemaVersion: 1, entries: [entry, entry] })).toThrow('duplicate');
  expect(() => decodeNotices({ schemaVersion: 1, entries: [{ ...entry, sourceUrl: 'file:///personal' }] })).toThrow('source URL');
});

test('diagnostics reads actual sections and range; partial/offline errors remain local', async () => {
  const { client, native } = await connected();
  const later = native.later.bind(native);
  native.later = async request => {
    const method = str(obj(request).method);
    if (method === 'server.getProcessDiagnostics') return native.good({ readAt: timestamp, processCount: 1, totalCpuPercent: 2, totalRssBytes: 1024, processes: [] });
    if (method === 'server.getProcessResourceHistory') { expect(obj(obj(request).payload)).toEqual({ windowMs: 300000, bucketMs: 15000 }); return native.good({ retainedSampleCount: 3, buckets: [], topProcesses: [] }); }
    if (method === 'server.getTraceDiagnostics') return native.bad('Trace unavailable');
    return later(request);
  };
  const result = await diagnosticsSettings(client, native, 'env1', '5m', true);
  expect(result.available).toBe(true); expect(result.sections).toHaveLength(3);
  expect(result.sections[0].rows[0].value).toBe('1'); expect(result.sections[2].error).toBe('Trace unavailable');
  expect(client.error).toBe('');
  expect((await diagnosticsSettings(client, native, 'env1', 'invalid', true)).error).toContain('Unsupported');
  expect((await diagnosticsSettings(client, native, 'deleted', '15m', true)).available).toBe(false);
  client.connection = 'disconnected';
  expect((await diagnosticsSettings(client, native, 'env1', '15m', true)).available).toBe(false);
});


test('storage cleanup validates capability, retention and stale checkout before scoped writes', async () => {
  const { client, native, command } = await connected();
  obj(native.config.environment).capabilities = { serverResolvedCommandContext: true, storageCleanup: true, projectWorktreeCleanup: true };
  native.config.settings = { storageCleanup: { logsAfterDays: null, worktreeOnMerge: false }, projectSettingsOverrides: { p1: { defaultRuntimeMode: 'full-access' }, sibling: { defaultRuntimeMode: 'approval-required' } } };
  const change = (scope: string, key: string, value: unknown) => command('setting-storage', scope, JSON.stringify({ key, value }));
  expect((await change('env1:', 'logsAfterDays', 3651)).message).toContain('3650');
  expect((await change('env1:deleted', 'mode', 'custom')).message).toContain('no longer available');
  expect(native.calls.filter(call => call.method === 'server.updateSettings')).toHaveLength(0);
  expect((await change('env1:p1', 'mode', 'custom')).message).toBe('');
  expect((await change('env1:p1', 'worktreeAfterDays', 12)).message).toBe('');
  const overrides = obj(obj(native.config.settings).projectSettingsOverrides);
  expect(obj(overrides.p1).defaultRuntimeMode).toBe('full-access');
  expect(obj(overrides.sibling).defaultRuntimeMode).toBe('approval-required');
  expect(obj(obj(overrides.p1).worktreeCleanup)).toEqual({ mode: 'custom', rules: { worktreeAfterDays: 12, worktreeOnDelete: false, worktreeOnMerge: false, worktreeUnchanged: false } });
  expect((await storageSettings(client, native, 'env1', 'p1', true)).worktrees.find(rule => rule.key === 'worktreeAfterDays')?.days).toBe('12');
  expect((await change('env1:p1', 'mode', 'inherit')).message).toBe('');
  expect(obj(obj(obj(native.config.settings).projectSettingsOverrides).p1).worktreeCleanup).toBeUndefined();
  expect((await change('env1:p1', 'logsAfterDays', 8)).message).toContain('environment');
  expect((await change('env1:', 'logsAfterDays', 8)).message).toBe('');
  expect(obj(obj(native.config.settings).storageCleanup).worktreeOnMerge).toBe(false);
  expect((await change('env1:', 'logsAfterDays', null)).message).toBe('');
  obj(obj(native.config.environment).capabilities).storageCleanup = false;
  expect((await change('env1:', 'worktreeOnDelete', true)).message).toContain('Update');
});


test('scoped source control writes preserve unrelated settings and reset the exact project tier', () => {
  const settings = { sourceControlWritingStyle: { mode: 'repo_conventions', customInstructions: 'keep', followChangeRequestTemplates: true }, projectSettingsOverrides: { p1: { defaultRuntimeMode: 'full-access' }, p2: { defaultAutoPull: true } } };
  expect(scopedSettingPatch(settings, 'p1', 'sourceControlWritingStyle.mode', 'custom', false)).toEqual({ projectSettingsOverrides: { p1: { defaultRuntimeMode: 'full-access', sourceControlWritingStyle: { mode: 'custom', customInstructions: 'keep', followChangeRequestTemplates: true } } } });
  expect(scopedSettingPatch(settings, 'p1', 'defaultAutoPull', true, false)).toEqual({ projectSettingsOverrides: { p1: { defaultRuntimeMode: 'full-access', defaultAutoPull: true } } });
  expect(scopedSettingPatch(settings, 'p1', 'defaultAutoPull', null, true)).toEqual({ projectSettingsOverrides: { p1: { defaultRuntimeMode: 'full-access' } } });
  expect(() => scopedSettingPatch(settings, '', 'defaultAutoPull', null, true)).toThrow('project');
  expect(() => scopedSettingPatch(settings, 'p1', 'pullRequestMergeMethod', 'invalid', false)).toThrow('supported');
  expect(() => scopedSettingPatch(settings, '', 'observability', {}, false)).toThrow('Unsupported');
});
test('scoped controls reject unavailable identities and gate device access by the actual hub', async () => {
  const { client, native } = await connected();
  expect((await scopedControls(client, native, 'deleted-environment', '', 'integrations', true)).available).toBe(false);
  expect((await scopedControls(client, native, client.environmentId, 'deleted-project', 'source-control', true)).rows).toEqual([]);
  const state = await scopedControls(client, native, client.environmentId, '', 'integrations', true);
  expect(state.rows.find(row => row.key === 'enableAgentDeviceAccess')?.disabled).toBe(true);
  const response = await client.command('setting-scoped', client.environmentId + ':deleted-project', JSON.stringify({ key: 'defaultAutoPull', value: true }), 0, native, storage().files);
  expect(response.message).toContain('no longer available');
  expect(native.calls.filter(call => call.method === 'server.updateSettings')).toEqual([]);
});

test('writing group reset is explicit; text matches reference trim and preserves sibling fields', () => {
  const settings = { sourceControlWritingStyle: { mode: 'custom', customInstructions: 'before', followChangeRequestTemplates: false }, projectSettingsOverrides: { p1: { sourceControlWritingStyle: { mode: 'custom', customInstructions: 'own', followChangeRequestTemplates: true }, defaultAutoPull: true } } };
  expect(() => scopedSettingPatch(settings, 'p1', 'sourceControlWritingStyle.customInstructions', null, true)).toThrow('group reset');
  expect(scopedSettingPatch(settings, 'p1', 'sourceControlWritingStyle', null, true)).toEqual({ projectSettingsOverrides: { p1: { defaultAutoPull: true } } });
  expect(obj(obj(obj(scopedSettingPatch(settings, 'p1', 'sourceControlWritingStyle.customInstructions', '  first\n second  ', false).projectSettingsOverrides).p1).sourceControlWritingStyle)).toEqual({ mode: 'custom', customInstructions: 'first\n second', followChangeRequestTemplates: true });
});
test('device access action cannot bypass disabled hub or stale project capabilities', async () => {
  const { client, native } = await connected();
  native.config.settings = { enableDeviceSupport: false, enableAgentDeviceAccess: false, defaultAutoPull: false };
  const command = (scope: string, key: string) => client.command('setting-scoped', scope, JSON.stringify({ key, value: true }), 0, native, storage().files);
  expect((await command('env1:', 'enableAgentDeviceAccess')).message).toContain('device hub');
  expect((await command('env1:p1', 'defaultAutoPull')).message).toContain('project overrides');
  expect(native.calls.filter(call => call.method === 'server.updateSettings')).toEqual([]);
});

test('scheduled defaults and edit conversion match reference daily and origin semantics', () => {
  const fresh = taskDraft(); expect(fresh.runtimeMode).toBe('full-access'); expect([fresh.monday, fresh.friday, fresh.sunday]).toEqual([true, true, false]);
  const existing = taskDraft({ id: 's1', schedule: { type: 'fixed_time', timeOfDay: '10:00' }, workspaceStrategy: { type: 'worktree', baseRef: 'release' } });
  expect([existing.sunday, existing.saturday, existing.startFromOrigin, existing.baseRef]).toEqual([true, true, false, 'release']);
  const args = ['env:', '', 'Fixture', 'Safe fixture prompt', false, 'fixed_time', '15', '', false, false, false, false, false, false, false, 'p1', '', 'worktree', '', true, '', 'fixture', 'luna', 'approval-required', 'default'];
  const payload = taskFromArguments(args);
  expect(payload.schedule).toEqual({ type: 'fixed_time', timeOfDay: '09:00' });
  expect(payload.workspaceStrategy).toEqual({ type: 'worktree', baseRef: 'main', startFromOrigin: true });
  expect(validateTaskInput(payload).enabled).toBe(false);
  expect(() => validateTaskInput({ ...payload, schedule: { type: 'fixed_time', timeOfDay: '25:61' } })).toThrow('valid time');
  expect(() => validateTaskInput({ ...payload, schedule: { type: 'interval', everyMs: 59999 } })).toThrow('one minute');
});

test('scheduled edit rechecks membership, forces requireExisting and preserves provider options', async () => {
  const { client, native } = await connected();
  let entries: Obj[] = [{ id: 's1', projectId: 'p1', modelSelection: { instanceId: 'codex-personal', model: 'model-a', options: [{ id: 'effort', value: 'high' }] } }];
  const writes: Obj[] = [], later = native.later.bind(native);
  native.later = async request => {
    const call = obj(request);
    if (call.method === 'scheduledTasks.list') return native.good({ tasks: entries });
    if (call.method === 'scheduledTasks.upsert') { writes.push(obj(call.payload)); return native.good({ task: obj(call.payload) }); }
    return later(request);
  };
  const input = { id: 's1', title: 'Fixture', prompt: 'Safe disabled fixture', enabled: false, schedule: { type: 'interval', everyMs: 60000 }, projectId: 'p1', threadId: null, workspaceStrategy: { type: 'root' }, modelSelection: { instanceId: 'codex-personal', model: 'model-a' }, runtimeMode: 'approval-required', interactionMode: 'default' };
  expect((await client.command('task-save', 'env1:p1', JSON.stringify(input), 0, native, storage().files)).message).toBe('');
  expect(writes[0].requireExisting).toBe(true); expect(obj(writes[0].modelSelection).options).toEqual([{ id: 'effort', value: 'high' }]); expect(writes[0].enabled).toBe(false);
  entries = [];
  expect((await client.command('task-save', 'env1:p1', JSON.stringify(input), 0, native, storage().files)).message).toContain('no longer exists');
  expect(writes.length).toBe(1);
  expect((await client.command('task-delete', 'env1:deleted-project', JSON.stringify({ id: 's1' }), 0, native, storage().files)).message).toContain('no longer available');
});

test('scheduled list query reaches the allowlisted backend and preserves readonly/offline scope state', async () => {
  const { client, native } = await connected();
  const later = native.later.bind(native);
  native.later = async request => obj(request).method === 'scheduledTasks.list' ? native.good({ tasks: [] }) : later(request);
  expect((await scheduledSettings(client, native, 'env1', '', '', true)).available).toBe(true);
  expect((await scheduledSettings(client, native, 'env1', '', 'missing-task', true)).error).toContain('no longer exists');
  expect((await scheduledSettings(client, native, 'env1', 'deleted-project', '', true)).available).toBe(false);
});


test('keybinding precedence is global across commands and physical mod/meta aliases', () => {
  const rules = [
    { command: 'sidebar.toggle', shortcut: { key: 'b', modKey: true } },
    { command: 'chat.new', shortcut: { key: 'b', metaKey: true } },
    { command: 'chat.new', shortcut: { key: 'n', modKey: true } },
  ];
  const keys = keyboardSettings({ keybindings: rules }, false);
  expect(keys.keySidebar).toBe(''); expect(keys.keyNewThread).toBe('Meta+n Meta+b');
  expect(keyboardSettings({ keybindings: rules }, false, { modalOpen: true }).keyNewThread).toBe('');
});

test('unknown negated shortcut context cannot activate or revive an older binding', () => {
  const earlier = { command: 'sidebar.toggle', shortcut: { key: 'b', modKey: true } };
  const later = { command: 'chat.new', shortcut: { key: 'b', modKey: true }, whenAst: { type: 'not', node: { type: 'identifier', name: 'unsupportedContext' } } };
  const keys = keyboardSettings({ keybindings: [earlier, later] }, false);
  expect(keys.keySidebar).toBe(''); expect(keys.keyNewThread).toBe('');
  const editable = { ...later, whenAst: { type: 'not', node: { type: 'identifier', name: 'editableFocus' } } };
  expect(keyboardSettings({ keybindings: [earlier, editable] }, true).keySidebar).toBe('Meta+b');
  expect(keyboardSettings({ keybindings: [earlier, editable] }, false).keyNewThread).toBe('Meta+b');
});


test('scoped search follows the mounted conditional fields without changing settings', () => {
  const settings = { branchNamingMode: 'static', sourceControlWritingStyle: { mode: 'repo_conventions' }, projectSettingsOverrides: { p1: { branchNamingMode: 'custom' } } };
  const scope = scopedSearchControls(settings, 'p1', { projectSettingsOverrides: true });
  expect(scope.some(row => row[1] === 'scoped-input-branchNameInstructions')).toBe(true);
  expect(scope.some(row => row[1] === 'scoped-input-branchNamePrefix')).toBe(false);
  expect(scope.some(row => row[1] === 'scoped-input-sourceControlWritingStyle.customInstructions')).toBe(false);
  expect(scopedSearchControls(settings, 'p1', {}).length).toBe(0);
  expect(settings.branchNamingMode).toBe('static');
});


test('provider catalog includes first instances and disabled or setup-needed drivers', async () => {
  const { client } = await connected();
  client.config.providers = [{ driver: 'codex', availability: 'unavailable', enabled: false, setup: { canAuthenticate: true } }];
  const drivers = client.providerDrivers();
  expect(drivers.map(driver => driver.id)).toContain('acpRegistry');
  expect(drivers.map(driver => driver.id)).toContain('claudeAgent');
  expect(drivers.find(driver => driver.id === 'codex')?.canAuthenticate).toBe(true);
});


test('keybinding grammar rejects invalid expressions before server writes', async () => {
  expect(validShortcut('mod+alt+shift+y')).toBe(true); expect(validShortcut('mod++')).toBe(true);
  expect(validShortcut('mod+x+y')).toBe(false); expect(validShortcut('mod')).toBe(false);
  expect(validWhen('!terminalFocus && (composerFocus || draftThreadRoute)')).toBe(true);
  expect(validWhen(')')).toBe(false); expect(validWhen('!unsupportedContext')).toBe(true);
  const { client, native, disk } = await connected();
  const result = await client.command('keybinding-save', 'env1', JSON.stringify({ command: 'sidebar.toggle', key: 'mod+y', when: ')' }), 0, native, disk.files);
  expect(result.message).toContain('valid shortcut condition');
  expect(native.calls.filter(call => call.method === 'server.upsertKeybinding')).toEqual([]);
});


test('denied snapshot setup leaves device preferences and backend unchanged', async () => {
  const { client, native, disk } = await connected();
  const later = native.later.bind(native);
  native.later = async request => {
    const call = obj(request);
    if (call.op === 'snapshotConfigure') return call.enabled ? native.bad('Screen Recording access is required.') : native.good({ enabled: false });
    if (call.op === 'snapshotState') return native.good({ mode: 'direct', enabled: false, screenRecording: false, accessibility: false, pending: [] });
    return later(request);
  };
  expect((await client.snapshotState(native)).screenRecording).toBe(false);
  const result = await client.command('setting-snapshot', 'snapShotEnabled', 'true', 0, native, disk.files);
  expect(result.message).toContain('Screen Recording'); expect(client.local.deviceSettings.snapShotEnabled).toBe(false);
  expect(client.error).toBe('');
  expect(native.calls.filter(call => call.method === 'server.updateSettings')).toEqual([]);
});

test('snapshot state cannot be adopted into a different draft owner', async () => {
  const { client, native } = await connected(); const later = native.later.bind(native);
  native.later = async request => {
    const call = obj(request);
    if (call.op === 'snapshotConfigure') return native.good({ enabled: false });
    if (call.op === 'snapshotState') { client.projectId = 'changed-owner'; return native.good({ pending: ['capture-one'] }); }
    return later(request);
  };
  await expect(client.snapshotState(native)).rejects.toThrow('capture draft changed');
});


test('revoked snapshot grants preserve enabled choice and reachable disable without backend writes', async () => {
  const { client, native, disk } = await connected(); const later = native.later.bind(native);
  client.local.deviceSettings.snapShotEnabled = true;
  native.later = async request => {
    const call = obj(request);
    if (call.op === 'snapshotConfigure') return call.enabled ? native.bad('Screen Recording access was revoked.') : native.good({ enabled: false });
    if (call.op === 'snapshotState') return native.good({ enabled: false, screenRecording: false, accessibility: false, pending: [], capturing: [] });
    return later(request);
  };
  const { snapshotSettings } = await import('./snapshot-settings');
  const state = await snapshotSettings(client, native, true);
  expect(state).toMatchObject({ available: true, enabled: true, screenRecording: false, accessibility: false });
  expect(state.error).toContain('revoked');
  expect(client.local.deviceSettings.snapShotEnabled).toBe(true);
  const disabled = await client.command('setting-snapshot', 'snapShotEnabled', 'false', 0, native, disk.files);
  expect(disabled.message).not.toContain('revoked');
  expect(client.local.deviceSettings.snapShotEnabled).toBe(false);
  expect(native.calls.filter(call => call.method === 'server.updateSettings')).toEqual([]);
});
