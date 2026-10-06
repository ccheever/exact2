// T3 Code 1e2ecbd975 keybindings.test.ts terminal describes (:183-345) over the clone's resolver
// (keyboard-dispatch.ts chordWinners: the last matching rule for a chord wins), macOS chords only.
import { describe, expect, test } from 'bun:test';
import { chordWinners, type DispatchContext } from './keyboard-dispatch';

const identifier = (name: string) => ({ type: 'identifier', name });
const not = (node: unknown) => ({ type: 'not', node });
const mod = (key: string, extra: Record<string, boolean> = {}) => ({ key, modKey: true, metaKey: false, ctrlKey: false, shiftKey: false, altKey: false, ...extra });
const DEFAULT_BINDINGS = [
  { shortcut: mod('j'), command: 'terminal.toggle' },
  { shortcut: mod('d'), command: 'terminal.split', whenAst: identifier('terminalFocus') },
  { shortcut: mod('d', { shiftKey: true }), command: 'terminal.splitVertical', whenAst: identifier('terminalFocus') },
  { shortcut: mod('n'), command: 'terminal.new', whenAst: identifier('terminalFocus') },
  { shortcut: mod('w'), command: 'terminal.close', whenAst: identifier('terminalFocus') },
  { shortcut: mod('w'), command: 'rightPanel.close', whenAst: not(identifier('terminalFocus')) },
  { shortcut: mod('d'), command: 'diff.toggle', whenAst: not(identifier('terminalFocus')) },
  { shortcut: mod('s', { shiftKey: true }), command: 'thread.settle', whenAst: not(identifier('terminalFocus')) },
];
const context = (terminalFocus: boolean): DispatchContext => ({ composerFocus: false, editableFocus: false, turnRunning: false, modelPickerOpen: false,
  draftThreadRoute: false, modalOpen: false, settingsOpen: false, diffOpen: false, terminalFocus, terminalOpen: terminalFocus });
const command = (chord: string, terminalFocus: boolean) => chordWinners(DEFAULT_BINDINGS, context(terminalFocus)).get(chord) ?? null;

describe('isTerminalToggleShortcut', () => {
  test('matches Cmd+J on macOS', () => expect(command('Meta+j', false)).toBe('terminal.toggle'));
  test('matches Cmd+J on macOS while terminalFocus is true', () => expect(command('Meta+j', true)).toBe('terminal.toggle'));
});

describe('settle thread shortcut', () => {
  test('resolves outside the terminal', () => expect(command('Meta+Shift+s', false)).toBe('thread.settle'));
  test('does not intercept the terminal', () => expect(command('Meta+Shift+s', true)).toBeNull());
});

describe('split/new/close terminal shortcuts', () => {
  test('requires terminalFocus for default split/new/close bindings', () => {
    expect(command('Meta+d', false)).toBe('diff.toggle');
    expect(command('Meta+Shift+d', false)).toBeNull();
    expect(command('Meta+n', false)).toBeNull();
    expect(command('Meta+w', false)).toBe('rightPanel.close');
  });
  test('matches split/new when terminalFocus is true', () => {
    expect(command('Meta+d', true)).toBe('terminal.split');
    expect(command('Meta+Shift+d', true)).toBe('terminal.splitVertical');
    expect(command('Meta+n', true)).toBe('terminal.new');
    expect(command('Meta+w', true)).toBe('terminal.close');
  });
});
