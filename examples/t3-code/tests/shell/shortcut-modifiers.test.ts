// The held modifiers (shortcut-modifiers.contract, app.contract modifierKey, app-window.contract modifierDown). The
// first block ports T3 Code 9c161462c9's apps/web/src/shortcutModifierState.browser.test.ts "shortcutModifierState"
// with its names; "preserves the current object" is the root's write guard here, since a Contract `fn` returns a new
// record. The hook's render test is the window's actions run over its state: a key or a focus at a field, the
// thread-jump modifiers the sidebar's view lists, and the views that read the result.
import { describe, expect, test } from 'bun:test';
import { contractFns } from '../support/contract-fn-fixture';
import { contractComponent, value, type Env } from '../support/contract-action-fixture';
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
  test('speed mode is Shift alone', () => {
    expect(fns.speedModifiers({ ...emptyState(), shiftKey: true })).toBe(true);
    for (const other of ['metaKey', 'ctrlKey', 'altKey'] as const) expect(fns.speedModifiers({ ...emptyState(), shiftKey: true, [other]: true })).toBe(false);
    expect(fns.speedModifiers(emptyState())).toBe(false);
  });
});

// ── The window's actions ────────────────────────────────────────────────────

const root = contractComponent('app.contract', 'T3Code');
const window = contractComponent('src/shell/app-window.contract', 'T3Window');
const scope = (): Env => ({ ...fns, ShortcutModifierState: (record: State) => record, releaseUndoClaim: () => {}, quitHoldKey: () => {}, quitTypesLatin: contractFns('src/shell/quit-hold.contract', ['QuitHoldState']).quitTypesLatin }); // quit-hold.test.ts runs the hold
const rootState = (): Env => ({ shortcutModifiers: emptyState(), speedModifierState: emptyState(), threadJumpHintsShown: false, undoHeldKey: '', composerFocused: false });

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
  test('the composer\'s own focus is a field\'s, as at a launch that seats the focus there', () => {
    const state = { ...rootState(), composerComposing: false };
    state.speedModifierState = { ...emptyState(), shiftKey: true };
    root.call('composerFocus', state, { ...scope(), modifierEditableFocus: () => root.call('modifierEditableFocus', state, scope(), []) }, []);
    expect(state.speedModifierState).toEqual(emptyState());
    // The window's focusin unheard (editable false), Shift alone is still no speed mode while the composer has the focus.
    root.call('modifierKey', state, scope(), [true, keyboardEventLike({ key: 'Shift', shiftKey: true }), false]);
    expect(fns.speedModifiers(state.speedModifierState as State)).toBe(false);
    expect(state.shortcutModifiers).toEqual({ ...emptyState(), shiftKey: true });
    root.call('modifierKey', state, scope(), [false, keyboardEventLike({ key: 'Shift' }), false]);
    state.composerFocused = false; // composerBlur
    root.call('modifierKey', state, scope(), [true, keyboardEventLike({ key: 'Shift', shiftKey: true }), false]);
    expect(fns.speedModifiers(state.speedModifierState as State)).toBe(true);
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
  test('the window hears every key through capture and the keyup; a focus at a field empties the route\'s state', () => {
    const calls: unknown[][] = [], selectors: string[] = [];
    const state: Env = { focusEditable: false, dialogFocusId: '', dialogFocusOwner: '', dialogFocusPopup: '', dialogFocusInvoker: '' };
    const props: Env = { shortcuts: [], modifierKey: (...args: unknown[]) => calls.push(args), modifierEditableFocus: () => calls.push(['field']), chipFocusChanged: () => {}, interacted: () => calls.push(['interacted']),
      querySelectorAll: (selector: string) => { selectors.push(selector); return ['composer', 'pull-requests-search', 'palette-input']; } };
    window.call('windowFocused', state, props, [{ target: 'composer' }]);
    expect(state.focusEditable).toBe(true);
    window.call('modifierDown', state, props, ['Shift', { key: 'Shift', shiftKey: true }]);
    window.call('windowFocused', state, props, [{ target: 'pr-row-7' }]);
    window.call('modifierUp', state, props, ['Shift', { key: 'Shift' }]);
    expect(calls).toEqual([['field'], [true, { key: 'Shift', shiftKey: true }, true], ['interacted'], [false, { key: 'Shift' }, false]]); // a keydown is an interaction (backgroundActivityReporter's)
    expect(new Set(selectors)).toEqual(new Set(['[data-editable~="yes"]']));
    // Focus leaving the window's subtree for nothing leaves no field focused.
    window.call('windowFocused', state, props, [{ target: 'composer' }]);
    window.call('windowBlurred', state, { ...props, none: null }, [{ target: 'composer', relatedTarget: null }]);
    expect(state.focusEditable).toBe(false);
  });
  test('a dialog\'s field (the palette\'s input) is a field too, though the dialog stops the focusin', () => {
    const calls: unknown[][] = [];
    const state: Env = { focusEditable: false, dialogFocusId: '', dialogFocusOwner: '', dialogFocusPopup: '', dialogFocusInvoker: '', dialogFrames: [], dialogSeated: [] };
    const scope: Env = { shortcuts: [], modifierKey: (...args: unknown[]) => calls.push(args), modifierEditableFocus: () => calls.push(['field']), chipFocusChanged: () => {}, stopPropagation: () => {}, interacted: () => {},
      querySelectorAll: (selector: string) => selector.includes('editable') ? ['palette-input'] : [], frame: () => ({ unavailable: true, width: 0, height: 0 }),
      dialogLineage: () => [], dialogReturn: (...args: unknown[]) => args, dialogFrame: (owner: string, returns: unknown) => ({ owner, returns }) };
    window.call('dialogEntered', state, scope, ['palette', { target: 'palette-input' }]);
    expect(state.focusEditable).toBe(true);
    window.call('modifierDown', state, scope, ['Shift', { key: 'Shift', shiftKey: true }]);
    window.call('dialogEntered', state, scope, ['palette', { target: 'palette-row-1' }]);
    expect(state.focusEditable).toBe(false);
    window.call('dialogMenuEntered', state, scope, ['menu', 'invoker', { target: 'palette-input' }]);
    expect(state.focusEditable).toBe(true);
    expect(calls).toEqual([['field'], [true, { key: 'Shift', shiftKey: true }, true], ['field']]);
  });
  test('a focus at a field empties the route\'s state, not the sidebar\'s', () => {
    const state = rootState();
    state.shortcutModifiers = { ...emptyState(), metaKey: true }; state.speedModifierState = { ...emptyState(), shiftKey: true };
    root.call('modifierEditableFocus', state, scope(), []);
    expect(state.speedModifierState).toEqual(emptyState());
    expect(state.shortcutModifiers).toEqual({ ...emptyState(), metaKey: true });
  });
});

// ── The root's timers: the hints' 200 ms, their immediate hide, and the blur ──

const appSource = readFileSync(appPath('app.contract'), 'utf8');
const taskOf = (name: string) => {
  const m = new RegExp(`^  task ${name} when (.+)\\n    after\\((\\d+), (\\w+)\\)`, 'm').exec(appSource);
  if (!m) throw new Error(`app.contract: no task ${name}`);
  const [, gate, key] = /^(.+?)(?: key=(.+))?$/.exec(m[1]!)!;
  return { gate: gate!, key, ms: Number(m[2]), action: m[3]! };
};
const deriveOf = (name: string) => new RegExp(`^  derive ${name} = (.+)$`, 'm').exec(appSource)![1]!;
const held = contractFns('src/shared/press-modifiers.contract');
const env = (over: Env = {}): Env => ({ ...fns, ...held, data: { legacy: { enabled: false, jumpModifiers: [] }, sidebar: { jumpModifiers: ['meta'] } },
  page: { hasFocus: true }, shortcutModifiers: emptyState(), speedModifierState: emptyState(), threadJumpHintsShown: false, ...over });
const withHeld = (over: Env = {}) => { const e = env(over); e.threadJumpsHeld = value(deriveOf('threadJumpsHeld'), e); return e; };

describe('the thread-jump hints show 200 ms after ⌘ alone and hide at once', () => {
  test('⌘ alone arms the 200 ms show; ⌘⇧ or nothing held does not', () => {
    const delay = taskOf('threadJumpHintDelay');
    expect([delay.ms, delay.action]).toEqual([200, 'threadJumpHintsShow']);
    expect(value(delay.gate, withHeld({ shortcutModifiers: { ...emptyState(), metaKey: true } }))).toBe(true);
    expect(value(delay.gate, withHeld({ shortcutModifiers: { ...emptyState(), metaKey: true, shiftKey: true } }))).toBe(false);
    expect(value(delay.gate, withHeld())).toBe(false);
    expect(value(delay.gate, withHeld({ shortcutModifiers: { ...emptyState(), metaKey: true }, threadJumpHintsShown: true }))).toBe(false);
    // The legacy sidebar's list when it is on.
    expect(value(delay.gate, withHeld({ shortcutModifiers: { ...emptyState(), ctrlKey: true }, data: { legacy: { enabled: true, jumpModifiers: ['control'] }, sidebar: { jumpModifiers: ['meta'] } } }))).toBe(true);
  });
  test('shown hints hide in the same commit ⌘ comes up, and the shown flag resets a millisecond later', () => {
    const shown = withHeld({ threadJumpHintsShown: true });
    expect(value(deriveOf('threadJumpHints'), shown)).toBe(false);
    expect(value(deriveOf('threadJumpHints'), withHeld({ threadJumpHintsShown: true, shortcutModifiers: { ...emptyState(), metaKey: true } }))).toBe(true);
    const reset = taskOf('threadJumpHintReset');
    expect([reset.ms, reset.action, value(reset.gate, shown)]).toEqual([1, 'threadJumpHintsHide', true]);
    const state: Env = { threadJumpHintsShown: false };
    root.call('threadJumpHintsShow', state, {}, []);
    expect(state.threadJumpHintsShown).toBe(true);
    root.call('threadJumpHintsHide', state, {}, []);
    expect(state.threadJumpHintsShown).toBe(false);
  });
  test('losing the window\'s focus forgets what is held, a millisecond later', () => {
    const blur = taskOf('modifiersBlur');
    expect([blur.ms, blur.action]).toEqual([1, 'modifiersForget']);
    expect(value(blur.gate, env({ page: { hasFocus: false }, shortcutModifiers: { ...emptyState(), metaKey: true } }))).toBe(true);
    expect(value(blur.gate, env({ page: { hasFocus: false }, speedModifierState: { ...emptyState(), shiftKey: true } }))).toBe(true);
    expect(value(blur.gate, env({ page: { hasFocus: true }, shortcutModifiers: { ...emptyState(), metaKey: true } }))).toBe(false);
    expect(value(blur.gate, env({ page: { hasFocus: false } }))).toBe(false);
  });
});

describe('the sidebar lists the thread jumps\' modifiers', () => {
  const client = (bindings: Record<string, unknown>[]) => ({ config: { keybindings: bindings } }) as unknown as T3Client;
  const jumps = Array.from({ length: 9 }, (_, index) => ({ command: `thread.jump.${index + 1}`, shortcut: { key: `${index + 1}`, modKey: true } }));
  test('each thread.jump.N shortcut in effect, named as heldModifiers names them; mod is ⌘ on a Mac, Control elsewhere', () => {
    expect(threadJumpModifiers(client(jumps))).toEqual(['meta']);
    expect(threadJumpModifiers(client(jumps), 'Linux x86_64')).toEqual(['control']);
    expect(threadJumpModifiers(client([...jumps, { command: 'thread.jump.2', shortcut: { key: 'j', ctrlKey: true, shiftKey: true } }]))).toEqual(['meta', 'control+shift']);
    expect(threadJumpModifiers(client([{ command: 'chat.new', shortcut: { key: 'n', modKey: true } }]))).toEqual([]);
  });
  test('a binding whose `when` fails, or whose chord a newer binding claims, is not in effect (effectiveShortcutsForCommand)', () => {
    const terminalOnly = { command: 'thread.jump.1', shortcut: { key: '1', altKey: true }, whenAst: { type: 'identifier', name: 'terminalFocus' } };
    expect(threadJumpModifiers(client([terminalOnly]))).toEqual([]);
    expect(threadJumpModifiers(client([jumps[0]!, { command: 'chat.new', shortcut: { key: '1', modKey: true } }]))).toEqual([]);
  });
});

describe('a field is a data-editable node', () => {
  test('every input, textarea, select, contenteditable and role=textbox node has an id and says so, for isEditableFocused', () => {
    const unmarked = appFiles().filter(file => file.endsWith('.contract')).flatMap(file => readFileSync(appPath(file), 'utf8').split('\n')
      .filter(line => (/^\s*(input|textarea|select) /.test(line) && !/ type="(checkbox|radio|range|color|file|hidden)"/.test(line) && !/ display="none"/.test(line))
        || / contenteditable=| role="textbox"/.test(line))
      .filter(line => !line.includes('data-editable="yes"') || !/ id=/.test(line)).map(line => `${file}: ${line.trim().slice(0, 80)}`));
    expect(unmarked).toEqual([]);
  });
});

describe('Shift alone shows a row\'s quick actions (PullRequestSpeedActions)', () => {
  const speed = [{ key: 'close', action: 'close' }];
  for (const [file, name] of [['src/pull-requests/pages-prs.contract', 'PrRowButton'], ['src/panel/panel.contract', 'LinkedPrRowView']] as const) {
    test(`${name}: shown while prSpeedMode or a quick action is pending`, () => {
      const component = contractComponent(file, name), state = component.state();
      const drawn = (prSpeedMode: boolean, speedPending = false, actions = speed) =>
        component.drawn('PrQuickActions(', state, { row: { speed: actions, speedPending, number: 7 }, prSpeedMode });
      expect(drawn(true)).toBe(true);
      expect(drawn(false)).toBe(false);
      expect(drawn(false, true)).toBe(true);
      expect(drawn(true, false, [])).toBe(false);
    });
  }
});

describe('a thread row shows its jump label only while the hints show', () => {
  for (const [file, name, marker] of [['src/sidebar/sidebar-row.contract', 'ThreadRow', 'text t.jump font-family'], ['src/sidebar/legacy-sidebar.contract', 'LegacyThread', 'text t.jump height']] as const) {
    test(`${name}: threadJumpHints and a label`, () => {
      const component = contractComponent(file, name), state = component.state();
      // The row's other props and derives at rest: each name the view reads before the label is '' (no hover, no card).
      const drawn = (threadJumpHints: boolean, jump: string) => {
        const scope: Env = { threadJumpHints, presets: [], KmItem: (item: Env) => item, t: new Proxy({ jump } as Env, { get: (row, key) => (key in row ? row[key as string] : '') }) };
        for (let tries = 0; tries < 100; tries++) {
          try { return component.drawn(marker, state, scope); } catch (error) {
            const missing = /^(\w+) is not defined$/.exec((error as Error).message)?.[1];
            if (!missing) throw error;
            scope[missing] = '';
          }
        }
        throw new Error(`${name}: too many names`);
      };
      expect(drawn(true, '⌘1')).toBe(true);
      expect(drawn(false, '⌘1')).toBe(false);
      expect(drawn(true, '')).toBe(false);
    });
  }
});

describe('a thread that opens gives the composer the focus (ChatView\'s thread effect)', () => {
  test('a frame after the thread changes, unless the terminal is open', () => {
    const task = taskOf('composerOnThread');
    expect([task.ms, task.action, task.key]).toEqual([1, 'composerOnThreadFocus', 'data.threadId']);
    expect(value(task.gate, { data: { threadId: 't1' }, terminalDrawer: { open: false } })).toBe(true);
    expect(value(task.gate, { data: { threadId: 't1' }, terminalDrawer: { open: true } })).toBe(false);
    expect(value(task.gate, { data: { threadId: '' }, terminalDrawer: { open: false } })).toBe(false);
    const focused: string[] = [];
    root.call('composerOnThreadFocus', {}, { focus: (id: string) => focused.push(id) }, []);
    expect(focused).toEqual(['composer']);
  });
});
