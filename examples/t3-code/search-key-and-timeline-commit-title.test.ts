// search-key-and-timeline-commit-title (T3 Code 1e2ecbd975, MIT; see LICENSE-T3). SK-2's guard: a real key reaches the
// clone's commands only through the hidden dispatch buttons `keyboardDispatch` makes (the session's key monitor presses
// the button whose `aria-keyshortcuts` matches, and the host files it as a menu key equivalent), so a reference binding
// without its button is a key a real keyboard loses. Every ⌘ binding of the reference's defaults (packages/shared/src/
// keybindings.ts DEFAULT_KEYBINDINGS, keybindingDefaults here) that holds in an open thread keeps its button and chord,
// from the composer and with nothing focused. SK-1's ⇧⌘F did reach its button through NSApplication.sendEvent; what
// took the real key is outside the app (the task record). TC-1: below.
import { describe, expect, test } from 'bun:test';
import { T3Client } from './client';
import { keyboardDispatch, type DispatchContext } from './keyboard-dispatch';
import { keybindingDefaults } from './keybinding-settings';
import { parseWhen } from './keybinding-view';

/** The server's resolved form of a default rule (the `shortcut` object and `whenAst` of ResolvedKeybindingRule). */
function resolved(rule: { key: string; command: string; when?: string }) {
  const tokens = rule.key.split('+');
  const key = tokens.pop() || '+';
  return { command: rule.command, shortcut: { key: key === '' ? '+' : key, modKey: tokens.includes('mod'), shiftKey: tokens.includes('shift'), altKey: tokens.includes('alt'), ctrlKey: tokens.includes('ctrl'), metaKey: false }, ...(rule.when ? { whenAst: parseWhen(rule.when) } : {}) };
}
function aria(rule: { key: string }): string {
  const tokens = rule.key.split('+');
  const key = tokens.pop()!;
  const named: Record<string, string> = { enter: 'Enter', arrowup: 'ArrowUp', arrowdown: 'ArrowDown' };
  return [...(tokens.includes('mod') ? ['Meta'] : []), ...(tokens.includes('alt') ? ['Alt'] : []), ...(tokens.includes('shift') ? ['Shift'] : []), named[key] ?? key].join('+');
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
const needsSurface = (when = '') => when.split(/\s*&&\s*/).includes('terminalFocus') || /previewFocus|usagePageOpen|modelPickerOpen|composerFocus/.test(when);
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
  test('SK-1: ⇧⌘F is projectSearch.toggle\'s button from the composer, with nothing focused and with the palette open', () => {
    for (const context of [thread, { ...thread, composerFocus: true, editableFocus: true }, { ...thread, paletteOpen: true, paletteMode: 'command' }]) {
      expect(keyboardDispatch(client(), [], '', '', context).filter(item => item.chord === 'Meta+Shift+f')).toEqual([expect.objectContaining({ command: 'projectSearch.toggle', kind: 'palette', target: 'content' })]);
    }
  });
});

// TC-1: CommitEvent (PullRequestTimelineTab.tsx) draws its title at rest. In the clone the macOS host presents a wide
// text's raster made off screen only for a selectable text (X82's pump), and no text under a button is one, so the
// title stayed blank until the hover colour redrew it. The row's texts sit beside the button, which lies over them.
describe('TC-1: the Timeline commit row keeps its title out of the button', () => {
  test('the button lies over the row and holds no text; the title keeps the reference tones', async () => {
    const lines = (await Bun.file(new URL('./pages-pr-timeline.contract', import.meta.url)).text()).split('\n');
    const start = lines.findIndex(line => line.trim() === 'when row.kind == "commit"');
    const end = lines.findIndex((line, index) => index > start && line.startsWith('    when '));
    const row = lines.slice(start + 1, end).filter(line => !line.trim().startsWith('//'));
    const indent = (line: string) => line.length - line.trimStart().length;
    const button = row.findIndex(line => line.trim().startsWith('button press=openCommit(row.id)'));
    expect(button).toBeGreaterThan(-1);
    // Last in the row, so nothing nests under it, and over the whole row.
    expect(button).toBe(row.length - 1);
    for (const attr of ['position="absolute"', 'left=0', 'top=0', 'width="100%"', 'height="100%"', 'aria-label=`View commit ${row.id}`', 'testId=`pull-request-timeline-commit-${row.sha}`']) expect(row[button]).toContain(attr);
    const title = row.find(line => line.trim().startsWith('text row.headline'))!;
    expect(indent(title)).toBeGreaterThan(indent(row[0]));
    expect(title).toContain('color=(over ? "light-dark(#1b4ed8, #346bf1)" : prtInk())');
    // The texts are the button's label's, not their own names.
    expect(row.find(line => line.includes('padding-top="0.375rem"'))).toContain('aria-hidden=true');
  });
});
