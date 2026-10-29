import { contractSource } from '../support/contract-action-fixture';
import { observeComposer } from '../support/composer-field-fixture';
// Remove-image focus return, prompt undo and background-environment Snoozed state.
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import { initialShell, type Obj } from '../../src/client/domain';
import { chordWinners, type DispatchContext } from '../../src/shell/keyboard-dispatch';
import { keybindingDefaults, shortcutInput } from '../../src/settings/keybinding-settings';
import { parseWhen } from '../../src/settings/keybinding-view';
import { T3Client as Client } from '../../src/client/client';
import type { Files } from '../../src/client/protocol';
import type { Native } from '../../src/client/protocol';
import { fleet, type FleetEntry } from '../../src/connections/settings-b-fleet';
import { sidebarLocal } from '../../src/sidebar/sidebar-commands';
import { setRuntimeClock } from '../../src/sidebar/sidebar-state';
import { sidebarSnapshot } from '../../src/sidebar/sidebar-view';

const source = (file: string) => contractSource(file);
// A draft with two images on thread t, the disk and the native module answering every call.
function removalFixture() {
  const client = new Client();
  Object.assign(client, { generation: 1, environmentId: 'env', origin: 'http://fixture', projectId: 'p', threadId: 't', connection: 'connected', configLive: true, shellLive: true, threadLive: true });
  client.shell = { sequence: 1, projects: [{ id: 'p', workspaceRoot: '/p' }], threads: [{ id: 't', projectId: 'p' }] };
  client.local.snapshotDrafts['env:t'] = ['img-a', 'img-b'].map(id => ({ id, name: `${id}.png`, mimeType: 'image/png', sizeBytes: 68, source: {} }));
  const files: Files = { fs: { async mkdir() {}, async readFile() { return new ArrayBuffer(0); }, async atomicWriteFile() {} } };
  const native: Native = { available: true, watch() {}, async later() { return { ok: true, generation: client.generation, value: {} }; } };
  observeComposer(client, '');
  return { client, native, files };
}

describe('the remove-image confirm gives the focus back (Base UI finalFocus)', () => {
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
