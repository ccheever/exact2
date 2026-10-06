// Lane settings-core: settings shell, General and Appearance logic.
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Obj } from './domain';
import type { Native } from './protocol';
import { applyCoreSetting, applyDeviceSetting, changedDeviceLabels, clientValue, decodeClientPrefs, effectiveSetting, generalSections, parseCoreTarget,
  parseProjectFile, resolveScope, restoreLabels, serverContext, serverValue, settingPlan } from './settings-core';
import { toasts } from './toast';
import { fleet } from './settings-b-fleet';

// The app's one fleet is shared across test files; these cases are single-environment unless they add entries.
beforeEach(() => { fleet.entries.clear(); fleet.saved = []; });
afterEach(() => { fleet.entries.clear(); fleet.saved = []; });
import { appearanceSections, mix, modeTiles, palette, themeCards, themeRoles } from './settings-appearance';
import { breadcrumbLabel, commandLabel, scopeAvailable, searchSettings, settingsNavigation } from './settings-search';
import { settingsCore } from './settings-core-view';

const provider = { instanceId: 'fixture', driver: 'codex', displayName: 'Exact fixture', enabled: true, installed: true, auth: { status: 'authenticated' }, status: 'ready',
  models: [{ slug: 'luna', name: 'GPT-5.6-Luna', isDefault: true, capabilities: { optionDescriptors: [{ id: 'reasoningEffort', type: 'select', options: [{ id: 'low', label: 'Low' }, { id: 'medium', label: 'Medium', isDefault: true }, { id: 'high', label: 'High' }] }] } },
    { slug: 'astra', name: 'GPT-6-Astra' }] };
type Fake = { local: Obj; ready: boolean; environmentId: string; config: Obj; writes: Obj[]; files: Record<string, string>; projectGroups(): { key: string; name: string; members: Obj[] }[]; settingsCoreRequest(native: Native, method: string, payload: Obj): Promise<Obj> };
function fake(settings: Obj = {}, ready = true): Fake {
  const client: Fake = {
    local: { deviceSettings: { composerCollapseOnScroll: true, planModeEnabled: false, timestampFormat: 'locale', appearanceMode: 'system', sendShortcut: 'enter' }, clientSettings: decodeClientPrefs({}), groupingMode: 'repository' },
    ready, environmentId: 'env1', writes: [], files: {},
    config: { environment: { environmentId: 'env1', label: 'Studio', serverVersion: '0.0.46', capabilities: { projectSettingsOverrides: true, threadAutoSettlement: true, threadRestartContinuation: true } }, providers: [provider], settings: { defaultRuntimeMode: 'full-access', projectSettingsOverrides: {}, ...settings } },
    projectGroups: () => [{ key: 'repo', name: 'Parity fixture', members: [{ id: 'p1', title: 'Parity fixture', workspaceRoot: '/a' }, { id: 'p2', title: 'Parity fixture', workspaceRoot: '/b' }] },
      { key: 'env1:/c', name: 'Single checkout two', members: [{ id: 'p3', title: 'Single checkout two', workspaceRoot: '/c' }] }],
    async settingsCoreRequest(_native, method, payload) {
      if (method === 'server.getSettings') return client.config.settings as Obj;
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
const native = { available: true } as unknown as Native;
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
    expect(sections.map(section => section.title)).toEqual(['New threads', 'Organization', 'Behavior', 'Projects & threads', 'Confirmations', 'Text generation', 'About', 'Diagnostics', 'Legacy features']);
    expect(sections.flatMap(section => section.rows.map(row => row.title))).toEqual(['Model', 'Permissions', 'Workspace', 'Submodules', 'Project grouping', 'Project order', 'Auto-resume limited threads',
      'Snooze limited threads', 'Working section (beta)', 'Auto-settle merged threads', 'Auto-settle inactive threads', 'Days of inactivity before auto-settle', 'Thread notifications',
      'In-app notifications', 'Time format', 'Response streaming', 'Hide whitespace changes', 'Default diff file state', 'Diff layout', 'Proactive panels', 'Show skills in slash menu',
      'Rich text composer', 'Collapse composer on scroll', 'Send shortcut', 'Follow-up behavior', 'Provider update checks', 'Continue threads after restarts', 'Background activity',
      'Start from origin', 'Add project starts in', 'Unpin confirmation', 'Archive confirmation', 'Delete confirmation', 'Quit shortcut', 'Text generation model', 'Version', 'Update track', 'Mobile app', 'Diagnostics',
      'Open source licenses', 'Plan mode (legacy)', 'Context window indicator (legacy)', 'Sidebar (legacy)']);
    const row = (id: string) => sections.flatMap(section => section.rows).find(entry => entry.id === id)!;
    expect([row('default-model').label, row('default-model').label2, row('default-model').status]).toEqual(['GPT-5.6-Luna', 'Medium', 'Automatic']);
    expect([row('default-permissions').label, row('default-permissions').icon, row('default-permissions').inheritance]).toEqual(['Full access', 'lock-open', 'default']);
    expect([row('snooze-limited-threads').checked, row('snooze-limited-threads').inheritance, row('snooze-limited-threads').resettable]).toEqual([true, 'environment', false]);
    expect([row('new-threads').label, row('worktree-submodules').label, row('days-before-auto-settle').value]).toEqual(['Current checkout', 'Recursive', '3']);
    expect(row('version').value).toBe('0.0.46-nightly.20261004.1'); // this app's own release, not the server's
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

describe('traits picker', () => {
  test('sections, Default badges and the bolt; a pick keeps the other traits (TraitsPicker)', () => {
    const tier = { id: 'serviceTier', label: 'Service Tier', type: 'select', options: [{ id: 'default', label: 'Standard', isDefault: true }, { id: 'priority', label: 'Fast', description: '2x speed, increased usage' }] };
    const descriptors = (provider.models[0]!.capabilities as Obj).optionDescriptors as Obj[];
    descriptors.push(tier);
    try {
      const client = as(fake({ defaultModelSelection: { instanceId: 'fixture', model: 'luna', options: [{ id: 'reasoningEffort', value: 'high' }, { id: 'serviceTier', value: 'priority' }] } }));
      const context = serverContext(client, resolveScope(client, '', '', ''), new Map());
      const model = generalSections(client, context).flatMap(section => section.rows).find(entry => entry.id === 'default-model')!;
      expect([model.label2, model.icon]).toEqual(['High', 'fast']);
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
    await applyCoreSetting(as(client), native, 'default-permissions:||repo|p2', 'approval-required');
    expect(client.writes.at(-1)).toEqual({ patch: { projectSettingsOverrides: { p2: { defaultRuntimeMode: 'approval-required' } } } });
    await applyCoreSetting(as(client), native, 'default-permissions:reset||repo|p2', '');
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
    const client = fake({ snoozeLimitedThreads: true, responseStreamingMode: 'turn' });
    applyDeviceSetting(client.local as never, 'chatWidth', 'full');
    expect(restoreLabels(client.local as never, client.config.settings as Obj, true)).toEqual(['Chat width', 'Snooze limited threads', 'Response streaming']);
    expect(restoreLabels(fake({ backgroundActivity: { profile: 'balanced', overrides: {}, schemaVersion: 1 }, textGenerationModelSelection: { model: 'gpt-6-luna', options: [{ value: 'low', id: 'reasoningEffort' }], instanceId: 'codex' } }).local as never,
      { backgroundActivity: { profile: 'balanced', overrides: {}, schemaVersion: 1 }, textGenerationModelSelection: { model: 'gpt-6-luna', options: [{ value: 'low', id: 'reasoningEffort' }], instanceId: 'codex' } }, true)).toEqual([]);
    await applyCoreSetting(as(client), native, 'restore-device-defaults:|||', '');
    expect(client.writes.at(-1)).toEqual({ patch: { snoozeLimitedThreads: false, responseStreamingMode: 'paragraph' } });
    expect((client.local.clientSettings as Obj).chatWidth).toBe('comfortable');
  });
  test('t3.json is read for each member of a project scope', async () => {
    const client = fake();
    client.files['/a'] = '{"worktreeSubmodules":"none"}';
    const core = await settingsCore(as(client), native, '', 'repo', 'p1', '', 'general', '', true);
    const row = core.sections.flatMap(section => section.rows).find(entry => entry.id === 'worktree-submodules')!;
    expect([row.label, row.inheritance, core.kind, core.scopeKey]).toEqual(['Skip', 'inherited', 'checkout', '|repo|p1']);
    const legacy = await settingsCore(as(client), native, '', '', '', 'p3', 'storage', '', true);
    expect([legacy.kind, legacy.projectLabel, legacy.showScope]).toEqual(['checkout', 'Single checkout two', true]);
  });
});

describe('navigation and search', () => {
  const context = { connected: true, autoSettle: true, scopeKind: 'all', keybindings: [{ command: 'sidebar.toggle', key: 'mod+b' }] };
  test('sidebar lists the reference sections; Project only for a project scope', () => {
    expect(settingsNavigation('', context).items.map(item => item.title)).toEqual(['General', 'Appearance', 'Keybindings', 'SnapShots', 'Providers', 'Integrations', 'Scheduled Tasks', 'Source Control', 'Storage', 'Connections', 'Archive']);
    expect(settingsNavigation('', { ...context, scopeKind: 'project' }).items[0]!.title).toBe('Project');
    expect(breadcrumbLabel('open-source-licenses')).toBe('Open source licenses');
  });
  test('search ranks like the reference and keybinding commands sort last', () => {
    expect(searchSettings('model', context).map(item => item.title).slice(0, 2)).toEqual(['Default model', 'Text generation model']);
    expect(searchSettings('mod+b', context).map(item => item.title)).toEqual(['Sidebar: Toggle']);
    expect(searchSettings('wrap', context)[0]!.id).toBe('word-wrap');
    expect(searchSettings('auto settle', { ...context, autoSettle: false }).map(item => item.id)).not.toContain('auto-settle-inactive-threads');
    expect(searchSettings('tailscale', context)).toEqual([]);
    const result = settingsNavigation('send shortcut', context);
    expect([result.firstRoute, result.firstTarget, result.items[0]!.section]).toEqual(['general', 'setting-send-shortcut', 'General']);
    expect(scopeAvailable('environment-defaults', 'project')).toBe(false);
    expect(commandLabel('composer.sendAlternate')).toBe('Composer: Opposite Queue or Steer Action');
  });
});

describe('appearance', () => {
  test('palette follows each half of the theme pair and contrast', () => {
    const stock = palette(decodeClientPrefs({}));
    expect(stock.canvas).toBe('light-dark(#fcfcfc, #0a0a0a)');
    expect(stock.accent).toBe('light-dark(#1b4ed8, #346bf1)');
    const mixed = palette(decodeClientPrefs({ themeLight: 'grove', themeDark: 'ocean' }));
    expect(mixed.canvas).toBe(`light-dark(${themeRoles('grove', 'light').canvas}, ${themeRoles('ocean', 'dark').canvas})`);
    expect(palette(decodeClientPrefs({ appearanceContrast: 200 })).text).toBe('light-dark(#000000, #ffffff)');
    expect(palette(decodeClientPrefs({ appearanceContrast: 50 })).text).not.toBe(stock.text);
    expect(mix('#000000', 0.5, '#ffffff')).toBe('#808080');
  });
  test('tiles, theme cards and rows', () => {
    const prefs = decodeClientPrefs({ themeLight: 'iris', themeDark: 'iris' });
    expect(modeTiles('dark', prefs).map(tile => [tile.label, tile.selected, tile.split])).toEqual([['System', false, true], ['Light', false, false], ['Dark', true, false]]);
    const cards = themeCards(prefs);
    expect(cards.map(card => card.label)).toEqual(['T3 Code', 'T3 Chat', 'Grove', 'Ocean', 'Ember', 'Iris']);
    expect(cards.find(card => card.id === 'iris')!.orbs.every(orb => orb.picked)).toBe(true);
    const rows = appearanceSections(decodeClientPrefs({ glassOpacity: 60 }), true).flatMap(section => section.rows);
    expect(rows.map(row => row.title)).toEqual(['Contrast', 'Glass opacity', 'Environment identification', 'Diff colors', 'Composer context', 'Chat width', 'Panel animations', 'Interface font', 'Monospace font', 'Word wrap']);
    expect(rows.find(row => row.id === 'setting-glass-opacity')!.label).toBe('60%');
    expect(rows.filter(row => row.kind === 'font').map(row => [row.info, row.amount])).toEqual([['preview-prompt', 16], ['preview-code-terminal', 13]]);
    const advanced = appearanceSections(decodeClientPrefs({ typographyAdvanced: true, fontSizeTerminal: 14 }), true).at(-1)!.rows;
    expect(advanced.map(row => [row.title, row.info])).toEqual([['Interface font', ''], ['Prompt font', 'preview-prompt'], ['Code font', 'preview-code'], ['Terminal font', 'preview-terminal'], ['Font smoothing', ''], ['Word wrap', '']]);
    expect(advanced.find(row => row.id === 'terminal-font')!.amount).toBe(14);
  });
});

describe('custom themes', () => {
  test('theme files: version, name, appearance, colors and variants; reserved ids refused', async () => {
    const { parseThemeFile, toHex } = await import('./settings-themes');
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
