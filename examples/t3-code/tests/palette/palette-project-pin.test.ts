// Pinned projects in the command palette (T3 Code 2359643cff, MIT, see LICENSE-T3: CommandPalette.tsx,
// CommandPalette.logic.ts and its test, CommandPaletteResults.tsx, ProjectPinButton.tsx, useOrderWhileOpen.ts,
// chatRouteCommands.ts chat.new and useHandleNewThread.ts).
import { afterEach, describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { obj, type Obj } from '../../src/client/domain';
import type { Files, Native } from '../../src/client/protocol';
import { fleet } from '../../src/connections/settings-b-fleet';
import { newThreadInItems, newThreadNoProjectIndex, projectSearchItems, resetPaletteOrder, rootActions } from '../../src/palette/palette';
import { paletteCommand } from '../../src/palette/palette-commands';
import { paletteView } from '../../src/palette/palette-view';
import { sidebarCommand } from '../../src/sidebar/sidebar-commands';
import { keybindingDefaults } from '../../src/settings/keybinding-settings';
import { parseWhen } from '../../src/settings/keybinding-view';
import { toasts } from '../../src/shell/toast';
import { contractComponent, type Env } from '../support/contract-action-fixture';

afterEach(() => fleet.entries.clear());
const at = '2026-11-01T00:00:00.000Z';
const resolved = (rule: { key: string; command: string; when?: string }) => {
  const tokens = rule.key.split('+'), key = tokens.pop() || '+';
  return { command: rule.command, shortcut: { key, modKey: tokens.includes('mod'), shiftKey: tokens.includes('shift'), altKey: tokens.includes('alt'), ctrlKey: tokens.includes('ctrl'), metaKey: false },
    ...(rule.when ? { whenAst: parseWhen(rule.when) } : {}) };
};

function rig(projects: Obj[] = [{ id: 'alpha', title: 'alpha', workspaceRoot: '/alpha' }, { id: 'beta', title: 'beta', workspaceRoot: '/beta', pinnedAt: at }], fail = '') {
  const writes: Obj[] = [];
  const client = new T3Client();
  Object.assign(client, { connection: 'connected', environmentId: 'env-a', origin: 'http://127.0.0.1:16618', loaded: true, configLive: true, shellLive: true, generation: 1, presentation: {} });
  client.config = { environment: { label: 'This Mac', capabilities: { projectOrganization: true } }, settings: {}, providers: [], keybindings: keybindingDefaults.map(resolved) };
  client.scopes = ['orchestration:read', 'orchestration:operate'];
  client.shell = { projects: projects.map(project => ({ createdAt: at, updatedAt: at, ...project })), threads: [], sequence: 1 };
  client.projectId = 'alpha';
  Object.assign(client, { restAccess: () => ({ ids: async (count: number) => Array.from({ length: count }, (_, index) => `c${index}`),
    request: async (method: string, payload: Obj) => { if (fail) throw new Error(fail); writes.push({ method, ...payload }); return {}; } }) });
  const native = { available: true, watch() {}, later: async (request: unknown) => ({ ok: true, generation: Number(obj(request).generation) || 1, value: {} }) } as unknown as Native;
  return { client, native, writes };
}
const pin = (client: T3Client, title: string) => client.shell.projects.find(project => project.title === title)!.pinnedAt = at;
const titles = (items: { row: { title: string } }[]) => items.map(item => item.row.title);

describe('newThreadNoProjectIndex', () => {
  const entry = (pinned = false, isPreferred = false, isScratch = false) => ({ pinned, isPreferred, isScratch });
  test('goes after the pinned projects and the first unpinned one', () => {
    expect(newThreadNoProjectIndex([entry(true), entry(true), entry(false, true), entry()])).toBe(3);
    expect(newThreadNoProjectIndex([entry(false, true), entry(), entry(false, false, true)])).toBe(1);
  });
  test('keeps a pinned or current scratch project in its own place', () => {
    expect(newThreadNoProjectIndex([entry(false, true, true), entry()])).toBe(0);
    expect(newThreadNoProjectIndex([entry(true), entry(true, false, true), entry()])).toBe(1);
  });
});

describe('project rows carry ProjectPinButton', () => {
  test('pinned projects first, each row with its pin and the shortcut in its title', () => {
    const { client } = rig();
    expect(projectSearchItems(client).map(item => [item.row.title, item.row.pin, item.row.pinTitle])).toEqual([['beta', 'pinned', 'Unpin beta (⇧⌘P)'], ['alpha', 'unpinned', 'Pin alpha (⇧⌘P)']]);
    expect(newThreadInItems(client).filter(item => item.row.pin !== '').map(item => [item.row.title, item.row.pin])).toEqual([['beta', 'pinned'], ['alpha', 'unpinned']]);
  });
  test('no pin where the server drops pins', () => {
    const { client } = rig();
    client.config = { ...client.config, environment: { label: 'Old', capabilities: {} } };
    expect(projectSearchItems(client).some(item => item.row.pin !== '')).toBe(false);
  });
  test('rows keep their order while the palette is open; reopened, pinned projects come first', async () => {
    const { client, native } = rig();
    expect(titles(projectSearchItems(client))).toEqual(['beta', 'alpha']);
    expect(titles(newThreadInItems(client))).toEqual(['beta', 'alpha']);
    pin(client, 'alpha');
    client.shell.projects.find(project => project.title === 'beta')!.pinnedAt = null;
    expect(titles(projectSearchItems(client))).toEqual(['beta', 'alpha']);
    // The jump numbers follow the rows as shown.
    expect(newThreadInItems(client).filter(item => item.row.pin !== '').map(item => [item.row.title, item.row.shortcut])).toEqual([['beta', '⌘1'], ['alpha', '⌘2']]);
    await paletteView(client, native, [false]);
    expect(titles(projectSearchItems(client))).toEqual(['alpha', 'beta']);
    resetPaletteOrder(client);
  });
  test('with every project archived and no No project home, New thread in offers only Add project', () => {
    const { client } = rig([{ id: 'alpha', title: 'alpha', workspaceRoot: '/alpha', archivedAt: at }]);
    expect(newThreadInItems(client).map(item => item.row.key)).toEqual(['add-project']);
  });
});

describe('pinning from the palette', () => {
  test('a row\'s pin writes project.update and keeps the palette open; the Pin project action closes it', async () => {
    const { client, native, writes } = rig();
    const alpha = projectSearchItems(client).find(item => item.row.title === 'alpha')!.row.pinKey;
    expect(await paletteCommand(client, native, {} as Files, 'project-pin', alpha, '')).toMatchObject({ ok: true, close: false });
    expect(writes).toEqual([{ method: 'projects.mutate', type: 'project.update', commandId: 'c0', projectId: 'alpha', pinned: true }]);
    const action = rootActions(client).find(item => item.row.key === 'project-pin')!;
    expect([action.row.title, action.row.description, action.row.icon, action.row.arg]).toEqual(['Pin project', 'alpha', 'pin', 'project-pin-action']);
    expect(await paletteCommand(client, native, {} as Files, action.row.arg, action.row.arg2, '')).toMatchObject({ ok: true, close: true });
    pin(client, 'alpha');
    expect(rootActions(client).find(item => item.row.key === 'project-pin')!.row.title).toBe('Unpin project');
  });
  test('the field pins the highlighted row by the pin shortcut; a row without a pin leaves the key alone', () => {
    const view = contractComponent('palette.contract', 'CommandPaletteBody');
    const press = (rows: Obj[]) => {
      const calls: unknown[][] = [];
      const scope: Env = { view: { rows, jumpKeys: [], accessoryKey: '', pinKeys: 'Meta+Shift+p', autoHighlight: true, count: rows.length }, highlight: 0, busy: false,
        first: (items: unknown[]) => items[0] ?? null, run: (...args: unknown[]) => calls.push(['run', ...args]), onKey: (key: string) => calls.push(['key', key]), preventDefault() {}, stopPropagation() {} };
      view.call('inputKey', {}, scope, ['P', { key: 'P', code: 'KeyP', metaKey: true, ctrlKey: false, altKey: false, shiftKey: true, repeat: false }]);
      return calls;
    };
    expect(press([{ index: 0, op: 'open-project', arg: 'alpha', arg2: '', jump: '', pin: 'unpinned', pinKey: 'group-alpha' }])).toEqual([['run', 'flow', 'project-pin', 'group-alpha']]);
    expect(press([{ index: 0, op: 'settings', arg: 'general', arg2: '', jump: '', pin: '', pinKey: '' }])).toEqual([['key', 'P']]);
  });
  test('the field reads thread.pin\'s chord', async () => {
    const { client, native } = rig();
    expect((await paletteView(client, native, [true, 'command', '', ''])).pinKeys).toBe('Meta+Shift+p');
    resetPaletteOrder(client);
  });
});

describe('archived projects', () => {
  test('adding an archived project\'s folder again unarchives it and opens it; a refusal is a toast', async () => {
    for (const fail of ['', 'Server refused']) {
      const { client, native, writes } = rig([{ id: 'alpha', title: 'alpha', workspaceRoot: '/alpha', archivedAt: at }], fail);
      const result = await paletteCommand(client, native, {} as Files, 'add-path', '/alpha', '');
      await Promise.resolve(); await new Promise(resolve => setTimeout(resolve, 0));
      expect(result).toMatchObject({ ok: true, close: true, project: 'alpha' });
      expect(writes).toEqual(fail ? [] : [{ method: 'projects.mutate', type: 'project.update', commandId: 'c0', projectId: 'alpha', archived: false }]);
      expect(toasts(client).map(toast => toast.title)).toEqual(fail ? ['Failed to unarchive project'] : []);
    }
  });
  test('New thread opens the picker when the only project is archived, and when no project can start one', async () => {
    const archived = rig([{ id: 'alpha', title: 'alpha', workspaceRoot: '/alpha', archivedAt: at }]);
    expect(await sidebarCommand(archived.client, archived.native, {} as Files, 'new-thread-click', '', '')).toBe('sidebar:palette-new-thread');
    const none = rig([]);
    none.client.projectId = '';
    expect(await sidebarCommand(none.client, none.native, {} as Files, 'new-thread-click', '', 'shift')).toBe('sidebar:palette-new-thread');
  });
});
