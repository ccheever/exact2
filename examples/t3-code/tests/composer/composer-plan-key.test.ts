import { describe, expect, test } from 'bun:test';
import { contractComponent, contractFns, type Env } from '../support/contract-action-fixture';

// app.contract's composer key action (composerKeyEvent → composerGrammarKey) and the command menu's
// highlight, run with Contract's action semantics. The reference is onComposerCommandKey (ChatComposer.tsx)
// behind ComposerPromptEditorTiptap's handleKeyDown, ComposerStashMenu's window listener and ChatView's
// thread.editQueuedMessage.
const root = contractComponent('app.contract', 'T3Code');
const keyFns = contractFns('src/composer/composer-keys.contract');
const { heldModifiers } = contractFns('src/shared/press-modifiers.contract');

type Options = { planVisible?: boolean; planActive?: boolean; connected?: boolean; inCode?: boolean; inList?: boolean; nested?: boolean; menuOpen?: boolean;
  menuCount?: number; stashOpen?: boolean; stashCount?: number; caret?: number; sendChords?: string; sendCommandChords?: string; queuedEdit?: string; workspaceFocus?: boolean };
function composer(options: Options = {}) {
  const calls: unknown[][] = [];
  const state: Env = { composerShift: false, composerCommand: false, composerComposing: false };
  const caret = options.caret ?? 2;
  const scope = (): Env => ({
    data: { composer: { planVisible: options.planVisible ?? true, planActive: options.planActive ?? false, sendChords: options.sendChords ?? 'Meta+Enter',
      sendCommandChords: options.sendCommandChords ?? '' }, connected: options.connected ?? true },
    richComposer: { mode: 'rich', historyPrevious: true, historyNext: true, chips: [] },
    composerRichCurrent: true, composerSelectionCurrent: true, composerSelectionStart: caret, composerSelectionEnd: caret, composerText: '- a',
    composerInCode: options.inCode ?? false, composerInList: options.inList ?? false, composerInNestedList: options.nested ?? false, composerCodeStart: false, composerRequiresModifier: false,
    editor: { menu: { open: options.menuOpen ?? false, count: options.menuCount ?? 3 }, stash: { menuOpen: options.stashOpen ?? false, count: options.stashCount ?? 2 } },
    shortcuts: [...(options.queuedEdit ? [{ kind: 'composer-key', chord: options.queuedEdit, target: 'cclocal:queued-edit', extra: 'run-1' }] : []),
      ...(options.workspaceFocus ? [{ kind: 'workspace-focus', chord: 'F6', target: 'next' }, { kind: 'workspace-focus', chord: 'Shift+F6', target: 'previous' }, { kind: 'workspace-focus', chord: 'Meta+F6', target: 'next' }] : [])],
    first: (items: unknown[]) => items[0], ...keyFns, heldModifiers, rangeFrame: () => ({ unavailable: true }),
    preventDefault: () => calls.push(['preventDefault']), stopPropagation: () => {},
    menuPick: (...args: unknown[]) => calls.push(['menuPick', ...args]),
    command: (...args: unknown[]) => calls.push(['command', ...args]),
    workspaceMove: (...args: unknown[]) => calls.push(['workspaceMove', ...args]),
    submit: (...args: unknown[]) => calls.push(['submit', ...args]),
    composerMenuKey: (...args: unknown[]) => calls.push(['menu', ...args]),
    composerStashKey: (...args: unknown[]) => calls.push(['stash', ...args]),
    composerIntentRequest: (kind: string, _text: string, key: string, shift: boolean) => calls.push(['intent', kind, key, shift]),
    composerGrammarKey: (...args: unknown[]) => root.call('composerGrammarKey', state, scope(), args),
    ...state,
  });
  const key = (k: string, modifiers: Partial<Record<'shiftKey' | 'ctrlKey' | 'altKey' | 'metaKey' | 'repeat', boolean>> = {}, code = '') =>
    root.call('composerKeyEvent', state, scope(), [k, { key: k, code, repeat: false, shiftKey: false, ctrlKey: false, altKey: false, metaKey: false, ...modifiers }]);
  return { calls, key };
}

describe('⇧⇥ in the composer (onComposerCommandKey)', () => {
  test('toggles Build and Plan where plan mode is available, and keeps the key', () => {
    const build = composer();
    build.key('Tab', { shiftKey: true });
    expect(build.calls).toEqual([['preventDefault'], ['menuPick', 'interaction', '', 'plan']]);
    const plan = composer({ planActive: true });
    plan.key('Tab', { shiftKey: true });
    expect(plan.calls).toEqual([['preventDefault'], ['menuPick', 'interaction', '', 'default']]);
  });
  test('toggles in a top-level list item and with a menu open; a code block and a nested list item outdent first', () => {
    for (const options of [{ inList: true }, { menuOpen: true }, { inList: true, nested: true, menuOpen: true }]) {
      const field = composer(options);
      field.key('Tab', { shiftKey: true });
      expect(field.calls).toEqual([['preventDefault'], ['menuPick', 'interaction', '', 'plan']]);
    }
    for (const options of [{ inCode: true }, { inList: true, nested: true }]) {
      const field = composer(options);
      field.key('Tab', { shiftKey: true });
      expect(field.calls).toEqual([['preventDefault'], ['intent', 'key', 'Tab', true]]);
    }
  });
  test('without plan mode the focus goes back, from a list item too; a code block still outdents', () => {
    const list = composer({ planVisible: false, inList: true });
    list.key('Tab', { shiftKey: true });
    expect(list.calls).toEqual([]);
    const tab = composer({ planVisible: false, inList: true });
    tab.key('Tab');
    expect(tab.calls).toEqual([['preventDefault'], ['intent', 'key', 'Tab', false]]);
    const code = composer({ planVisible: false, inCode: true });
    code.key('Tab', { shiftKey: true });
    expect(code.calls).toEqual([['preventDefault'], ['intent', 'key', 'Tab', true]]);
    const nested = composer({ planVisible: false, inList: true, nested: true });
    nested.key('Tab', { shiftKey: true });
    expect(nested.calls).toEqual([['preventDefault'], ['intent', 'key', 'Tab', true]]);
    const text = composer({ planVisible: false });
    text.key('Tab', { shiftKey: true });
    expect(text.calls).toEqual([]);
  });
  test('plain Tab is not the toggle, and a disconnected composer keeps the key without a change', () => {
    const tab = composer();
    tab.key('Tab');
    expect(tab.calls).toEqual([]);
    const offline = composer({ connected: false });
    offline.key('Tab', { shiftKey: true });
    expect(offline.calls).toEqual([['preventDefault']]);
  });
});

describe('the open command menu takes its keys from the composer (onComposerCommandKey)', () => {
  test('↑ and ↓ move the highlight with any modifier held; Return and Tab pick; Escape dismisses', () => {
    for (const [k, modifiers] of [['ArrowDown', {}], ['ArrowDown', { shiftKey: true }], ['ArrowUp', { altKey: true }], ['Enter', {}], ['Enter', { shiftKey: true }], ['Tab', {}], ['Escape', {}]] as const) {
      const field = composer({ menuOpen: true });
      field.key(k, modifiers);
      expect(field.calls).toEqual([['preventDefault'], ['menu', k]]);
    }
  });
  test('an empty list keeps only Escape; ⇧⇥ stays the plan toggle', () => {
    const empty = composer({ menuOpen: true, menuCount: 0 });
    empty.key('ArrowDown');
    expect(empty.calls).toEqual([]);
    empty.key('Escape');
    expect(empty.calls).toEqual([['preventDefault'], ['menu', 'Escape']]);
    const backtab = composer({ menuOpen: true });
    backtab.key('Tab', { shiftKey: true });
    expect(backtab.calls).toEqual([['preventDefault'], ['menuPick', 'interaction', '', 'plan']]);
  });
  test('a send chord that is no plain send goes to Send; ⌘↩ that is a plain send picks', () => {
    const running = composer({ menuOpen: true, sendChords: 'Meta+Enter', sendCommandChords: 'Meta+Enter' });
    running.key('Enter', { metaKey: true });
    expect(running.calls).toEqual([['preventDefault'], ['submit', 'key:meta']]);
    const idle = composer({ menuOpen: true, sendChords: 'Meta+Enter', sendCommandChords: '' });
    idle.key('Enter', { metaKey: true });
    expect(idle.calls).toEqual([['preventDefault'], ['menu', 'Enter']]);
  });
});

describe('the composer\'s send chords (composerSubmissionIntentForKey through the server keybindings)', () => {
  test('a send chord sends with its modifiers, once per press', () => {
    const field = composer({ sendChords: 'Meta+Enter Meta+Alt+Enter' });
    field.key('Enter', { metaKey: true });
    field.key('Enter', { metaKey: true, altKey: true });
    field.key('Enter', { metaKey: true, repeat: true });
    expect(field.calls).toEqual([['preventDefault'], ['submit', 'key:meta'], ['preventDefault'], ['submit', 'key:meta+alt'], ['preventDefault']]);
  });
  test('Return outside the chords is the field\'s: a plain send or ⇧↩\'s line break through the edit', () => {
    const field = composer({ sendChords: 'Meta+Enter' });
    field.key('Enter');
    field.key('Enter', { shiftKey: true });
    expect(field.calls).toEqual([['preventDefault'], ['intent', 'key', 'Enter', false], ['preventDefault'], ['intent', 'key', 'Enter', true]]);
  });
});

describe('the stash menu takes Escape, ↑ ↓, Return and ⌘⌫ from the composer (ComposerStashMenu)', () => {
  test('each key goes to the stash, before a send chord', () => {
    for (const [k, modifiers] of [['Escape', {}], ['ArrowDown', {}], ['ArrowUp', { shiftKey: true }], ['Enter', {}], ['Enter', { metaKey: true }], ['Backspace', { metaKey: true }], ['Backspace', { ctrlKey: true }]] as const) {
      const field = composer({ stashOpen: true });
      field.key(k, modifiers);
      expect(field.calls).toEqual([['preventDefault'], ['stash', k]]);
    }
  });
  test('a plain ⌫ deletes text; an empty stash keeps only Escape; an open command menu comes first', () => {
    const field = composer({ stashOpen: true });
    field.key('Backspace');
    expect(field.calls.filter(call => call[0] === 'stash')).toEqual([]);
    const empty = composer({ stashOpen: true, stashCount: 0 });
    empty.key('Enter');
    expect(empty.calls.filter(call => call[0] === 'stash')).toEqual([]);
    const menu = composer({ stashOpen: true, menuOpen: true });
    menu.key('ArrowDown');
    expect(menu.calls).toEqual([['preventDefault'], ['menu', 'ArrowDown']]);
  });
});

describe('thread.editQueuedMessage with the caret at the start (ChatView isCaretAtStart)', () => {
  test('⌥↑ at the start edits the last queued message; a held key acts once', () => {
    const field = composer({ caret: 0, queuedEdit: 'Alt+ArrowUp' });
    field.key('ArrowUp', { altKey: true });
    field.key('ArrowUp', { altKey: true, repeat: true });
    expect(field.calls).toEqual([['preventDefault'], ['command', 'cclocal:queued-edit', 'run-1', '', 0], ['preventDefault']]);
  });
  test('elsewhere the key moves the caret; another chord or no queued message is left alone', () => {
    const middle = composer({ caret: 2, queuedEdit: 'Alt+ArrowUp' });
    middle.key('ArrowUp', { altKey: true });
    expect(middle.calls).toEqual([]);
    const other = composer({ caret: 0, queuedEdit: 'Alt+ArrowUp' });
    other.key('ArrowUp');
    expect(other.calls.filter(call => call[0] === 'command')).toEqual([]);
    const none = composer({ caret: 0 });
    none.key('ArrowUp', { altKey: true });
    expect(none.calls).toEqual([]);
  });
  test('a rebound letter chord matches by its physical key, whatever the layout types', () => {
    const field = composer({ caret: 0, queuedEdit: 'Alt+e' });
    field.key('´', { altKey: true }, 'KeyE');
    expect(field.calls).toEqual([['preventDefault'], ['command', 'cclocal:queued-edit', 'run-1', '', 0]]);
  });
});

describe('the window keeps the menus\' highlights (composerMenuKey, composerStashKey)', () => {
  const run = (action: string, k: string, editor: Env, slots: Env) => {
    const calls: unknown[][] = [];
    const state: Env = { ...slots };
    const derived = (): Env => ({
      composerMenuActive: state.composerMenuHighlightKey === editor.menu.searchKey && (state.composerMenuHighlight as number) < editor.menu.count ? state.composerMenuHighlight : 0,
      composerStashActive: (state.composerStashHighlight as number) < editor.stash.count ? state.composerStashHighlight : 0,
    });
    const scope = (): Env => ({ editor, max: Math.max, first: (items: unknown[]) => items[0], editorOp: (...args: unknown[]) => calls.push(args),
      composerMenuMove: (delta: number) => root.call('composerMenuMove', state, { ...scope(), ...derived() }, [delta]),
      composerStashMove: (delta: number) => root.call('composerStashMove', state, { ...scope(), ...derived() }, [delta]), ...derived() });
    root.call(action, state, scope(), [k]);
    return { calls, state, active: derived() };
  };
  const rows = [{ id: 'r0', index: 0 }, { id: 'r1', index: 1 }, { id: 'r2', index: 2 }];
  const editor = { menu: { count: 3, searchKey: 'a', rows }, stash: { count: 2, rows: [{ id: 's0', index: 0 }, { id: 's1', index: 1 }] } };
  test('↓ and ↑ wrap the command menu\'s highlight within its query; Return picks it; Escape dismisses', () => {
    expect(run('composerMenuKey', 'ArrowDown', editor, { composerMenuHighlight: 2, composerMenuHighlightKey: 'a' }).active.composerMenuActive).toBe(0);
    expect(run('composerMenuKey', 'ArrowUp', editor, { composerMenuHighlight: 0, composerMenuHighlightKey: 'a' }).active.composerMenuActive).toBe(2);
    // A highlight made under another query starts over at the first row.
    expect(run('composerMenuKey', 'ArrowDown', editor, { composerMenuHighlight: 2, composerMenuHighlightKey: 'b' }).active.composerMenuActive).toBe(1);
    expect(run('composerMenuKey', 'Enter', editor, { composerMenuHighlight: 1, composerMenuHighlightKey: 'a' }).calls).toEqual([['pick', 'r1', '']]);
    expect(run('composerMenuKey', 'Escape', editor, { composerMenuHighlight: 1, composerMenuHighlightKey: 'a' }).calls).toEqual([['dismiss', '', '']]);
  });
  test('the stash restores or deletes its highlighted prompt, and Escape closes it', () => {
    expect(run('composerStashKey', 'ArrowDown', editor, { composerStashHighlight: 1 }).active.composerStashActive).toBe(0);
    expect(run('composerStashKey', 'Enter', editor, { composerStashHighlight: 1 }).calls).toEqual([['stash-restore', 's1', '']]);
    expect(run('composerStashKey', 'Backspace', editor, { composerStashHighlight: 0 }).calls).toEqual([['stash-delete', 's0', '']]);
    expect(run('composerStashKey', 'Escape', editor, { composerStashHighlight: 0 }).calls).toEqual([['stash-menu', '', 'close']]);
  });
});

describe('a composer menu follows its trigger (openOptions, frameMoved)', () => {
  const frames = contractFns('src/shell/layout-frames.contract', { floor: Math.floor });
  const run = (action: string, args: unknown[], optionsOpen: string, boxes: Record<string, { x: number; width: number }>) => {
    const state: Env = { optionsAnchorX: -1, optionsAnchorWidth: 0, layoutFrames: '' };
    const scope = (): Env => ({ ...frames, optionsOpen, focus: () => {}, data: { composer: { runtimeSelected: 0 } },
      frame: (id: string) => boxes[id] ? { ...boxes[id], y: 0, height: 28, unavailable: false } : { x: 0, y: 0, width: 0, height: 0, unavailable: true },
      measureOptionsTrigger: (name: string) => root.call('measureOptionsTrigger', state, scope(), [name]) });
    root.call(action, state, scope(), args);
    return state;
  };
  test('opening measures the trigger; a layout change while it is open measures it again', () => {
    expect(run('measureOptionsTrigger', ['runtime'], '', { 'composer-runtime-trigger': { x: 410, width: 96 } })).toMatchObject({ optionsAnchorX: 410, optionsAnchorWidth: 96 });
    expect(run('frameMoved', [0, 0], 'runtime', { 'composer-runtime-trigger': { x: 330, width: 96 } })).toMatchObject({ optionsAnchorX: 330, optionsAnchorWidth: 96 });
    expect(run('frameMoved', [0, 0], '', { 'composer-runtime-trigger': { x: 330, width: 96 } })).toMatchObject({ optionsAnchorX: -1 });
  });
  test('a trigger that is not laid out leaves the menu at the card\'s edge', () => {
    expect(run('measureOptionsTrigger', ['effort'], '', {})).toMatchObject({ optionsAnchorX: -1, optionsAnchorWidth: 0 });
    expect(frames.optionsTriggerId!('context')).toBe('context-window-meter');
    expect(frames.optionsTriggerId!('implement')).toBe('implement-actions');
  });
});

// AppSidebarLayout's capture listener (T3 Code v0.1.5, #25877): F6 and ⇧F6 leave the rich field as they leave any other
// control; the field takes its keys before the dispatch buttons hear them, so its own key action moves the focus.
describe('F6 in the composer (workspace.focusNext, workspace.focusPrevious)', () => {
  test('F6, ⌘F6 and ⇧F6 move to the next or previous area from the composer, and keep the key', () => {
    const field = composer({ workspaceFocus: true });
    field.key('F6', {}, 'F6');
    field.key('F6', { metaKey: true }, 'F6');
    field.key('F6', { shiftKey: true }, 'F6');
    expect(field.calls).toEqual([['preventDefault'], ['workspaceMove', 1, 'composer'], ['preventDefault'], ['workspaceMove', 1, 'composer'],
      ['preventDefault'], ['workspaceMove', -1, 'composer']]);
  });
  test('without a binding F6 stays the field\'s', () => {
    const field = composer();
    field.key('F6', {}, 'F6');
    expect(field.calls).toEqual([]);
  });
});
