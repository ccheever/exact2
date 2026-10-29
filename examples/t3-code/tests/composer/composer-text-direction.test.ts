import { describe, expect, test } from 'bun:test';
import { composerMarkdown } from '../../src/composer/composer-markdown';
import { composerChips } from '../../src/composer/composer-source-chips';
import { composerRichProjection } from '../../src/composer/composer-rich-projection';
import { composerParagraphDirs, composerTextDirections } from '../../src/composer/composer-text-direction';

/** One direction per source line, over the rich composer's blocks and chips. */
const directions = (value: string) => composerTextDirections(value, composerMarkdown(value).blocks, composerChips(value));

// composer-text-direction.test.ts at T3 Code 7c3b3b3605ef counts per block; here each block's lines take its direction.
describe('composerTextDirections', () => {
  test('does not count inline code toward a line\'s direction', () => {
    expect(directions('`useEffect` و `useLayoutEffect` في `React`')).toEqual(['rtl']);
  });

  test('gives a list one direction, from all of its items', () => {
    expect(directions('- Vitest\n- اختبر الدالة الجديدة\n- وأضف تعليق')).toEqual(['rtl', 'rtl', 'rtl']);
  });

  test('continues the line above until a line has letters', () => {
    expect(directions('اكتب الدالة\n\n42\nfix it')).toEqual(['rtl', 'rtl', 'rtl', 'ltr']);
  });

  test('keeps a code block left to right inside Arabic text', () => {
    expect(directions('اكتب الدالة\n```\n// احسب مجموع القيم هنا\n```\nثم اختبرها')).toEqual(['rtl', 'ltr', 'ltr', 'ltr', 'rtl']);
  });

  test('a code block leaves the line above it to the lines below that have no letters', () => {
    expect(directions('اكتب الدالة\n```\nsum()\n```\n42')).toEqual(['rtl', 'ltr', 'ltr', 'ltr', 'rtl']);
  });

  test('reads Arabic, Hebrew and English lines each by their own words', () => {
    expect(directions('fix the failing test\nاكتب دالة تجمع الأرقام\nתקן את הבאג\nStrictMode: يعيد الـ render مرتين في التطوير'))
      .toEqual(['ltr', 'rtl', 'rtl', 'rtl']);
    // A tie goes to the first word; one Arabic word in English stays English.
    expect(directions('React مكتبة\nمكتبة React\ntranslate the label مرحبا for the login screen')).toEqual(['ltr', 'rtl', 'ltr']);
  });

  test('a quote is one block while its marker stays the same', () => {
    expect(directions('> First line\n> שורה שנייה בעברית\n> שורה שלישית')).toEqual(['rtl', 'rtl', 'rtl']);
    expect(directions('> First line\n>שורה שנייה בעברית')).toEqual(['ltr', 'rtl']);
  });

  test('a list ends where its root changes kind, and nested items stay in it', () => {
    expect(directions('- one\n  - اثنان ثلاثة خمسة\n- four')).toEqual(['rtl', 'rtl', 'rtl']);
    expect(directions('- one two\n* اثنان')).toEqual(['ltr', 'rtl']);
    expect(directions('1. one two\n1) اثنان')).toEqual(['ltr', 'rtl']);
    expect(directions('- [ ] write the test\n- اثنان')).toEqual(['ltr', 'rtl']);
    // A blank line ends the list.
    expect(directions('- اثنان ثلاثة\n\n- one two three')).toEqual(['rtl', 'rtl', 'ltr']);
  });

  test('markers and chips are not words', () => {
    // A task's [x] is its marker, a heading's #s its prefix.
    expect(directions('- [x] اكتب الاختبار\n## مراجعة')).toEqual(['rtl', 'rtl']);
    // A mention counts as a space, as an atom node does.
    expect(directions('@README.md @CHANGELOG.md راجع الملف')).toEqual(['rtl']);
    expect(directions('@README.md @CHANGELOG.md ')).toEqual([null]);
  });

  test('a line no direction reaches has none, and takes the page\'s in the field\'s words', () => {
    expect(directions('')).toEqual([null]);
    expect(directions('42\nمرحبا')).toEqual([null, 'rtl']);
    expect(composerParagraphDirs('42\nمرحبا', composerMarkdown('42\nمرحبا').blocks)).toBe('ltr rtl');
  });
});

describe('the rich field\'s paragraphDirs', () => {
  const project = (value: string, styling = true, literal = false) => composerRichProjection({ value, owner: 'o', styling, literal });
  test('one word per source line', () => {
    expect(project('Hello\nمرحبا بالعالم\n```\nx\n```').dirs).toBe('ltr rtl ltr ltr ltr');
  });
  test('the plain and literal fields take none', () => {
    expect(project('مرحبا', false).dirs).toBe('');
    expect(project('مرحبا', true, true).dirs).toBe('');
  });
});
