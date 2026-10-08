// The modifier state the Pull Requests page's Shift quick actions read (pr-handoffs-and-quick-actions),
// ported with its names from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3) apps/web/src/shortcutModifierState.ts:
// areShortcutModifierStatesEqual and shortcutModifierStateAfterKeyboardEvent, and the route's
// speedMode (routes/_chat.pull-requests.tsx: Shift alone). The hook's window listeners are the native
// flags monitor here (modules/apple/T3Sidebar.swift `speedMode`, reported as
// `presentation.prSpeedMode`), which reads the keyboard's own flags rather than a browser's event
// flags; these functions are the rule both follow, and what the ported tests pin.
export type ShortcutModifierState = { metaKey: boolean; ctrlKey: boolean; altKey: boolean; shiftKey: boolean };
export type KeyboardEventLike = ShortcutModifierState & { type: string; key: string };

export const EMPTY_SHORTCUT_MODIFIER_STATE: ShortcutModifierState = { metaKey: false, ctrlKey: false, altKey: false, shiftKey: false };

export function areShortcutModifierStatesEqual(left: ShortcutModifierState, right: ShortcutModifierState): boolean {
  return left.metaKey === right.metaKey && left.ctrlKey === right.ctrlKey && left.altKey === right.altKey && left.shiftKey === right.shiftKey;
}

function normalizeModifierKey(key: string): keyof ShortcutModifierState | null {
  switch (key) {
    case 'Meta': case 'OS': case 'Command': return 'metaKey';
    case 'Control': return 'ctrlKey';
    case 'Alt': case 'Option': return 'altKey';
    case 'Shift': return 'shiftKey';
    default: return null;
  }
}

export function shortcutModifierStateAfterKeyboardEvent(currentState: ShortcutModifierState, event: KeyboardEventLike): ShortcutModifierState {
  const normalizedModifierKey = normalizeModifierKey(event.key);
  let nextState: ShortcutModifierState;
  if (normalizedModifierKey) {
    nextState = { ...currentState, [normalizedModifierKey]: event.type === 'keydown' };
  } else {
    // Flags on non-modifier keys may only clear a bit, never set one: a dictation tool's synthetic ⌘V
    // can leave later key events reporting metaKey=true until the reader physically taps ⌘.
    nextState = {
      metaKey: currentState.metaKey && event.metaKey,
      ctrlKey: currentState.ctrlKey && event.ctrlKey,
      altKey: currentState.altKey && event.altKey,
      shiftKey: currentState.shiftKey && event.shiftKey,
    };
  }
  return areShortcutModifierStatesEqual(currentState, nextState) ? currentState : nextState;
}

/** The Pull Requests page's speedMode: Shift held alone (useShortcutModifierState(true) clears it while an editable control has focus). */
export const speedMode = (state: ShortcutModifierState) => state.shiftKey && !state.metaKey && !state.ctrlKey && !state.altKey;
