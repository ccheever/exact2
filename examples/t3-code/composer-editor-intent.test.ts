import { describe, expect, test } from 'bun:test';
import { DEFAULT_SEND_RULES, chordCommand, gestureChord, sendChords, sendIntent } from './composer-editor-intent';
import type { Obj } from './domain';

const key = (modifiers: string, ageMs = 5): Obj => ({ modifiers, source: 'key', ageMs });
const id = (name: string): Obj => ({ type: 'identifier', name });
const rule = (command: string, shortcut: Obj, whenAst?: Obj): Obj => ({ command, shortcut: { key: 'enter', modKey: false, metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...shortcut }, ...(whenAst ? { whenAst } : {}) });
/** A HEAD server's resolved rules: the late default ⌘↩ background rule sits before ⌥⌘↩ (keybindings.ts, 263097a). */
const head: Obj = { keybindings: [
  rule('thread.steerQueuedMessage', { modKey: true, shiftKey: true }, { type: 'not', node: id('terminalFocus') }),
  ...DEFAULT_SEND_RULES,
  { command: 'chat.new', shortcut: { key: 'n', modKey: true }, whenAst: { type: 'not', node: id('terminalFocus') } },
] };
/** A pre-263097a config: ⌘↩ in a draft is not a background send. */
const old: Obj = { keybindings: [DEFAULT_SEND_RULES[0], DEFAULT_SEND_RULES[2], DEFAULT_SEND_RULES[3]] };

describe('composerSubmissionIntentForKey (5c0429b, 263097a)', () => {
  test('⌘↩ in a new-thread draft starts it in the background again', () => {
    expect(sendIntent(head, key('meta'), false, true)).toBe('background');
    expect(sendIntent(old, key('meta'), false, true)).toBe('foreground');
  });
  test('⌥⌘↩ starts a draft in the background', () => {
    expect(sendIntent(head, key('meta+alt'), false, true)).toBe('background');
  });
  test('⌥⌘↩ on an existing thread sends and opens a new thread, running or not', () => {
    expect(sendIntent(head, key('meta+alt'), false, false)).toBe('background');
    expect(sendIntent(head, key('meta+alt'), true, false)).toBe('background');
  });
  test('⌘↩ while a turn runs is the follow-up alternate; idle it is a plain send', () => {
    expect(sendIntent(head, key('meta'), true, false)).toBe('alternate');
    expect(sendIntent(head, key('meta'), false, false)).toBe('foreground');
  });
  test('Return, a stale gesture and ⇧⌘↩ are plain sends', () => {
    expect(sendIntent(head, key(''), true, false)).toBe('foreground');
    expect(sendIntent(head, key('meta+alt', 5000), false, false)).toBe('foreground');
    expect(sendIntent(head, key('meta+shift'), true, false)).toBe('foreground');
    expect(sendIntent(head, {}, false, true)).toBe('foreground');
  });
  test('a pointer press only takes the alternate', () => {
    expect(sendIntent(head, { modifiers: 'meta', source: 'pointer', ageMs: 5 }, true, false)).toBe('alternate');
    expect(sendIntent(head, { modifiers: 'meta+alt', source: 'pointer', ageMs: 5 }, false, false)).toBe('foreground');
    expect(sendIntent(head, { modifiers: 'meta', source: 'pointer', ageMs: 5 }, false, true)).toBe('foreground');
  });
  test('rebinding follows the server: a user rule added last wins', () => {
    const custom = { keybindings: [...arrOf(head), rule('composer.sendAndNewThread', { modKey: true, shiftKey: true, altKey: true }, { type: 'not', node: id('draftThreadRoute') })] };
    expect(sendIntent(custom, key('meta+alt+shift'), false, false)).toBe('background');
    const removed = { keybindings: arrOf(head).filter(entry => entry.command !== 'composer.sendAndNewThread') };
    expect(sendIntent(removed, key('meta+alt'), false, false)).toBe('foreground');
  });
  test('an unknown context blocks its chord instead of exposing an older rule', () => {
    const unknown = { keybindings: [...arrOf(head), rule('chat.new', { modKey: true, altKey: true }, id('somethingNew'))] };
    expect(chordCommand(unknown, 'Meta+Alt+Enter', { draftThreadRoute: false })).toBe('');
    expect(sendIntent(unknown, key('meta+alt'), false, false)).toBe('foreground');
  });
  test('a server with no keybindings falls back to the HEAD defaults', () => {
    expect(sendIntent({}, key('meta'), false, true)).toBe('background');
    expect(sendIntent({}, key('meta+alt'), false, false)).toBe('background');
  });
});

describe('send button chords', () => {
  test('gesture modifiers become aria-keyshortcuts chords', () => {
    expect(gestureChord('meta+control+alt+shift')).toBe('Meta+Control+Alt+Shift+Enter');
    expect(gestureChord('')).toBe('Enter');
  });
  test('the existing thread takes ⌥⌘↩; a draft takes ⌘↩ and ⌥⌘↩', () => {
    expect(sendChords(head, false, false)).toBe('Meta+Enter Meta+Alt+Enter');
    expect(sendChords(head, true, false)).toBe('Meta+Enter Meta+Alt+Enter');
    expect(sendChords(head, false, true)).toBe('Meta+Enter Meta+Alt+Enter');
    expect(sendChords({ keybindings: [] }, false, false)).toBe('Meta+Enter');
  });
});

function arrOf(config: Obj): Obj[] { return config.keybindings as Obj[]; }
