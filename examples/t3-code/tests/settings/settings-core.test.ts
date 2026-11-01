import { AUTH_STANDARD_CLIENT_SCOPES } from '../../src/connections/remote-scopes';
// Settings shell, General and Appearance logic.
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { applyCoreSetting, applyDeviceSetting, changedDeviceLabels, clientValue, decodeClientPrefs, effectiveSetting, generalSections, idleCompactionRow, parseCoreTarget,
  parseProjectFile, resolveScope, restoreLabels, serverContext, serverValue, settingPlan } from '../../src/settings/settings-core';
import { toasts } from '../../src/shell/toast';
import { fleet } from '../../src/connections/settings-b-fleet';
import { applyBrowserDefault, browserDefaultsView } from '../../src/browser/browser-defaults';
import { integrationRows } from '../../src/settings/source-control-view';
import { DEFAULT_SNOOZE_PRESETS } from '../../src/settings/settings-snooze';

// The app's one fleet is shared across test files; these cases are single-environment unless they add entries.
beforeEach(() => { fleet.entries.clear(); fleet.saved = []; });
afterEach(() => { fleet.entries.clear(); fleet.saved = []; });
import { appearanceSections, defaultCodeFamily, fontStack, mix, modeTiles, palette, themeCards, themeRoles } from '../../src/appearance/settings-appearance';
import { breadcrumbLabel, commandLabel, scopeAvailable, searchSettings, settingsNavigation } from '../../src/settings/settings-search';
import { settingsCore } from '../../src/settings/settings-core-view';
import { scopedProviderPage } from '../../src/providers/providers-scope';
import { worktreeRows } from '../../src/settings/settings-data';
import { contractComponent, value } from '../support/contract-action-fixture';
import { contractFn } from '../support/contract-fn-fixture';
import { appFiles, appText } from '../support/app-files';

// The composer the model picker serves: gpt-5.4 on codex, its checked level the third row.
const chosen = { providerId: 'codex', modelId: 'gpt-5.4', composer: { effortFocusLevel: 2 } };
const provider = { instanceId: 'fixture', driver: 'codex', displayName: 'Exact fixture', enabled: true, installed: true, auth: { status: 'authenticated' }, status: 'ready',
  models: [{ slug: 'luna', name: 'GPT-5.6-Luna', isDefault: true, capabilities: { optionDescriptors: [{ id: 'reasoningEffort', type: 'select', options: [{ id: 'low', label: 'Low' }, { id: 'medium', label: 'Medium', isDefault: true }, { id: 'high', label: 'High' }] }] } },
    { slug: 'astra', name: 'GPT-6-Astra' }] };
type Fake = { scopes: readonly string[]; local: Obj; ready: boolean; environmentId: string; config: Obj; writes: Obj[]; files: Record<string, string>; projectGroups(): { key: string; name: string; members: Obj[] }[]; settingsCoreRequest(native: Native, method: string, payload: Obj): Promise<Obj> };
function fake(settings: Obj = {}, ready = true): Fake {
  const client: Fake = {
    local: { deviceSettings: { planModeEnabled: false, timestampFormat: 'locale', appearanceMode: 'system', sendShortcut: 'enter' }, clientSettings: decodeClientPrefs({}), groupingMode: 'repository' },
    scopes: AUTH_STANDARD_CLIENT_SCOPES, ready, environmentId: 'env1', writes: [], files: {},
    config: { environment: { environmentId: 'env1', label: 'Studio', serverVersion: '0.0.46', capabilities: { projectSettingsOverrides: true, threadAutoSettlement: true, threadRestartContinuation: true } }, providers: [provider], settings: { defaultRuntimeMode: 'full-access', projectSettingsOverrides: {}, ...settings } },
    projectGroups: () => [{ key: 'repo', name: 'Parity fixture', members: [{ id: 'p1', title: 'Parity fixture', workspaceRoot: '/a' }, { id: 'p2', title: 'Parity fixture', workspaceRoot: '/b' }] },
      { key: 'env1:/c', name: 'Single checkout two', members: [{ id: 'p3', title: 'Single checkout two', workspaceRoot: '/c' }] }],
    async settingsCoreRequest(_native, method, payload) {
      if (method === 'server.getConfig') return client.config as Obj;
      if (method === 'projects.readFile') { const file = client.files[String(payload.cwd)]; if (file === undefined) throw new Error('missing'); return { contents: file, truncated: false }; }
      client.writes.push(payload);
      const patch = payload.patch as Obj, current = client.config.settings as Obj;
      const overrides = { ...(current.projectSettingsOverrides as Obj) };
      for (const [id, entry] of Object.entries((patch.projectSettingsOverrides as Obj) || {})) { if (entry === null) delete overrides[id]; else overrides[id] = entry; }
      client.config.settings = { ...current, ...patch, projectSettingsOverrides: overrides };
      return client.config.settings as Obj;
    },
  };
  return client;
}
const native: Native = { available: true, watch() {}, later: async () => ({ ok: true, generation: 0, value: { fonts: [] } }) };
const as = (client: Fake) => client as unknown as T3Client;

describe('device settings', () => {
  test('decode keeps valid reference ClientSettings values and drops the rest', () => {
    const prefs = decodeClientPrefs({ chatWidth: 'wide', glassOpacity: 55, appearanceContrast: 133, fontSizeCode: 30, theme: 'grove', diffLayout: 'diagonal', fontFamilySans: 'Inter' });
    expect(prefs.chatWidth).toBe('wide');
    expect(prefs.glassOpacity).toBe(55);
    expect(prefs.appearanceContrast).toBe(100);
    expect(prefs.fontSizeCode).toBe(13);
    expect(prefs.theme).toBe('grove');
    expect(prefs.diffLayout).toBe('stacked');
    expect(prefs.fontFamilySans).toBe('Inter');
    expect(clientValue('fontFamilySans', 'x"; color: red')).toBeUndefined();
  });
  test('switches, selects and grouping write the device record; a whole theme sets both halves', () => {
    const client = fake();
    expect(applyDeviceSetting(client.local as never, 'diffIgnoreWhitespace', 'false')).toBe(true);
    expect(applyDeviceSetting(client.local as never, 'sendShortcut', 'mod-enter')).toBe(true);
    expect(applyDeviceSetting(client.local as never, 'theme', 'iris')).toBe(true);
    expect(applyDeviceSetting(client.local as never, 'themeDark', 'ember')).toBe(true);
    const prefs = client.local.clientSettings as Obj;
    expect([prefs.diffIgnoreWhitespace, prefs.themeLight, prefs.themeDark]).toEqual([false, 'iris', 'ember']);
    expect((client.local.deviceSettings as Obj).sendShortcut).toBe('mod-enter');
    client.local.groupingMode = 'repository_path';
    applyDeviceSetting(client.local as never, 'projectGrouping', 'false');
    expect(client.local.groupingMode).toBe('separate');
    applyDeviceSetting(client.local as never, 'projectGrouping', 'true');
    expect(client.local.groupingMode).toBe('repository_path');
    expect(() => applyDeviceSetting(client.local as never, 'chatWidth', 'huge')).toThrow('Unsupported device setting.');
    expect(applyDeviceSetting(client.local as never, 'unknownKey', 'x')).toBe(false);
    expect(changedDeviceLabels(client.local as never)).toEqual(expect.arrayContaining(['sendShortcut', 'diffIgnoreWhitespace', 'theme', 'projectGrouping']));
  });
});

describe('scope', () => {
  test('resolves all, environment, project group and checkout; removed targets stay unavailable', () => {
    const client = as(fake());
    expect(resolveScope(client, '', '', '').kind).toBe('all');
    expect(resolveScope(client, 'env1', '', '').kind).toBe('environment');
    expect(resolveScope(client, 'env1', '', '').connective).toBe('on');
    const project = resolveScope(client, '', 'repo', '');
    expect([project.kind, project.members.length, project.projectLabel, project.projectMark]).toEqual(['project', 2, 'Parity fixture', 'PF']);
    expect(resolveScope(client, '', 'repo', 'p2').members.map(member => member.id)).toEqual(['p2']);
    expect(resolveScope(client, '', 'gone', '').message).toBe('This project is no longer available.');
    expect(resolveScope(client, 'env9', '', '').message).toBe('This environment is no longer available.');
    expect(resolveScope(client, '', '', 'p1').message).toBe('Select a project to choose one of its checkouts.');
    expect(resolveScope(client, '', 'repo', '').projectChoices.map(choice => choice.label)).toEqual(['All projects', 'Parity fixture', 'Single checkout two']);
    expect(resolveScope(client, '', '', '').environmentChoices.map(choice => choice.label)).toEqual(['All environments', 'Studio']);
  });
  test('effective values: project override, environment, t3.json for file-backed keys, built-in', () => {
    const settings = { defaultThreadEnvMode: null, worktreeSubmodules: 'none', projectSettingsOverrides: { p1: { defaultRuntimeMode: 'approval-required' } } };
    expect(effectiveSetting(settings, 'p1', 'defaultRuntimeMode', null)).toEqual({ value: 'approval-required', source: 'project' });
    expect(effectiveSetting(settings, 'p2', 'defaultThreadEnvMode', { defaultThreadEnvMode: 'worktree' })).toEqual({ value: 'worktree', source: 't3.json' });
    expect(effectiveSetting(settings, '', 'defaultThreadEnvMode', { defaultThreadEnvMode: 'worktree' })).toEqual({ value: 'local', source: 'environment' });
    expect(effectiveSetting(settings, 'p2', 'worktreeSubmodules', { worktreeSubmodules: 'top-level' })).toEqual({ value: 'none', source: 'environment' });
    expect(parseProjectFile('{"defaultThreadEnvMode":"worktree"}')).toEqual({ defaultThreadEnvMode: 'worktree' });
    expect(parseProjectFile('nope')).toBeNull();
  });
  test('project writes replace each member entry; clearing drops only that key', () => {
    const client = as(fake({ projectSettingsOverrides: { p1: { defaultAutoPull: true } } }));
    const scope = resolveScope(client, '', 'repo', '');
    const settings = client.config.settings as Obj;
    const patches = (key: string, value: unknown, clear: boolean, target = scope) => settingPlan(client, target, key, value as never, clear, settings).serverWrites.map(write => write.patch);
    expect(patches('defaultRuntimeMode', 'auto', false)).toEqual([{ projectSettingsOverrides: { p1: { defaultAutoPull: true, defaultRuntimeMode: 'auto' }, p2: { defaultRuntimeMode: 'auto' } } }]);
    expect(patches('defaultAutoPull', undefined, true)).toEqual([{ projectSettingsOverrides: { p1: null, p2: null } }]);
    expect(settingPlan(client, scope, 'snoozeLimitedThreads', true, false, settings).unavailableReason).toBe('This setting is environment-wide and cannot be overridden by a project.');
    expect(patches('snoozeLimitedThreads', true, false, resolveScope(client, '', '', ''))).toEqual([{ snoozeLimitedThreads: true }]);
    expect(patches('defaultThreadEnvMode', undefined, true, resolveScope(client, '', '', ''))).toEqual([{ defaultThreadEnvMode: null }]);
  });
});

describe('general rows', () => {
  test('every reference row in order, with live values and inheritance', () => {
    const client = as(fake({ snoozeLimitedThreads: true }));
    const sections = generalSections(client, serverContext(client, resolveScope(client, '', '', ''), new Map()));
    // Every page's sections in nav order (SettingsPanels.tsx and WorktreesSettings.tsx at 6d450a4284); Maintenance's
    // Diagnostics is the storage panel's, after its own sections (settings-data.ts).
    expect(sections.map(section => section.title)).toEqual(['Notifications', 'Sidebar', 'Confirmations', 'Display', 'Composer', 'Follow-ups', 'Diffs', 'Pull requests', 'New projects', 'New threads',
      'Snooze', 'Usage limits', 'Auto-settle', 'Text generation', 'Creation', 'Updates', 'Background activity', 'App', 'Legal', 'Legacy']);
    expect(sections.flatMap(section => section.rows.map(row => row.title))).toEqual(['Thread notifications', 'In-app notifications', 'Project grouping', 'Project order', 'Working section',
      'Unpin confirmation', 'Archive confirmation', 'Delete confirmation', 'Quit shortcut', 'Time format', 'Rich text composer', 'Send shortcut',
      'Follow-up behavior', 'Diff layout', 'Hide whitespace changes', 'Default diff file state', 'Diff colors', 'Proactive panels', 'Open pull requests in T3 Code', 'Add project starts in', 'Model', 'Fallback model', 'Permissions', 'Workspace',
      'Snooze options', 'Resume after usage limits', 'Snooze until limits reset', 'Auto-settle merged threads', 'Auto-settle inactive threads', 'Days of inactivity before auto-settle',
      'Text generation model', 'Start from remote', 'Provider update checks', 'Continue threads after restarts', 'Background activity',
      'Version', 'Update track', 'Mobile app', 'Open source licenses', 'Plan mode (legacy)', 'Context window indicator (legacy)']);
    // AdvancedSettings: Chat's Show skills, Threads' Response streaming and Worktrees' Submodules fold at their section's end.
    expect(sections.flatMap(section => (section.advanced ?? []).map(row => [section.title, row.title]))).toEqual([['Composer', 'Show skills in slash menu'],
      ['Text generation', 'Thread title instructions'], ['Text generation', 'Response streaming'], ['Creation', 'Submodules']]);
    expect(generalSections(client, serverContext(client, resolveScope(client, '', '', ''), new Map()), 'general').map(section => section.title)).toEqual(['Notifications', 'Sidebar', 'Confirmations', 'Display']);
    const row = (id: string) => sections.flatMap(section => [...section.rows, ...(section.advanced ?? [])]).find(entry => entry.id === id)!;
    expect([row('default-model').label, row('default-model').label2, row('default-model').status, row('default-model').description])
      .toEqual(['GPT-5.6-Luna', 'Medium', '', 'Default model for new threads, selected automatically. Projects can override it.']);
    expect([row('default-permissions').label, row('default-permissions').icon, row('default-permissions').inheritance]).toEqual(['Full access', 'lock-open', 'default']);
    expect([row('snooze-limited-threads').checked, row('snooze-limited-threads').inheritance, row('snooze-limited-threads').resettable]).toEqual([true, 'environment', false]);
    expect([row('new-threads').label, row('worktree-submodules').label, row('days-before-auto-settle').value]).toEqual(['Current checkout', 'Recursive', '3']);
    expect(row('version').value).toBe('0.1.6'); // this app's own release, not the server's
  });
  test('Fallback model: None until set, the picked model once saved, no traits, and absent in a project scope', async () => {
    const client = fake();
    const rowOf = (scope = resolveScope(as(client), '', '', '')) => generalSections(as(client), serverContext(as(client), scope, new Map())).flatMap(section => section.rows).find(entry => entry.id === 'fallback-model');
    expect(rowOf()).toMatchObject({ kind: 'model', label: 'None', driver: '', label2: '', value: 'fixture|luna', resettable: false,
      description: "Used for new threads when the default model's provider is out of usage." });
    await applyCoreSetting(as(client), native, 'fallback-model:|||', 'fixture|astra');
    expect(client.writes.at(-1)).toEqual({ patch: { fallbackModelSelection: { instanceId: 'fixture', model: 'astra' } } });
    expect(rowOf()).toMatchObject({ label: 'GPT-6-Astra', driver: 'codex', label2: '', options2: [], resettable: true });
    await expect(applyCoreSetting(as(client), native, 'fallback-model:|||', 'gone|x')).rejects.toThrow('unavailable');
    await applyCoreSetting(as(client), native, 'fallback-model:reset|||', '');
    expect(client.writes.at(-1)).toEqual({ patch: { fallbackModelSelection: null } });
    expect(rowOf(resolveScope(as(client), '', 'repo', ''))).toBeUndefined();
  });
  test('Snooze options: the environment\'s list, its edits and reset, and no row in a project scope', async () => {
    const client = fake();
    const rowOf = (scope = resolveScope(as(client), '', '', '')) => generalSections(as(client), serverContext(as(client), scope, new Map())).flatMap(section => section.rows).find(entry => entry.id === 'snooze-options');
    expect(rowOf()).toMatchObject({ kind: 'snooze', title: 'Snooze options', resettable: false, description: 'Choices in every snooze menu, listed soonest first. Weekdays mean the next one after today.' });
    expect(rowOf()!.snooze[0]!.rows).toHaveLength(5);
    await applyCoreSetting(as(client), native, 'snooze-options:when:4|||', 'friday');
    expect(client.writes.at(-1)).toEqual({ patch: { snoozePresets: [{ kind: 'after', minutes: 60 }, { kind: 'after', minutes: 180 }, { kind: 'at', day: 'today', time: '18:00' },
      { kind: 'at', day: 'tomorrow', time: '09:00' }, { kind: 'at', day: 'friday', time: '09:00' }] } });
    expect(rowOf()).toMatchObject({ resettable: true, inheritance: 'environment' });
    await applyCoreSetting(as(client), native, 'snooze-options:reset|||', '');
    expect(client.writes.at(-1)).toEqual({ patch: { snoozePresets: DEFAULT_SNOOZE_PRESETS } });
    // An edit that leaves the list as it was (a blank amount, the unit already shown, the same time) writes nothing.
    const writes = client.writes.length;
    for (const [part, raw] of [['amount:0', ''], ['amount:0', '1'], ['unit:0', 'hours'], ['time:3', '09:00'], ['when:4', 'monday']])
      await applyCoreSetting(as(client), native, `snooze-options:${part}|||`, raw);
    expect(client.writes.length).toBe(writes);
    expect(rowOf(resolveScope(as(client), '', 'repo', ''))).toBeUndefined();
  });
  test('project scope: overrides, t3.json and environment-wide rows', () => {
    const client = as(fake({ projectSettingsOverrides: { p1: { defaultRuntimeMode: 'approval-required' }, p2: { defaultRuntimeMode: 'approval-required' } } }));
    const scope = resolveScope(client, '', 'repo', '');
    const sections = generalSections(client, serverContext(client, scope, new Map([['p1', { defaultThreadEnvMode: 'worktree' }], ['p2', { defaultThreadEnvMode: 'worktree' }]])));
    const row = (id: string) => sections.flatMap(section => section.rows).find(entry => entry.id === id)!;
    expect([row('default-permissions').label, row('default-permissions').inheritance, row('default-permissions').resettable]).toEqual(['Supervised', 'overridden', true]);
    expect([row('new-threads').label, row('new-threads').inheritanceSummary]).toEqual(['New worktree', "Inherited from the repository's t3.json"]);
    expect(row('default-model').inheritanceSummary).toBe('Inherited from Studio');
    expect([row('snooze-limited-threads').inert, row('snooze-limited-threads').note]).toEqual([true, 'Environment-wide setting. Select an environment to change it.']);
    expect(row('default-model').description).toBe('Model for new threads in this project.');
  });
  test('values are validated against the reference schema', () => {
    const client = as(fake());
    const context = serverContext(client, resolveScope(client, '', '', ''), new Map());
    expect(serverValue('defaultRuntimeMode', 'auto', context)).toBe('auto');
    expect(() => serverValue('defaultRuntimeMode', 'root', context)).toThrow();
    expect(serverValue('sidebarAutoSettleAfterDays', '12', context, 'days')).toBe(12);
    expect(() => serverValue('sidebarAutoSettleAfterDays', '91', context, 'days')).toThrow('1 to 90');
    expect(serverValue('sidebarAutoSettleAfterDays', 'false', context)).toBeNull();
    expect(serverValue('backgroundActivity', 'battery-saver', context)).toEqual({ schemaVersion: 1, profile: 'battery-saver', overrides: {} });
    expect(serverValue('defaultModelSelection', 'fixture|astra', context)).toEqual({ instanceId: 'fixture', model: 'astra' });
    expect(serverValue('defaultModelSelection', 'high', context, 'effort')).toEqual({ instanceId: 'fixture', model: 'luna', options: [{ id: 'reasoningEffort', value: 'high' }] });
    expect(() => serverValue('defaultModelSelection', 'gone|x', context)).toThrow('unavailable');
    expect(parseCoreTarget('default-model:effort|env1|repo:a/b|p1')).toEqual({ row: 'default-model', part: 'effort', machine: 'env1', projectKey: 'repo:a/b', checkout: 'p1' });
  });
});

describe('default model', () => {
  // resolveEffectiveDefaultModelSelection: the row shows what a new draft would run, as the composer resolves it.
  const effort = (ids: string[]) => ({ optionDescriptors: [{ id: 'effort', label: 'Effort', type: 'select', options: ids.map((id, index) => ({ id, label: id, isDefault: index === 0 })) }] });
  const claude = { instanceId: 'claude_work', driver: 'claudeAgent', displayName: 'Claude', enabled: true, installed: true, auth: { status: 'authenticated' }, status: 'ready',
    models: [{ slug: 'claude-opus-5-5', name: 'Opus 5.5', isDefault: true, capabilities: effort(['medium', 'high']) }, { slug: 'claude-opus-5', name: 'Opus 5', capabilities: effort(['high', 'max']) }] };
  const modelRow = (hidden: string[]) => {
    const client = fake({ defaultModelSelection: { instanceId: 'claude_work', model: 'claude-opus-5', options: [{ id: 'effort', value: 'max' }] },
      providerModelPreferences: { claude_work: { hiddenModels: hidden, modelOrder: [] } } });
    client.config.providers = [claude];
    const context = serverContext(as(client), resolveScope(as(client), '', '', ''), new Map());
    return generalSections(as(client), context).flatMap(section => section.rows).find(entry => entry.id === 'default-model')!;
  };
  test('a hidden saved model shows the model it falls back to, with that model\'s default traits', () => {
    const row = modelRow(['claude-opus-5']);
    expect([row.value, row.label, row.label2]).toEqual(['claude_work|claude-opus-5-5', 'Opus 5.5', 'medium']);
  });
  test('a visible saved model keeps its saved traits', () => {
    const row = modelRow([]);
    expect([row.value, row.label, row.label2]).toEqual(['claude_work|claude-opus-5', 'Opus 5', 'max']);
  });
});

describe('traits picker', () => {
  test('sections, Default badges and the speed label; a pick keeps the other traits (TraitsPicker)', () => {
    const tier = { id: 'serviceTier', label: 'Service Tier', type: 'select', options: [{ id: 'default', label: 'Standard', isDefault: true }, { id: 'priority', label: 'Fast', description: '2x speed, increased usage' }] };
    const descriptors = (provider.models[0]!.capabilities as Obj).optionDescriptors as Obj[];
    descriptors.push(tier);
    try {
      const client = as(fake({ defaultModelSelection: { instanceId: 'fixture', model: 'luna', options: [{ id: 'reasoningEffort', value: 'high' }, { id: 'serviceTier', value: 'priority' }] } }));
      const context = serverContext(client, resolveScope(client, '', '', ''), new Map());
      const model = generalSections(client, context).flatMap(section => section.rows).find(entry => entry.id === 'default-model')!;
      expect([model.label2, model.icon]).toEqual(['High Fast', '']);
      expect(model.options2.map(entry => [entry.value, entry.icon, entry.selected])).toEqual([['section:reasoningEffort', 'section', false], ['reasoningEffort=low', '', false],
        ['reasoningEffort=medium', 'default', false], ['reasoningEffort=high', '', true], ['section:serviceTier', 'section-rule', false], ['serviceTier=default', 'default', false], ['serviceTier=priority', '', true]]);
      expect(model.options2.find(entry => entry.value === 'serviceTier=priority')!.detail).toBe('2x speed, increased usage');
      expect(serverValue('defaultModelSelection', 'reasoningEffort=low', context, 'effort')).toEqual({ instanceId: 'fixture', model: 'luna', options: [{ id: 'serviceTier', value: 'priority' }, { id: 'reasoningEffort', value: 'low' }] });
      expect(serverValue('defaultModelSelection', 'serviceTier=default', context, 'effort')).toEqual({ instanceId: 'fixture', model: 'luna', options: [{ id: 'reasoningEffort', value: 'high' }, { id: 'serviceTier', value: 'default' }] });
      expect(() => serverValue('defaultModelSelection', 'section:serviceTier', context, 'effort')).toThrow();
    } finally { descriptors.pop(); }
  });
});

describe('writes through the command', () => {
  test('environment and project writes reach server.updateSettings; device rows never do', async () => {
    const client = fake();
    await applyCoreSetting(as(client), native, 'snooze-limited-threads:|||', 'true');
    expect(client.writes.at(-1)).toEqual({ patch: { snoozeLimitedThreads: true } });
    await applyCoreSetting(as(client), native, 'compact-idle-threads:|||', 'beforeCacheExpires');
    expect(client.writes.at(-1)).toEqual({ patch: { compactIdleThreads: true } });
    await applyCoreSetting(as(client), native, 'thread-title-instructions:|||', '  Lead with the ticket.  ');
    expect(client.writes.at(-1)).toEqual({ patch: { threadTitleInstructions: 'Lead with the ticket.' } });
    await applyCoreSetting(as(client), native, 'default-permissions:||repo|p2', 'approval-required');
    expect(client.writes.at(-1)).toEqual({ patch: { projectSettingsOverrides: { p2: { defaultRuntimeMode: 'approval-required' } } } });
    await applyCoreSetting(as(client), native, 'default-permissions:reset||repo|p2', '');
    expect((client.config.settings as Obj).projectSettingsOverrides).toEqual({});
    // Thread title instructions are project-scoped (PROJECT_SCOPED_SERVER_SETTING_KEYS).
    await applyCoreSetting(as(client), native, 'thread-title-instructions:||repo|p2', 'Short.');
    expect(client.writes.at(-1)).toEqual({ patch: { projectSettingsOverrides: { p2: { threadTitleInstructions: 'Short.' } } } });
    await applyCoreSetting(as(client), native, 'thread-title-instructions:reset||repo|p2', '');
    expect((client.config.settings as Obj).projectSettingsOverrides).toEqual({});
    const count = client.writes.length;
    await applyCoreSetting(as(client), native, 'diff-layout:diffLayout|||', 'split');
    await applyCoreSetting(as(client), native, 'time-format:timestampFormat|||', '24-hour');
    expect(client.writes.length).toBe(count);
    expect((client.local.clientSettings as Obj).diffLayout).toBe('split');
    await applyCoreSetting(as(client), native, 'diff-layout:reset|||', 'diffLayout');
    expect((client.local.clientSettings as Obj).diffLayout).toBe('stacked');
    // useRunScopedPlan: a refused plan writes nothing and warns "Setting not saved" with the reason.
    const before = client.writes.length;
    await applyCoreSetting(as(client), native, 'snooze-limited-threads:||gone|', 'true');
    expect(toasts(as(client)).at(-1)).toMatchObject({ kind: 'warning', title: 'Setting not saved', description: 'This project is no longer available.' });
    await applyCoreSetting(as(client), native, 'snooze-limited-threads:||repo|', 'true');
    expect(toasts(as(client)).at(-1)).toMatchObject({ kind: 'warning', description: 'This setting is environment-wide and cannot be overridden by a project.' });
    expect(client.writes.length).toBe(before);
  });
  test('disconnected server rows refuse; restore resets device and environment values', async () => {
    const offline = fake({}, false);
    await applyCoreSetting(as(offline), native, 'snooze-limited-threads:|||', 'true');
    expect(offline.writes).toEqual([]);
    expect(toasts(as(offline)).at(-1)).toMatchObject({ kind: 'warning', title: 'Setting not saved', description: 'Connect an environment to save this setting.' });
    const client = fake({ snoozeLimitedThreads: true, responseStreamingMode: 'turn', threadTitleInstructions: 'Lead with the ticket.' });
    applyDeviceSetting(client.local as never, 'chatWidth', 'full');
    expect(restoreLabels(client.local as never, client.config.settings as Obj)).toEqual(['Chat width', 'Snooze until limits reset', 'Response streaming', 'Thread title instructions']);
    expect(restoreLabels(fake({ backgroundActivity: { profile: 'balanced', overrides: {}, schemaVersion: 1 }, textGenerationModelSelection: { model: 'gpt-6-luna', options: [{ value: 'low', id: 'reasoningEffort' }], instanceId: 'codex' } }).local as never,
      { backgroundActivity: { profile: 'balanced', overrides: {}, schemaVersion: 1 }, textGenerationModelSelection: { model: 'gpt-6-luna', options: [{ value: 'low', id: 'reasoningEffort' }], instanceId: 'codex' } })).toEqual([]);
    await applyCoreSetting(as(client), native, 'restore-device-defaults:|||', '');
    expect(client.writes.at(-1)).toEqual({ patch: { snoozeLimitedThreads: false, responseStreamingMode: 'paragraph', threadTitleInstructions: '' } });
    expect((client.local.clientSettings as Obj).chatWidth).toBe('comfortable');
  });
  // CompactIdleThreadsRow and scopedSettings.ts forDestination: a server before modes keeps only the switch, a later one
  // kept the mode in that field, a current one has `idleCompaction`.
  test('Compact idle threads writes the form each server reads; Never needs every target to store modes', () => {
    const patches = (settings: Obj, mode: string | undefined, clear = false) => {
      const client = as(fake(settings));
      return settingPlan(client, resolveScope(client, '', '', ''), 'idleCompaction', mode as never, clear, client.config.settings as Obj).serverWrites.map(write => write.patch);
    };
    expect(patches({ compactIdleThreads: false }, 'beforeCacheExpires')).toEqual([{ compactIdleThreads: true }]);
    expect(patches({ compactIdleThreads: true }, 'onReturn')).toEqual([{ compactIdleThreads: false }]);
    expect(patches({ compactIdleThreads: 'onReturn' }, 'off')).toEqual([{ compactIdleThreads: 'off' }]);
    expect(patches({ compactIdleThreads: false, idleCompaction: 'onReturn' }, 'off')).toEqual([{ idleCompaction: 'off' }]);
    expect(patches({ compactIdleThreads: true, idleCompaction: 'beforeCacheExpires' }, undefined, true)).toEqual([{ idleCompaction: 'onReturn' }]);
    expect(() => serverValue('idleCompaction', 'true', serverContext(as(fake()), resolveScope(as(fake()), '', '', ''), new Map()))).toThrow('Choose a supported idle compaction mode.');
    const contextOf = (settings: Obj) => { const client = as(fake(settings)); return serverContext(client, resolveScope(client, '', '', ''), new Map()); };
    const view = (row: ReturnType<typeof idleCompactionRow>) => [row.label, row.resettable, row.inheritance, row.options.map(option => [option.selected, option.disabled])];
    expect(idleCompactionRow(contextOf({}))).toMatchObject({ id: 'compact-idle-threads', title: 'Compact idle threads', kind: 'select', width: 224, resetLabel: 'compact idle threads' });
    expect(idleCompactionRow(contextOf({})).options.map(option => option.label)).toEqual(['When you return', 'Before the cache expires (beta)', 'Never']);
    expect(view(idleCompactionRow(contextOf({})))).toEqual(['When you return', false, 'default', [[true, false], [false, false], [false, true]]]);
    expect(view(idleCompactionRow(contextOf({ compactIdleThreads: true })))).toEqual(['Before the cache expires (beta)', true, 'environment', [[false, false], [true, false], [false, true]]]);
    expect(view(idleCompactionRow(contextOf({ compactIdleThreads: false, idleCompaction: 'off' })))).toEqual(['Never', true, 'environment', [[false, false], [false, false], [true, false]]]);
    expect(idleCompactionRow(contextOf({ compactIdleThreads: 'off' })).label).toBe('Never');
    // Targets that resolve different modes read Mixed, with no option chosen.
    const context = contextOf({ compactIdleThreads: false, idleCompaction: 'onReturn' });
    const mixed = idleCompactionRow({ ...context, targets: [context.targets[0]!, { ...context.targets[0]!, environmentId: 'env2', settings: { compactIdleThreads: true } }] });
    expect(view(mixed)).toEqual(['Mixed', false, 'mixed', [[false, false], [false, false], [false, true]]]);
  });
  // CompactIdleThreadsRow (SettingsPanels.tsx:1441) ends Providers › Advanced: the open page's answer builds it and
  // names the scope its edits write to, with the environment the route resolved when none was chosen.
  test("Providers' open page builds Compact idle threads and names its write scope; the closed page's answer has neither", async () => {
    const page = async (active: boolean) => await scopedProviderPage(as(fake({ compactIdleThreads: true })), null, ['', active, 0, 0, '', '', '', '']) as Obj;
    const open = await page(true), closed = await page(false);
    expect([closed.environment, closed.writeScope, closed.advanced]).toEqual(['', '', []]);
    expect([open.environment, open.writeScope]).toEqual(['env1', 'env1||']);
    expect((open.advanced as Obj[]).map(row => [row.id, row.label, row.divider, row.description])).toEqual([['compact-idle-threads', 'Before the cache expires (beta)', true,
      'When to compact an idle Claude thread over 200,000 tokens: after you return, just before its prompt cache expires, or never. Each Claude provider can override this.']]);
  });
  test("restore lists and resets the Browser defaults (getChangedBrowserSettingLabels)", async () => {
    // Settings › Integrations › Browser's rows are device-local: listed and reset with no environment connected.
    const client = fake({}, false);
    applyBrowserDefault(client, 'frame-rate', '60');
    applyBrowserDefault(client, 'zoom', '1.25');
    applyBrowserDefault(client, 'viewport', 'iphone-12-pro');
    expect(restoreLabels(client.local as never, null)).toEqual(['Browser viewport', 'Browser zoom', 'Recording frame rate']);
    applyDeviceSetting(client.local as never, 'chatWidth', 'wide');
    applyDeviceSetting(client.local as never, 'browserLinkTarget', 'app');
    applyBrowserDefault(client, 'appearance', 'dark');
    applyBrowserDefault(client, 'key-presses', 'true');
    applyBrowserDefault(client, 'auto-show', 'false');
    expect(restoreLabels(client.local as never, null)).toEqual(['Chat width', 'Browser viewport', 'Browser zoom', 'Browser appearance',
      'Recording frame rate', 'Recording key presses', 'Open links in', 'Floating preview']);
    await applyCoreSetting(as(client), native, 'restore-device-defaults:|||', '');
    expect(restoreLabels(client.local as never, null)).toEqual([]);
    expect(browserDefaultsView(client)).toMatchObject({ viewportValue: 'fill', zoomLabel: '100%', appearanceLabel: 'System', frameRateLabel: '30 fps', keyPresses: false, autoShow: true });
  });
  test('restore lists and re-grants Agent browser access after the browser rows (useSettingsRestore)', async () => {
    const client = fake({ enableAgentBrowserAccess: false, snoozeLimitedThreads: true });
    applyBrowserDefault(client, 'zoom', '1.25');
    // The reference's order: the environment rows, getChangedBrowserSettingLabels, then "Agent browser access".
    expect(restoreLabels(client.local as never, client.config.settings as Obj)).toEqual(['Snooze until limits reset', 'Browser zoom', 'Agent browser access']);
    expect(restoreLabels(client.local as never, null)).toEqual(['Browser zoom']);
    expect(restoreLabels(client.local as never, { enableAgentBrowserAccess: true })).toEqual(['Browser zoom']);
    await applyCoreSetting(as(client), native, 'restore-device-defaults:|||', '');
    expect(client.writes.at(-1)).toEqual({ patch: { snoozeLimitedThreads: false, enableAgentBrowserAccess: true } });
    expect(restoreLabels(client.local as never, client.config.settings as Obj)).toEqual([]);
    expect(integrationRows(client.config.settings as Obj, '', 'Studio', true).browser[0]).toMatchObject({ title: 'Agent browser access', checked: true });
    // Alone, it still enables Restore and names itself; Confirm writes only it.
    const only = fake({ enableAgentBrowserAccess: false });
    expect(await settingsCore(as(only), native, '', '', '', '', 'general', '', true)).toMatchObject({ restoreCount: 1, restoreText: 'This will reset: Agent browser access.' });
    await applyCoreSetting(as(only), native, 'restore-device-defaults:|||', '');
    expect(only.writes).toEqual([{ patch: { enableAgentBrowserAccess: true } }]);
  });
  test('a project or checkout scope restores device settings only: no override is set or cleared (useSettingsRestore)', async () => {
    // The reference's one patch carries environment-wide keys, so planScopedSettingsPatch plans no server write there; the
    // device keys save, so no warning. Clearing p1's override here would turn its agent browser access off (the environment's).
    const overrides = { p1: { enableAgentBrowserAccess: true, responseStreamingMode: 'token' }, p2: { sidebarAutoSettleAfterDays: 7 } };
    const client = fake({ enableAgentBrowserAccess: false, projectSettingsOverrides: overrides });
    for (const id of ['restore-device-defaults:||repo|', 'restore-device-defaults:||repo|p1']) {
      applyDeviceSetting(client.local as never, 'diffLayout', 'split');
      const notices = toasts(as(client)).length;
      expect(await applyCoreSetting(as(client), native, id, '')).toBe('Device settings restored');
      expect([client.writes, (client.local.clientSettings as Obj).diffLayout, toasts(as(client)).length]).toEqual([[], 'stacked', notices]);
      expect((client.config.settings as Obj).projectSettingsOverrides).toEqual(overrides);
    }
    // The environment scope still re-grants it on the environment.
    await applyCoreSetting(as(client), native, 'restore-device-defaults:|||', '');
    expect(client.writes).toEqual([{ patch: { enableAgentBrowserAccess: true } }]);
  });
  test('Restore defaults lists the scope target\'s values, in the reference\'s order (useSettingsRestore over useScopedSettings)', async () => {
    // As checked on the reference: the environment snoozes limited threads; the checkout p1 of "repo" turns
    // Auto-settle merged threads off and waits for the full response. Each scope lists what its representative target resolves to.
    const client = fake({ snoozeLimitedThreads: true, projectSettingsOverrides: { p1: { sidebarAutoSettleOnMerge: false, responseStreamingMode: 'turn' } } });
    const list = async (projectKey: string, checkout: string, route = 'general') => (await settingsCore(as(client), native, '', projectKey, checkout, '', route, '', true)).restoreText;
    expect(await list('', '')).toBe('This will reset: Snooze until limits reset.');
    // p1's overrides over its environment, and New thread mode: the t3.json tier resolves the unset key to its built-in "local",
    // which is not the default's null (the reference lists it in every project scope once the file is read).
    const p1 = 'This will reset: Auto-settle merged threads, Snooze until limits reset, Response streaming, New thread mode.';
    expect(await list('repo', 'p1')).toBe(p1);
    // Every route reads the scope (Restore defaults is the settings header's), and the project scope's target is its first member.
    expect([await list('repo', 'p1', 'source-control'), await list('repo', '')]).toEqual([p1, p1]);
    expect(await list('repo', 'p2')).toBe('This will reset: Snooze until limits reset, New thread mode.');
    // Device values take their places among the environment's; a font counts its family and its size.
    for (const [key, value] of [['confirmThreadDelete', 'false'], ['wordWrap', 'false'], ['fontSizeCode', '14'], ['chatWidth', 'wide']] as const)
      applyDeviceSetting(client.local as never, key, value);
    expect(await list('', '')).toBe('This will reset: Chat width, Snooze until limits reset, Word wrap, Code font, Delete confirmation.');
    // No connected target lists the device values alone (DEFAULT_SERVER_SETTINGS).
    expect(restoreLabels(client.local as never, null)).toEqual(['Chat width', 'Word wrap', 'Code font', 'Delete confirmation']);
  });
  test('Restore defaults lists "Theme mix" for a theme on one appearance, after Follow system (useTheme themeHalves)', async () => {
    // The library's Use puts a one-palette theme on its own half (assignHalf): the reference stores the mix and lists it
    // (`themeHalves !== null`), so Restore defaults is enabled with it alone; a whole theme (setTheme) clears the mix.
    const client = fake();
    client.local.customThemes = [{ id: 'dusk', label: 'Dusk', appearance: 'dark', light: null, dark: { canvas: '#101820', accent: '#44cc88', text: '#f0f0f0' } }];
    applyDeviceSetting(client.local as never, 'theme', 'dusk');
    expect([(client.local.clientSettings as Obj).theme, (client.local.clientSettings as Obj).themeDark]).toEqual(['t3-code', 'dusk']);
    expect(await settingsCore(as(client), native, '', '', '', '', 'general', '', true)).toMatchObject({ restoreCount: 1, restoreText: 'This will reset: Theme mix.' });
    applyDeviceSetting(client.local as never, 'appearanceMode', 'dark');
    expect(restoreLabels(client.local as never, null)).toEqual(['Follow system', 'Theme mix']);
    applyDeviceSetting(client.local as never, 'theme', 'grove');
    expect(restoreLabels(client.local as never, null)).toEqual(['Theme', 'Follow system']);
    // A half over a whole theme is both.
    applyDeviceSetting(client.local as never, 'theme', 'dusk');
    expect(restoreLabels(client.local as never, null)).toEqual(['Theme', 'Follow system', 'Theme mix']);
    await applyCoreSetting(as(client), native, 'restore-device-defaults:|||', '');
    expect(restoreLabels(client.local as never, null)).toEqual([]);
  });
  test('t3.json is read for each member of a project scope', async () => {
    const client = fake();
    client.files['/a'] = '{"worktreeSubmodules":"none"}';
    // Worktrees' rows are the storage panel's to draw (settings-data.ts worktreeRows), Submodules under Creation's Advanced.
    const core = await settingsCore(as(client), native, '', 'repo', 'p1', '', 'worktrees', '', true);
    expect([core.sections, core.kind, core.scopeKey]).toEqual([[], 'checkout', '|repo|p1']);
    const rows = await worktreeRows(as(client), native, resolveScope(as(client), '', 'repo', 'p1'), false);
    const row = rows.creationAdvanced.find(entry => entry.id === 'worktree-submodules')!;
    expect([rows.creation.map(entry => entry.id), row.label, row.inheritance, row.divider]).toEqual([['start-from-origin'], 'Skip', 'inherited', false]);
    // A bare project id (Project settings' target) is its project on every environment: /projects/$projectKey
    // redirects with the key and no machine (routes/projects.$projectKey.tsx).
    const legacy = await settingsCore(as(client), native, '', '', '', 'p3', 'storage', '', true);
    expect([legacy.kind, legacy.projectLabel, legacy.showScope]).toEqual(['project', 'Single checkout two', true]);
  });
  test("the Project page's Model row is General's, in the project scope", async () => {
    // ProjectSettingsPanel renders ProjectDefaultsSettings' modelRow: ProviderModelPicker + TraitsPicker.
    const client = fake();
    const project = await settingsCore(as(client), native, '', 'repo', '', '', 'projects', '', true);
    expect(project.projectModel.map(row => [row.id, row.kind, row.description, row.label, row.label2, row.status, row.divider]))
      .toEqual([['default-model', 'model', 'Model for new threads in this project.', 'GPT-5.6-Luna', 'Medium', '', false]]);
    expect(project.scopeKey).toBe('|repo|');
    // acf7043b72: an environment or checkout the project is not on offers "Show all environments", which keeps the project.
    const stale = async (machine: string, projectKey: string, checkout: string, route = 'projects') => {
      const core = await settingsCore(as(client), native, machine, projectKey, checkout, '', route, '', true);
      return [core.kind, core.message, core.showAllEnvironments, core.allEnvironmentsProject, core.allEnvironmentsMember];
    };
    expect(await stale('env9', 'repo', '')).toEqual(['unavailable', 'This environment is no longer available.', true, 'repo', 'p1']);
    expect(await stale('', 'repo', 'gone')).toEqual(['unavailable', 'This checkout is no longer available in the selected project and environment.', true, 'repo', 'p1']);
    expect((await stale('', 'gone', ''))[2]).toBe(false);
    expect((await stale('env9', 'repo', '', 'general'))[2]).toBe(false);
    // SettingsScopeBoundary: a device-only page (General, Chat, Diffs, About) keeps its rows on any scope, and no message.
    for (const route of ['general', 'chat', 'about']) {
      const core = await settingsCore(as(client), native, 'env9', 'repo', '', '', route, '', true);
      expect([route, core.kind, core.message, core.showScope, core.sections.length > 0]).toEqual([route, 'unavailable', '', false, true]);
    }
    const offline = await settingsCore(as(fake({}, false)), native, '', '', '', '', 'diffs', '', true);
    expect([offline.message, offline.sections.map(section => section.title)]).toEqual(['', ['Diffs', 'Pull requests']]);
    expect((await settingsCore(as(client), native, '', '', '', '', 'projects', '', true)).projectModel).toEqual([]);
    expect((await settingsCore(as(client), native, '', 'repo', '', '', 'general', '', true)).projectModel).toEqual([]);
    // TraitsPicker's trigger names every trait (buildTraitsTriggerDisplay), as the composer's does: "Medium · 1M".
    const wide = fake();
    const context = { id: 'contextWindow', label: 'Context window', type: 'select', options: [{ id: '200k', label: '200K' }, { id: '1m', label: '1M', isDefault: true }] };
    const luna = provider.models[0]!;
    wide.config.providers = [{ ...provider, models: [{ ...luna, capabilities: { optionDescriptors: [...luna.capabilities!.optionDescriptors, context] } }] }];
    const traits = (await settingsCore(as(wide), native, '', 'repo', '', '', 'projects', '', true)).projectModel[0]!;
    expect([traits.label2, traits.options2.map(entry => entry.label)]).toEqual(['Medium · 1M', ['reasoningEffort', 'Low', 'Medium', 'High', 'Context window', '200K', '1M']]);
  });
});

describe('navigation and search', () => {
  const context = { connected: true, autoSettle: true, scopeKind: 'all', keybindings: [{ command: 'sidebar.toggle', key: 'mod+b' }] };
  test('sidebar lists the reference pages by concern (SETTINGS_NAV_GROUPS, 7c3b3b3605ef)', () => {
    const items = settingsNavigation('', context).items;
    expect(items.map(item => item.title)).toEqual(['General', 'Appearance', 'Chat', 'Diffs & pull requests', 'Keybindings', 'SnapShots', 'Providers', 'Threads', 'Browser', 'Devices',
      'Scheduled tasks', 'Projects', 'Source control', 'Worktrees', 'Environments', 'Sharing & access', 'Maintenance', 'Labs', 'About', 'Archive']);
    expect(items.filter(item => item.groupStart).map(item => [item.title, item.heading])).toEqual([['Providers', 'Agents'], ['Projects', 'Code'], ['Environments', 'Connections'], ['Maintenance', '']]);
    expect(items.find(item => item.route === 'access')).toMatchObject({ title: 'Sharing & access', icon: 'shield-check', heading: '' });
    expect(settingsNavigation('', { ...context, scopeKind: 'project' }).items.map(item => item.title)).toEqual(items.map(item => item.title));
    expect([breadcrumbLabel('open-source-licenses'), breadcrumbLabel('access')]).toEqual(['Open source licenses', 'Sharing & access']);
  });
  test('search ranks like the reference and keybinding commands sort last', () => {
    expect(searchSettings('model', context).map(item => item.title).slice(0, 2)).toEqual(['Default model', 'Text generation model']);
    expect(searchSettings('mod+b', context).map(item => item.title)).toEqual(['Sidebar: Toggle']);
    expect(searchSettings('wrap', context)[0]!.id).toBe('word-wrap');
    expect(searchSettings('auto settle', { ...context, autoSettle: false }).map(item => item.id)).not.toContain('auto-settle-inactive-threads');
    expect(searchSettings('tailscale', context)).toEqual([]);
    // The About rows (472ea7ec5f): Version by its update words, and Update track, both on About.
    expect(searchSettings('check for updates', context).map(item => item.id)).toContain('app-version');
    expect(searchSettings('update track', context)).toEqual([expect.objectContaining({ id: 'update-track', title: 'Update track', route: 'about' })]);
    expect(searchSettings('nightly', context).map(item => [item.id, item.target])).toContainEqual(['update-track', 'update-track']);
    const result = settingsNavigation('send shortcut', context);
    expect([result.firstRoute, result.firstTarget, result.items[0]!.section]).toEqual(['chat', 'send-shortcut', 'Chat']);
    expect(scopeAvailable('environment-defaults', 'project')).toBe(false);
    expect(commandLabel('composer.sendAlternate')).toBe('Composer: Opposite Queue or Steer Action');
  });
  test('the reference app finds the same settings, in its order', () => {
    // The desktop app's results for ten queries, keybinding commands left out (they follow the server's bindings).
    const desktop = { ...context, localBackend: true, localEnvironment: true };
    const titles = (query: string) => searchSettings(query, desktop).filter(item => item.route !== 'keybindings').map(item => `${item.title} / ${breadcrumbLabel(item.route)}`);
    expect(titles('update')).toEqual(['Update track / About', 'Provider update checks / Maintenance', 'Continue threads after restarts / Maintenance', 'Version / About']);
    expect(titles('credits')).toEqual(['Remove agent credits when merging / Source control']);
    expect(titles('clients')).toEqual(['This machine / Sharing & access']);
    expect(titles('worktree')).toEqual(['Worktree cleanup / Worktrees', 'Worktree location / Worktrees', 'Worktree branch naming / Worktrees', 'Keep worktrees with local changes / Worktrees',
      'New threads / Threads', 'Submodules / Worktrees', 'Start from remote / Worktrees', 'Actions / Projects']);
    expect(titles('theme')).toEqual(['Themes / Appearance']);
    expect(titles('model')).toEqual(['Default model / Threads', 'Text generation model / Threads', 'Source control writer model / Source control', 'Review guide model / Source control',
      'Project defaults and overrides / Threads', 'Providers / Providers', 'Health check interval / Providers']);
    expect(titles('token')).toEqual(['GitHub accounts and token / Source control', 'Thread details / Appearance', 'Compact idle threads / Providers', 'Response streaming / Threads', 'Context window indicator (legacy) / Labs', 'Cursor account usage / Providers',
      'GitCafe credentials / Source control', 'Bitbucket credentials / Source control']);
    expect(titles('mcp')).toEqual([]);
    expect(titles('sidebar')).toEqual(['Project grouping / General', 'Project order / General', 'Working section / General', 'Auto-settle inactive threads / Threads',
      'Auto-settle merged threads / Threads', 'Keep threads with a PR / Threads', 'Days of inactivity before auto-settle / Threads', 'Environment icon / Sharing & access']);
    expect(titles('settle')).toEqual(['Auto-settle inactive threads / Threads', 'Auto-settle merged threads / Threads', 'Days of inactivity before auto-settle / Threads', 'Keep threads with a PR / Threads']);
    // 7c3b3b3605ef's moves: network access on Sharing & access, pull request sharing on Environments, the fetch interval in
    // Background activity's dialog (its jump lands on that row), Diff colors on Diffs.
    expect(titles('network access')).toEqual(['Network access / Sharing & access', 'Tailscale HTTPS / Sharing & access']);
    expect(titles('pull request sharing')).toEqual(['Pull request sharing / Environments']);
    expect(settingsNavigation('git fetch', desktop)).toMatchObject({ firstRoute: 'maintenance', firstTarget: 'background-activity' });
    expect(titles('diff colors')).toEqual(['Diff colors / Diffs & pull requests']);
    // Environment icon is offered wherever Sharing & access is, as upstream's item has no condition.
    expect(searchSettings('environment icon', { ...desktop, localBackend: false }).map(item => item.id)).toContain('environment-icon');
  });
  test('the settings answer names the jump it is for and the reveal stop of the fold its row sits under', async () => {
    const core = await settingsCore(as(fake()), native, '', '', '', '', 'chat', 'skills-in-slash-menu', true);
    expect(core).toMatchObject({ route: 'chat', jumpTarget: 'skills-in-slash-menu', jumpStop: 'settings-advanced-composer-reveal', notice: '' });
    expect(core.sections.find(section => section.id === 'composer')!.advanced.map(row => row.id)).toEqual(['skills-in-slash-menu']);
    expect(await settingsCore(as(fake()), native, '', '', '', '', 'chat', 'send-shortcut', true)).toMatchObject({ jumpTarget: 'send-shortcut', jumpStop: '' });
  });
  test('a jump to a separate font row draws those rows, asks for Separate fonts and waits until it reads on', async () => {
    const typography = (core: Awaited<ReturnType<typeof settingsCore>>) => core.sections.find(section => section.id === 'typography')!;
    const off = fake();
    for (const target of ['prompt-font', 'terminal-font']) {
      const core = await settingsCore(as(off), native, '', '', '', '', 'appearance', target, true);
      // TypographySection's showAdvanced: the rows show at once, and the answer holds the jump until the write lands.
      expect(core).toMatchObject({ jumpTarget: '', separateFontsWanted: true });
      expect(typography(core).rows.map(row => row.id)).toEqual(['interface-font', 'prompt-font', 'code-font', 'terminal-font', 'word-wrap']);
      expect(typography(core).advanced[0]).toMatchObject({ id: 'separate-fonts', checked: true });
    }
    // Closed Settings asks for nothing.
    expect(await settingsCore(as(off), native, '', '', '', '', 'appearance', 'prompt-font', false)).toMatchObject({ separateFontsWanted: false });
    await applyCoreSetting(as(off), native, 'separate-fonts:typographyAdvanced|||', 'true');
    expect(await settingsCore(as(off), native, '', '', '', '', 'appearance', 'prompt-font', true)).toMatchObject({ jumpTarget: 'prompt-font', separateFontsWanted: false });
    // Any other target is the jump's at once; Font smoothing lands under Typography's fold.
    const smoothing = await settingsCore(as(fake()), native, '', '', '', '', 'appearance', 'font-smoothing', true);
    expect(smoothing).toMatchObject({ jumpTarget: 'font-smoothing', jumpStop: 'settings-advanced-typography-reveal', separateFontsWanted: false });
    expect(typography(smoothing).rows.map(row => row.id)).toEqual(['interface-font', 'code-font', 'word-wrap']);
  });
});

describe('appearance', () => {
  test('palette follows each half of the theme pair and contrast', () => {
    const stock = palette(decodeClientPrefs({}));
    expect(stock.canvas).toBe('light-dark(#fcfcfc, #000000)');
    expect(stock.accent).toBe('light-dark(#1b4ed8, #346bf1)');
    const mixed = palette(decodeClientPrefs({ themeLight: 'grove', themeDark: 'ocean' }));
    expect(mixed.canvas).toBe(`light-dark(${themeRoles('grove', 'light').canvas}, ${themeRoles('ocean', 'dark').canvas})`);
    expect(palette(decodeClientPrefs({ appearanceContrast: 200 })).text).toBe('light-dark(#000000, #ffffff)');
    expect(palette(decodeClientPrefs({ appearanceContrast: 50 })).text).not.toBe(stock.text);
    expect(mix('#000000', 0.5, '#ffffff')).toBe('#808080');
    // ui/combobox.tsx ComboboxItem (the font picker's rows): highlighted foreground/9%, selected foreground/6%.
    expect([stock.optionHighlight, stock.optionSelected]).toEqual(['light-dark(#18181b17, #f5f5f517)', 'light-dark(#18181b0f, #f5f5f50f)']);
    expect(Object.keys(stock)).not.toContain('heading');
  });
  test('tiles, theme cards and rows', () => {
    const prefs = decodeClientPrefs({ themeLight: 'iris', themeDark: 'iris' });
    expect(modeTiles('dark', prefs).map(tile => [tile.label, tile.selected, tile.split])).toEqual([['System', false, true], ['Light', false, false], ['Dark', true, false]]);
    const cards = themeCards(prefs);
    expect(cards.map(card => card.label)).toEqual(['T3 Code', 'T3 Chat', 'Grove', 'Ocean', 'Ember', 'Iris']);
    expect(cards.find(card => card.id === 'iris')!.orbs.every(orb => orb.picked)).toBe(true);
    const sections = appearanceSections(decodeClientPrefs({ glassOpacity: 60 })), rows = sections.flatMap(section => section.rows);
    // AppearanceSettingsPanel at 6d450a4284: Interface, then Typography, each ending with its Advanced fold.
    expect(sections.map(section => [section.id, section.rows.map(row => row.title), (section.advanced ?? []).map(row => row.title)])).toEqual([
      ['appearance-interface', ['Contrast', 'Glass opacity', 'Chat width', 'Thread details', 'Effort display', 'Render math'], ['Panel animations', 'Composer context']],
      ['typography', ['Interface font', 'Monospace font', 'Word wrap'], ['Separate fonts', 'Font smoothing']]]);
    // Environment identification is not offered: this stable app has no Dev or Nightly stage to identify.
    expect(appearanceSections(decodeClientPrefs({}))[0]!.advanced!.map(row => row.id)).toEqual(['panel-animations', 'composer-context']);
    // Separate fonts is a switch on typographyAdvanced with no reset, as SeparateFontsRow.
    expect(sections[1]!.advanced![0]).toMatchObject({ id: 'separate-fonts', kind: 'switch', checked: false, resettable: false, target: 'typographyAdvanced',
      description: 'Set the prompt, code, and terminal fonts separately instead of one monospace font.' });
    expect(rows.find(row => row.id === 'setting-glass-opacity')!.label).toBe('60%');
    expect(rows.filter(row => row.kind === 'font').map(row => [row.info, row.amount])).toEqual([['preview-prompt', 16], ['preview-code-terminal', 13]]);
    // PromptFontPreview is the composer at the Prompt font size, whatever the Interface font size.
    expect(appearanceSections(decodeClientPrefs({ fontSizeInterface: 12 })).flatMap(section => section.rows).filter(row => row.kind === 'font').map(row => [row.amount, row.previewSize])).toEqual([[12, 14], [13, 13]]);
    const advanced = appearanceSections(decodeClientPrefs({ typographyAdvanced: true, fontSizeTerminal: 14 })).at(-1)!.rows;
    expect(advanced.map(row => [row.title, row.info])).toEqual([['Interface font', ''], ['Prompt font', 'preview-prompt'], ['Code font', 'preview-code'], ['Terminal font', 'preview-terminal'], ['Word wrap', '']]);
    expect(advanced.find(row => row.id === 'terminal-font')!.amount).toBe(14);
    // WordWrapRow's reset reads "word wrapping" (SettingResetButton label), not the row's title.
    expect(advanced.find(row => row.id === 'word-wrap')).toMatchObject({ resetLabel: 'word wrapping', resettable: false });
    expect(appearanceSections(decodeClientPrefs({ wordWrap: false })).at(-1)!.rows.find(row => row.id === 'word-wrap')).toMatchObject({ resetLabel: 'word wrapping', resettable: true });
    // A jump to a separate font row draws the separate rows with Separate fonts on (TypographySection showAdvanced).
    for (const target of ['prompt-font', 'terminal-font']) {
      const typography = appearanceSections(decodeClientPrefs({}), [], target).at(-1)!;
      expect(typography.rows.map(row => row.id)).toEqual(['interface-font', 'prompt-font', 'code-font', 'terminal-font', 'word-wrap']);
      expect(typography.advanced![0]!.checked).toBe(true);
    }
    expect(appearanceSections(decodeClientPrefs({}), [], 'font-smoothing').at(-1)!.rows.map(row => row.id)).toEqual(['interface-font', 'code-font', 'word-wrap']);
  });
});

describe('custom themes', () => {
  test('theme files: version, name, appearance, colors and variants; reserved ids refused', async () => {
    const { parseThemeFile, toHex } = await import('../../src/appearance/settings-themes');
    expect(toHex('oklch(0.591646 0.217985 0.584)')).toBe(themeRoles('t3-chat', 'light').accent);
    expect(toHex('#abc')).toBe('#aabbcc');
    expect(toHex('rgb(255, 0, 0)')).toBe('#ff0000');
    const theme = parseThemeFile(JSON.stringify({ version: 1, name: 'Dusk', appearance: 'dark', colors: { canvas: '#101820', accent: 'oklch(0.7 0.15 250)' }, variants: { light: { canvas: '#f8fafc' } } }), []);
    expect([theme.id, theme.label, theme.dark?.canvas, theme.light?.canvas]).toEqual(['dusk', 'Dusk', '#101820', '#f8fafc']);
    expect(() => parseThemeFile('{"version":2}', [])).toThrow('unsupported version');
    expect(() => parseThemeFile(JSON.stringify({ version: 1, id: 'grove', name: 'Grove', appearance: 'light', colors: {} }), [])).toThrow('reserved');
    expect(() => parseThemeFile(JSON.stringify({ version: 1, name: 'X', appearance: 'light', colors: { canvas: 'not-a-color' } }), [])).toThrow('not a valid color');
    expect(parseThemeFile(JSON.stringify({ version: 1, name: 'Dusk', appearance: 'dark', colors: {} }), ['dusk']).id).toBe('dusk-2');
  });
  test('import, duplicate and remove through the command; palette and cards follow', async () => {
    const client = fake();
    await applyCoreSetting(as(client), native, 'theme-import:|||', JSON.stringify({ version: 1, name: 'Dusk', appearance: 'dark', colors: { canvas: '#101820' } }));
    expect((client.local.customThemes as Obj[]).map(theme => theme.id)).toEqual(['dusk']);
    expect((client.local.clientSettings as Obj).themeDark).toBe('dusk');
    expect((client.local.clientSettings as Obj).themeLight).toBe('t3-code');
    const core = await settingsCore(as(client), native, '', '', '', '', 'appearance', '', true);
    expect(core.palette.canvas).toBe('light-dark(#fcfcfc, #101820)');
    expect(core.themes.map(card => [card.id, card.custom])).toContainEqual(['dusk', true]);
    const parts = ['grove', 'Grove copy', '#eeffee', '#118844', '#112211', '#44cc88'].map(encodeURIComponent).join('|');
    await applyCoreSetting(as(client), native, 'theme-save:|||', parts);
    const prefs = client.local.clientSettings as Obj;
    expect([prefs.theme, prefs.themeLight, prefs.themeDark]).toEqual(['grove-copy', 'grove-copy', 'grove-copy']);
    const copy = (client.local.customThemes as Obj[]).find(theme => theme.id === 'grove-copy') as Obj;
    expect([(copy.light as Obj).canvas, (copy.light as Obj).accent, (copy.light as Obj).sidebar]).toEqual(['#eeffee', '#118844', themeRoles('grove', 'light').sidebar]);
    await expect(applyCoreSetting(as(client), native, 'theme-save:|||', ['grove', '', '#fff', '#000', '#000', '#fff'].join('|'))).rejects.toThrow('Name the theme');
    await applyCoreSetting(as(client), native, 'theme-remove:|||', 'grove-copy');
    expect([(client.local.clientSettings as Obj).themeLight, (client.local.customThemes as Obj[]).length]).toEqual(['t3-code', 1]);
  });
});


describe('Settings menu keyboard rows', () => {
  test('shares eligible select and trait rows with its trigger, excluding disabled choices and section labels', () => {
    const KmItem = (item: unknown) => item;
    const options = [
      { id: 'low', label: 'Low', disabled: false },
      { id: 'unavailable', label: 'Unavailable', disabled: true },
      { id: 'high', label: 'High', disabled: false },
    ];
    const select = contractComponent('settings-rows.contract', 'CoreSelect');
    expect(select.derives({}, { rowId: 'reasoning', options, KmItem }).keyItems).toEqual([
      { id: 'setting-reasoning-menu-low', label: 'Low' }, { id: 'setting-reasoning-menu-high', label: 'High' },
    ]);
    expect(select.derives({}, { rowId: 'reasoning', options: options.filter(option => option.disabled), KmItem }).keyItems).toEqual([]);
    const traits = contractComponent('settings-rows.contract', 'CoreRowView');
    expect(traits.derives({}, { row: { id: 'model', options2: [
      { id: 'reasoning', label: 'Reasoning', icon: 'section' }, { id: 'low', label: 'Low', icon: '' },
      { id: 'tier', label: 'Service Tier', icon: 'section-rule' }, { id: 'fast', label: 'Fast', icon: '' },
    ] }, KmItem }).traitsItems).toEqual([
      { id: 'setting-model-effort-menu-low', label: 'Low' }, { id: 'setting-model-effort-menu-fast', label: 'Fast' },
    ]);
  });

  test('pointer focus uses the actual enabled row and leaves a highlighted row at its menu', () => {
    const menu = contractComponent('settings-rows.contract', 'CoreMenu');
    const state = menu.state(), focused: string[] = [];
    const scope = { menuId: 'select-general-reasoning', focus: (id: string) => focused.push(id) };
    menu.call('hover', state, scope, ['high', 'setting-reasoning-menu-high', false, true]);
    expect(focused).toEqual(['setting-reasoning-menu-high']);
    menu.call('focused', state, scope, ['high', true]);
    menu.call('hover', state, scope, ['high', 'setting-reasoning-menu-high', false, false]);
    expect(focused).toEqual(['setting-reasoning-menu-high', 'select-general-reasoning-keys']);
    focused.length = 0;
    menu.call('hover', state, scope, ['unavailable', 'setting-reasoning-menu-unavailable', true, true]);
    expect(focused).toEqual([]);
  });
});


test('scheduled-task opening keeps its environment-qualified invoker and existing command target', async () => {
  const row = contractComponent('settings-scheduled.contract', 'TaskRow');
  const kmOpenKey = await contractFn('menu-keys.contract', 'kmOpenKey', ['k']);
  const kmArrow = await contractFn('menu-keys.contract', 'kmArrow', ['k']);
  const kmTriggerKey = await contractFn('menu-keys.contract', 'kmTriggerKey', ['k'], { kmOpenKey, kmArrow });
  const kmTriggerTarget = await contractFn('menu-keys.contract', 'kmTriggerTarget', ['items', 'k']);
  const calls: unknown[][] = [];
  const scope = {
    item: { id: 'daily', environmentId: 'remote' }, KmItem: (item: unknown) => item, encodeURIComponent,
    kmTriggerKey, kmTriggerTarget,
    preventDefault: () => calls.push(['preventDefault']), stopPropagation: () => calls.push(['stopPropagation']),
    showPopover: (...args: unknown[]) => calls.push(['showPopover', ...args]), focus: (id: string) => calls.push(['focus', id]),
  };
  const derived = row.derives({}, scope);
  expect(derived.target).toBe('id=daily&environmentId=remote');
  row.call('triggerKey', {}, { ...scope, ...derived }, ['ArrowUp', { metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, repeat: false }]);
  expect(calls).toEqual([
    ['preventDefault'], ['stopPropagation'], ['showPopover', 'task-menu-daily', { source: 'task-actions-remote|daily' }],
    ['focus', 'task-delete-daily'],
  ]);
});

// ThreadTitleInstructionsSettings: the field saves on blur, trimmed, once edited and changed (or Mixed); the row keys it
// on its value, so it keeps the typed text until the new value arrives.
describe('thread title instructions field', () => {
  const field = contractComponent('settings-rows.contract', 'CoreTextArea');
  const fixture = (value: string, mixed = false) => {
    const commits: unknown[][] = [];
    const scope = { rowId: 'thread-title-instructions', value, mixed, placeholder: '', label: 'Thread title instructions', disabled: false, commit: (...args: unknown[]) => commits.push(args) };
    return { commits, scope, state: field.state(scope) };
  };
  test('a blur without an edit, or with only padding added, saves nothing', () => {
    const { commits, scope, state } = fixture('Lead with the ticket.');
    expect(state.draft).toBe('Lead with the ticket.');
    field.call('leave', state, scope, ['Lead with the ticket.']);
    field.call('edit', state, scope, [' Lead with the ticket.  ']);
    field.call('leave', state, scope, [' Lead with the ticket.  ']);
    expect(commits).toEqual([]);
    expect(state.draft).toBe(' Lead with the ticket.  ');
  });
  test('an edit saves its trimmed text once; a Mixed field saves any edit, even to the empty text', () => {
    const { commits, scope, state } = fixture('');
    field.call('edit', state, scope, ['  Keep it short.\n']);
    field.call('leave', state, scope, ['  Keep it short.\n']);
    field.call('leave', state, scope, ['  Keep it short.\n']);
    expect(commits).toEqual([['Keep it short.']]);
    const mixed = fixture('', true);
    field.call('edit', mixed.state, mixed.scope, ['']);
    field.call('leave', mixed.state, mixed.scope, ['']);
    expect(mixed.commits).toEqual([['']]);
  });
  test('only a textarea row draws it, below its text, keyed on its value and Mixed', () => {
    const row = contractComponent('settings-rows.contract', 'CoreRowView');
    const scope = (kind: string) => ({ row: { kind, info: '', options2: [], layerGroups: [], inheritance: '' } });
    expect(row.drawn('CoreTextArea(rowId=row.id', {}, scope('textarea'))).toBe(true);
    expect(row.drawn('CoreTextArea(rowId=row.id', {}, scope('text'))).toBe(false);
    expect(row.drawn('row flex-shrink=0 min-width="10rem"', {}, scope('textarea'))).toBe(false);
    expect(appText('src/settings/settings-rows.contract')).toContain('        for key in [`${row.mixed}:${row.value}`] key=key\n          CoreTextArea(rowId=row.id,');
  });
  test("its row: ThreadTitleInstructionsSettings' text and placeholders, a reset once set or Mixed, and a project's own value", () => {
    const rowOf = (settings: Obj, project = '', other?: Obj) => {
      const client = as(fake(settings));
      const context = serverContext(client, resolveScope(client, project ? 'env1' : '', project, ''), new Map());
      const scoped = other ? { ...context, targets: [context.targets[0]!, { ...context.targets[0]!, environmentId: 'env2', settings: other }] } : context;
      const row = generalSections(client, scoped, 'threads').flatMap(section => [...section.rows, ...(section.advanced ?? [])]).find(entry => entry.id === 'thread-title-instructions')!;
      return [row.description, row.placeholder, row.resettable, row.value, row.inheritance];
    };
    const text = 'Appended to the prompt for new and regenerated thread titles.', placeholder = 'Keep titles to a few words. Lead with the main subject.';
    expect(rowOf({})).toEqual([text, placeholder, false, '', 'environment']);
    expect(rowOf({ threadTitleInstructions: 'Lead with the ticket.' })).toEqual([text, placeholder, true, 'Lead with the ticket.', 'environment']);
    expect(rowOf({ threadTitleInstructions: 'Short.' }, '', { threadTitleInstructions: 'Long.' })).toEqual([text, 'Mixed. Enter instructions to apply to all selected targets.', true, '', 'mixed']);
    // A project scope's own setting (PROJECT_SCOPED), not the environment's: it inherits until the project sets one.
    expect(rowOf({}, 'repo')).toEqual([text, placeholder, false, '', 'inherited']);
  });
  test('it saves instructions of any length, trimmed (TrimmedString)', () => {
    const context = serverContext(as(fake()), resolveScope(as(fake()), '', '', ''), new Map());
    expect(serverValue('threadTitleInstructions', ` ${'a'.repeat(5000)}\n`, context)).toBe('a'.repeat(5000));
  });
  test("its row ends 0.25rem below the field, which keeps the reference's pb-3.5 under it", () => {
    const row = contractComponent('settings-rows.contract', 'CoreRowView');
    const bottom = (kind: string, info = '') => row.attribute('column hover=rowHover', 'padding-bottom', {}, { row: { kind, info, options2: [], layerGroups: [], inheritance: '' } });
    expect([bottom('textarea'), bottom('text'), bottom('font', 'preview-prompt')]).toEqual(['0.25rem', '0.75rem', '0.25rem']);
    const field = contractComponent('settings-rows.contract', 'CoreTextArea');
    expect([field.attribute('column width="100%" max-width="42rem"', 'padding-bottom', {}, {}), field.attribute('column width="100%" max-width="42rem"', 'max-width', {}, {})]).toEqual(['0.875rem', '42rem']);
  });
});

// ModelPickerContent onWindowKeyDown: a tab or a model picked by its shortcut is selected, then announced
// (composer-controls-commands.ts announcePickerShortcut).
describe('model picker shortcut presses', () => {
  test('⇧⌘↑/⇧⌘↓ and ⌘1–9 select as a click does, then announce through the root', () => {
    const dispatch = contractComponent('settings-shortcuts.contract', 'ShortcutDispatch');
    const calls: unknown[][] = [];
    const record = (name: string) => (...args: unknown[]) => calls.push([name, ...args]);
    const scope = { selectProvider: record('selectProvider'), selectModel: record('selectModel'), command: record('command'), pickerAnnounce: record('pickerAnnounce'), modelPickTaken: true };
    dispatch.call('providerByKey', {}, scope, ['favorites']);
    dispatch.call('modelByKey', {}, scope, ['claude-opus-5', 'claude']);
    expect(calls).toEqual([['selectProvider', 'favorites'], ['pickerAnnounce', 'cclocal:picker-provider-key', 'favorites', ''],
      ['selectModel', 'claude-opus-5', 'claude', false], ['pickerAnnounce', 'cclocal:picker-model-key', 'claude-opus-5', 'claude']]);
    expect(appText('app.contract')).toContain('\n  provide\n    interacted\n    pickerAnnounce\n    modelPickTaken\n');
    expect(appText('settings-shortcuts.contract')).toContain('component ShortcutDispatch\n  inject\n    pickerAnnounce: action\n    modelPickTaken: bool\n');
  });
  // handleModelSelect returns a slug only for a pick it made, and only then is it announced (ModelPickerContent.tsx:
  // 473-496, 797-817): a ⌘1–9 pick a command in flight drops is not.
  test('⌘1–9 announce only a pick selectModel takes, by its own rule', () => {
    const dispatch = contractComponent('settings-shortcuts.contract', 'ShortcutDispatch');
    const calls: unknown[][] = [];
    const record = (name: string) => (...args: unknown[]) => calls.push([name, ...args]);
    for (const taken of [false, true]) dispatch.call('modelByKey', {}, { selectModel: record('selectModel'), pickerAnnounce: record('pickerAnnounce'), modelPickTaken: taken }, ['gpt-6', 'codex']);
    expect(calls).toEqual([['selectModel', 'gpt-6', 'codex', false], ['selectModel', 'gpt-6', 'codex', false], ['pickerAnnounce', 'cclocal:picker-model-key', 'gpt-6', 'codex']]);
    // modelPickTaken is true exactly when selectModel takes the pick, for each target, with and without a command in flight.
    const root = contractComponent('app.contract', 'T3Code');
    const rule = appText('app.contract').match(/^  derive modelPickTaken = (.+)$/m)![1]!;
    for (const target of ['', 'task-model:t1', 'source-control-writer-model:|ctx', 'review-guide-model:|ctx']) for (const commandPending of [false, true]) {
      const state: Obj = { commandPending, modelTarget: { id: target }, composerTarget: '', pendingModal: '', modelsOpen: true, taskModelPick: '' };
      let took = false;
      root.call('selectModel', state, { command: () => { took = true; return {}; }, restRaw: () => { took = true; }, focus: () => {}, catalog: { multiple: false }, composerTargetData: chosen }, ['gpt-6', 'codex', false]);
      expect([target, commandPending, value(rule, { modelTarget: { id: target }, commandPending })]).toEqual([target, commandPending, took || state.taskModelPick !== '']);
    }
    // The rule names exactly the targets selectModel branches on, read from selectModel itself.
    const text = appText('app.contract'), from = text.indexOf('  action selectModel(');
    const body = text.slice(from, text.indexOf('\n  action ', from + 1));
    const prefixes = (code: string) => [...new Set([...code.matchAll(/modelTarget\.id\.startsWith\("([^"]+)"\)/g)].map(m => m[1]!))].sort();
    expect(prefixes(rule)).toEqual(prefixes(body));
  });
  // handleInstanceModelChange focuses the checked level after every pick (ProviderModelPicker.tsx:177-186); a pick of
  // the model already chosen draws no new levels for modelEffortFocused to wait on.
  test("a pick of the composer's own model focuses its checked level at once; another model's waits for its levels", () => {
    const root = contractComponent('app.contract', 'T3Code');
    const focused: unknown[] = [];
    const scope = { command: () => ({}), focus: (id: string) => focused.push(id), catalog: { multiple: false }, composerTargetData: chosen };
    for (const [id, provider] of [['gpt-5.4', 'codex'], ['gpt-6', 'codex']]) {
      const state: Obj = { commandPending: false, modelTarget: { id: '' }, composerTarget: '', pendingModal: '', modelsOpen: true, taskModelPick: '' };
      root.call('selectModel', state, scope, [id, provider, false]);
      expect(state.modelsOpen).toBe(true);
    }
    expect(focused).toEqual(['model-effort-row-2', 'model-search']);
  });
  // ModelPickerContent's announce blocks nothing (ModelPickerContent.tsx:779-795).
  test('an announcement waits for no command and refreshes only the shell: quick presses are each announced, and a pick right after is sent', () => {
    expect(appText('app.contract')).toContain('\n  mutation pickerAnnounced as shape Change queue refreshes shell //');
    const root = contractComponent('app.contract', 'T3Code');
    const sent: unknown[][] = [];
    const scope = { command: (...args: unknown[]) => { sent.push(args); return {}; }, focus: () => {}, catalog: { multiple: false }, composerTargetData: chosen };
    // Two quick ⇧⌘↓ while another command is in flight: both are announced, and the command keeps its guard.
    const busy: Obj = { commandPending: true };
    root.call('pickerAnnounce', busy, scope, ['cclocal:picker-provider-key', 'codex', '']);
    root.call('pickerAnnounce', busy, scope, ['cclocal:picker-provider-key', 'claudeAgent', '']);
    expect([sent, busy.commandPending]).toEqual([[['cclocal:picker-provider-key', 'codex', '', 0], ['cclocal:picker-provider-key', 'claudeAgent', '', 0]], true]);
    // ⇧⌘↓, then Return before its announcement is answered: the pick is sent.
    sent.length = 0;
    const idle: Obj = { commandPending: false, modelTarget: { id: '' }, composerTarget: '', pendingModal: '', modelsOpen: true, taskModelPick: '' };
    root.call('pickerAnnounce', idle, scope, ['cclocal:picker-provider-key', 'codex', '']);
    expect(idle.commandPending).toBe(false);
    root.call('selectModel', idle, scope, ['gpt-6', 'codex', false]);
    expect([sent, idle.commandPending]).toEqual([[['cclocal:picker-provider-key', 'codex', '', 0], ['model', 'gpt-6', 'codex', 0]], true]);
  });
});

// The reference's code font as its renderer draws it: Chromium resolves neither ui-monospace nor "SF Mono" by name, so
// DEFAULT_CODE_FONT_STACK lands on Menlo on macOS (a CDP probe of the reference renders its --font-mono in Menlo-Regular).
describe('code font', () => {
  test("the default code font and its label are the reference's as drawn: Menlo here", () => {
    expect([fontStack('', true), fontStack('', false)]).toEqual(['Menlo, Consolas, "Liberation Mono", monospace', 'system-ui']);
    expect([defaultCodeFamily([]), defaultCodeFamily([{ family: 'Consolas', monospace: true }]), defaultCodeFamily([{ family: 'SF Mono', monospace: true }, { family: 'Menlo', monospace: true }])])
      .toEqual(['Menlo', 'Consolas', 'Menlo']);
    const rows = appearanceSections(decodeClientPrefs({ typographyAdvanced: true })).at(-1)!.rows.filter(row => row.id === 'code-font' || row.id === 'terminal-font');
    expect(rows.map(row => [row.label, row.options[0]!.label, row.driver])).toEqual([['Menlo', 'Menlo', 'Menlo, Consolas, "Liberation Mono", monospace'], ['Menlo', 'Menlo', 'Menlo, Consolas, "Liberation Mono", monospace']]);
    const single = appearanceSections(decodeClientPrefs({})).at(-1)!.rows.find(row => row.id === 'code-font')!;
    expect([single.title, single.label, single.options[0]!.label]).toEqual(['Monospace font', 'Menlo', 'Menlo']);
    // Every code view reads the look's codeFont: chat Markdown, diffs, file previews and inline documents.
    expect(appText('settings-appearance-look.ts')).toContain('codeFont: fontStack(prefs.fontFamilyCode, true)');
  });
  test('the other monospace text, Tailwind\'s font-mono there, takes the same stack; no view names ui-monospace, SF Mono here', () => {
    // Each view writes the stack as a literal, which the compiler declares as one plan stack.
    const contracts = appFiles().filter(name => name.endsWith('.contract')).map(name => appText(name));
    const stacks = contracts.flatMap(text => [...text.matchAll(/"((?:[^"\\]|\\.)*Menlo(?:[^"\\]|\\.)*)"/g)].map(match => match[1]!.replaceAll('\\"', '"')));
    expect(stacks.length).toBeGreaterThan(150);
    expect([...new Set(stacks)]).toEqual([fontStack('', true)]);
    expect(contracts.filter(text => text.includes('ui-monospace'))).toEqual([]);
  });
});

describe('paints', () => {
  // SettingsSidebarNav's "/" Kbd is ui/kbd.tsx's: the foreground at 7% under muted text, the page's paint, not the sidebar's.
  test('the search field\'s "/" chip paints as the foreground', () => {
    const nav = contractComponent('settings-core.contract', 'SettingsNav');
    expect(nav.attribute('id=`paint-SettingsNav-5-${route}`', 'data-paint', { navFocused: '', navFocusedRoute: '' }, { nav: { searching: false, items: [] }, route: 'general', navActiveRoute: (route: string) => route })).toBe('text');
  });
  // ui/combobox.tsx ComboboxItem: highlighted bg-foreground/[0.09], selected bg-foreground/[0.06].
  test("the font picker's rows fill with the palette's option highlight, else its selection", () => {
    const picker = contractComponent('settings-a-fonts.contract', 'FontFamilyPicker');
    const families = [{ id: 'sf', label: 'SF Pro', value: '', selected: true }, { id: 'inter', label: 'Inter', value: 'Inter', selected: false }];
    const fill = (cursor: string, family: Obj) => picker.attribute('role="option" aria-selected=family.selected', 'background-color', { query: '', cursor },
      { families, family, pal: { optionHighlight: 'highlight', optionSelected: 'selected' } });
    expect([fill('inter', families[1]!), fill('inter', families[0]!), fill('sf', families[0]!), fill('-', families[1]!)]).toEqual(['highlight', 'selected', 'highlight', '#00000000']);
  });
  // ComboboxItem at sm and up: min-h-7, px-2 py-1 around a text-sm line, so 28pt rows.
  test("the font picker's rows are ComboboxItem's 28pt", () => {
    const picker = contractComponent('settings-a-fonts.contract', 'FontFamilyPicker');
    const families = [{ id: 'sf', label: 'SF Pro', value: '', selected: true, detail: '' }];
    const scope = { families, family: families[0], pal: {} };
    const row = (attr: string) => picker.attribute('role="option" aria-selected=family.selected', attr, { query: '', cursor: '' }, scope);
    expect(['min-height', 'padding-top', 'padding-bottom', 'padding-left', 'padding-right'].map(row)).toEqual(['1.75rem', '0.25rem', '0.25rem', '0.5rem', '0.5rem']);
    expect(() => row('height')).toThrow('has no height');
    expect(picker.attribute('text family.label font-family=family.detail', 'line-height', { query: '', cursor: '' }, scope)).toBe('1.25rem');
  });
  // FontFamilyPicker sizes its LegendList at items × 30 inside ComboboxListVirtualized's py-1, to max-h-72 (288pt).
  test('the font list is 30pt an item, so a short one keeps room under its rows', () => {
    const picker = contractComponent('settings-a-fonts.contract', 'FontFamilyPicker');
    const families = (count: number) => Array.from({ length: count }, (_, at) => ({ id: `f${at}`, label: `Font ${at}`, value: at ? `Font ${at}` : '', selected: at === 0, detail: '' }));
    const height = (count: number, query = '') => picker.attribute('scroll height=', 'height', { query, cursor: '' }, { families: families(count), pal: {}, min: Math.min });
    expect([height(2), height(9), height(12), height(3, 'zzz')]).toEqual(['4.25rem', '17.375rem', '18rem', 'auto']);
  });
});

// A stacked provider row's select fills the row (ProviderSettingsSelect `w-full`, `w-56` beside the title).
describe('select sizes', () => {
  // SelectPopup's min-w-(--anchor-width): a select that fills its row measures its trigger as it is focused or pressed.
  test("a stacked select's popup is at least as wide as its trigger", () => {
    const select = contractComponent('settings-kit.contract', 'SelectControl');
    const scope = (width: number) => ({ width, options: [], selectId: 'provider-field-idleCompaction', label: 'Compact idle threads', KmItem: (args: Obj) => args, frame: () => ({ width: 481 }) });
    const state: Obj = { anchorWidth: 0 };
    const popup = (width: number) => select.callArguments('SkPopup(', state, scope(width)).width;
    expect([popup(0), popup(224)]).toEqual([160, 224]);
    select.call('measure', state, scope(0), []);
    expect([state.anchorWidth, popup(0), popup(224)]).toEqual([481, 481, 224]);
    const sk = contractComponent('settings-kit.contract', 'SkPopup');
    expect(sk.attribute('column id=menuId', 'min-width', {}, { width: 481, endAlign: 0, menuId: 'm', invoker: 'i', label: '', list: true, items: [], keyed: 0 })).toBe(481);
    expect(appText('settings-kit.contract')).toContain('button popovertarget=`${selectId}-menu` press=measure focus=measure key=triggerKey role="combobox"');
    // Opened from the keys it measures again, so a trigger that grew with the window or the zoom sizes its popup.
    const keyed: Obj = { anchorWidth: 481 }, opened: unknown[][] = [];
    select.call('triggerKey', keyed, { ...scope(0), frame: () => ({ width: 600 }), kmTriggerKey: () => true, kmTriggerTarget: () => '', preventDefault: () => {}, stopPropagation: () => {},
      showPopover: (...args: unknown[]) => opened.push([keyed.anchorWidth, ...args]), focus: () => {} }, ['ArrowDown', { metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, repeat: false }]);
    expect([keyed.anchorWidth, select.callArguments('SkPopup(', keyed, scope(0)).width, opened.length]).toEqual([600, 600, 1]);
  });
  test('a select of width 0 fills its row; a provider field passes 0 once its row stacks', () => {
    const select = contractComponent('settings-kit.contract', 'SelectControl');
    const sized = (width: number) => ({ width, options: [], selectId: 'provider-field-authMethod', KmItem: (args: Obj) => args });
    expect([0, 224].map(width => select.attribute('box position="relative"', 'width', {}, sized(width)))).toEqual(['100%', 224]);
    expect([0, 224].map(width => select.attribute('role="combobox" aria-label=label', 'width', {}, sized(width)))).toEqual(['100%', 224]);
    const field = contractComponent('providers.contract', 'ProviderConfigField');
    const scope = (wide: boolean) => ({ wide, writable: true, instance: 'antigravity', field: { key: 'authMethod', label: 'Sign-in method', control: 'select', value: '', options: [] }, SelectOption: (args: Obj) => args,
      pick: () => {} });
    expect([true, false].map(wide => field.callArguments('SettingsPageSelect(', field.state(), scope(wide)).width)).toEqual([224, 0]);
  });
});
