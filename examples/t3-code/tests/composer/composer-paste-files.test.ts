// The window's side of a drop, a paste and Attach files, run with Contract's action semantics (contract-action-fixture.ts):
// the composer's `paste` (onComposerPaste, shouldHandleComposerAttachmentPaste), the chat column's drop overlay
// (makeWorkspaceFileDropHandlers) and a legacy row's drop (handleThreadFileDrop).
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { contractComponent, value, type Env } from '../support/contract-action-fixture';
import { appPath } from '../support/app-files';

/** composer.contract's `fn` lines, as functions of their parameters (each sees the ones before it). */
function contractFns(file: string): Env {
  const fns: Env = {};
  for (const line of readFileSync(appPath(file), 'utf8').split('\n')) {
    const m = /^fn (\w+)\(([^)]*)\): \w+(?:\[\])? = (.+)$/.exec(line);
    if (!m) continue;
    const params = [...m[2]!.matchAll(/(\w+):/g)].map(p => p[1]!);
    fns[m[1]!] = (...args: unknown[]) => value(m[3]!, { ...fns, ...Object.fromEntries(params.map((p, i) => [p, args[i]])) });
  }
  return fns;
}
const fns = contractFns('composer.contract');
const takes = fns.composerPasteTakesFiles as (files: string[], text: string) => boolean;

describe('a paste into the composer (onComposerPaste)', () => {
  test('an image of any type, or files with no text, attach; text beside a plain file stays text, a Finder copy\'s names too', () => {
    expect(takes(['doc:/1/image.png'], '')).toBe(true);
    expect(takes(['doc:/1/scan.TIFF'], 'some caption')).toBe(true);
    expect(takes(['doc:/1/notes.txt'], '')).toBe(true);
    expect(takes(['doc:/1/notes.txt'], 'notes.txt')).toBe(false);
    expect(takes(['doc:/1/notes.txt'], 'hello world')).toBe(false);
    expect(takes([], '')).toBe(false);
  });

  test('a paste of files is prevented and attached, Paste as Text armed by its key; a text paste is the field\'s', () => {
    const card = contractComponent('composer.contract', 'ComposerCard'), state = card.state();
    const calls: unknown[][] = [];
    let now = 5000;
    const scope = { ...fns, data: { projectId: 'p1' }, performanceNow: () => now, composerKeyEvent: () => {}, onBlur: () => calls.push(['blur']),
      preventDefault: () => calls.push(['preventDefault']), attachFiles: (...args: unknown[]) => calls.push(['attachFiles', ...args]) };
    const image = [{ files: ['doc:/1/image.png'], text: '' }];
    card.call('pasted', state, scope, [{ files: [], text: 'hello' }]);
    expect(calls).toEqual([]);
    card.call('pasted', state, scope, image);
    expect(calls).toEqual([['preventDefault'], ['attachFiles', 'paste', ['doc:/1/image.png'], [], false, '']]);
    // isPasteAsTextShortcut: ⇧⌘V with no ⌥ or ⌃, by the key's character (event.key).
    const chord = (k: string, code: string, held: Record<string, boolean>) => ({ key: k, code, metaKey: false, shiftKey: false, ctrlKey: false, altKey: false, repeat: false, ...held });
    const pasteAsText = chord('V', 'KeyV', { metaKey: true, shiftKey: true });
    const armed = () => { const before = calls.length; card.call('pasted', state, scope, image); return (calls.slice(before).find(c => c[0] === 'attachFiles') ?? [])[4]; };
    card.call('richKey', state, scope, ['V', pasteAsText]);
    expect(armed()).toBe(true);
    // One paste consumes it (consumePasteAsTextShortcut).
    expect(armed()).toBe(false);
    for (const [k, e] of [['v', chord('v', 'KeyV', { metaKey: true })], ['V', chord('V', 'KeyV', { metaKey: true, shiftKey: true, ctrlKey: true })],
      ['v', chord('v', 'KeyV', { metaKey: true, shiftKey: true, altKey: true })], ['ㅍ', chord('ㅍ', 'KeyV', { metaKey: true, shiftKey: true })], ['B', chord('B', 'KeyB', { metaKey: true, shiftKey: true })]] as const) {
      card.call('richKey', state, scope, [k, e]);
      expect(armed()).toBe(false);
    }
    // Armed for one second (armPasteAsTextShortcut's deadline); another key in between leaves it armed.
    card.call('richKey', state, scope, ['V', pasteAsText]);
    now += 400;
    card.call('richKey', state, scope, ['a', chord('a', 'KeyA', {})]);
    now += 600;
    expect(armed()).toBe(true);
    card.call('richKey', state, scope, ['V', pasteAsText]);
    now += 1001;
    expect(armed()).toBe(false);
    // Leaving the composer disarms it, as the reference's window blur does, and tells the window.
    card.call('richKey', state, scope, ['V', pasteAsText]);
    card.call('richBlurred', state, scope, []);
    expect(calls.at(-1)).toEqual(['blur']);
    expect(armed()).toBe(false);
    // With no thread open (no project), a paste is the field's.
    const before = calls.length;
    card.call('pasted', state, { ...scope, data: { projectId: '' } }, [{ files: ['doc:/1/image.png'], text: '' }]);
    expect(calls.length).toBe(before);
  });
});

/** The overlay's `if` in ChatColumn's view, read over the column's state (its other derives need the whole snapshot). */
const overlayLine = readFileSync(appPath('app-main.contract'), 'utf8').split('\n').map(line => line.trim()).find(line => line.startsWith('if fileDrag'))!;
const overlay = (state: Env) => value(overlayLine.slice(3), state) as boolean;

describe('a drop on the chat column (makeWorkspaceFileDropHandlers)', () => {
  test('the overlay shows from dragenter until dragleave or the drop, which attaches the files', () => {
    // Only the drop's slot: the column's others hold shapes this fixture does not construct.
    const column = contractComponent('app-main.contract', 'ChatColumn'), state: Env = { fileDrag: false, overHeader: false };
    const calls: unknown[][] = [];
    const scope = { attachFiles: (...args: unknown[]) => calls.push(args) };
    column.call('fileDragEntered', state, scope, []);
    expect(state.fileDrag).toBe(true);
    column.call('fileDragLeft', state, scope, []);
    expect(state.fileDrag).toBe(false);
    column.call('fileDragEntered', state, scope, []);
    column.call('filesDropped', state, scope, [{ files: ['doc:/1/notes.txt', 'doc:/2/src'] }]);
    expect(state.fileDrag).toBe(false);
    // Over the header the overlay hides and a drop there attaches nothing (the reference's target is under the header).
    column.call('fileDragEntered', state, scope, []);
    column.call('headerDragged', state, scope, [true]);
    expect(overlay(state)).toBe(false);
    column.call('headerDragged', state, scope, [false]);
    expect(overlay(state)).toBe(true);
    column.call('headerDragged', state, scope, [true]);
    column.call('filesDropped', state, scope, [{ files: ['doc:/9/ignored.txt'] }]);
    expect([state.fileDrag, state.overHeader]).toEqual([false, false]);
    column.call('filesPicked', state, scope, [[{ path: 'app:/tmp/picked/1/a.txt', name: 'a.txt', type: 'text/plain', size: 1 }]]);
    expect(calls).toEqual([['drop', ['doc:/1/notes.txt', 'doc:/2/src'], [], false, ''], ['pick', [], [{ path: 'app:/tmp/picked/1/a.txt', name: 'a.txt', type: 'text/plain', size: 1 }], false, '']]);
  });
});

describe('a drop on a legacy row (handleThreadFileDrop)', () => {
  test('a drag rings the row; its drop names the row, whether it is the open thread\'s and its drop key', () => {
    const row = contractComponent('legacy-sidebar.contract', 'LegacyThread'), state = row.state();
    const calls: unknown[][] = [];
    const scope = { t: { id: 'thread-2', dropKey: 'fleet:local:thread-2', active: false }, page: '', isOpen: false, attachFiles: (...args: unknown[]) => calls.push(args) };
    row.call('fileDragged', state, scope, [true]);
    expect(state.fileDrag).toBe(true);
    row.call('filesDropped', state, scope, [{ files: ['doc:/1/notes.txt'] }]);
    expect(state.fileDrag).toBe(false);
    expect(calls).toEqual([['row', ['doc:/1/notes.txt'], [], false, '0\tthread-2\tfleet:local:thread-2']]);
  });
});

describe('a drop on a default sidebar row (Sidebar.tsx SidebarThreadRow)', () => {
  test('a drag rings the row; a drop names the row and whether it is open; a draft row takes none', () => {
    const row = contractComponent('sidebar-row.contract', 'ThreadRow'), state: Env = { fileDrag: false };
    const calls: unknown[][] = [];
    // The open thread's row; a parent standing in for an open subagent is highlighted but not it (resolveSidebarRowThreadId).
    const scope = { t: { id: 't-2', draft: false, routeThread: true }, page: '', isOpen: true, attachFiles: (...args: unknown[]) => calls.push(args) };
    row.call('fileDragged', state, scope, [true]);
    expect(state.fileDrag).toBe(true);
    row.call('filesDropped', state, scope, [{ files: ['doc:/1/notes.txt'] }]);
    expect(state.fileDrag).toBe(false);
    expect(calls).toEqual([['row', ['doc:/1/notes.txt'], [], false, '1\tt-2\t']]);
    row.call('filesDropped', state, { ...scope, t: { id: 't-1', draft: false, routeThread: false } }, [{ files: ['doc:/1/notes.txt'] }]);
    expect(calls.at(-1)).toEqual(['row', ['doc:/1/notes.txt'], [], false, '0\tt-1\t']);
    calls.length = 1;
    const draft = { ...scope, t: { id: 'd-1', draft: true } };
    row.call('fileDragged', state, draft, [true]);
    row.call('filesDropped', state, draft, [{ files: ['doc:/1/notes.txt'] }]);
    expect([state.fileDrag, calls.length]).toEqual([false, 1]);
  });
});
