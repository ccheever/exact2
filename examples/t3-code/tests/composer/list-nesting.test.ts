// Numbered lists nest and lift as T3 Code v0.1.5 composer-list-continuation.test.ts says (#17138).
import { describe, expect, test } from 'bun:test';
import { applyComposerEdit, type ComposerEdit } from '../../src/composer/composer-markdown-edit';
import { listContinuationForEnter, listIndentForTab } from '../../src/composer/composer-list-continuation';
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
