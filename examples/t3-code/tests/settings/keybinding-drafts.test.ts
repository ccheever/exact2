// T3 Code 74541bab17 (MIT; see LICENSE-T3): Settings › Keybindings keeps one editor per row (#11109): a row's id is
// its command, shortcut and condition, so saving one binding leaves the others' unsaved edits in place. The rows'
// drafts live in the root (app.contract keybindingDrafts), run here with Contract's action semantics.
import { describe, expect, test } from 'bun:test';
import { keptWhen, keybindingSettings } from '../../src/settings/keybinding-settings';
import { buildRows, conflictLabels, parseWhen, rowId, shortcutConflictKey } from '../../src/settings/keybinding-view';
import { contractComponent, type Env } from '../support/contract-action-fixture';

const rule = (command: string, key: string, when?: string) => {
  const tokens = key.split('+'), last = tokens.pop()!;
  return { command, shortcut: { key: last, modKey: tokens.includes('mod'), shiftKey: tokens.includes('shift'), altKey: false, ctrlKey: false, metaKey: false }, ...(when ? { whenAst: parseWhen(when) } : {}) };
};

describe('buildKeybindingRows: stable row ids', () => {
  test('sibling rows keep their ids when a saved binding moves to the end', () => {
    const bindings = [rule('chat.new', 'mod+n'), rule('terminal.toggle', 'mod+j'), rule('terminal.toggle', 'mod+k', 'terminalFocus')];
    const before = buildRows(bindings, ''), after = buildRows([...bindings.slice(1), rule('chat.new', 'mod+shift+n')], '');
    const ids = (rows: typeof before, command: string) => rows.filter(row => row.command === command).map(row => row.id);
    expect(ids(after, 'terminal.toggle')).toEqual(ids(before, 'terminal.toggle'));
    expect(ids(after, 'chat.new')).not.toEqual(ids(before, 'chat.new'));
  });
  test('a resolved duplicate is one row; commands, shortcuts and conditions stay apart', () => {
    const bindings = [rule('terminal.toggle', 'mod+escape', 'terminalFocus'), rule('terminal.toggle', 'mod+escape', '(terminalFocus)'), rule('terminal.toggle', 'mod+escape', '!terminalFocus'),
      rule('terminal.toggle', 'mod+j', 'terminalFocus'), rule('chat.new', 'mod+escape', 'terminalFocus')];
    const rows = buildRows(bindings, '');
    expect(rows.map(({ command, key, condition }) => [command, key, condition])).toEqual([
      ['chat.new', 'mod+esc', 'terminalFocus'], ['terminal.toggle', 'mod+esc', 'terminalFocus'], ['terminal.toggle', 'mod+esc', '!terminalFocus'], ['terminal.toggle', 'mod+j', 'terminalFocus']]);
    expect(new Set(rows.map(row => row.id)).size).toBe(4);
    expect(rows.find(row => row.command === 'chat.new')?.conflict).toContain('Terminal: Toggle');
    expect(buildRows(bindings.slice(1), '').map(row => row.id)).toEqual(rows.map(row => row.id));
  });
  test('a held draft reads its own conflict and validity', async () => {
    const terminal = rowId('terminal.toggle', 'mod+j', '');
    const page = await keybindingSettings({ connection: 'disconnected' } as never, null, '', '', true, 'new', '', '', '',
      [{ row: terminal, command: 'terminal.toggle', key: 'mod+b', when: '' }, { row: 'new', command: 'chat.new', key: 'mod+n', when: '' }, { row: 'x', command: 'chat.new', key: 'mod+u', when: '((' }]);
    expect(page.held.map(entry => [entry.row, entry.conflict !== '', entry.valid])).toEqual([[terminal, true, true], ['x', true, false]]);
    expect(page.held[0]!.conflict).toContain('Sidebar');
  });
});

describe('one draft per row (app.contract)', () => {
  const root = contractComponent('app.contract', 'T3Code');
  const draft = (fields: Env) => fields;
  function settings() {
    const state: Env = { keybindingOpen: false, keybindingPrevious: '', keybindingCommand: '', keybindingKey: '', keybindingWhen: '', keybindingRecording: false,
      keybindingDrafts: [], keybindingSaving: '', modalError: '', commandPending: false, pendingModal: '', restEditor: '', settingsRefresh: 0, noticeExit: '',
      routeKeyboard: '', renameAfterOp: '', lineageStopping: false };
    const sent: unknown[][] = [];
    const scope: Env = { bindings: { ids: ['a', 'b'] }, KeybindingDraft: draft, settingsEnvironmentId: 'env', settingsProjectId: '', command: (...args: unknown[]) => sent.push(args), routeKeyboardComplete: () => '', routeKeyboardOwned: false };
    for (const name of ['editKeybinding', 'restRaw']) scope[name] = (...args: unknown[]) => root.call(name, state, scope, args);
    const call = (name: string, ...args: unknown[]) => root.call(name, state, scope, args);
    return { state, scope, sent, call };
  }

  test('editing another row holds this row\'s draft; going back takes it up again', () => {
    const s = settings();
    s.call('recordKeybinding', 'a', 'chat.new', 'mod+n', '');
    s.call('captureKeybinding', 'mod+n', 'mod+shift+y');
    s.call('editKeybinding', 'b', 'terminal.toggle', 'mod+j', '');
    expect(s.state).toMatchObject({ keybindingPrevious: 'b', keybindingKey: 'mod+j', keybindingDrafts: [{ row: 'a', command: 'chat.new', key: 'mod+shift+y', when: '' }] });
    // The row passes its held values back as it is edited again.
    s.call('recordKeybinding', 'a', 'chat.new', 'mod+shift+y', '');
    expect(s.state).toMatchObject({ keybindingPrevious: 'a', keybindingKey: 'mod+shift+y', keybindingRecording: true, keybindingDrafts: [{ row: 'b', command: 'terminal.toggle', key: 'mod+j', when: '' }] });
  });

  test('saving one row ends only its draft; another settings write leaves them all', () => {
    const s = settings();
    s.call('newKeybinding');
    s.call('chooseKeybindingCommand', 'chat.new');
    s.call('editKeybinding', 'a', 'thread.pin', 'mod+shift+p', '');
    s.call('captureKeybinding', 'mod+shift+p', 'mod+alt+p');
    s.call('editKeybinding', 'b', 'terminal.toggle', 'mod+j', '');
    s.call('captureKeybinding', 'mod+j', 'mod+shift+j');
    expect(s.state.keybindingDrafts).toEqual([{ row: 'new', command: 'chat.new', key: '', when: '' }, { row: 'a', command: 'thread.pin', key: 'mod+alt+p', when: '' }]);
    // Save the held row a, while b is the one being edited.
    s.call('keybindingRest', 'a', 'action=save&previous=a&command=thread.pin&key=mod%2Balt%2Bp&when=');
    expect(s.state).toMatchObject({ keybindingSaving: 'a', pendingModal: 'rest', commandPending: true });
    expect(s.sent).toEqual([['rest:keybinding', 'env:', 'action=save&previous=a&command=thread.pin&key=mod%2Balt%2Bp&when=', 0]]);
    root.call('commandCompleted', s.state, { ...s.scope, changed: { message: '', relaunch: false, routeReorder: 0 } }, []);
    expect(s.state).toMatchObject({ keybindingOpen: true, keybindingPrevious: 'b', keybindingKey: 'mod+shift+j', keybindingDrafts: [{ row: 'new', command: 'chat.new', key: '', when: '' }] });
    // The New keybinding row's save closes it and keeps b.
    s.call('keybindingRest', 'new', 'action=save&command=chat.new&key=mod%2Bu&when=');
    root.call('commandCompleted', s.state, { ...s.scope, changed: { message: '', relaunch: false, routeReorder: 0 } }, []);
    expect(s.state).toMatchObject({ keybindingOpen: false, keybindingPrevious: 'b', keybindingDrafts: [] });
    // Opening keybindings.json (another settings write) leaves the edited row alone.
    s.call('restRaw', 'rest:keybinding-open', 'env:', '');
    root.call('commandCompleted', s.state, { ...s.scope, changed: { message: '', relaunch: false, routeReorder: 0 } }, []);
    expect(s.state).toMatchObject({ keybindingSaving: '', keybindingPrevious: 'b', keybindingKey: 'mod+shift+j' });
  });

  test('a held draft whose row left the bindings is dropped', () => {
    const s = settings();
    s.state.keybindingDrafts = [{ row: 'gone', command: 'chat.new', key: 'mod+u', when: '' }, { row: 'a', command: 'thread.pin', key: 'mod+alt+p', when: '' }];
    s.call('editKeybinding', 'b', 'terminal.toggle', 'mod+j', '');
    expect(s.state.keybindingDrafts).toEqual([{ row: 'a', command: 'thread.pin', key: 'mod+alt+p', when: '' }]);
    s.state.keybindingDrafts = [{ row: 'gone', command: 'chat.new', key: 'mod+u', when: '' }, { row: 'new', command: 'chat.new', key: '', when: '' }];
    s.call('keybindingRest', 'b', 'action=save&previous=b&command=terminal.toggle&key=mod%2Bk&when=');
    root.call('commandCompleted', s.state, { ...s.scope, changed: { message: '', relaunch: false, routeReorder: 0 } }, []);
    expect(s.state.keybindingDrafts).toEqual([{ row: 'new', command: 'chat.new', key: '', when: '' }]);
  });
});

// KeybindingsSettings at 2359643cff: conflicts compare the chord each shortcut resolves to, and closing the When
// builder over an unparsed expression keeps the last valid one, so Save is enabled again.
describe('resolved-chord conflicts and the When builder on close', () => {
  test('mod+k collides with meta+k on a Mac and with ctrl+k elsewhere, in any modifier order', () => {
    const rows = [{ id: 'a', command: 'commandPalette.toggle', key: 'mod+k', when: '' }, { id: 'b', command: 'chat.new', key: 'shift+meta+n', when: '' }];
    expect(conflictLabels(rows, { id: 'x', key: 'meta+k', when: '' })).toEqual(['Command Palette: Toggle']);
    expect(conflictLabels(rows, { id: 'x', key: 'ctrl+k', when: '' })).toEqual([]);
    expect(conflictLabels(rows, { id: 'x', key: 'ctrl+k', when: '' }, 'Win32')).toEqual(['Command Palette: Toggle']);
    expect(conflictLabels(rows, { id: 'x', key: 'mod+shift+n', when: '' })).toEqual(['Chat: New']);
    expect(conflictLabels(rows, { id: 'x', key: 'mod+', when: '' })).toEqual([]);
    expect([shortcutConflictKey('mod+k'), shortcutConflictKey('cmd+k'), shortcutConflictKey('mod+k', 'Linux x86_64'), shortcutConflictKey('mod+space'), shortcutConflictKey('k+j')])
      .toEqual(['k|meta|||', 'k|meta|||', 'k||ctrl||', ' |meta|||', null]);
  });
  test('the draft keeps its last valid expression, and closing an invalid builder restores it', async () => {
    const row = rowId('chat.new', 'mod+n', '!terminalFocus');
    const page = (when: string) => keybindingSettings({ connection: 'disconnected' } as never, null, '', '', true, row, 'mod+n', when);
    expect((await page('!terminalFocus')).whenView.fallback).toBe('!terminalFocus');
    expect((await page('!terminalFocus &&')).whenView).toMatchObject({ fallback: '!terminalFocus', error: 'Use variables with !, &&, ||, and parentheses.' });
    expect((await page('!terminalFocus && composerFocus')).whenView.fallback).toBe('!terminalFocus && composerFocus');
    expect((await page('((')).whenView.fallback).toBe('!terminalFocus && composerFocus');
    expect(keptWhen('other', '((', false)).toBe('');
    const editor = contractComponent('settings-keybindings.contract', 'WhenEditor');
    const close = (error: string, newState = 'closed') => {
      const sent: string[] = [];
      editor.call('toggled', {}, { view: { error, fallback: 'composerFocus' }, setWhen: (value: string) => sent.push(value) }, [{ newState }]);
      return sent;
    };
    expect(close('Use variables with !, &&, ||, and parentheses.')).toEqual(['composerFocus']);
    expect(close('')).toEqual([]);
    expect(close('Use variables with !, &&, ||, and parentheses.', 'open')).toEqual([]);
  });
});
