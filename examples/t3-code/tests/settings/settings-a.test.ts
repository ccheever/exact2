// General/Appearance/Integrations/Diagnostics follow-ups.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { applyCoreSetting, decodeClientPrefs, formatSettingValue, generalSections, resolveScope, serverContext } from '../../src/settings/settings-core';
import { advancedBackgroundValue, backgroundDialog, normalizeBackground, profileOption, resolveBackground } from '../../src/settings/settings-a-background';
import { appPath } from '../support/app-files';

type Fake = { local: Obj; ready: boolean; environmentId: string; config: Obj; writes: Obj[]; projectGroups(): { key: string; name: string; members: Obj[] }[]; settingsCoreRequest(native: Native, method: string, payload: Obj): Promise<Obj> };
export function fakeClient(settings: Obj = {}, ready = true): Fake {
  const client: Fake = {
    local: { deviceSettings: { planModeEnabled: false, timestampFormat: 'locale', appearanceMode: 'system', sendShortcut: 'enter' }, clientSettings: decodeClientPrefs({}), groupingMode: 'repository' },
    ready, environmentId: 'env1', writes: [],
    config: { environment: { environmentId: 'env1', label: 'Studio', serverVersion: '0.0.46', capabilities: { projectSettingsOverrides: true } }, providers: [], settings: { projectSettingsOverrides: {}, ...settings } },
    projectGroups: () => [{ key: 'repo', name: 'Parity fixture', members: [{ id: 'p1', title: 'Parity fixture', workspaceRoot: '/a' }] }],
    async settingsCoreRequest(_native, method, payload) {
      if (method === 'server.getConfig') return client.config as Obj;
      if (method === 'projects.readFile') throw new Error('missing');
      client.writes.push(payload);
      client.config.settings = { ...(client.config.settings as Obj), ...(payload.patch as Obj) };
      return client.config.settings as Obj;
    },
  };
  return client;
}
export const fakeNative = { available: true } as unknown as Native;
const as = (client: Fake) => client as unknown as T3Client;
const backgroundRow = (client: Fake, machine = '', project = '') => {
  const scope = resolveScope(as(client), machine, project, '');
  return generalSections(as(client), serverContext(as(client), scope, new Map())).flatMap(section => section.rows).find(row => row.id === 'background-activity')!;
};

describe('background activity, advanced', () => {
  test('presets resolve and collapse back to their profile', () => {
    expect(resolveBackground({}).automaticGitFetchInterval).toBe(30);
    expect(resolveBackground({ backgroundActivity: { profile: 'battery-saver' } }).hostPowerMonitorIdleInterval).toBe(600);
    expect(normalizeBackground(resolveBackground({ backgroundActivity: { profile: 'performance' } }))).toEqual({ schemaVersion: 1, profile: 'performance', overrides: {} });
    // A custom value equal to another preset normalizes to that preset (the reference's profile scan).
    const custom = { backgroundActivity: { schemaVersion: 1, profile: 'custom', baseProfile: 'balanced', overrides: { automaticGitFetchInterval: 45000 } } };
    expect(profileOption(custom)).toBe('advanced');
    expect(profileOption({ backgroundActivity: { schemaVersion: 1, profile: 'custom', baseProfile: 'balanced', overrides: { automaticGitFetchInterval: 30000 } } })).toBe('balanced');
  });
  test('each dialog control writes the full custom override set in milliseconds', () => {
    const git = advancedBackgroundValue({}, 'git', '45') as Obj;
    expect(git.profile).toBe('custom');
    expect(git.baseProfile).toBe('balanced');
    expect((git.overrides as Obj).automaticGitFetchInterval).toBe(45000);
    expect((git.overrides as Obj).providerHealthRefreshInterval).toBe(300000);
    expect(((advancedBackgroundValue({}, 'active', '2') as Obj).overrides as Obj).hostPowerMonitorActiveInterval).toBe(5000);
    expect(((advancedBackgroundValue({}, 'git', '') as Obj).overrides as Obj).automaticGitFetchInterval).toBe(0);
    expect(((advancedBackgroundValue({}, 'battery', 'true') as Obj).overrides as Obj).pauseWhenOnBattery).toBe(true);
    const policy = advancedBackgroundValue({ backgroundActivity: { profile: 'custom', baseProfile: 'balanced', overrides: { automaticGitFetchInterval: 45000 } } }, 'policy', 'performance') as Obj;
    expect(policy).toEqual({ schemaVersion: 1, profile: 'custom', baseProfile: 'performance', overrides: { automaticGitFetchInterval: 45000 } });
    expect(advancedBackgroundValue({}, 'reset', '')).toEqual({ schemaVersion: 1, profile: 'balanced', overrides: {} });
    expect(() => advancedBackgroundValue({}, 'policy', 'turbo')).toThrow();
    expect(() => advancedBackgroundValue({}, 'locked', 'maybe')).toThrow();
  });
  test('the dialog model shows resolved seconds and switches', () => {
    const dialog = backgroundDialog({ backgroundActivity: { profile: 'custom', baseProfile: 'battery-saver', overrides: { pauseWhenOnBattery: false } } }, true);
    expect([dialog.policyLabel, dialog.git, dialog.health, dialog.active, dialog.idle]).toEqual(['Battery saver', 0, 900, 60, 600]);
    expect(dialog.switches.map(entry => [entry.label, entry.checked])).toEqual([['Pause when host is locked', true], ['Pause on host low power', true], ['Pause on client low power', true], ['Pause on battery', false]]);
  });
  test('Advanced is offered whenever the selection is one environment, with the configure button once custom', () => {
    const client = fakeClient();
    const row = backgroundRow(client);
    const advanced = row.options.find(entry => entry.id === 'advanced')!;
    expect([advanced.label, advanced.disabled, row.value2]).toEqual(['Advanced', false, '']);
    client.config.settings = { ...(client.config.settings as Obj), backgroundActivity: { schemaVersion: 1, profile: 'custom', baseProfile: 'balanced', overrides: { automaticGitFetchInterval: 45000 } } };
    const custom = backgroundRow(client);
    expect([custom.label, custom.value2, custom.description]).toEqual(['Advanced', 'configure', 'Uses custom intervals. Shared policy: Balanced.']);
    // A removed environment is not one environment: the option stays disabled with its qualifier.
    const gone = backgroundRow({ ...client, environmentId: '' } as Fake);
    expect(gone.options.find(entry => entry.id === 'advanced')!.disabled).toBe(true);
  });
  test("the inheritance popover reads Custom for the policy, as SettingInheritance's formatValue has no profile labels", () => {
    const client = fakeClient();
    expect(backgroundRow(client).layers.map(layer => [layer.label, layer.detail])).toEqual([['Environment', 'Inherits'], ['Default', 'Custom']]);
    client.config.settings = { ...(client.config.settings as Obj), backgroundActivity: { schemaVersion: 1, profile: 'performance', overrides: {} } };
    expect(backgroundRow(client).layers.map(layer => [layer.label, layer.detail])).toEqual([['Environment', 'Custom'], ['Default', 'Custom']]);
    // The rest of formatValue: model selections, writing styles, merge methods and the writer model's unset value.
    expect([formatSettingValue('textGenerationModelSelection', { instanceId: 'codex', model: 'gpt-6' }), formatSettingValue('sourceControlWritingStyle', { mode: 'conventional_commits' }),
      formatSettingValue('pullRequestMergeMethod', 'squash'), formatSettingValue('pullRequestMergeMethod', null), formatSettingValue('sourceControlWriterModelSelection', null),
      formatSettingValue('worktreeSubmodules', 'top-level'), formatSettingValue('branchNamingMode', 'static'), formatSettingValue('branchNamePrefix', '')])
      .toEqual(['gpt-6', 'Conventional Commits', 'Squash and merge', 'Last selected', 'Text generation model', 'Top level only', 'static', 'Empty']);
  });
  test('the command writes server.updateSettings with the computed value', async () => {
    const client = fakeClient();
    await applyCoreSetting(as(client), fakeNative, 'background-advanced:health|||', '600');
    expect(((client.writes[0]!.patch as Obj).backgroundActivity as Obj).baseProfile).toBe('balanced');
    expect((((client.writes[0]!.patch as Obj).backgroundActivity as Obj).overrides as Obj).providerHealthRefreshInterval).toBe(600000);
    await applyCoreSetting(as(client), fakeNative, 'background-advanced:reset|||', '');
    expect((client.writes[1]!.patch as Obj).backgroundActivity).toEqual({ schemaVersion: 1, profile: 'balanced', overrides: {} });
  });
});

import { editDraft, editorView, previewTheme, saveDraft, serializeTheme, syncDraft, themeEditorCommand, updateFamily } from '../../src/appearance/settings-appearance-editor';
import { deviceTool } from '../../src/device/settings-a-integrations';
import { archiveCommand, archiveConfirmation } from '../../src/settings/settings-a-archive';
import { branchRef } from '../../src/settings/scheduled-view';
import { look } from '../../src/appearance/settings-appearance-look';
import { parseThemeFile } from '../../src/appearance/settings-themes';

describe('theme editor', () => {
  test('a create draft seeds every family from the active theme and paints a preview', () => {
    const client = fakeClient();
    const draft = syncDraft(as(client), 'create', '', { theme: 't3-code', themeLight: 't3-code', themeDark: 't3-code' }, 'light')!;
    const view = editorView(draft);
    expect([view.title, view.saveLabel, view.rows.map(row => [row.label, row.value])]).toEqual(['Create theme', 'Create theme', [['Background', '#fcfcfc'], ['Accent', '#1b4ed8']]]);
    expect(view.groups.map(group => group.title)).toEqual(['Foundation', 'Brand & content', 'Context', 'Status']);
    expect(view.groups[0]!.rows.map(row => row.label)).toEqual(['Background', 'Surface', 'Raised surface', 'Overlay', 'Text', 'Muted text', 'Border', 'Input']);
    editDraft(as(client), 'color:canvas', '#101820');
    expect(previewTheme(as(client))!.light!.chrome).toBe('#101820');
    editDraft(as(client), 'filter', 'side');
    expect(editorView(draft).groups.flatMap(group => group.rows.map(row => row.label))).toEqual(['Sidebar background', 'Sidebar controls', 'Sidebar selection']);
    // Reopening the same dialog keeps the draft; closing drops it.
    expect(syncDraft(as(client), 'create', '', { theme: 't3-code', themeLight: 't3-code', themeDark: 't3-code' }, 'light')).toBe(draft);
    expect(syncDraft(as(client), '', '', { theme: 't3-code', themeLight: 't3-code', themeDark: 't3-code' }, 'light')).toBeNull();
    expect(previewTheme(as(client))).toBeNull();
  });
  test('families write their related roles with readable foregrounds', () => {
    const next = updateFamily({ canvas: '#fcfcfc', accent: '#1b4ed8' }, 'sidebar', '#101010');
    expect([next.sidebar, next.sidebarForeground]).toEqual(['#101010', '#fffaff']);
    expect(updateFamily({}, 'mutedForeground', '#777777').placeholder).toBe('#777777');
    expect(() => updateFamily({}, 'text', 'not a colour')).toThrow();
  });
  test('save installs a new theme as the active pair, or replaces the edited one', async () => {
    const client = fakeClient();
    syncDraft(as(client), 'create', '', { theme: 't3-code', themeLight: 't3-code', themeDark: 't3-code' }, 'light');
    editDraft(as(client), 'color:accent', '#ff0000');
    await themeEditorCommand(as(client), null, 'theme-editor-save', '', 'Aurora');
    const themes = client.local.customThemes as { id: string; label: string; light: Record<string, string> }[];
    expect([themes[0]!.id, themes[0]!.label, themes[0]!.light.accent]).toEqual(['aurora', 'Aurora', '#ff0000']);
    expect((client.local.clientSettings as Obj).themeDark).toBe('aurora');
    syncDraft(as(client), 'edit', 'aurora', { theme: 'aurora', themeLight: 'aurora', themeDark: 'aurora' }, 'light');
    editDraft(as(client), 'name', 'Aurora 2');
    saveDraft(as(client));
    expect((client.local.customThemes as { id: string; label: string }[]).map(theme => [theme.id, theme.label])).toEqual([['aurora', 'Aurora 2']]);
    // The exported file reads back as the same theme.
    const parsed = parseThemeFile(serializeTheme((client.local.customThemes as never[])[0]!), []);
    expect([parsed.id, parsed.label, parsed.light!.accent]).toEqual(['aurora', 'Aurora 2', '#ff0000']);
  });
});

describe('device tools, refs, archive menu, look', () => {
  test('DeviceToolVersions labels and the update action', () => {
    const state = { supportsToolUpdate: true, supportsToolInspection: true, hosts: [{ kind: 'local', tools: { hub: { requiredVersion: '0.12.0', installedVersions: [], runningVersion: null }, agent: { requiredVersion: '1.0.0', installedVersions: ['1.0.0'], runningVersion: '1.0.0' } } }] };
    const hub = deviceTool('hub', state), agent = deviceTool('agent', state);
    expect([hub.label, hub.aria, hub.running, hub.installed, hub.update]).toEqual(['Not installed', 'Device hub: not installed. Show details', 'Not running', 'None', 'Update to v0.12.0']);
    expect([agent.label, agent.update]).toEqual(['v1.0.0', '']);
    expect(deviceTool('hub', null).label).toBe('Version unknown');
    // The popover is end-aligned by its `position-area`; no estimated label width offsets it.
    expect('offset' in hub).toBe(false);
  });
  test('base branch refs carry their tag', () => {
    expect(branchRef({ name: 'main', current: true }, '/a')).toEqual({ value: 'main', label: 'main', search: 'main', badge: 'current', remote: false });
    expect(branchRef({ name: 'origin/Feature', isRemote: true }, '/a')).toMatchObject({ badge: 'remote', remote: true });
    // A current branch keeps its tag and still says it is remote or not: the trigger reads `remote`.
    expect(branchRef({ name: 'origin/main', current: true, isRemote: true }, '/a')).toMatchObject({ badge: 'current', remote: true });
    expect(branchRef({ name: 'wt', worktreePath: '/b' }, '/a').badge).toBe('worktree');
  });
  test('the archive context menu unarchives, or confirms a delete only while Delete confirmation is on', async () => {
    // #292: the menu is the panel's Contract popover (settings-archive.contract ArchiveMenu);
    // its choice arrives as `archive-pick` with the row's scope, and no native menu is asked for.
    const client = fakeClient();
    const calls: string[] = [], native: Obj[] = [];
    Object.assign(client, { restAccess: () => ({ call: async (request: Obj) => { native.push(request); return {}; } }), manageArchivedThread: async (_n: unknown, _s: unknown, op: string, scope: string) => { calls.push(`${op} ${scope}`); } });
    await archiveCommand(as(client), fakeNative, {} as never, 'archive-pick', 'env1:p1:t1', 'delete');
    // The live store holds no archived thread, so the confirm says "this thread".
    expect(archiveConfirmation(as(client))).toEqual({ id: 'env1:p1:t1', title: 'this thread' });
    await archiveCommand(as(client), fakeNative, {} as never, 'archive-delete', 'env1:p1:t1', '');
    expect(calls).toEqual(['delete-archived-thread env1:p1:t1']);
    (client.local.clientSettings as Obj).confirmThreadDelete = false;
    await archiveCommand(as(client), fakeNative, {} as never, 'archive-pick', 'env1:p1:t2', 'delete');
    await archiveCommand(as(client), fakeNative, {} as never, 'archive-pick', 'env1:p1:t3', 'unarchive');
    await archiveCommand(as(client), fakeNative, {} as never, 'archive-pick', 'env1:p1:t4', '');
    expect(calls).toEqual(['delete-archived-thread env1:p1:t1', 'delete-archived-thread env1:p1:t2', 'unarchive-thread env1:p1:t3']);
    expect(archiveConfirmation(as(client)).id).toBe('');
    expect(native).toEqual([]);
    await expect(archiveCommand(as(client), fakeNative, {} as never, 'archive-menu', 'env1:p1:t5', '')).rejects.toThrow('Unknown settings action');
  });
  test('the archive menu is the panel\'s one context popover: Unarchive, a separator, Delete with its trash', async () => {
    const view = await Bun.file(appPath('settings-archive.contract')).text();
    expect(view).toContain('column width="100%" position="relative" contextmenu=menuFor(thread.scope) contextPopover="archive-menu" testId=`archived-row-${thread.id}`');
    const menu = view.slice(view.indexOf('component ArchiveMenu'));
    const rows = menu.split('\n').map(line => line.trim()).filter(line => /^(button|hr|image|text) /.test(line));
    expect(rows.map(line => line.split(' ')[0])).toEqual(['button', 'text', 'hr', 'button', 'image', 'text']);
    expect(rows[0]).toContain('press=raw("rest:archive-pick", owner, "unarchive") popovertarget="archive-menu" popovertargetaction="hide" role="menuitem"');
    expect(rows[3]).toContain('press=raw("rest:archive-pick", owner, "delete") popovertarget="archive-menu" popovertargetaction="hide" role="menuitem"');
    expect(rows[4]).toBe('image "symbol:sf/trash" width=12 height=12 object-fit="contain"');
    expect([rows[1], rows[5]].map(line => /^text "([^"]+)"/.exec(line)![1])).toEqual(['Unarchive', 'Delete']);
    expect(menu).toContain('column id="archive-menu" popover="auto" position-try-fallbacks="none" role="menu"');
    const panel = await import('../support/contract-action-fixture').then(m => m.contractComponent('settings-archive.contract', 'ArchivedPanel'));
    const state = panel.state();
    panel.call('menuFor', state, {}, ['env1:p1:t9']);
    expect(state.menuOf).toBe('env1:p1:t9');
  });
  test('look follows chat width, diff colours and themes, and draws no environment identification', () => {
    const client = Object.assign(fakeClient(), { preferencesLoaded: true });
    Object.assign(client.local.clientSettings as Obj, { chatWidth: 'wide', diffColorScheme: 'blue-orange', themeLight: 'grove', themeDark: 'grove' });
    const value = look(as(client));
    expect([value.chatMax, value.diff, value.themed]).toEqual([1152, 'blue-orange', true]);
    expect(Object.keys(value).filter(key => key === 'artwork' || key === 'pill')).toEqual([]);
    expect(value.sidebar).toContain('light-dark(');
    expect(look(as(fakeClient())).themed).toBe(false);
  });
  test('the user message surface is index.css --message-surface on the stock theme, the theme role otherwise', () => {
    const client = fakeClient();
    expect(look(as(client)).message).toMatch(/#ebebed.*#1e1e1e|^#1e1e1e$|^#ebebed$/);
    Object.assign(client.local.clientSettings as Obj, { themeLight: 'grove', themeDark: 'iris' });
    expect(look(as(client)).message).toMatch(/#cce1d7.*#4b3d72|^#4b3d72$|^#cce1d7$/);
  });
});

import { convertVsCodeTheme, importThemeText, parseJsonc, themeImportCommand } from '../../src/appearance/settings-appearance-import';

describe('add a theme', () => {
  test('JSONC and VS Code themes convert into T3 Code themes', () => {
    expect(parseJsonc('{ // c\n "a": [1, 2,], /* x */ "b": "//not" , }')).toEqual({ a: [1, 2], b: '//not' });
    const theme = convertVsCodeTheme({ name: 'night-owl', type: 'dark', colors: { 'editor.background': '#011627', 'editor.foreground': '#d6deeb', focusBorder: '#122d42', 'sideBar.background': '#011627', 'button.background': '#7e57c2' } }, []);
    expect([theme.id, theme.label, theme.appearance, theme.dark!.canvas, theme.dark!.text, theme.dark!.messageAction]).toEqual(['night-owl', 'Night Owl', 'dark', '#011627', '#d6deeb', '#7e57c2']);
    expect(() => convertVsCodeTheme({ colors: { 'editor.foreground': '#fff' } }, [])).toThrow('editor.background');
    expect(importThemeText('{"version":1,"name":"Aurora","appearance":"light","colors":{"canvas":"#ffffff"}}', []).id).toBe('aurora');
  });
  test('pasted JSON is added, made active, and announced', async () => {
    const client = fakeClient();
    await themeImportCommand(as(client), null, 'add', '{"version":1,"name":"Aurora","appearance":"light","colors":{"canvas":"#ffffff"}}');
    expect((client.local.customThemes as { id: string }[]).map(theme => theme.id)).toEqual(['aurora']);
    expect((client.local.clientSettings as Obj).themeLight).toBe('aurora');
    await expect(themeImportCommand(as(client), null, 'add', '{"version":2}')).rejects.toThrow();
    await expect(themeImportCommand(as(client), null, 'search', 'Dracula')).rejects.toThrow('macOS app');
  });
});


describe('terminal appearance in the snapshot', () => {
  test('stock, selected theme, custom roles and simple/advanced font preferences reach the bridge', () => {
    const client = fakeClient();
    const prefs = client.local.clientSettings as Obj;
    let value = look(as(client));
    expect(JSON.parse(value.terminalLight).background).toEqual({ r: 252, g: 252, b: 252 });
    expect(JSON.parse(value.terminalDark).background).toEqual({ r: 10, g: 10, b: 10 });
    Object.assign(prefs, { themeLight: 'grove', themeDark: 'ocean', fontFamilyCode: 'Menlo', fontSizeCode: 15 });
    value = look(as(client));
    expect(JSON.parse(value.terminalLight).background).toEqual({ r: 243, g: 247, b: 244 });
    expect(JSON.parse(value.terminalDark).cursor).toEqual({ r: 112, g: 185, b: 238 });
    expect([value.terminalFont, value.terminalSize]).toEqual(['Menlo', 15]);
    Object.assign(prefs, { typographyAdvanced: true, fontFamilyTerminal: '', fontSizeTerminal: 12, themeDark: 'custom' });
    Object.assign(client.local, { customThemes: [{ id: 'custom', appearance: 'dark', dark: {
      terminalBackground: '#123456', terminalForeground: '#abcdef', terminalCursor: '#fedcba', terminalSelection: '#11223380',
    } }] });
    value = look(as(client));
    expect([value.terminalFont, value.terminalSize]).toEqual(['', 12]);
    expect(JSON.parse(value.terminalDark)).toEqual({ dark: true, background: { r: 18, g: 52, b: 86 },
      foreground: { r: 171, g: 205, b: 239 }, cursor: { r: 254, g: 220, b: 186 }, selectionBackground: '#11223380' });
  });
});
