// The window's side of a drop, a paste and Attach files, run with Contract's action semantics (contract-action-fixture.ts):
// the composer's `paste` (onComposerPaste, shouldHandleComposerAttachmentPaste), the chat column's drop overlay
// (makeWorkspaceFileDropHandlers) and a sidebar row's drop (handleThreadFileDrop).
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { contractComponent, value, type Env } from '../support/contract-action-fixture';
import { appPath } from '../support/app-files';
import { pasteDisposition, utf8Length } from '../../src/composer/composer-editor-files';

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
Object.assign(fns, { min: Math.min, max: Math.max });
const takes = fns.composerPasteTakesFiles as (files: string[], text: string) => boolean;
const disposition = fns.composerPasteDisposition as (text: string, valueLength: number, start: number, end: number, limit: number) => string;

describe('a paste into the composer (onComposerPaste)', () => {
  test('an image of any type, or files with no text, attach; text beside a plain file stays text, a Finder copy\'s names too', () => {
    expect(takes(['doc:/1/image.png'], '')).toBe(true);
    expect(takes(['doc:/1/scan.TIFF'], 'some caption')).toBe(true);
    expect(takes(['doc:/1/notes.txt'], '')).toBe(true);
    expect(takes(['doc:/1/notes.txt'], 'notes.txt')).toBe(false);
    expect(takes(['doc:/1/notes.txt'], 'hello world')).toBe(false);
    expect(takes([], '')).toBe(false);
  });

  test('a large text paste folds into a file, at 32 KB in UTF-16 units or UTF-8 bytes or past the prompt limit (foldPastedText)', () => {
    const limit = 50 * 1024 * 1024;
    // Both: Contract decides from lengths, holding the rest ("measure") for pasteDisposition's bytes.
    const both = (text: string, valueLength: number, start: number, end: number, cap: number, expected: string, contract = expected) => {
      expect(pasteDisposition(text, valueLength, start, end, cap)).toBe(expected as never);
      expect(disposition(text, valueLength, start, end, cap)).toBe(contract);
    };
    both('', 0, 0, 0, limit, 'inline');
    both('x'.repeat(10_922), 0, 0, 0, limit, 'inline');
    both('x'.repeat(32_767), 0, 0, 0, limit, 'inline', 'measure');
    both('x'.repeat(32_768), 0, 0, 0, limit, 'fold');
    // 10,923 three-byte characters are 32,769 bytes; 10,922 are 32,766.
    both('가'.repeat(10_923), 0, 0, 0, limit, 'fold', 'measure');
    both('가'.repeat(10_922), 0, 0, 0, limit, 'inline');
    // An emoji is two units and four bytes.
    both('😀'.repeat(8_192), 0, 0, 0, limit, 'fold', 'measure');
    both('😀'.repeat(8_191), 0, 0, 0, limit, 'inline', 'measure');
    // Small, but past 120,000 units with the prompt (the selection it replaces counted out).
    both('abc', 119_998, 5, 5, limit, 'fold');
    both('abc', 119_998, 5, 7, limit, 'inline');
    // No staging (no project, a question that takes no files, a full message, no uploads): inline, or refused past the prompt limit.
    both('x'.repeat(40_000), 0, 0, 0, 0, 'inline');
    both('x'.repeat(130_000), 0, 0, 0, 0, 'too-large-message');
    // Over the server's file limit: the same, with its own refusal.
    both('x'.repeat(40_000), 0, 0, 0, 39_999, 'inline');
    both('x'.repeat(40_000), 0, 0, 0, 40_000, 'fold', 'measure');
    both('x'.repeat(40_000), 0, 0, 0, 120_000, 'fold');
    both('x'.repeat(130_000), 0, 0, 0, 100_000, 'too-large-attach');
    // A literal answer (-1) keeps every paste inline (isLiteralPendingAnswer).
    both('x'.repeat(130_000), 0, 0, 0, -1, 'inline');
  });

  test('lengths decide where they can and agree with the byte count; an unpaired surrogate is three bytes, as TextEncoder writes U+FFFD', () => {
    const pieces = ['a', 'é', '가', '😀', '\n', '\uD800', '\uDFFF'];
    let seed = 7;
    const random = () => (seed = (seed * 1_103_515_245 + 12_345) % 2_147_483_648) / 2_147_483_648;
    for (let n = 0; n < 400; n++) {
      const parts: string[] = [], size = Math.floor(random() * 50_000);
      while (parts.length < size / 4) parts.push(pieces[Math.floor(random() * pieces.length)]!.repeat(1 + Math.floor(random() * 8)));
      const text = parts.join(''), valueLength = Math.floor(random() * 100_000), start = Math.floor(random() * valueLength), end = start + Math.floor(random() * (valueLength - start));
      const cap = [0, -1, 40_000, 90_000, 50 * 1024 * 1024][Math.floor(random() * 5)]!;
      const held = disposition(text, valueLength, start, end, cap);
      if (held !== 'measure') expect(held).toBe(pasteDisposition(text, valueLength, start, end, cap));
    }
    expect(utf8Length('\uD800')).toBe(3);
    expect(utf8Length('a\uDFFFb😀')).toBe(9);
    expect(() => disposition('\uD800'.repeat(12_000), 0, 0, 0, 50 * 1024 * 1024)).not.toThrow();
    expect(pasteDisposition('\uD800'.repeat(11_000), 0, 0, 0, 50 * 1024 * 1024)).toBe('fold');
  });

  test('a paste of files is prevented and attached, Paste as Text armed by its key; a text paste may fold unless armed', () => {
    // The field's events reach the window's editing engine (composer-members.contract), which keeps where a held paste
    // goes and arms Paste as Text; the card's paste reads them.
    const card = contractComponent('composer.contract', 'ComposerCard'), state = card.state();
    const root = contractComponent('app.contract', 'T3Code');
    const engine: Env = { composerPasteValue: '', composerPasteStart: 0, composerPasteEnd: 0, composerPasteAsTextUntil: 0 };
    const calls: unknown[][] = [];
    let now = 5000;
    const rootScope = () => ({ performanceNow: () => now });
    const key = (k: string, e: Env) => root.call('composerPasteKey', engine, rootScope(), [k, e]);
    const moved = (e: Env) => root.call('composerPasteAt', engine, rootScope(), [e]);
    const base = { ...fns, data: { projectId: 'p1', snapshotOwner: 'snap' }, richComposerOwner: 'field', text: 'ab', editor: { foldLimit: 50 * 1024 * 1024 },
      round: Math.round, performanceNow: () => now, onBlur: () => calls.push(['blur']), composerPasteTaken: () => root.call('composerPasteTaken', engine, rootScope(), []),
      preventDefault: () => calls.push(['preventDefault']), attachFiles: (...args: unknown[]) => calls.push(['attachFiles', ...args]) };
    const scope = () => ({ ...base, ...engine });
    const paste = (e: Env, extra: Env = {}) => card.call('pasted', state, { ...scope(), ...extra }, [e]);
    const image = { files: ['doc:/1/image.png'], text: '' };
    paste({ files: [], text: 'hello' });
    expect(calls).toEqual([]);
    // A large paste is held and goes to foldPaste through attachFiles' queue: its record, the field's value and the text.
    // The field has reported no selection yet, so the paste goes at its end.
    const large = 'x'.repeat(40_000);
    paste({ files: [], text: large });
    expect(calls).toEqual([['preventDefault'], ['attachFiles', 'fold', [`fold\tsnap\tfield\t2\t2\t${5000 * 1000}`, 'ab', large], [], false, '']]);
    calls.length = 0;
    moved({ value: 'ab', selectionStart: 0, selectionEnd: 1 });
    paste({ files: [], text: large });
    expect((calls[1]![2] as string[])[0]).toBe(`fold\tsnap\tfield\t0\t1\t${5000 * 1000}`);
    calls.length = 0;
    paste(image);
    expect(calls).toEqual([['preventDefault'], ['attachFiles', 'paste', ['doc:/1/image.png'], [], false, '']]);
    // isPasteAsTextShortcut: ⇧⌘V with no ⌥ or ⌃, by the key's character (event.key).
    const chord = (k: string, code: string, held: Record<string, boolean>) => ({ key: k, code, metaKey: false, shiftKey: false, ctrlKey: false, altKey: false, repeat: false, ...held });
    const pasteAsText = chord('V', 'KeyV', { metaKey: true, shiftKey: true });
    const armed = () => { const before = calls.length; paste(image); return (calls.slice(before).find(c => c[0] === 'attachFiles') ?? [])[4]; };
    key('V', pasteAsText);
    expect(armed()).toBe(true);
    // One paste consumes it (consumePasteAsTextShortcut).
    expect(armed()).toBe(false);
    for (const [k, e] of [['v', chord('v', 'KeyV', { metaKey: true })], ['V', chord('V', 'KeyV', { metaKey: true, shiftKey: true, ctrlKey: true })],
      ['v', chord('v', 'KeyV', { metaKey: true, shiftKey: true, altKey: true })], ['ㅍ', chord('ㅍ', 'KeyV', { metaKey: true, shiftKey: true })], ['B', chord('B', 'KeyB', { metaKey: true, shiftKey: true })]] as const) {
      key(k, e);
      expect(armed()).toBe(false);
    }
    // Armed for one second (armPasteAsTextShortcut's deadline); another key in between leaves it armed.
    key('V', pasteAsText);
    now += 400;
    key('a', chord('a', 'KeyA', {}));
    now += 600;
    expect(armed()).toBe(true);
    key('V', pasteAsText);
    now += 1001;
    expect(armed()).toBe(false);
    // Paste as Text keeps its text inline however large (bypassAutoAttachment): no fold is asked for.
    key('V', pasteAsText);
    const text = calls.length;
    paste({ files: [], text: 'x'.repeat(40_000) });
    expect(calls.length).toBe(text);
    paste({ files: [], text: 'x'.repeat(40_000) });
    expect(calls.at(-1)?.[1]).toBe('fold');
    // Leaving the composer disarms it, as the reference's window blur does.
    key('V', pasteAsText);
    root.call('composerBlur', engine, { ...rootScope(), fieldRich: { mode: 'rich' } }, []);
    expect(armed()).toBe(false);
    // With no thread open (no project), a paste is the field's.
    const before = calls.length;
    paste({ files: ['doc:/1/image.png'], text: '' }, { data: { projectId: '' } });
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
