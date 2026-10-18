import { describe, expect, test } from 'bun:test';
import { composerLanguageEdit, composerTaskEdit, type ComposerReplacement } from '../../src/composer/composer-block-actions';

function apply(source: string, replacement: ComposerReplacement | null): string {
  if (!replacement) throw new Error('expected a source replacement');
  return source.slice(0, replacement.start) + replacement.text + source.slice(replacement.end);
}

describe('composer source-backed block actions', () => {
  test('language picker preserves whitespace, metadata, body and other occurrences', () => {
    const value = '``` js title=a  \nx\n```\n\n```py\ny\n```';
    const header = { start: 0, end: value.indexOf('\n') };
    expect(apply(value, composerLanguageEdit({ value, current: value, header, language: 'rust' })))
      .toBe('``` rust title=a  \nx\n```\n\n```py\ny\n```');
    expect(apply(value, composerLanguageEdit({ value, current: value, header, language: '' })))
      .toBe('``` text title=a  \nx\n```\n\n```py\ny\n```');
  });
  test('plain text clears a bare language and supports an unclosed tilde block', () => {
    const value = '~~~ts\n😀';
    expect(apply(value, composerLanguageEdit({ value, current: value, header: { start: 0, end: 5 }, language: '' })))
      .toBe('~~~\n😀');
  });
  test('picker refuses stale source, wrong occurrence, unknown language and unchanged values', () => {
    const value = '```rust\nx\n```', input = { value, current: value, header: { start: 0, end: 7 }, language: 'rust' };
    expect(composerLanguageEdit(input)).toBeNull();
    expect(composerLanguageEdit({ ...input, current: value + '!' })).toBeNull();
    expect(composerLanguageEdit({ ...input, header: { start: 1, end: 7 }, language: 'go' })).toBeNull();
    expect(composerLanguageEdit({ ...input, language: 'unknown' })).toBeNull();
  });
  test('task toggle changes exactly one source unit after indentation and tabs', () => {
    const value = '😀\n  -\t[X]  task\n- [ ] next', start = value.indexOf('  -'), end = value.indexOf('\n-');
    const input = { value, current: value, block: { start, end }, checked: false };
    const replacement = composerTaskEdit(input);
    expect(replacement).toEqual({ start: value.indexOf('X'), end: value.indexOf('X') + 1, text: ' ' });
    expect(apply(value, replacement)).toBe('😀\n  -\t[ ]  task\n- [ ] next');
    expect(composerTaskEdit({ ...input, checked: true })).toBeNull();
    expect(composerTaskEdit({ ...input, current: value + '!' })).toBeNull();
    expect(composerTaskEdit({ ...input, block: { start: start + 1, end } })).toBeNull();
  });
  test('literal task source in a code body cannot emit a checkbox edit', () => {
    const value = '```\n- [ ] task\n```';
    expect(composerTaskEdit({ value, current: value, block: { start: 4, end: 14 }, checked: true })).toBeNull();
  });
});
