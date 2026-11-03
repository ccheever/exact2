// Chromium's :focus-visible, which Base UI's tooltips open on, counts a key as keyboard input only without ⌘, ⌃ or ⌥.
// Measured on the Beta app: from the thread search after a click, ⌘B leaves the focus on the sidebar toggle with no
// ring and no tooltip; after typing there, with both. The root's `keyboardLast` (app.contract modifierKey) follows
// that rule; tip-focus.test.ts has the Tip's side.
import { describe, expect, test } from 'bun:test';
import { contractComponent } from '../support/contract-action-fixture';

const root = contractComponent('app.contract', 'T3Code');
const key = (keyboardLast: boolean, e: Partial<KeyboardEvent>) => {
  const state: Record<string, unknown> = { keyboardLast, undoHeldKey: '', shortcutModifiers: {}, speedModifierState: {}, composerFocused: false };
  const stub = () => ({});
  root.call('modifierKey', state, {
    quitHoldKey: () => {}, preventDefault: () => {}, stopPropagation: () => {}, releaseUndoClaim: () => {}, quitTypesLatin: () => true,
    shortcutModifierStateAfterKeyboardEvent: stub, areShortcutModifierStatesEqual: () => true, emptyShortcutModifierState: stub,
  }, [true, { key: 'b', code: 'KeyB', repeat: false, metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...e }, false]);
  return state.keyboardLast;
};

describe('a chord is not keyboard input to :focus-visible', () => {
  test('a key without ⌘, ⌃ or ⌥ is; ⌘B, ⌃Tab or ⌥X leaves the last input as it was', () => {
    expect(key(false, { key: 'a' })).toBe(true);
    expect(key(false, { key: 'Tab', shiftKey: true })).toBe(true);
    for (const chord of [{ metaKey: true }, { ctrlKey: true, key: 'Tab' }, { altKey: true, key: 'x' }]) {
      expect(key(false, chord)).toBe(false);
      expect(key(true, chord)).toBe(true);
    }
  });
});
