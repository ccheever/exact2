// Focus the reference moves itself, which the Mac host's `autofocus` (HTML's rule: no autofocus while another element
// holds the focus) would leave where it was; the main sidebar trigger as one node; the remove-image confirm's final
// focus; ⌘Z in the prompt; and the Snoozed shelf of a background environment. Reference: T3 Code 1e2ecbd975.
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { readdirSync } from 'node:fs';
import type { T3Client } from './client';
import { initialShell, type Obj } from './domain';
import { chordWinners, type DispatchContext } from './keyboard-dispatch';
import { keybindingDefaults, shortcutInput } from './keybinding-settings';
import { parseWhen } from './keybinding-view';
import { T3Client as Client } from './client';
import type { Files } from './protocol';
import type { Native } from './protocol';
import { fleet, type FleetEntry } from './settings-b-fleet';
import { sidebarLocal } from './sidebar-commands';
import { setRuntimeClock } from './sidebar-state';
import { sidebarSnapshot } from './sidebar-view';

const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
// A draft with two images on thread t, the disk and the native module answering every call.
function removalFixture() {
  const client = new Client();
  Object.assign(client, { generation: 1, environmentId: 'env', origin: 'http://fixture', projectId: 'p', threadId: 't', connection: 'connected', configLive: true, shellLive: true, threadLive: true });
  client.shell = { sequence: 1, projects: [{ id: 'p', workspaceRoot: '/p' }], threads: [{ id: 't', projectId: 'p' }] };
  client.local.snapshotDrafts['env:t'] = ['img-a', 'img-b'].map(id => ({ id, name: `${id}.png`, mimeType: 'image/png', sizeBytes: 68, source: {} }));
  const files: Files = { fs: { async mkdir() {}, async readFile() { return new ArrayBuffer(0); }, async atomicWriteFile() {} } };
  const native: Native = { available: true, watch() {}, async later() { return { ok: true, generation: client.generation, value: {} }; } };
  return { client, native, files };
}
const files = readdirSync(new URL('./', import.meta.url)).filter(name => name.endsWith('.contract')).sort();

// An in-page field the reference focuses as it mounts (React's autoFocus, a mount effect or a popup's initialFocus):
// each carries an `id` and, right before it, the DialogOpenFocus or FieldOpenSelect that focuses it at its first layout.
const MOUNT_FOCUS: [string, string, string][] = [
  ['sidebar-row.contract', '`sidebar-rename-${t.id}`', 'Sidebar.tsx: the row title input autoFocus, onFocus select()'],
  ['legacy-sidebar.contract', '`rename-${t.id}`', 'LegacySidebar.tsx handleRenameInputRef: focus(); select()'],
  ['legacy-sidebar.contract', '`thread-archive-confirm-${t.id}`', 'LegacySidebar.tsx handleStartArchiveConfirmation: focus()'],
  ['sidebar-overlays.contract', '"scope-search"', 'Sidebar.tsx: the project scope ComboboxPopup focuses its search'],
  ['citation-comment.contract', '"citation-comment-text"', 'AssistantCitationChip.tsx: initialFocus on the comment'],
  ['diff-comments.contract', '"diff-comment-draft-text"', 'DiffCommentAnnotation.tsx: focusOnMount'],
  ['settings-keybindings.contract', '"keybindings-filter"', 'KeybindingsSettings.tsx: the search autoFocus'],
  ['settings-licenses.contract', '"license-search"', 'OpenSourceLicenses.tsx: the search autoFocus'],
  ['providers.contract', '"custom-model-slug"', 'ProviderModelsSection.tsx: the custom model input autoFocus'],
  ['providers-upkeep.contract', '`custom-model-name-${editor.slug}`', 'CustomModelEditor.tsx: Display name autoFocus'],
  ['pages-pr-edit.contract', '`${testId}-text`', 'PullRequestMarkdownEditor.tsx: the textarea autoFocus'],
  ['pages-pr-edit.contract', '"pull-request-title-input"', 'PullRequestDetailPanel.tsx: the title input autoFocus'],
  ['pages-pr-threads.contract', '`pull-request-thread-reply-text-${card.id}`', 'PullRequestReviewAnnotation.tsx: Reply autoFocus'],
  ['pages-pr-threads.contract', '"pr-code-draft-text"', 'DiffCommentAnnotation.tsx: focusOnMount'],
];

describe('a field the reference focuses as it mounts takes the focus whatever held it', () => {
  for (const [file, id, reference] of MOUNT_FOCUS) {
    test(`${file} ${id} (${reference})`, async () => {
      const lines = (await source(file)).split('\n');
      const at = lines.findIndex(line => line.includes(`id=${id} `) && line.includes('autofocus'));
      expect(at).toBeGreaterThan(0);
      const before = lines.slice(Math.max(0, at - 2), at).map(line => line.trim());
      expect(before.some(line => line === `DialogOpenFocus(target=${id})` || line.startsWith(`FieldOpenSelect(target=${id}, `))).toBe(true);
    });
  }
  test('the flat sidebar\'s rename field selects its title as the legacy one does', async () => {
    expect(await source('sidebar-row.contract')).toContain('FieldOpenSelect(target=`sidebar-rename-${t.id}`, length=t.title.length)');
  });
});

describe('the main sidebar trigger is one node, open or closed (AppSidebarLayout SidebarControl)', () => {
  test('no surface draws its own; the window draws SidebarToggle once and Settings draws SettingsSidebarToggle once', async () => {
    let window = 0, settings = 0;
    for (const file of files) {
      const text = await source(file);
      expect([file, text.includes('aria-label="Toggle sidebar"')]).toEqual([file, false]);
      window += text.split('\n').filter(line => line.trim().startsWith('SidebarToggle(')).length;
      settings += text.split('\n').filter(line => line.trim().startsWith('SettingsSidebarToggle(')).length;
    }
    expect([window, settings]).toEqual([1, 1]);
  });
  test('it is a toggle named "Toggle main sidebar", its icon the only part that changes', async () => {
    const toggle = await source('sidebar-toggle.contract');
    expect(toggle).toContain('aria-label="Toggle main sidebar" aria-pressed=open aria-keyshortcuts=keySidebar testId="toggle-sidebar"');
    expect(toggle).toMatch(/\n {10}if open\n {12}Icon\(name="sidebar"[^\n]*\n {10}else\n {12}Icon\(name="panel-left"/);
    const main = (await source('app-main.contract')).split('\n');
    const at = main.findIndex(line => line.trim().startsWith('SidebarToggle('));
    const line = (name: string) => main.findIndex(text => text.trim().startsWith(name));
    // After the page (above it in paint order, after it in Tab order, as the reference's SidebarControl follows the
    // page), under the page's dialogs that cover the window (the reference portals its dialogs after SidebarControl).
    expect(line('PagesLanding(')).toBeLessThan(at);
    for (const dialog of ['ResetCreditDialog(', 'UsagePricesLayer(', 'UsageModelLayer(']) expect(line(dialog)).toBeGreaterThan(at);
    // Inert while any of them is open, the pull request's confirm and stack dialogs included.
    expect(main[at]).toContain('inert=(toggleInert or (pageCover and utilityPage == "usage" and (usageConfirmOpen or usage.detail.open or usage.prices.open)) or prDetail.actions.dialogOpen or prDetail.actions.stack.dialogOpen)');
    // The pull request's dialogs are the window's, drawn after the pages.
    const window = (await source('app-window.contract')).split('\n');
    const pages = window.findIndex(text => text.trim().startsWith('PagesCover('));
    expect(window.findIndex(text => text.trim().startsWith('WindowOverlays('))).toBeGreaterThan(pages);
    expect((await source('app-overlays.contract'))).toContain('    PrActionLayers(actions=prDetail.actions');
    // No z-index: the dialogs' own (40 to 80) and the right sheet's (10, 11) order them above it among its siblings.
    expect(toggle).not.toContain('z-index');
    const core = await source('settings-core.contract');
    expect(core).toContain('aria-label="Toggle main sidebar" aria-pressed=open aria-keyshortcuts=keySidebar testId="settings-toggle-sidebar"');
  });
});

describe('the remove-image confirm gives the focus back (Base UI finalFocus)', () => {
  test('a Remove that asks has an id, so Cancel returns to it, and names the element after it when it held the focus', async () => {
    const editor = await source('composer-editor.contract'), snapshot = await source('snapshot.contract');
    expect(editor).toContain('button id=`remove-snapshot-${id}` press=ask(removeFocused ? `ui:confirm:${after}` : "ui:confirm", id, "remove-snapshot"');
    expect(snapshot).toContain('button id=`remove-snapshot-${image.id}` press=ask(removeFocused ? `ui:confirm:${after}` : "ui:confirm", image.id, "remove-snapshot"');
  });
  test('Confirm hands that element to the removal, whose result focuses it once the tile is gone', async () => {
    const app = await source('app.contract');
    expect(app).toContain('      confirmValue = value == "remove-snapshot" ? op.slice(11) : ""');
    expect(app).toContain('        if result.message.startsWith("focus:")');
    const f = removalFixture();
    expect((await f.client.command('remove-snapshot', 'img-a', 'preview-image-img-b', 0, f.native, f.files)).message).toBe('focus:preview-image-img-b');
    expect(f.client.local.snapshotDrafts['env:t']!.map(image => image.id)).toEqual(['img-b']);
    // A Remove that did not hold the focus (a pointer press, or the plain Remove) moves nothing.
    expect((await f.client.command('remove-snapshot', 'img-b', '', 0, f.native, f.files)).message).toBe('');
  });
});

describe('⌘Z in the prompt is the prompt\'s (thread.undo: !terminalFocus && !editableFocus)', () => {
  const context = (editable: boolean): DispatchContext => ({ composerFocus: editable, editableFocus: editable, turnRunning: false, modelPickerOpen: false,
    draftThreadRoute: false, modalOpen: false, settingsOpen: false, diffOpen: false });
  test('the clone\'s default binding undoes with the focus outside a field, and not with the composer focused', () => {
    const entry = keybindingDefaults.find(binding => binding.command === 'thread.undo');
    expect(entry).toEqual({ key: 'mod+z', command: 'thread.undo', when: '!terminalFocus && !editableFocus' });
    // As the server resolves it: the key's tokens to a shortcut, the condition to its AST.
    const tokens = entry!.key.split('+'), shortcut: Obj = { key: tokens.at(-1)! };
    for (const mod of tokens.slice(0, -1)) shortcut[`${mod}Key`] = true;
    expect(shortcutInput(shortcut)).toBe(entry!.key);
    const binding = { command: entry!.command, shortcut, whenAst: parseWhen(entry!.when!) };
    expect([...chordWinners([binding], context(false)).values()]).toEqual(['thread.undo']);
    expect([...chordWinners([binding], context(true)).values()]).toEqual([]);
  });
});

// The view state only: the real-input failure (a click on the header leaving it collapsed) is not reproduced here.
describe('the Snoozed shelf state toggles over a background environment\'s thread', () => {
  beforeEach(() => setRuntimeClock(() => Date.now()));
  afterEach(() => { setRuntimeClock(() => Number.NaN); fleet.entries.clear(); });
  const NOW = Date.now(), iso = (offset: number) => new Date(NOW + offset).toISOString();
  const caps = { threadSettlement: true, threadSnooze: true, threadPinning: true };
  const row = (id: string, projectId: string, extra: Obj = {}): Obj => ({ id, projectId, title: `Thread ${id}`, status: 'idle', latestRunId: null,
    createdAt: iso(-60_000), updatedAt: iso(-60_000), archivedAt: null, settledOverride: null, settledAt: null,
    modelSelection: { instanceId: 'codex', model: 'm' }, lineage: { relationshipToParent: null }, ...extra });
  test('the shelf toggles and paints the snoozed row', async () => {
    const key = 'https://second.example\nenv-b';
    fleet.entries.set(key, { key, origin: 'https://second.example', environmentId: 'env-b', phase: 'connected', message: '', traceId: '', generation: 2, synchronized: 2,
      lastEvent: 0, subscriptions: {}, config: { environment: { environmentId: 'env-b', label: 'Lane Second', capabilities: caps } },
      shell: { ...initialShell(), projects: [{ id: 'pb', title: 'background-only', workspaceRoot: '/b' }], threads: [row('tb', 'pb', { snoozedUntil: iso(3_600_000), snoozedAt: iso(-1000) })] },
      scopes: [], error: '', requested: true } as FleetEntry);
    const client = { shell: { projects: [{ id: 'p1', title: 'shared-notes', workspaceRoot: '/a' }], threads: [row('a', 'p1')], sequence: 1 },
      config: { environment: { capabilities: caps }, providers: [], keybindings: [] }, environmentId: 'env-a', threadId: '', projectId: 'p1', query: '',
      connection: 'connected', writable: true, ready: true, presentation: {},
      local: { drafts: {}, selections: {}, snapshotDrafts: {}, snapshotReleases: [], composerControls: { contexts: {} }, deviceSettings: { timestampFormat: '24-hour' }, clientSettings: {}, sidebarWidth: 256 },
      projectGroups() { return []; } } as unknown as T3Client;
    const helpers = { projectIdentity: (name: string) => ({ projectMark: name.slice(0, 2), projectInk: 'ink', projectSurface: 'surface' }), providerBadge: () => ({ providerBadge: '', providerBadgeColor: '' }) };
    const shelf = () => { const view = sidebarSnapshot(client, NOW, helpers); return [view.sidebar.snoozedCount, view.sidebar.snoozedExpanded, view.threads.filter(t => t.section === 'snoozed').map(t => t.id)]; };
    expect(shelf()).toEqual([1, false, []]);
    await sidebarLocal(client, {} as Native, 'shelf', 'snoozed', '');
    expect(shelf()).toEqual([1, true, ['fleet:env-b:tb']]);
    await sidebarLocal(client, {} as Native, 'shelf', 'snoozed', '');
    expect(shelf()).toEqual([1, false, []]);
  });
});
