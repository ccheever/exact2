// search-key-and-timeline-commit-title (T3 Code 1e2ecbd975, MIT; see LICENSE-T3). SK-2's guard: a real key reaches the
// clone's commands only through the hidden dispatch buttons `keyboardDispatch` makes (the session's key monitor presses
// the button whose `aria-keyshortcuts` matches, and the host files it as a menu key equivalent), so a reference binding
// without its button is a key a real keyboard loses. Every ⌘ binding of the reference's defaults (packages/shared/src/
// keybindings.ts DEFAULT_KEYBINDINGS, keybindingDefaults here) that holds in an open thread keeps its button and chord,
// from the composer and with nothing focused. The AppKit half, macos/tests/real-key-chords, sends each of them through
// NSApplication.sendEvent (a real key's path) past the app's own monitors, text view and menu bar to the host's monitor
// and menu item. SK-1's ⇧⌘F did reach its button that way; what took the real key is outside the app (the task
// record). TC-1: below.
import { describe, expect, test } from 'bun:test';
import { T3Client } from './client';
import { keyboardDispatch, type DispatchContext } from './keyboard-dispatch';
import { keybindingDefaults } from './keybinding-settings';
import { parseWhen } from './keybinding-view';

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
  Object.assign(c, { connection: 'connected', configLive: true, shellLive: true, threadLive: true, generation: 1, environmentId: 'env1', origin: 'http://127.0.0.1:1', scopes: ['orchestration:read', 'orchestration:operate'] });
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

describe('SK-2: every reference ⌘ binding of an open thread keeps its dispatch button', () => {
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
    const source = await Bun.file(new URL('./macos/tests/real-key-chords/main.swift', import.meta.url)).text();
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
  test('SK-1: ⇧⌘F is projectSearch.toggle\'s button from the composer, with nothing focused and with the palette open', () => {
    for (const context of [thread, { ...thread, composerFocus: true, editableFocus: true }, { ...thread, paletteOpen: true, paletteMode: 'command' }]) {
      expect(keyboardDispatch(client(), [], '', '', context).filter(item => item.chord === 'Meta+Shift+f')).toEqual([expect.objectContaining({ command: 'projectSearch.toggle', kind: 'palette', target: 'content' })]);
    }
  });
});

// TC-1: CommitEvent (PullRequestTimelineTab.tsx) draws its title at rest. In the clone the macOS host presented a wide
// text's raster made off screen only for a selectable text (X82's pump), and no text under a button is one, so the title
// stayed blank until the hover colour redrew it; TC-1 laid the button over texts beside it, and followups-1012 FU-2 kept
// the title selectable. Main #494 (X82, adopted in round 11) has the pump paint every shown text, so the row is the
// reference's one button holding its marker and texts again (adopt-main-fixes-r11 RT-2): nothing over the row, no text
// selectable or `aria-hidden`, the button's label the row's name.
describe('TC-1: the Timeline commit row is one button holding its texts, as the reference\'s CommitEvent', () => {
  test('the button holds the marker, the title, the sha and age and the counts; the title keeps the reference tones', async () => {
    const lines = (await Bun.file(new URL('./pages-pr-timeline.contract', import.meta.url)).text()).split('\n');
    const start = lines.findIndex(line => line.trim() === 'when row.kind == "commit"');
    const end = lines.findIndex((line, index) => index > start && line.startsWith('    when '));
    const row = lines.slice(start + 1, end).filter(line => !line.trim().startsWith('//'));
    const indent = (line: string) => line.length - line.trimStart().length;
    // The row's first line is the button, and every other line nests under it.
    expect(row[0]!.trim()).toStartWith('button press=openCommit(row.id) hover=hover aria-label=`View commit ${row.id}` position="relative"');
    for (const attr of ['width="100%"', 'padding-left="3rem"', 'text-align="left"', 'display="flex"', 'flex-direction="column"', 'testId=`pull-request-timeline-commit-${row.sha}`']) expect(row[0]).toContain(attr);
    expect(row[0]).not.toContain('position="absolute"');
    for (const line of row.slice(1)) expect(indent(line)).toBeGreaterThan(indent(row[0]!));
    expect(row.filter(line => line.trim().startsWith('button '))).toHaveLength(1);
    expect(row[1]!.trim()).toStartWith('PrdMarker(row=row, pinned=false, muted=false, icon="git-commit-horizontal")');
    const title = row.find(line => line.trim().startsWith('text row.headline'))!;
    expect(title).toContain('color=(over ? "light-dark(#1b4ed8, #346bf1)" : prtInk())');
    expect(title).toContain('testId=`pull-request-timeline-commit-title-${row.sha}`');
    for (const text of ['text row.sha', 'text row.age', 'text row.additions', 'text row.deletions']) expect(row.some(line => line.trim().startsWith(text))).toBe(true);
    // No stand-in is left: nothing hidden from accessibility, nothing made unselectable by hand, no help-tag opt-out.
    for (const attr of ['aria-hidden', 'user-select', 'title=""', 'z-index']) expect(row.filter(line => line.includes(attr))).toEqual([]);
  });
});
