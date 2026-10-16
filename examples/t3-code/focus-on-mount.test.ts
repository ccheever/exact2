// Focus the reference moves itself, which the Mac host's `autofocus` (HTML's rule: no autofocus while another element
// holds the focus) would leave where it was; the main sidebar trigger as one node; the remove-image confirm's final
// focus; ⌘Z in the prompt; and the Snoozed shelf of a background environment. Reference: T3 Code 1e2ecbd975.
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { readdirSync } from 'node:fs';
import type { T3Client } from './client';
import { initialShell, type Obj } from './domain';
import { chordWinners, type DispatchContext } from './keyboard-dispatch';
import type { Native } from './protocol';
import { fleet, type FleetEntry } from './settings-b-fleet';
import { sidebarLocal } from './sidebar-commands';
import { setRuntimeClock } from './sidebar-state';
import { sidebarSnapshot } from './sidebar-view';

const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
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
    const window = await source('app-window.contract');
    const at = window.split('\n').findIndex(line => line.trim().startsWith('SidebarToggle('));
    // After the chat, the pages and the right sheets: above them in paint order and after them in Tab order, as the
    // reference's SidebarControl follows the page.
    expect(window.split('\n')[at - 1]!.trim().startsWith('RightSheets(')).toBe(true);
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
  test('Confirm focuses that element once the removal has landed', async () => {
    const app = await source('app.contract');
    expect(app).toContain('    confirmedFocus = accept and confirmOp == "remove-snapshot" ? confirmReturn : ""\n');
    expect(app).toContain('    if confirmedFocus != ""\n      focus(confirmedFocus)\n      confirmedFocus = ""\n    pendingModal = ""\n    commandPending = false\n');
  });
});

describe('⌘Z in the prompt is the prompt\'s (thread.undo: !terminalFocus && !editableFocus)', () => {
  const binding = { command: 'thread.undo', shortcut: { key: 'z', modKey: true },
    whenAst: { type: 'and', left: { type: 'not', node: { type: 'identifier', name: 'terminalFocus' } }, right: { type: 'not', node: { type: 'identifier', name: 'editableFocus' } } } };
  const context = (editable: boolean): DispatchContext => ({ composerFocus: editable, editableFocus: editable, turnRunning: false, modelPickerOpen: false,
    draftThreadRoute: false, modalOpen: false, settingsOpen: false, diffOpen: false });
  test('undoes with the focus outside a field, and not with the composer focused', () => {
    expect([...chordWinners([binding], context(false)).values()]).toEqual(['thread.undo']);
    expect([...chordWinners([binding], context(true)).values()]).toEqual([]);
  });
});

describe('the Snoozed shelf opens on a background environment\'s thread', () => {
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
