// Numbered lists nest and lift as T3 Code v0.1.6 composer-list-continuation.test.ts says (#17138, #27016).
import { describe, expect, test } from 'bun:test';
import { applyComposerEdit, type ComposerEdit } from '../../src/composer/composer-markdown-edit';
import { listContinuationForEnter, listIndentForTab, listOutdentForTab } from '../../src/composer/composer-list-continuation';
import { composerMarkdownKey } from '../../src/composer/composer-markdown-edit';
import { composerRichProjection } from '../../src/composer/composer-rich-projection';
import { composerChips } from '../../src/composer/composer-source-chips';

const apply = (value: string, edit: ComposerEdit | null) => applyComposerEdit(value, edit!);

describe('Enter on an empty nested item', () => {
  test('lifts it one level per press before leaving the list', () => {
    let value = '1. parent\n   1. child\n      1. ';
    for (const expected of ['1. parent\n   1. child\n   2. ', '1. parent\n   1. child\n2. ', '1. parent\n   1. child\n']) {
      value = apply(value, listContinuationForEnter(value, value.length, composerChips(value)));
      expect(value).toBe(expected);
    }
  });

  test('renumbers the parent list after lifting', () => {
    const value = '1. a\n   1. \n2. b', edit = listContinuationForEnter(value, value.indexOf('\n2.'), [])!;
    expect(apply(value, edit)).toBe('1. a\n2. \n3. b');
    expect(edit.selection.start).toBe('1. a\n2. '.length);
  });
});

describe('Tab on a list item', () => {
  test.each([
    ['1. parent\n2. child', '1. parent\n   1. child'],
    ['10. parent\n11. child', '10. parent\n    1. child'],
    ['1.\tparent\n2. child', '1.\tparent\n    1. child'],
    ['1. @"file\n   9. name"\n2. child', '1. @"file\n   9. name"\n   1. child'],
  ])('nests %j as a first child at its parent\'s content column, the caret on its text', (value, expected) => {
    const cursor = value.lastIndexOf('child') + 2, edit = listIndentForTab(value, cursor, cursor, composerChips(value))!;
    expect(apply(value, edit)).toBe(expected);
    expect(edit.selection.start).toBe(expected.lastIndexOf('child') + 2);
  });

  test('continues the numbering of an existing nested sibling', () => {
    const value = '1. parent\n   1. first\n2. second';
    expect(apply(value, listIndentForTab(value, value.length, value.length, []))).toBe('1. parent\n   1. first\n   2. second');
  });

  test('indents two spaces with no item above', () => {
    expect(apply('- a', listIndentForTab('- a', 3, 3, []))).toBe('  - a');
  });
});

describe('⇧Tab on a nested list item', () => {
  test.each(['- parent\n- child', '1. parent\n2. child', '  7. child'])('undoes Tab on %j and keeps the caret on its text', value => {
    const cursor = value.lastIndexOf('child') + 2;
    const nest = listIndentForTab(value, cursor, cursor, [])!, nested = apply(value, nest);
    const edit = listOutdentForTab(nested, nest.selection.start, nest.selection.start, [])!;
    expect(apply(nested, edit)).toBe(value);
    expect(edit.selection.start).toBe(cursor);
  });

  test.each([
    ['1. a\n   1. b\n2. c', '1. a\n2. b\n3. c'],
    ['- a\n  5. b', '- a\n5. b'],
  ])('outdents %j to %j and renumbers only within the parent\'s list', (value, expected) => {
    expect(apply(value, listOutdentForTab(value, value.indexOf('b'), value.indexOf('b'), []))).toBe(expected);
  });

  test('leaves a top-level item alone so ⇧Tab can still switch Build/Plan', () => {
    expect(listOutdentForTab('- item', 6, 6, [])).toBeNull();
  });

  test('the rich composer outdents too, and marks the lines ⇧Tab moves out', () => {
    const value = '1. a\n   1. b\n2. c', caret = value.indexOf('b');
    const outcome = composerMarkdownKey(value, { start: caret, end: caret }, { name: 'Tab', shift: true, command: false } as never);
    expect(outcome.kind === 'edit' ? applyComposerEdit(value, outcome.edit) : outcome.kind).toBe('1. a\n2. b\n3. c');
    expect(composerMarkdownKey('- item', { start: 6, end: 6 }, { name: 'Tab', shift: true, command: false } as never)).toEqual({ kind: 'default' });
    for (const styling of [true, false]) {
      const blocks = composerRichProjection({ owner: 'o', value, styling, literal: false } as never).blocks;
      expect(blocks.filter(block => block.kind === 'list').map(block => block.nested)).toEqual([false, true, false]);
    }
  });
});
