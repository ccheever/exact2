// Five differences from T3 Code 1e2ecbd975 (MIT,
// see LICENSE-T3): the scheduled task's base-branch picker (WorktreeBaseBranchPicker, usePaginatedBranches,
// BranchPicker), a one-palette theme (themePalette.ts resolveThemeAppearance, ThemeSettings.tsx Use and Create
// theme), the theme editor's submit label (ThemeEditorPanel.tsx) and the Connections failure text (Electron IPC).
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import { initialShell, obj, type Obj } from '../../src/client/domain';
import { watchLive } from '../../src/client/live-streams';
import { scheduledPage, scheduledRefsWanted, pickerOf } from '../../src/settings/scheduled-view';
import { noteRefScrollEnd } from '../../src/composer/ref-paging';
import { effectiveMode, palette, themeRoles } from '../../src/appearance/settings-appearance';
import { look, windowAppearanceMode } from '../../src/appearance/settings-appearance-look';
import { applyDeviceSetting, decodeClientPrefs, DEVICE_DEFAULTS } from '../../src/settings/settings-core';
import { editDraft, editorView, saveDraft, submitLabel, syncDraft, themeLocal } from '../../src/appearance/settings-appearance-editor';
import type { CustomTheme } from '../../src/appearance/settings-themes';
import { ipcFailure, networkUi, resetNetwork, runNetworkOp } from '../../src/connections/connections-network';
import { settingsCore } from '../../src/settings/settings-core-view';
import { DesktopServerExposureModePersistenceError, DesktopTailscaleServePersistenceError } from '../../src/connections/server-exposure';
import { primary } from '../../src/connections/local-primary';
import { resetPrimary } from '../support/local-primary-fixture';
import { parseLocalBackendStatus } from '../../src/connections/local-backend';
import { toasts } from '../../src/shell/toast';
import { T3Client as Client } from '../../src/client/client';

const NOW = Date.parse('2026-10-10T12:00:00.000Z');

// ── The base-branch picker ─────────────────────────────────────
const REFS = [{ name: 'trunk', current: true, isRemote: false }, ...Array.from({ length: 120 }, (_, index) => ({ name: `topic-${String(index + 1).padStart(3, '0')}`, isRemote: false })),
  { name: 'main', isDefault: true, isRemote: false }];
/** GitVcsDriverCore listRefs over the 122 refs: the query lowercased and matched anywhere, pages of `limit` by cursor. */
function listRefs(payload: Obj): Obj {
  const query = String(payload.query ?? '').toLowerCase(), cursor = Number(payload.cursor ?? 0), limit = Number(payload.limit ?? 100);
  const matches = REFS.filter(ref => !query || ref.name.toLowerCase().includes(query));
  const page = matches.slice(cursor, cursor + limit);
  return { refs: page, isRepo: true, hasPrimaryRemote: true, nextCursor: cursor + limit < matches.length ? cursor + limit : null, totalCount: matches.length };
}
function fakeClient(projects: Obj[] = [{ id: 'p1', title: 'many-refs', workspaceRoot: '/many' }]) {
  const reads: Obj[] = [];
  const client = {
    environmentId: 'env', threadId: '', projectId: 'p1', ready: true, writable: true, revision: 0, generation: 1, busy: false, presentation: {},
    config: { environment: { label: 'Laptop', capabilities: {} }, providers: [{ instanceId: 'codex', enabled: true, installed: true, status: 'ready', auth: { status: 'authenticated' }, models: [{ slug: 'gpt', name: 'GPT' }] }], settings: {} },
    local: { drafts: {} as Record<string, string> },
    shell: { ...initialShell(), projects, threads: [] },
    projectGroups() { return [{ key: 'g1', name: 'many-refs', members: this.shell.projects }]; },
    restAccess: () => ({
      read: async (method: string, payload: Obj) => { if (method === 'vcs.listRefs') { reads.push(payload); return listRefs(payload); } return {}; },
      request: async () => ({}), call: async () => ({ id: 'sub-1' }), ids: async (count: number) => Array.from({ length: count }, (_, index) => `id-${index}`),
      http: async () => ({ snapshotSequence: 1, projects: [], threads: [] }), write: async () => ({}),
    }),
  };
  return { client: client as unknown as T3Client & typeof client, reads };
}
const native = { available: true, watch: () => {}, later: async () => ({ ok: true, value: {} }) } as unknown as Native;
/** A native that counts the page's own wakes (T3LatinChords `wake`) and what it watches. */
function wakingNative() {
  const wakes: string[] = [], watched: string[] = [];
  const module = { available: true, watch: (topic: string) => { watched.push(topic); },
    later: async (request: Obj) => { if (request.op === 'wake') wakes.push(String(request.topic)); return { ok: true, generation: 0, value: request.op === 'installedFonts' ? { fonts: [] } : {} }; } } as unknown as Native;
  return { module, wakes, watched };
}
const KEY = 'new'; // a new task's dialog key, whatever its environment
const picker = (query: string, ref = '', project = 'p1') => `key=${encodeURIComponent(KEY)}&project=${project}&ref=${encodeURIComponent(ref)}&query=${encodeURIComponent(query)}`;

describe('the scheduled task\'s base-branch picker', () => {
  test('the first 100 of 122 refs with "Showing 100 of 122 refs"; a scroll toward the end loads the rest', async () => {
    const { client, reads } = fakeClient();
    await watchLive(client, native);
    const open = async (taskBase = '') => (await scheduledPage(client, native, 'env', '', 'task', '', true, NOW, '', '', '', taskBase)).branches[0]!;
    let group = await open();
    expect([group.refs.length, group.refs[0]!.value, group.refs.at(-1)!.value, group.status]).toEqual([100, 'trunk', 'topic-099', 'Showing 100 of 122 refs']);
    // The draft's base, main, is past the first page: selectedRefQuery finds it by name ("From origin/main").
    expect(reads).toEqual([{ cwd: '/many', limit: 100 }, { cwd: '/many', query: 'main', limit: 10 }]);
    expect(group.selected.map(ref => [ref.value, ref.remote])).toEqual([['main', false]]);
    expect(scheduledRefsWanted(client)).toBe(false);
    // RefPageScroll reports a scroll within 96pt of the list's end: the page is wanted, "Loading more refs..." first,
    // then the shell clock's next look reads it (cursor 100).
    noteRefScrollEnd(client, 'task-refs');
    expect(scheduledRefsWanted(client)).toBe(true);
    group = await open();
    expect([group.refs.length, group.status, reads.length]).toEqual([100, 'Loading more refs...', 2]);
    group = await open();
    expect([group.refs.length, group.refs.at(-1)!.value, group.status, reads.at(-1)]).toEqual([122, 'main', '', { cwd: '/many', limit: 100, cursor: 100 }]);
    expect(scheduledRefsWanted(client)).toBe(false);
    // Asked again (a wake, the minute tick): nothing is read.
    await open();
    expect(reads.length).toBe(3);
  });

  test('the search is the server\'s (sanitized, any case), past the first page; clearing it shows the first page kept', async () => {
    const { client, reads } = fakeClient();
    const { module, wakes, watched } = wakingNative();
    await watchLive(client, module);
    const open = async (taskBase = '') => (await scheduledPage(client, module, 'env', '', 'task', '', true, NOW, '', '', '', taskBase)).branches[0]!;
    await open();
    expect(wakes).toEqual([]);
    // A search not read yet: no refs and "Loading refs..." (usePaginatedBranches' first page pending), the page watches
    // its wake topic and wakes it, so it is asked again for the read.
    let group = await open(picker('TOPIC-12'));
    expect([group.refs.length, group.status, reads.length, wakes, watched]).toEqual([0, 'Loading refs...', 2, ['t3.notify'], ['t3.notify']]);
    expect(group.selected.map(ref => ref.value)).toEqual(['main']); // the base keeps its label (the lookup is kept)
    group = await open(picker('TOPIC-12'));
    expect([group.refs.map(ref => ref.value), group.status, reads.at(-1), wakes.length]).toEqual([['topic-120'], '', { cwd: '/many', limit: 100, query: 'TOPIC-12' }, 1]);
    await open(picker(' topic 11 '));
    group = await open(picker(' topic 11 '));
    expect(reads.at(-1)).toEqual({ cwd: '/many', limit: 100, query: 'topic-11' });
    expect(group.refs.map(ref => ref.value)).toEqual(['topic-110', 'topic-111', 'topic-112', 'topic-113', 'topic-114', 'topic-115', 'topic-116', 'topic-117', 'topic-118', 'topic-119']);
    // A search read before shows its answer at once.
    const read = reads.length;
    group = await open(picker('TOPIC-12'));
    expect([group.refs.map(ref => ref.value), reads.length - read, wakes.length]).toEqual([['topic-120'], 0, 2]);
    group = await open(picker(''));
    expect([group.refs.length, group.status, reads.length - read, wakes.length]).toEqual([100, 'Showing 100 of 122 refs', 0, 2]);
    // Another editor's picker is not this one's.
    expect(pickerOf(picker('x'), 'env:task-9')).toBeNull();
  });

  test('a list shown again starts from its first page; another list\'s scrolls do not page it', async () => {
    const { client, reads } = fakeClient([{ id: 'p1', title: 'many-refs', workspaceRoot: '/many' }, { id: 'p2', title: 'more-refs', workspaceRoot: '/more' }]);
    await watchLive(client, native);
    const open = async (taskBase = '') => (await scheduledPage(client, native, 'env', '', 'task', '', true, NOW, '', '', '', taskBase)).branches;
    const of = (groups: Awaited<ReturnType<typeof open>>, id: string) => groups.find(group => group.projectId === id)!;
    await open();
    expect(reads.filter(read => read.limit === 100).map(read => read.cwd)).toEqual(['/many', '/more']); // each project's first page
    // P1 scrolled to its end: its second page.
    noteRefScrollEnd(client, 'task-refs');
    await open();
    expect(of(await open(), 'p1').refs.length).toBe(122);
    // P2 chosen: its first page and "Showing 100 of 122 refs", no "Loading more refs...", no read of its cursor.
    const read = reads.length;
    let groups = await open(picker('', '', 'p2'));
    expect([of(groups, 'p2').refs.length, of(groups, 'p2').status]).toEqual([100, 'Showing 100 of 122 refs']);
    groups = await open(picker('', '', 'p2'));
    expect([of(groups, 'p2').refs.length, of(groups, 'p2').status, reads.length - read, scheduledRefsWanted(client)]).toEqual([100, 'Showing 100 of 122 refs', 0, false]);
    // P1 again: from its first page (usePaginatedBranches resets its cursors for another targetKey).
    groups = await open(picker('', '', 'p1'));
    expect([of(groups, 'p1').refs.length, of(groups, 'p1').status, reads.length - read]).toEqual([100, 'Showing 100 of 122 refs', 0]);
    // A search's results scrolled, then the search cleared: the list shows its first page, no next page read.
    await open(picker('topic', '', 'p1'));
    groups = await open(picker('topic', '', 'p1'));
    expect([of(groups, 'p1').refs.length, of(groups, 'p1').status]).toEqual([100, 'Showing 100 of 120 refs']);
    noteRefScrollEnd(client, 'task-refs');
    await open(picker('topic', '', 'p1'));
    expect(of(await open(picker('topic', '', 'p1')), 'p1').refs.length).toBe(120);
    const cleared = reads.length;
    groups = await open(picker('', '', 'p1'));
    groups = await open(picker('', '', 'p1'));
    expect([of(groups, 'p1').refs.length, of(groups, 'p1').status, reads.length - cleared]).toEqual([100, 'Showing 100 of 122 refs', 0]);
  });

  test('a failed page is the status line ("Failed to load refs." without a message) and "No refs found."', async () => {
    const { client } = fakeClient();
    client.restAccess = () => ({ read: async () => { throw new Error(''); } }) as never;
    await watchLive(client, native);
    const group = (await scheduledPage(client, native, 'env', '', 'task', '', true, NOW)).branches[0]!;
    expect([group.refs.length, group.error, group.status]).toEqual([0, 'Failed to load refs.', 'Failed to load refs.']);
  });
});

// ── A one-palette theme ──────────────────────────────────────────────
const DUSK: CustomTheme = { id: 'dusk', label: 'Dusk', appearance: 'dark', light: null, dark: { canvas: '#101820', accent: '#44cc88', text: '#f0f0f0' } };
const NOON: CustomTheme = { id: 'noon', label: 'Noon', appearance: 'light', light: { canvas: '#fff8e0', accent: '#cc6600' }, dark: null };
const prefs = (themeLight: string, themeDark: string, theme = 't3-code') => ({ ...decodeClientPrefs({}), theme, themeLight, themeDark });

describe('a theme with one palette', () => {
  test('the appearance it lacks is the default theme\'s, not its own other palette', () => {
    expect(themeRoles('dusk', 'light', [DUSK]).canvas).toBe(themeRoles('t3-code', 'light').canvas);
    expect(themeRoles('dusk', 'dark', [DUSK]).canvas).toBe('#101820');
  });

  test('as the whole theme (setTheme) it resolves every mode to its own appearance (resolveThemeAppearance)', () => {
    const whole = prefs('dusk', 'dusk', 'dusk');
    expect(['system', 'light', 'dark'].map(mode => effectiveMode(mode, whole, [DUSK]))).toEqual(['dark', 'dark', 'dark']);
    // On its half only (the library's Use), System keeps following macOS and the light half keeps its theme.
    const half = prefs('t3-code', 'dusk');
    expect(['system', 'light', 'dark'].map(mode => effectiveMode(mode, half, [DUSK]))).toEqual(['system', 'light', 'dark']);
    expect(effectiveMode('system', prefs('noon', 'noon', 'noon'), [NOON])).toBe('light');
    // The window and the palette draw in the resolved mode: Dusk's canvas, not light-dark().
    expect(palette(whole, [DUSK], 'system').canvas).toBe('#101820');
    const local = { deviceSettings: { ...DEVICE_DEFAULTS }, clientSettings: whole, customThemes: [DUSK] };
    expect(windowAppearanceMode({ local } as unknown as T3Client)).toBe('dark');
    expect(look({ local, diffState: undefined } as unknown as T3Client).mode).toBe('dark');
  });

  test('the whole app wears the editor\'s draft in the appearance being edited, the window too (applyThemeColorPreview)', async () => {
    // Dusk the whole theme, mode System on a light Mac: dark. Create theme opens on Dark; its Light draft turns the app light.
    const whole = prefs('dusk', 'dusk', 'dusk');
    const client = { local: { deviceSettings: { ...DEVICE_DEFAULTS, appearanceMode: 'system' }, clientSettings: whole, customThemes: [DUSK] }, diffState: undefined,
      ready: true, environmentId: 'env1', revision: 0, config: { environment: { environmentId: 'env1', label: 'Studio' }, providers: [], settings: {} }, projectGroups: () => [] } as unknown as T3Client;
    expect([look(client).mode, windowAppearanceMode(client)]).toEqual(['dark', 'dark']);
    const wakes: string[] = [];
    const module = { available: true, watch: () => {}, later: async (request: Obj) => { if (request.op === 'wake') wakes.push(String(request.topic)); return { ok: true, generation: 0, value: request.op === 'installedFonts' ? { fonts: [] } : {} }; } } as unknown as Native;
    const core = (kind: string) => settingsCore(client, module, '', '', '', '', 'appearance', '', true, kind, kind ? '#1' : '', 'embedded', false, look(client).mode);
    await core('');
    expect(wakes).toEqual([]);
    let page = await core('create');
    expect([page.editor.appearance, look(client).mode, windowAppearanceMode(client), page.palette.canvas, wakes]).toEqual(['dark', 'dark', 'dark', '#101820', []]);
    editDraft(client, 'appearance', 'light');
    page = await core('create');
    expect([look(client).mode, windowAppearanceMode(client), look(client).canvas, page.palette.canvas]).toEqual(['light', 'light', '#fcfcfc', '#fcfcfc']);
    // The editor opening or closing is no command: the page wakes the data source, whose read sends the window's mode.
    expect(wakes).toEqual(['t3.notify']);
    await core('');
    expect([look(client).mode, windowAppearanceMode(client), wakes]).toEqual(['dark', 'dark', ['t3.notify', 't3.notify']]);
    // In mode Dark, a Light draft is light all the same (the `dark` class follows the draft, not the mode).
    (client.local.deviceSettings as { appearanceMode: string }).appearanceMode = 'dark';
    (client.local as unknown as { clientSettings: unknown }).clientSettings = prefs('t3-code', 't3-code');
    await core('create');
    editDraft(client, 'appearance', 'light');
    expect([look(client).mode, windowAppearanceMode(client)]).toEqual(['light', 'light']);
  });

  test('the library\'s Use puts a one-palette theme on its own half (assignHalf); a two-palette one is the whole theme', () => {
    const local = { deviceSettings: { ...DEVICE_DEFAULTS }, clientSettings: prefs('t3-code', 't3-code'), customThemes: [DUSK, NOON], groupingMode: 'repository' };
    applyDeviceSetting(local, 'theme', 'dusk');
    expect([local.clientSettings.theme, local.clientSettings.themeLight, local.clientSettings.themeDark]).toEqual(['t3-code', 't3-code', 'dusk']);
    applyDeviceSetting(local, 'theme', 'noon');
    expect([local.clientSettings.theme, local.clientSettings.themeLight, local.clientSettings.themeDark]).toEqual(['t3-code', 'noon', 'dusk']);
    applyDeviceSetting(local, 'theme', 'grove');
    expect([local.clientSettings.theme, local.clientSettings.themeLight, local.clientSettings.themeDark]).toEqual(['grove', 'grove', 'grove']);
  });

  test('Create theme starts from what is on screen: Dusk on Dark as the whole theme; the default on Light beside a Dusk half', () => {
    const client = { local: { deviceSettings: { appearanceMode: 'system' }, clientSettings: decodeClientPrefs({}), customThemes: [DUSK] } } as unknown as T3Client;
    // The whole theme: the app resolves dark, so the editor opens on Dark with Dusk's palette; its Light is the default.
    let view = editorView(syncDraft(client, 'create', '#1', { theme: 'dusk', themeLight: 'dusk', themeDark: 'dusk' }, 'dark'), [DUSK]);
    expect([view.appearance, view.rows[0]!.value]).toEqual(['dark', '#101820']);
    editDraft(client, 'appearance', 'light');
    expect(editorView(syncDraft(client, 'create', '#1', { theme: 'dusk', themeLight: 'dusk', themeDark: 'dusk' }, 'dark'), [DUSK]).rows[0]!.value).toBe('#fcfcfc');
    // Dusk on the dark half only, a light Mac: Light, from the default (#fcfcfc), not from Dusk (#0a0a0a before).
    view = editorView(syncDraft(client, 'create', '#2', { theme: 't3-code', themeLight: 't3-code', themeDark: 'dusk' }, 'light'), [DUSK]);
    expect([view.appearance, view.rows[0]!.value, view.rows[1]!.value]).toEqual(['light', '#fcfcfc', '#1b4ed8']);
  });
});

// ── The theme editor's submit label ──────────────────────────────────
describe('the theme editor\'s submit', () => {
  test('Create theme; Add <appearance> palette for a name an installed theme has; Save changes; Merge into “<name>”', () => {
    const custom = [DUSK, NOON];
    expect(submitLabel({ name: '', editingId: '', appearance: 'light' }, custom)).toEqual({ label: 'Create theme', icon: 'paintbrush' });
    expect(submitLabel({ name: 'Aurora', editingId: '', appearance: 'dark' }, custom)).toEqual({ label: 'Create theme', icon: 'paintbrush' });
    expect(submitLabel({ name: ' dusk ', editingId: '', appearance: 'light' }, custom)).toEqual({ label: 'Add light palette', icon: 'plus' });
    expect(submitLabel({ name: 'Noon', editingId: 'noon', appearance: 'light' }, custom)).toEqual({ label: 'Save changes', icon: '' });
    expect(submitLabel({ name: 'Dusk', editingId: 'noon', appearance: 'light' }, custom)).toEqual({ label: 'Merge into “Dusk”', icon: '' });
  });

  test('the typed name reaches the draft as it is typed; a name taken on this appearance flips the draft to the free one', () => {
    const client = { local: { deviceSettings: { appearanceMode: 'system' }, clientSettings: decodeClientPrefs({}), customThemes: [DUSK, NOON] } } as unknown as T3Client;
    const draft = syncDraft(client, 'create', '#3', { theme: 't3-code', themeLight: 't3-code', themeDark: 't3-code' }, 'dark')!;
    const view = () => editorView(draft, [DUSK, NOON]);
    expect([view().appearance, view().saveLabel]).toEqual(['dark', 'Create theme']);
    themeLocal(client, 'name', `session-${draft.sessionId}|name`, 'Dus', 1);
    expect(view().saveLabel).toBe('Create theme');
    themeLocal(client, 'name', `session-${draft.sessionId}|name`, 'Dusk', 2);
    expect([view().appearance, view().saveLabel, view().saveIcon]).toEqual(['light', 'Add light palette', 'plus']);
    themeLocal(client, 'name', `session-${draft.sessionId}|name`, 'Du', 1); // an older keystroke changes nothing
    expect(draft.name).toBe('Dusk');
  });

  test('an edit renamed onto another theme folds its palettes into it and retires itself', () => {
    const client = { local: { deviceSettings: { appearanceMode: 'system' }, clientSettings: decodeClientPrefs({}), customThemes: [DUSK, NOON] } } as unknown as T3Client;
    syncDraft(client, 'edit', 'noon#4', { theme: 'noon', themeLight: 'noon', themeDark: 'noon' }, 'light');
    editDraft(client, 'name', 'Dusk');
    const { theme, context } = saveDraft(client);
    const themes = (client.local as unknown as { customThemes: CustomTheme[] }).customThemes;
    expect([themes.map(entry => entry.id), theme.id, theme.appearance, obj(theme.light).canvas, obj(theme.dark).canvas, context]).toEqual([['dusk'], 'dusk', 'dark', '#fff8e0', '#101820', { created: false, mergedAppearance: 'light' }]);
    // A palette both have cannot merge.
    (client.local as unknown as { customThemes: CustomTheme[] }).customThemes = [DUSK, { ...NOON, id: 'dusk-2', label: 'Dusk 2', appearance: 'dark', light: null, dark: NOON.light }];
    syncDraft(client, 'edit', 'dusk-2#5', { theme: 't3-code', themeLight: 't3-code', themeDark: 't3-code' }, 'dark');
    editDraft(client, 'name', 'Dusk');
    expect(() => saveDraft(client)).toThrow('“Dusk” already has a dark palette. Pick another name.');
  });
});

// ── Settings › Connections failures ──────────────────────────────────
describe('Network access and Tailscale HTTPS failures read as the reference\'s IPC rejection', () => {
  beforeEach(() => resetNetwork());
  afterEach(() => { resetNetwork(); resetPrimary(); });
  const ready = () => parseLocalBackendStatus({ state: 'ready', enabled: true, port: 16101, httpBaseUrl: 'http://127.0.0.1:16101', wsBaseUrl: 'ws://127.0.0.1:16101', bearerReady: true, environmentId: 'env-local', label: 'Lane Mac' });
  const LAN = { en0: [{ address: '192.168.1.20', family: 'IPv4', internal: false }] };
  /** desktop-settings.json's directory is read-only: T3DesktopSettings.swift's temporary file cannot be written. */
  const readOnly = { available: true, watch: () => {}, later: async (request: unknown) => {
    const op = obj(request).op;
    if (op === 'localNetworkFacts') return { ok: true, generation: 1, value: { interfaces: LAN, lanHostOverride: '', httpsEndpointUrls: [], tailscale: { read: true, magicDnsName: '', tailnetIpv4Addresses: [] }, probe: { url: '', read: true, reachable: false } } };
    if (op === 'desktopSettingsSet') return { ok: false, generation: 1, error: { kind: 'DesktopSettings', message: 'You don’t have permission to save the file.' } };
    return { ok: true, generation: 1, value: {} };
  } } as unknown as Native;

  test('the exact text of the reference\'s toast and row', async () => {
    primary.update(ready(), true);
    const client = new Client();
    await runNetworkOp(client, readOnly, 'network-access', '', 'on');
    const exposure = "Error invoking remote method 'desktop:set-server-exposure-mode': DesktopServerExposureModePersistenceError: Failed to persist desktop server exposure mode network-accessible.";
    expect([networkUi.exposureError, toasts(client).at(-1)?.title, toasts(client).at(-1)?.description]).toEqual([exposure, 'Could not update network access', exposure]);
    await runNetworkOp(client, readOnly, 'tailscale-serve', '', 'on:443');
    const tailscale = "Error invoking remote method 'desktop:set-tailscale-serve-enabled': DesktopTailscaleServePersistenceError: Failed to persist desktop Tailscale Serve settings (enabled: true, port: 443).";
    expect([networkUi.exposureError, toasts(client).at(-1)?.title, toasts(client).at(-1)?.description]).toEqual([tailscale, 'Could not set up Tailscale HTTPS', tailscale]);
    // The renderer-side check is the clone's own (the reference disables the button): no IPC prefix.
    await runNetworkOp(client, readOnly, 'tailscale-serve', '', 'on:70000');
    expect(networkUi.exposureError).toBe('Enter a port from 1 to 65535.');
  });

  test('the tagged errors carry their names into the message, as Electron\'s error.toString()', () => {
    expect(ipcFailure('desktop:set-server-exposure-mode', new DesktopServerExposureModePersistenceError('local-only', null)).message)
      .toBe("Error invoking remote method 'desktop:set-server-exposure-mode': DesktopServerExposureModePersistenceError: Failed to persist desktop server exposure mode local-only.");
    expect(String(new DesktopTailscaleServePersistenceError(false, null, null))).toBe('DesktopTailscaleServePersistenceError: Failed to persist desktop Tailscale Serve settings (enabled: false, port: unchanged).');
  });
});
