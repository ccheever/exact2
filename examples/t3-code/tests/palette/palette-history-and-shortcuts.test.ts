// T3 Code 74541bab17 (MIT; see LICENSE-T3): the command palette's history and focus (CommandPalette.logic.ts
// reduceCommandPaletteSearchState, OpenChildSearch/BackToCommand, CommandPaletteDialog finalFocus), its new-thread
// chords (handleKeyDown), and ⌘O on a shown file (FilePreviewPanel's OpenInPicker). The palette's state is the root's
// (app.contract), run here with Contract's action semantics.
import { afterEach, describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import { keybindingDefaults } from '../../src/settings/keybinding-settings';
import { parseWhen } from '../../src/settings/keybinding-view';
import { keyboardDispatch, type DispatchContext } from '../../src/shell/keyboard-dispatch';
import { newThreadInItems, rootActions } from '../../src/palette/palette';
import { contractComponent, type Env } from '../support/contract-action-fixture';
import { primaryAt, primaryOff, resetPrimary } from '../support/local-primary-fixture';

const root = contractComponent('app.contract', 'T3Code');
type Palette = { parent: string; popOnEmpty: boolean; back: boolean; count: number; rows: Obj[] };

/** The palette's root state, with the scope its actions read; calls to the root's own actions run in place. */
function palette(view: Partial<Palette> = {}, composer = 0) {
  const state: Env = { paletteOpen: true, paletteMode: 'command', palettePage: '', paletteQuery: '', paletteCursor: -1, paletteScroll: 0, paletteBusy: false,
    paletteTrailPages: [], paletteTrailQueries: [], paletteReturnQuery: '', paletteRestore: false, modelsOpen: false, optionsOpen: '', projectsOpen: false, modalError: '' };
  const focused: string[] = [];
  const scope: Env = {
    palette: { parent: '', popOnEmpty: false, back: false, count: 0, rows: [], ...view }, paletteHighlight: -1, paletteListHeight: 330,
    settingsOpen: false, pageCover: false, promptLinkPage: { show: false }, settingsThemeDialog: '', commandPending: false,
    frame: (id: string) => ({ width: id === 'composer' ? composer : 0 }), focus: (id: string) => focused.push(id), first: (items: unknown[]) => items[0] ?? null,
  };
  for (const name of ['paletteGo', 'paletteFocusBack']) scope[name] = (...args: unknown[]) => root.call(name, state, scope, args);
  const call = (name: string, ...args: unknown[]) => root.call(name, state, scope, args);
  return { state, scope, focused, call };
}

describe('going back in the palette keeps the search that led there (#15940)', () => {
  test('a submenu opened from a search returns to that search; Backspace and Back alike', () => {
    const p = palette();
    p.call('paletteEdit', 'theme');
    p.call('paletteRun', 'page', 'theme', '');
    expect(p.state).toMatchObject({ palettePage: 'theme', paletteQuery: '', paletteTrailPages: [''], paletteTrailQueries: ['theme'] });
    p.call('paletteEdit', 'dra');
    (p.scope.palette as Palette).back = true;
    p.call('paletteKey', 'Back');
    expect(p.state).toMatchObject({ palettePage: '', paletteQuery: 'theme', paletteTrailPages: [], paletteTrailQueries: [] });
    p.call('paletteRun', 'page', 'theme', '');
    p.call('paletteKey', 'Backspace');
    expect(p.state).toMatchObject({ palettePage: '', paletteQuery: 'theme' });
  });

  test('each level keeps its own query, and a page reached directly goes back to an empty search', () => {
    const p = palette({ back: true });
    p.call('paletteEdit', 'add');
    p.call('paletteRun', 'page', 'add-project', '');
    p.call('paletteEdit', 'mac');
    p.call('paletteRun', 'browse', 'add-project/env:e/browse', '~/');
    expect(p.state).toMatchObject({ palettePage: 'add-project/env:e/browse', paletteQuery: '~/', paletteTrailPages: ['', 'add-project'], paletteTrailQueries: ['add', 'mac'] });
    // The browse page pops itself when its path is cleared (popOnEmpty), to the page that led there.
    (p.scope.palette as Palette).popOnEmpty = true; (p.scope.palette as Palette).parent = 'add-project';
    p.call('paletteEdit', '');
    expect(p.state).toMatchObject({ palettePage: 'add-project', paletteQuery: 'mac', paletteTrailPages: [''] });
    (p.scope.palette as Palette).parent = '';
    p.call('paletteKey', 'Back');
    expect(p.state).toMatchObject({ palettePage: '', paletteQuery: 'add' });
    // Opened on a page (⌘⌥A's Change theme): Back has no search to restore.
    p.call('paletteShow', 'command', 'theme');
    p.call('paletteKey', 'Back');
    expect(p.state).toMatchObject({ palettePage: '', paletteQuery: '', paletteTrailPages: [] });
  });

  test('Escape from the file picker or content search opened from the list restores the list\'s search; ⌘P\'s does not', () => {
    const p = palette();
    p.call('paletteEdit', 'go to');
    p.call('paletteRun', 'mode', 'files', '');
    expect(p.state).toMatchObject({ paletteMode: 'files', paletteQuery: '', paletteReturnQuery: 'go to' });
    p.call('paletteEdit', 'app.ts');
    p.call('paletteClose');
    expect(p.state).toMatchObject({ paletteOpen: true, paletteMode: 'command', paletteQuery: 'go to', paletteReturnQuery: '' });
    expect(p.focused).toEqual(['palette-input', 'palette-input']); // the mode's field, then the list's
    p.call('paletteShow', 'files', '');
    p.call('paletteClose');
    expect(p.state).toMatchObject({ paletteMode: 'command', paletteQuery: '' });
  });
});

describe('palette focus returns where it was when no composer is shown (#18850)', () => {
  test('closing focuses a shown composer; over Settings or a page it hands the focus back to the window', () => {
    const shown = palette({}, 640);
    shown.call('paletteClose');
    expect(shown.focused).toEqual(['composer']);
    expect(shown.state.paletteRestore).toBe(false);
    const settings = palette({}, 640);
    settings.scope.settingsOpen = true;
    settings.call('paletteClose');
    expect(settings.focused).toEqual([]);
    expect(settings.state).toMatchObject({ paletteOpen: false, paletteRestore: true });
    // ⌘K again closes it the same way.
    const page = palette();
    page.scope.pageCover = true;
    page.call('paletteShow', 'command', '');
    expect(page.state).toMatchObject({ paletteOpen: false, paletteRestore: true });
    // A pull request page's picker goes back to its own control.
    const picker = palette();
    picker.state.palettePage = 'pr-link-thread';
    picker.call('paletteClose');
    expect([picker.focused, picker.state.paletteRestore]).toEqual([['pull-request-more'], false]);
  });

  test('PaletteFinalFocus returns the focus the open palette took, by the last view\'s owner, and forgets its frames', () => {
    const finalFocus = contractComponent('palette.contract', 'PaletteFinalFocus');
    const owners = ['command-palette', 'project-file-picker', 'project-content-search', 'link-pull-request-dialog', 'pull-request-thread-picker'];
    for (const restore of [true, false]) {
      const calls: unknown[][] = [];
      const scope: Env = { owners, restore, owner: 'command-palette', dialogOpened: (owner: string) => calls.push(['opened', owner]),
        dialogClosed: (owner: string) => calls.push(['closed', owner]), dialogForget: (list: string[]) => calls.push(['forget', list.length]) };
      const state: Env = { shown: false, heldOwner: '' };
      finalFocus.call('laidOut', state, scope, [2, 1]);
      finalFocus.call('laidOut', state, { ...scope, owner: 'project-file-picker' }, [3, 1]);
      finalFocus.call('laidOut', state, scope, [1, 1]);
      expect(calls).toEqual([['opened', 'command-palette'], restore ? ['closed', 'project-file-picker'] : ['forget', 5]]);
      expect(state.shown).toBe(false);
    }
  });
});

/** The server's resolved form of a default rule. */
function resolved(rule: { key: string; command: string; when?: string }) {
  const tokens = rule.key.split('+');
  const key = tokens.pop() || '+';
  return { command: rule.command, shortcut: { key, modKey: tokens.includes('mod'), shiftKey: tokens.includes('shift'), altKey: tokens.includes('alt'), ctrlKey: tokens.includes('ctrl'), metaKey: false },
    ...(rule.when ? { whenAst: parseWhen(rule.when) } : {}) };
}
function client(): T3Client {
  const c = new T3Client();
  Object.assign(c, { connection: 'connected', configLive: true, shellLive: true, generation: 1, environmentId: 'env1', origin: 'http://127.0.0.1:16891' });
  c.config = { environment: { label: 'Local', capabilities: {} }, keybindings: keybindingDefaults.map(resolved), settings: {}, scratchWorkspaceRoot: '/scratch', availableEditors: ['vscode'] };
  c.shell = { sequence: 1, projects: [{ id: 'p1', title: 'Widgets', workspaceRoot: '/repo' }, { id: 'p2', title: 'Gadgets', workspaceRoot: '/gadgets' }],
    threads: [{ id: 't1', projectId: 'p1', title: 'One', updatedAt: '2026-10-03T01:00:00.000Z' }] };
  c.projectId = 'p1'; c.threadId = 't1';
  return c;
}

describe('new-thread chords run the palette row that lists them (#19055)', () => {
  test('the rows carry the field\'s chords: ⇧⌘N on New thread in, ⌥⌘N on the no-project rows, ⌘1… on the project picks', () => {
    const c = client();
    const jump = (key: string, items = rootActions(c)) => items.find(item => item.row.key === key)?.row.jump;
    expect(jump('new-thread')).toBe('Meta+Shift+n');
    expect(jump('new-thread-without-project')).toBe('Meta+Alt+n');
    const picks = newThreadInItems(c);
    expect(jump('new-thread-in:no-project', picks)).toBe('Meta+Alt+n');
    expect(jump('new-thread-in:p1', picks)).toBe('Meta+1');
  });

  test('the field runs the displayed row by its chord, as the key arrives (Shift\'s capital, Option\'s character)', () => {
    const view = contractComponent('palette.contract', 'CommandPaletteBody');
    const rows = [{ index: 0, op: 'new-thread', arg: 'p1', arg2: '', jump: 'Meta+Shift+n' }, { index: 1, op: 'flow', arg: 'scratch', arg2: '', jump: 'Meta+Alt+n' }];
    const press = (k: string, code: string, mods: string) => {
      const calls: unknown[][] = [];
      const scope: Env = { view: { rows, jumpKeys: ['Meta+1'], accessoryKey: '' }, busy: false, first: (items: unknown[]) => items[0] ?? null,
        run: (...args: unknown[]) => calls.push(['run', ...args]), onKey: (key: string) => calls.push(['key', key]), preventDefault() {}, stopPropagation() {} };
      const event = { key: k, code, metaKey: mods.includes('meta'), ctrlKey: false, altKey: mods.includes('alt'), shiftKey: mods.includes('shift'), repeat: false };
      view.call('inputKey', {}, scope, [k, event]);
      return calls;
    };
    expect(press('N', 'KeyN', 'meta shift')).toEqual([['run', 'new-thread', 'p1', '']]);
    expect(press('˜', 'KeyN', 'meta alt')).toEqual([['run', 'flow', 'scratch', '']]);
    expect(press('1', 'Digit1', 'meta')).toEqual([]); // a jump chord with no row is taken and runs nothing
    expect(press('n', 'KeyN', 'meta')).toEqual([['key', 'n']]); // ⌘N lists no row: the root's keys
  });
});

describe('⌘O opens the shown file once, else the workspace (#18983)', () => {
  afterEach(() => resetPrimary());
  const context: DispatchContext = { composerFocus: false, editableFocus: false, turnRunning: false, modelPickerOpen: false, draftThreadRoute: false, modalOpen: false, settingsOpen: false, diffOpen: false };
  const opens = (extra: Partial<DispatchContext>) => keyboardDispatch(client(), [], '', '', { ...context, ...extra }).filter(row => row.command === 'editor.openFavorite').map(row => [row.kind, row.target, row.extra]);
  test('one button: the file surface\'s path while its picker can open it', () => {
    primaryAt('http://127.0.0.1:16891', 'env1');
    expect(opens({})).toEqual([['flow', 'open-favorite', '/repo']]);
    expect(opens({ fileSurface: true, fileOpenPath: '/repo/README.md' })).toEqual([['command', 'chatlocal:surface-files-open-editor', '/repo/README.md']]);
    // The file shown, its picker unable to open it: the window's fallback, on this machine only.
    expect(opens({ fileSurface: true, fileOpenPath: '' })).toEqual([['flow', 'open-favorite', '/repo']]);
  });
  test('a remote file surface without a picker opens nothing', () => {
    // Not this machine (no primary at this origin): the bearer route has no local editor fallback.
    expect(opens({ fileSurface: true, fileOpenPath: '' })).toEqual([]);
  });
});

describe('custom shortcuts work with the Local environment off (#15825)', () => {
  afterEach(() => resetPrimary());
  test('the dispatch reads the focused environment\'s keybindings, with no primary to defer to', () => {
    primaryOff();
    const c = client();
    c.config = { ...c.config, keybindings: [...(c.config.keybindings as Obj[]), resolved({ key: 'mod+shift+y', command: 'chat.newLocal', when: '!terminalFocus' })] };
    const context: DispatchContext = { composerFocus: false, editableFocus: false, turnRunning: false, modelPickerOpen: false, draftThreadRoute: false, modalOpen: false, settingsOpen: false, diffOpen: false };
    const chords = keyboardDispatch(c, [], '', '', context).filter(row => row.command === 'chat.newLocal').map(row => row.chord);
    expect(chords).toContain('Meta+Shift+y');
  });
});
