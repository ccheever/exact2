import { expect, test } from 'bun:test';
import { documentShortcutAllowed, documentShortcutCommand, documentShortcutsMessage } from '../../src/timeline/inline-document-shortcuts';
const shortcut = { key: 'k', metaKey: true, ctrlKey: false, altKey: false, shiftKey: false };
const binding = (command: string) => ({ command, shortcut });
const key = { ...shortcut, key: 'K', repeat: false };

test('only configured view shortcuts pass, and a conflicting write binding blocks the chord', () => {
  expect(documentShortcutCommand({ keybindings: [binding('commandPalette.toggle')] }, key)).toBe('commandPalette.toggle');
  expect(documentShortcutCommand({ keybindings: [binding('thread.delete'), binding('commandPalette.toggle')] }, key)).toBe('');
  expect(documentShortcutCommand({ keybindings: [binding('commandPalette.toggle')] }, { ...key, repeat: true })).toBe('');
  expect(documentShortcutsMessage({ keybindings: [binding('thread.delete'), binding('commandPalette.toggle')] }).params).toEqual({ shortcuts: [], blocked: [shortcut] });
  expect(documentShortcutsMessage({ keybindings: [binding('commandPalette.toggle')] }).params.shortcuts).toEqual([shortcut]);
  expect(documentShortcutAllowed('composer.send')).toBe(false);
  expect(documentShortcutAllowed('thread.jump.1')).toBe(true);
  expect(documentShortcutAllowed('thread.jump.delete')).toBe(false);
});
