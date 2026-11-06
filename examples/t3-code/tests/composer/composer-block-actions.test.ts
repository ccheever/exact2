import { describe, expect, test } from 'bun:test';
import { composerLanguageEdit, type ComposerReplacement } from '../../src/composer/composer-block-actions';

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
});
