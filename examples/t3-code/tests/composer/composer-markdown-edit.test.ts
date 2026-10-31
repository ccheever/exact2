import { describe, expect, test } from 'bun:test';
import { codeAt, composerMarkdown } from '../../src/composer/composer-markdown';
import { applyComposerEdit, composerCodeInput, composerMarkdownKey, type ComposerKeyResult } from '../../src/composer/composer-markdown-edit';

const key = (source: string, start: number, name: string, shift = false, end = start, command = false) => composerMarkdownKey(source, { start, end }, { name, shift, command });
function result(source: string, outcome: ComposerKeyResult): { source: string; start: number; end: number } {
  if (outcome.kind !== 'edit') throw new Error(`expected an edit, got ${outcome.kind}`);
  return { source: applyComposerEdit(source, outcome.edit), ...outcome.edit.selection };
}
describe('composer source edit rules', () => {
  test('Enter opens a fence with an empty body line, where typed text stays inside the fence', () => {
    const entered = result('```ts title=x', key('```ts title=x', 13, 'Enter'));
    expect(entered).toEqual({ source: '```ts title=x\n\n```', start: 14, end: 14 });
    // The field's own typing at the caret, without an app edit.
    const typed = entered.source.slice(0, 14) + 'const x = 1' + entered.source.slice(14);
    expect(codeAt(composerMarkdown(typed).blocks, { start: 25, end: 25 })?.close).toEqual({ start: 26, end: 29 });
    // A canonical empty fence from elsewhere: a pasted first line still adds the closing line's own newline.
    const canonical = '```ts title=x\n```';
    expect(result(canonical, composerCodeInput(canonical, { start: 14, end: 14 }, 'const x = 1'))).toEqual({ source: '```ts title=x\nconst x = 1\n```', start: 25, end: 25 });
    expect(result('~~~py', key('~~~py', 5, 'Enter'))).toEqual({ source: '~~~py\n\n~~~', start: 6, end: 6 });
    expect(key('- ```ts', 7, 'Enter')).toEqual({ kind: 'send' });
    expect(key('> ```ts', 7, 'Enter')).toEqual({ kind: 'send' });
  });
  test('code Enter keeps indentation, replaces selection, and Cmd/Ctrl+Enter sends', () => {
    const source = '```ts\n  foo\n```';
    expect(result(source, key(source, 11, 'Enter'))).toEqual({ source: '```ts\n  foo\n  \n```', start: 14, end: 14 });
    expect(result(source, key(source, 8, 'Enter', false, 11))).toEqual({ source: '```ts\n  \n  \n```', start: 11, end: 11 });
    expect(key(source, 11, 'Enter', false, 11, true)).toEqual({ kind: 'send' });
  });
  test('a closing fence or two trailing blank lines exits without losing body or fence metadata', () => {
    const source = '~~~py\nx\n~~~~  ';
    expect(result(source, key(source, source.length, 'Enter')).source).toBe(source + '\n');
    const blank = '```ts\nx\n \n\t\n```';
    expect(result(blank, key(blank, blank.indexOf('\n```'), 'Enter'))).toEqual({ source: '```ts\nx\n```\n', start: 12, end: 12 });
    const open = '```ts\nx\n\n';
    expect(result(open, key(open, open.length, 'Enter')).source).toBe('```ts\nx\n```\n');
  });
  test('an exit that leaves the code empty keeps its body line, as Enter does', () => {
    // A pasted closing line made the outer fence longer than the one the person typed (the logical fence).
    const pasted = '````ts\n```\n````';
    expect(result(pasted, composerMarkdownKey(pasted, { start: 10, end: 10 }, { name: 'Enter', shift: false, command: false }, '```')))
      .toEqual({ source: '```ts\n\n```\n', start: 11, end: 11 });
    const blank = '```ts\n\n\n\n```';
    expect(result(blank, key(blank, 8, 'Enter'))).toEqual({ source: '```ts\n\n```\n', start: 11, end: 11 });
    expect(codeAt(composerMarkdown('```ts\n\n```\n').blocks, { start: 6, end: 6 })?.close).toEqual({ start: 7, end: 10 });
  });
  test('a restored fence without a body line keeps its closing line whole while it is typed in', () => {
    const source = '```ts\n```';
    // Text there opens the body line instead of joining the closing fence.
    expect(result(source, composerCodeInput(source, { start: 6, end: 6 }, 'x'))).toEqual({ source: '```ts\nx\n```', start: 7, end: 7 });
    expect(result(source, composerCodeInput(source, { start: 6, end: 6 }, '```'))).toEqual({ source: '````ts\n```\n````', start: 10, end: 10 });
    // Backspace unwraps the empty code, its language line included; Tab and Enter open the body line.
    expect(result(source, key(source, 6, 'Backspace'))).toEqual({ source: '', start: 0, end: 0 });
    const between = 'before\n```ts\n```\nafter';
    expect(result(between, key(between, 13, 'Backspace'))).toEqual({ source: 'before\n\nafter', start: 7, end: 7 });
    expect(result(source, key(source, 6, 'Tab'))).toEqual({ source: '```ts\n  \n```', start: 8, end: 8 });
    expect(key(source, 6, 'Tab', true)).toEqual({ kind: 'handled' });
    expect(result(source, key(source, 6, 'Enter'))).toEqual({ source: '```ts\n\n\n```', start: 7, end: 7 });
    expect(key(source, 6, 'Enter', false, 6, true)).toEqual({ kind: 'send' });
    // Further into the closing line is ordinary text.
    expect(key(source, 7, 'Backspace')).toEqual({ kind: 'default' });
  });
  test('Backspace at body start unwraps every code line in one edit', () => {
    const source = 'before\n```ts\nx\ny\n```\nafter';
    expect(result(source, key(source, 13, 'Backspace'))).toEqual({ source: 'before\nx\ny\nafter', start: 7, end: 7 });
    expect(key(source, 14, 'Backspace')).toEqual({ kind: 'default' });
  });
  test('Tab indents two spaces, Shift+Tab preserves partial selections, and empty lines stay empty', () => {
    const source = '```\na\n\tb\n\nc\n```';
    expect(result(source, key(source, 4, 'Tab', false, 11))).toEqual({ source: '```\n  a\n  \tb\n\n  c\n```', start: 6, end: 17 });
    const indented = '```\n  a\n b\n\tc\n```';
    expect(result(indented, key(indented, 5, 'Tab', true, 13))).toEqual({ source: '```\na\nb\nc\n```', start: 4, end: 9 });
    expect(result('```\na\n```', key('```\na\n```', 5, 'Tab')).source).toBe('```\na  \n```');
  });
  test('a selection spanning identical fence occurrences never edits through their boundary', () => {
    const source = '```\nx\n```\n```\nx\n```';
    expect(key(source, 4, 'Tab', false, 15)).toEqual({ kind: 'default' });
  });
  test('Shift+Enter continues ordered, bullet, task and quote prefixes, and exits empty items', () => {
    for (const [source, expected] of [['003) item', '003) item\n004) '], ['  + item', '  + item\n  + '], ['- [x] task', '- [x] task\n- [ ] '], ['> quote', '> quote\n> ']]) {
      expect(result(source ?? '', key(source ?? '', source?.length ?? 0, 'Enter', true)).source).toBe(expected);
    }
    for (const source of ['- [ ] ', '002) ', '> ']) expect(result(source, key(source, source.length, 'Enter', true))).toEqual({ source: '', start: 0, end: 0 });
    expect(key('- item', 6, 'Enter')).toEqual({ kind: 'send' });
  });
  test('list Tab indents and keeps the source caret; ⇧Tab never outdents a list; ordinary Tab remains native', () => {
    expect(result('- item', key('- item', 6, 'Tab'))).toEqual({ source: '  - item', start: 8, end: 8 });
    expect(key('  - item', 8, 'Tab', true)).toEqual({ kind: 'default' });
    expect(key('- a\n  - b', 9, 'Tab', true)).toEqual({ kind: 'default' });
    expect(key('- item', 6, 'Tab', true)).toEqual({ kind: 'default' });
    expect(result('```\n  a\n```', key('```\n  a\n```', 7, 'Tab', true))).toEqual({ source: '```\na\n```', start: 5, end: 5 });
    expect(key('ordinary', 8, 'Tab')).toEqual({ kind: 'default' });
    expect(key('😀', 99, 'Enter')).toEqual({ kind: 'default' });
  });
  test('Bold and Italic belong to the Markdown field, not the source rules', () => {
    // format("composer", "bold"|"italic") edits at the field's selection (LLP 1045 D6).
    expect(key('😀 selected text', 3, 'Bold', false, 11, true)).toEqual({ kind: 'default' });
    expect(key('word', 0, 'Italic', false, 4, true)).toEqual({ kind: 'default' });
  });
});

describe('literal fence insertion in code', () => {
  test('a pasted closing line extends both outer fences in the same undoable source edit', () => {
    const source = '```ts\nx\n```';
    const outcome = composerCodeInput(source, { start: 7, end: 7 }, '\n```\ny');
    expect(outcome.kind).toBe('edit');
    if (outcome.kind !== 'edit') throw new Error('missing protected insertion');
    expect([outcome.edit.start, outcome.edit.end]).toEqual([0, source.length]);
    expect(result(source, outcome)).toEqual({ source: '````ts\nx\n```\ny\n````', start: 14, end: 14 });
  });
  test('longer and tilde closing runs choose a longer outer fence and preserve following prose', () => {
    const source = 'before\n~~~py title=x\nold body\n~~~~  \nafter';
    const start = source.indexOf('old'), end = source.indexOf('\n~~~~');
    const inserted = result(source, composerCodeInput(source, { start, end }, 'one\n~~~~~ \ntwo'));
    expect(inserted.source).toBe('before\n~~~~~~py title=x\none\n~~~~~ \ntwo\n~~~~~~\nafter');
    expect(inserted.source.slice(inserted.start, inserted.start + 1)).toBe('\n');
    expect(inserted.source.slice(0, inserted.start)).toEndWith('two');
    const parsed = composerMarkdown(inserted.source);
    expect(parsed.blocks.map(block => block.kind)).toEqual(['paragraph', 'code', 'paragraph']);
    const code = parsed.blocks[1];
    if (code?.kind !== 'code') throw new Error('missing code');
    expect(inserted.source.slice(code.body.start, code.body.end)).toBe('one\n~~~~~ \ntwo');
  });
  test('unclosed fences stay unclosed and different or indented runs do not force an extension', () => {
    const source = '```js\nx';
    expect(result(source, composerCodeInput(source, { start: 7, end: 7 }, '\n````\ny'))).toEqual({ source: '`````js\nx\n````\ny', start: 16, end: 16 });
    const ordinary = '```\nx\n```';
    expect(result(ordinary, composerCodeInput(ordinary, { start: 5, end: 5 }, '\n~~~\n  ```')).source).toBe('```\nx\n~~~\n  ```\n```');
  });
});
