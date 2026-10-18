// T3 Code 1e2ecbd975 (MIT; see LICENSE-T3). The guard: a real key reaches the
// clone's commands only through the hidden dispatch buttons `keyboardDispatch` makes (the session's key monitor presses
// the button whose `aria-keyshortcuts` matches, and the host files it as a menu key equivalent), so a reference binding
// without its button is a key a real keyboard loses. Every ⌘ binding of the reference's defaults (packages/shared/src/
// keybindings.ts DEFAULT_KEYBINDINGS, keybindingDefaults here) that holds in an open thread keeps its button and chord,
// from the composer and with nothing focused. The AppKit half, macos/tests/real-key-chords, sends each of them through
// NSApplication.sendEvent (a real key's path) past the app's own monitors, text view and menu bar to the host's monitor
// and menu item. ⇧⌘F did reach its button that way; what took the real key is outside the app. The Timeline commit
// row: below.
import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { keyboardDispatch, type DispatchContext } from '../../src/shell/keyboard-dispatch';
import { keybindingDefaults } from '../../src/settings/keybinding-settings';
import { parseWhen } from '../../src/settings/keybinding-view';
import { appPath } from '../support/app-files';

/** The server's resolved form of a default rule (the `shortcut` object and `whenAst` of ResolvedKeybindingRule). */
function resolved(rule: { key: string; command: string; when?: string }) {
  const tokens = rule.key.split('+');
  const key = tokens.pop() || '+';
  return { command: rule.command, shortcut: { key, modKey: tokens.includes('mod'), shiftKey: tokens.includes('shift'), altKey: tokens.includes('alt'), ctrlKey: tokens.includes('ctrl'), metaKey: false }, ...(rule.when ? { whenAst: parseWhen(rule.when) } : {}) };
}
function aria(rule: { key: string }): string {
  const tokens = rule.key.split('+');
  const key = tokens.pop() || '+';
  // keyboard-dispatch.ts NAMED's spelling: `mod++` is Meta+Plus.
  const named: Record<string, string> = { enter: 'Enter', arrowup: 'ArrowUp', arrowdown: 'ArrowDown', '+': 'Plus' };
  return [...(tokens.includes('mod') ? ['Meta'] : []), ...(tokens.includes('alt') ? ['Alt'] : []), ...(tokens.includes('shift') ? ['Shift'] : []), named[key] ?? key].join('+');
}
/** The string literals of the Swift array that starts with `opening`, up to its matching `]`: a bracket or a `//` inside
 * a literal, an escaped quote, a comment between elements and a nested array are each read as Swift reads them. */
function swiftStrings(source: string, opening: string): string[] {
  const start = source.indexOf(opening);
  if (start < 0) throw new Error(`no ${opening}`);
  const strings: string[] = [];
  for (let i = start + opening.length, depth = 1; i < source.length; i++) {
    const c = source[i]!;
    if (c === '"') {
      let text = '';
      for (i++; source[i] !== '"'; i++) text += source[i] === '\\' ? source[++i] : source[i];
      strings.push(text);
    } else if (source.startsWith('//', i)) i = source.indexOf('\n', i);
    else if (c === '[') depth++;
    else if (c === ']' && --depth === 0) return strings;
  }
  throw new Error(`${opening} is not closed`);
}
function client(): T3Client {
  const c = new T3Client();
  Object.assign(c, { connection: 'connected', configLive: true, shellLive: true, threadLive: true, generation: 1, environmentId: 'env1', origin: 'http://127.0.0.1:1', scopes: ['orchestration:read', 'orchestration:operate', 'filesystem:read', 'terminal:read', 'terminal:operate', 'preview:operate'] });
  c.config = { environment: { label: 'Local', capabilities: { threadPinning: true } }, keybindings: keybindingDefaults.map(resolved), settings: {}, scratchWorkspaceRoot: '/scratch', availableEditors: ['vscode'] };
  c.shell = { sequence: 1, projects: [{ id: 'p1', title: 'Widgets', workspaceRoot: '/repo' }],
    threads: [{ id: 't1', projectId: 'p1', title: 'One', updatedAt: '2026-10-03T01:00:00.000Z' }, { id: 't2', projectId: 'p1', title: 'Two', updatedAt: '2026-10-03T00:00:00.000Z' }] };
  c.projectId = 'p1'; c.threadId = 't1';
  return c;
}
const thread: DispatchContext = { composerFocus: false, editableFocus: false, turnRunning: false, modelPickerOpen: false, draftThreadRoute: false, modalOpen: false, settingsOpen: false, diffOpen: false, prNumber: '168' };
// Commands a visible control or the composer answers (keyboard-dispatch.ts's header, keyboardSettings,
// T3Composer, T3ComposerQueueKey, the context strip), and rules whose `when` needs a focused terminal or
// preview, an open model picker or the Usage page, none of which an open thread has here.
const elsewhere = new Set(['sidebar.toggle', 'modelPicker.toggle', 'thread.steerQueuedMessage', 'thread.editQueuedMessage', 'editor.openFavorite']);
// A `when` is a conjunction (`a && !b`); a rule needs the surface only when a token is that fact itself, so a negated
// `!composerFocus` or `!previewFocus` holds in an open thread and keeps its rule.
const surfaces = ['terminalFocus', 'previewFocus', 'usagePageOpen', 'modelPickerOpen', 'composerFocus'];
const needsSurface = (when = '') => when.split(/\s*&&\s*/).some(token => surfaces.includes(token));
// Commands with no target in this fixture: Undo needs the sidebar's live notice, Back and Forward a history,
// Previous a thread above the first, Thread 3-9 more threads.
const stateful = (command: string) => ['thread.undo', 'thread.previous', 'navigation.back', 'navigation.forward'].includes(command) || /^thread\.jump\.[3-9]$/.test(command);
const main = keybindingDefaults.filter(rule => rule.key.includes('mod+') && !elsewhere.has(rule.command) && !rule.command.startsWith('composer.') && !needsSurface(rule.when));

describe('Every reference ⌘ binding of an open thread keeps its dispatch button', () => {
  for (const [label, context] of [['nothing focused', thread], ['the composer focused', { ...thread, composerFocus: true, editableFocus: true }]] as const) {
    test(label, () => {
      const items = keyboardDispatch(client(), [{ id: 't1', section: 'active', selected: true }, { id: 't2', section: 'active' }], '', '', context);
      const missing = main.filter(rule => !stateful(rule.command) && !items.some(item => item.command === rule.command && item.chord.toLowerCase() === aria(rule).toLowerCase()));
      expect(missing.map(rule => `${rule.key} ${rule.command}`)).toEqual([]);
    });
  }
  // macos/tests/real-key-chords sends every such chord through NSApplication.sendEvent past the app's own key monitors
  // and menu bar to the host's monitor and menu item: its list is every ⌘ rule that holds in an open thread, the
  // composer's own send chords aside.
  test('the AppKit guard sends every reference ⌘ chord of an open thread', async () => {
    const source = await Bun.file(appPath('macos/tests/real-key-chords/main.swift')).text();
    const swift = swiftStrings(source, 'let referenceChords = [');
    const reference = [...new Set(keybindingDefaults.filter(rule => rule.key.includes('mod+') && !needsSurface(rule.when)).map(rule => rule.key))];
    expect([...swift].sort()).toEqual([...reference].sort());
  });
  test('a negated surface keeps its rule; a chord on + is Meta+Plus', () => {
    expect([needsSurface('!composerFocus'), needsSurface('!previewFocus && !terminalFocus'), needsSurface('!terminalFocus && !editableFocus')]).toEqual([false, false, false]);
    expect([needsSurface('composerFocus && !draftThreadRoute'), needsSurface('modelPickerOpen && isDesktop'), needsSurface('terminalFocus')]).toEqual([true, true, true]);
    expect(main.some(rule => rule.command === 'thread.undo')).toBe(true); // `!terminalFocus && !editableFocus`
    expect([aria({ key: 'mod++' }), aria({ key: 'mod+shift+[' }), aria({ key: 'mod+alt+shift+a' })]).toEqual(['Meta+Plus', 'Meta+Shift+[', 'Meta+Alt+Shift+a']);
    expect(swiftStrings('let xs = ["a]", "b\\"c", // "d"\n "e"] + ["f"]', 'let xs = [')).toEqual(['a]', 'b"c', 'e']);
  });
  test('⇧⌘F is projectSearch.toggle\'s button from the composer, with nothing focused and with the palette open', () => {
    for (const context of [thread, { ...thread, composerFocus: true, editableFocus: true }, { ...thread, paletteOpen: true, paletteMode: 'command' }]) {
      expect(keyboardDispatch(client(), [], '', '', context).filter(item => item.chord === 'Meta+Shift+f')).toEqual([expect.objectContaining({ command: 'projectSearch.toggle', kind: 'palette', target: 'content' })]);
    }
  });
});
