// Lane settings-a: General/Appearance/Integrations/Diagnostics follow-ups.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Obj } from './domain';
import type { Native } from './protocol';
import { applyCoreSetting, decodeClientPrefs, generalSections, resolveScope, serverContext } from './settings-core';
import { advancedBackgroundValue, backgroundDialog, normalizeBackground, profileOption, resolveBackground } from './settings-a-background';

type Fake = { local: Obj; ready: boolean; environmentId: string; config: Obj; writes: Obj[]; projectGroups(): { key: string; name: string; members: Obj[] }[]; settingsCoreRequest(native: Native, method: string, payload: Obj): Promise<Obj> };
export function fakeClient(settings: Obj = {}, ready = true): Fake {
  const client: Fake = {
    local: { deviceSettings: { composerCollapseOnScroll: true, planModeEnabled: false, timestampFormat: 'locale', appearanceMode: 'system', sendShortcut: 'enter' }, clientSettings: decodeClientPrefs({}), groupingMode: 'repository' },
    ready, environmentId: 'env1', writes: [],
    config: { environment: { environmentId: 'env1', label: 'Studio', serverVersion: '0.0.46', capabilities: { projectSettingsOverrides: true } }, providers: [], settings: { projectSettingsOverrides: {}, ...settings } },
    projectGroups: () => [{ key: 'repo', name: 'Parity fixture', members: [{ id: 'p1', title: 'Parity fixture', workspaceRoot: '/a' }] }],
    async settingsCoreRequest(_native, method, payload) {
      if (method === 'server.getSettings') return client.config.settings as Obj;
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
  test('the command writes server.updateSettings with the computed value', async () => {
    const client = fakeClient();
    await applyCoreSetting(as(client), fakeNative, 'background-advanced:health|||', '600');
    expect(((client.writes[0]!.patch as Obj).backgroundActivity as Obj).baseProfile).toBe('balanced');
    expect((((client.writes[0]!.patch as Obj).backgroundActivity as Obj).overrides as Obj).providerHealthRefreshInterval).toBe(600000);
    await applyCoreSetting(as(client), fakeNative, 'background-advanced:reset|||', '');
    expect((client.writes[1]!.patch as Obj).backgroundActivity).toEqual({ schemaVersion: 1, profile: 'balanced', overrides: {} });
  });
});

import { editDraft, editorView, previewTheme, saveDraft, serializeTheme, syncDraft, themeEditorCommand, updateFamily } from './settings-appearance-editor';
import { deviceTool, labelWidth } from './settings-a-integrations';
import { archiveCommand, archiveConfirmation } from './settings-a-archive';
import { branchRef } from './scheduled-view';
import { look } from './settings-appearance-look';
import { parseThemeFile } from './settings-themes';

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
    expect(Math.abs(labelWidth('Not installed') - 93.3)).toBeLessThan(6);
  });
  test('base branch refs carry their tag', () => {
    expect(branchRef({ name: 'main', current: true }, '/a')).toEqual({ value: 'main', label: 'main', search: 'main', badge: 'current' });
    expect(branchRef({ name: 'origin/Feature', isRemote: true }, '/a').badge).toBe('remote');
    expect(branchRef({ name: 'wt', worktreePath: '/b' }, '/a').badge).toBe('worktree');
  });
  test('the archive context menu unarchives, or confirms a delete only while Delete confirmation is on', async () => {
    const client = fakeClient();
    const calls: string[] = [];
    let clicked: string | null = 'delete';
    Object.assign(client, { restAccess: () => ({ call: async () => ({ clicked }) }), manageArchivedThread: async (_n: unknown, _s: unknown, op: string, scope: string) => { calls.push(`${op} ${scope}`); } });
    await archiveCommand(as(client), fakeNative, {} as never, 'archive-menu', 'env1:p1:t1', 'Old thread');
    expect(archiveConfirmation(as(client))).toEqual({ id: 'env1:p1:t1', title: 'Old thread' });
    await archiveCommand(as(client), fakeNative, {} as never, 'archive-delete', 'env1:p1:t1', '');
    expect(calls).toEqual(['delete-archived-thread env1:p1:t1']);
    (client.local.clientSettings as Obj).confirmThreadDelete = false;
    await archiveCommand(as(client), fakeNative, {} as never, 'archive-menu', 'env1:p1:t2', 'Other');
    clicked = 'unarchive';
    await archiveCommand(as(client), fakeNative, {} as never, 'archive-menu', 'env1:p1:t3', 'Third');
    clicked = null;
    await archiveCommand(as(client), fakeNative, {} as never, 'archive-menu', 'env1:p1:t4', 'Fourth');
    expect(calls).toEqual(['delete-archived-thread env1:p1:t1', 'delete-archived-thread env1:p1:t2', 'unarchive-thread env1:p1:t3']);
    expect(archiveConfirmation(as(client)).id).toBe('');
  });
  test('look follows chat width, diff colours, identification and themes', () => {
    const client = fakeClient();
    Object.assign(client.local.clientSettings as Obj, { chatWidth: 'wide', diffColorScheme: 'blue-orange', environmentIdentificationMode: 'pill', themeLight: 'grove', themeDark: 'grove' });
    const value = look(as(client));
    expect([value.chatMax, value.diff, value.artwork, value.pill, value.themed]).toEqual([1152, 'blue-orange', false, 'Nightly', true]);
    expect(value.sidebar).toContain('light-dark(');
    expect(look(as(fakeClient())).themed).toBe(false);
  });
});

import { convertVsCodeTheme, importThemeText, parseJsonc, themeImportCommand } from './settings-appearance-import';

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
