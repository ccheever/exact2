// The held modifiers (shortcut-modifiers.contract, app.contract modifierKey, app-window.contract modifierDown). The
// first block ports T3 Code 9c161462c9's apps/web/src/shortcutModifierState.browser.test.ts "shortcutModifierState"
// with its names; "preserves the current object" is the root's write guard here, since a Contract `fn` returns a new
// record. The hook's render test is the window's actions run over its state: a key or a focus at a field, the
// thread-jump modifiers the sidebar's view lists, and the views that read the result.
import { describe, expect, test } from 'bun:test';
import { contractFns } from '../support/contract-fn-fixture';
import { contractComponent, type Env } from '../support/contract-action-fixture';
import { readFileSync } from 'node:fs';
import { appFiles, appPath } from '../support/app-files';
import { threadJumpModifiers } from '../../src/sidebar/sidebar-view';
import type { T3Client } from '../../src/client/client';

type State = { metaKey: boolean; ctrlKey: boolean; altKey: boolean; shiftKey: boolean };
const fns = contractFns('src/shell/shortcut-modifiers.contract', ['ShortcutModifierState']) as unknown as {
  emptyShortcutModifierState: () => State;
  areShortcutModifierStatesEqual: (left: State, right: State) => boolean;
  shortcutModifierStateAfterKeyboardEvent: (current: State, down: boolean, e: Partial<State> & { key: string }) => State;
  anyShortcutModifier: (state: State) => boolean;
  speedModifiers: (state: State) => boolean;
  threadJumpModifiers: (state: State) => boolean;
};
const emptyState = (): State => ({ metaKey: false, ctrlKey: false, altKey: false, shiftKey: false });
const keyboardEventLike = (init: Partial<State> & { key: string }) => ({ ...emptyState(), ...init });
const after = (state: State, type: 'keydown' | 'keyup', init: Partial<State> & { key: string }) =>
  fns.shortcutModifierStateAfterKeyboardEvent(state, type === 'keydown', keyboardEventLike(init));

describe('shortcutModifierState', () => {
  test('compares modifier states by value', () => {
    expect(fns.areShortcutModifierStatesEqual({ metaKey: false, ctrlKey: true, altKey: false, shiftKey: true }, { metaKey: false, ctrlKey: true, altKey: false, shiftKey: true })).toBe(true);
    expect(fns.areShortcutModifierStatesEqual({ metaKey: false, ctrlKey: true, altKey: false, shiftKey: true }, { metaKey: false, ctrlKey: false, altKey: false, shiftKey: true })).toBe(false);
  });
  test('tracks bare modifier keydown and keyup events explicitly', () => {
    let state = emptyState();
    state = after(state, 'keydown', { key: 'Meta', metaKey: false });
    expect(state).toEqual({ metaKey: true, ctrlKey: false, altKey: false, shiftKey: false });
    state = after(state, 'keydown', { key: 'Shift', metaKey: true, shiftKey: false });
    expect(state).toEqual({ metaKey: true, ctrlKey: false, altKey: false, shiftKey: true });
    state = after(state, 'keyup', { key: 'Meta', metaKey: true, shiftKey: true });
    expect(state).toEqual({ metaKey: false, ctrlKey: false, altKey: false, shiftKey: true });
    state = after(state, 'keyup', { key: 'Shift', shiftKey: true });
    expect(state).toEqual(emptyState());
  });
  test('names every modifier key the DOM gives', () => {
    for (const [key, flag] of [['OS', 'metaKey'], ['Command', 'metaKey'], ['Control', 'ctrlKey'], ['Alt', 'altKey'], ['Option', 'altKey']] as const)
      expect(after(emptyState(), 'keydown', { key })).toEqual({ ...emptyState(), [flag]: true });
  });
  test('ignores poisoned modifier flags on non-modifier keys', () => {
    expect(after(emptyState(), 'keydown', { key: 'Enter', metaKey: true })).toEqual(emptyState());
  });
  test('clears a held modifier when a non-modifier key reports it released', () => {
    expect(after({ ...emptyState(), metaKey: true }, 'keydown', { key: 'a', metaKey: false })).toEqual(emptyState());
  });
  test('speed mode is Shift alone, the thread jumps ⌘ alone', () => {
    expect(fns.speedModifiers({ ...emptyState(), shiftKey: true })).toBe(true);
    for (const other of ['metaKey', 'ctrlKey', 'altKey'] as const) expect(fns.speedModifiers({ ...emptyState(), shiftKey: true, [other]: true })).toBe(false);
    expect(fns.speedModifiers(emptyState())).toBe(false);
    expect(fns.threadJumpModifiers({ ...emptyState(), metaKey: true })).toBe(true);
    expect(fns.threadJumpModifiers({ ...emptyState(), metaKey: true, shiftKey: true })).toBe(false);
  });
});

// ── The window's actions ────────────────────────────────────────────────────

const root = contractComponent('app.contract', 'T3Code');
const window = contractComponent('src/shell/app-window.contract', 'T3Window');
const scope = (): Env => ({ ...fns, ShortcutModifierState: (record: State) => record });
const rootState = (): Env => ({ shortcutModifiers: emptyState(), speedModifierState: emptyState(), threadJumpHintsShown: false });

describe('the window tracks held modifiers from its key events', () => {
  test('Shift alone at a row is speed mode until it comes up; ⌘ joining ends it', () => {
    const state = rootState();
    root.call('modifierKey', state, scope(), [true, keyboardEventLike({ key: 'Shift', shiftKey: true }), false]);
    expect(fns.speedModifiers(state.speedModifierState as State)).toBe(true);
    root.call('modifierKey', state, scope(), [true, keyboardEventLike({ key: 'Meta', metaKey: true, shiftKey: true }), false]);
    expect(fns.speedModifiers(state.speedModifierState as State)).toBe(false);
    root.call('modifierKey', state, scope(), [false, keyboardEventLike({ key: 'Meta', shiftKey: true }), false]);
    expect(fns.speedModifiers(state.speedModifierState as State)).toBe(true);
    root.call('modifierKey', state, scope(), [false, keyboardEventLike({ key: 'Shift' }), false]);
    expect(state.speedModifierState).toEqual(emptyState());
    expect(state.shortcutModifiers).toEqual(emptyState());
  });
  test('a key at a field empties the route\'s state but not the sidebar\'s (useShortcutModifierState(true))', () => {
    const state = rootState();
    root.call('modifierKey', state, scope(), [true, keyboardEventLike({ key: 'Shift', shiftKey: true }), false]);
    root.call('modifierKey', state, scope(), [true, keyboardEventLike({ key: 'A', shiftKey: true }), true]);
    expect(state.speedModifierState).toEqual(emptyState());
    expect(state.shortcutModifiers).toEqual({ ...emptyState(), shiftKey: true });
    // Back at a row with Shift still down, a letter cannot set it again: only Shift's own key does.
    root.call('modifierKey', state, scope(), [true, keyboardEventLike({ key: 'B', shiftKey: true }), false]);
    expect(fns.speedModifiers(state.speedModifierState as State)).toBe(false);
  });
  test('an unchanged state is not written again (the hook returns before dispatching)', () => {
    const state = rootState(), held = { ...emptyState(), shiftKey: true };
    state.shortcutModifiers = held; state.speedModifierState = held;
    root.call('modifierKey', state, scope(), [true, keyboardEventLike({ key: 'a', shiftKey: true }), false]);
    expect(state.shortcutModifiers).toBe(held);
    expect(state.speedModifierState).toBe(held);
  });
  test('losing the window\'s focus forgets both', () => {
    const state = rootState();
    state.shortcutModifiers = { ...emptyState(), metaKey: true }; state.speedModifierState = { ...emptyState(), shiftKey: true };
    root.call('modifiersForget', state, scope(), []);
    expect(state.shortcutModifiers).toEqual(emptyState());
    expect(state.speedModifierState).toEqual(emptyState());
  });
  test('the window hears every key through capture and the keyup, and says whether the focus is at a field', () => {
    const calls: unknown[][] = [], selectors: string[] = [];
    const state: Env = { focusEditable: false, dialogFocusId: '', dialogFocusOwner: '', dialogFocusPopup: '', dialogFocusInvoker: '' };
    const props: Env = { modifierKey: (...args: unknown[]) => calls.push(args), chipFocusChanged: () => {},
      querySelectorAll: (selector: string) => { selectors.push(selector); return ['composer', 'pull-requests-search']; } };
    window.call('windowFocused', state, props, [{ target: 'composer' }]);
    expect(state.focusEditable).toBe(true);
    window.call('modifierDown', state, props, ['Shift', { key: 'Shift', shiftKey: true }]);
    window.call('windowFocused', state, props, [{ target: 'pr-row-7' }]);
    window.call('modifierUp', state, props, ['Shift', { key: 'Shift' }]);
    expect(calls).toEqual([[true, { key: 'Shift', shiftKey: true }, true], [false, { key: 'Shift' }, false]]);
    expect(new Set(selectors)).toEqual(new Set(['[data-editable~="yes"]']));
    // Focus leaving the window's subtree for nothing leaves no field focused.
    window.call('windowFocused', state, props, [{ target: 'composer' }]);
    window.call('windowBlurred', state, { ...props, none: null }, [{ target: 'composer', relatedTarget: null }]);
    expect(state.focusEditable).toBe(false);
  });
});

describe('the sidebar lists the thread jumps\' modifiers', () => {
  const client = (bindings: { command: string; shortcut: Record<string, unknown> }[]) => ({ config: { keybindings: bindings } }) as unknown as T3Client;
  test('each thread.jump.N shortcut\'s set, named as heldModifiers names them, mod as ⌘', () => {
    const jumps = Array.from({ length: 9 }, (_, index) => ({ command: `thread.jump.${index + 1}`, shortcut: { key: `${index + 1}`, modKey: true } }));
    expect(threadJumpModifiers(client(jumps))).toEqual(['meta']);
    expect(threadJumpModifiers(client([...jumps, { command: 'thread.jump.2', shortcut: { key: 'j', ctrlKey: true, shiftKey: true } }]))).toEqual(['meta', 'control+shift']);
    expect(threadJumpModifiers(client([{ command: 'chat.new', shortcut: { key: 'n', modKey: true } }]))).toEqual([]);
  });
});

describe('a field is a data-editable node', () => {
  test('every text input and textarea with an id says so, for the window\'s isEditableFocused', () => {
    const unmarked = appFiles().filter(file => file.endsWith('.contract')).flatMap(file => readFileSync(appPath(file), 'utf8').split('\n')
      .filter(line => /^\s*(input|textarea) /.test(line) && / id=/.test(line) && !/ type="(checkbox|radio|range|color|file|hidden)"/.test(line) && !/ display="none"/.test(line))
      .filter(line => !line.includes('data-editable="yes"')).map(line => `${file}: ${line.trim().slice(0, 80)}`));
    expect(unmarked).toEqual([]);
  });
});
