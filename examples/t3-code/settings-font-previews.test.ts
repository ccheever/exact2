// Settings › Appearance typography samples (settings-font-previews.ts) against the reference
// T3 Code 1e2ecbd975 (MIT, see LICENSE-T3). The expected changes and spans were recorded from
// the reference's own dependencies: jsdiff 9.0.0 `diffWordsWithSpace` and @pierre/diffs
// 1.3.0-beta.10 `pushOrJoinSpan` ("word-alt"); the sample's classes from its
// `preloadPatchFile` HTML (pierre-light: export #D32A61, function #A631BE, formatUser #693ACF,
// user #D47628, the template's "<" #199F43, the comment #737373).
import { describe, expect, test } from 'bun:test';
import { readdirSync } from 'node:fs';
import { diffWordsWithSpace, fontDiffPreview, wordAltMarks } from './settings-font-previews';

const changes = (before: string, after: string) => diffWordsWithSpace(before, after).map(change => [change.added ? '+' : change.removed ? '-' : '=', change.value]);
const spans = (before: string, after: string) => wordAltMarks(before, after).map(side => side.map(segment => [segment.mark ? 1 : 0, segment.text]));
const DELETED = '  return user.name.toUpperCase();', ADDED = '  return `${user.name} <${user.email}>`; // 0O 1lI';

describe('jsdiff diffWordsWithSpace port', () => {
  test('the sample pair', () => {
    expect(changes(DELETED, ADDED)).toEqual([['=', '  return '], ['+', '`${'], ['=', 'user.name'], ['+', '} <${user'], ['=', '.'], ['-', 'toUpperCase()'], ['+', 'email}>`'], ['=', ';'], ['+', ' // 0O 1lI']]);
  });
  test('words, punctuation, identical, empty and Latin-1 lines', () => {
    expect(changes('foo bar baz', 'foo qux baz')).toEqual([['=', 'foo '], ['-', 'bar'], ['+', 'qux'], ['=', ' baz']]);
    expect(changes('const a = 1;', 'let a = 2;')).toEqual([['-', 'const'], ['+', 'let'], ['=', ' a = '], ['-', '1'], ['+', '2'], ['=', ';']]);
    expect(changes('return x', 'return x')).toEqual([['=', 'return x']]);
    expect(changes('', 'added')).toEqual([['+', 'added']]);
    expect(changes('café au lait', 'cafe au lait')).toEqual([['-', 'café'], ['+', 'cafe'], ['=', ' au lait']]);
  });
});

describe('word-alt emphasis (pushOrJoinSpan)', () => {
  test('a one-character neutral run joins the change before it; the last item stands alone', () => {
    expect(spans(DELETED, ADDED)).toEqual([
      [[0, '  return user.name.'], [1, 'toUpperCase();']],
      [[0, '  return '], [1, '`${'], [0, 'user.name'], [1, '} <${user.email}>`;'], [1, ' // 0O 1lI']],
    ]);
    expect(spans('const a = 1;', 'let a = 2;')).toEqual([[[1, 'const'], [0, ' a = '], [1, '1'], [0, ';']], [[1, 'let'], [0, ' a = '], [1, '2'], [0, ';']]]);
    expect(spans('', 'added')).toEqual([[], [[1, 'added']]]);
  });
});

describe('CodeFontPreview content', () => {
  const preview = fontDiffPreview('red-green');
  test('the header: modified src/formatUser.ts, -1 +1', () => {
    expect([preview.path, preview.status, preview.deletions, preview.additions, preview.scheme]).toEqual(['src/formatUser.ts', 'modified', 1, 1, 'red-green']);
    expect(fontDiffPreview('blue-orange').scheme).toBe('blue-orange');
  });
  test('unified rows: context 1, deletion 2, addition 2, context 3', () => {
    expect(preview.lines.map(line => [line.tone, line.number, line.segments.map(segment => segment.text).join('')])).toEqual([
      ['context', '1', 'export function formatUser(user: User) {'], ['deletion', '2', DELETED], ['addition', '2', ADDED], ['context', '3', '}'],
    ]);
  });
  test('TypeScript classes and the emphasis sit where the reference draws them', () => {
    const marked = (index: number) => preview.lines[index]!.segments.filter(segment => segment.mark).map(segment => segment.text).join('|');
    expect(preview.lines[0]!.segments.filter(segment => segment.syntax).map(segment => [segment.text, segment.syntax]).slice(0, 3)).toEqual([['export', 'kw'], ['function', 'decl'], ['formatUser', 'fn']]);
    expect(marked(1)).toBe('toUpperCase|();');
    expect(marked(2).replace(/\|/g, '')).toBe('`${} <${user.email}>`; // 0O 1lI');
    expect(preview.lines[2]!.segments.find(segment => segment.text === '// 0O 1lI')).toMatchObject({ syntax: 'com', mark: true });
    expect(preview.lines[2]!.segments.find(segment => segment.text === ' <')).toMatchObject({ syntax: 'str', mark: true });
    expect(preview.lines[1]!.segments.find(segment => segment.text === 'user')).toMatchObject({ syntax: 'var', mark: false });
  });
});

// The interface font size conversion (bf35a2d49, 6d87fd91a) turned unitless line heights
// (CSS multiples of the font size) into rem as if they were pixels: 1.625 became
// "0.1015625rem", a 1.6px line box that clipped Markdown table text to a sliver.
describe('contract line heights', () => {
  test('no line height is a pixel-sized rem', async () => {
    const dir = new URL('./', import.meta.url), found: string[] = [];
    for (const name of readdirSync(dir).filter(file => file.endsWith('.contract')).sort()) {
      (await Bun.file(new URL(name, dir)).text()).split('\n').forEach((line, index) => {
        for (const match of line.matchAll(/line-height="([0-9.]+)rem"/g)) if (Number(match[1]) < 0.5) found.push(`${name}:${index + 1} ${match[0]}`);
      });
    }
    expect(found).toEqual([]);
  });
});
