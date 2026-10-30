import { expect, test } from 'bun:test';
import { mobileFileMarkdownBlocks } from './file-markdown';
import { mobileMarkdownBlocks } from './markdown-blocks';
import { contractFns } from '../tests/support/contract-action-fixture';

test('file preview projects isolated plain headings and preserves fenced code identity and bytes', () => {
  const body = '# Title\n\nParagraph with `code`.\n\n## Section\n\n```ts\nconst value = 1;\n\n```\n\nAfter.';
  const blocks = mobileFileMarkdownBlocks(body, false);
  expect(blocks.filter(block => block.kind === 'heading').map(block => [block.level, block.text]))
    .toEqual([[1, 'Title'], [2, 'Section']]);
  expect(blocks.find(block => block.kind === 'code'))
    .toEqual({ ...mobileMarkdownBlocks(body, false).find(block => block.kind === 'code'), level: 0, columns: 0, rows: [] });
  expect(new Set(blocks.map(block => block.id)).size).toBe(blocks.length);
  expect(blocks.filter(block => block.kind === 'markdown').map(block => block.text).join(''))
    .toContain('Paragraph with `code`.');
  expect(mobileMarkdownBlocks('# Title', false)[0]?.kind).toBe('markdown');
});

test('all six heading levels normalize preview newlines without changing the input', () => {
  const body = [1, 2, 3, 4, 5, 6].map(level => `${'#'.repeat(level)} Heading ${level}`).join('\r\n\r\n');
  const headings = mobileFileMarkdownBlocks(body, true);
  expect(headings.map(block => [block.kind, block.level, block.text])).toEqual([
    ['heading', 1, 'Heading 1'], ['heading', 2, 'Heading 2'], ['heading', 3, 'Heading 3'],
    ['heading', 4, 'Heading 4'], ['heading', 5, 'Heading 5'], ['heading', 6, 'Heading 6'],
  ]);
  expect(body).toContain('\r\n');
});

test.each([
  '# ~~Deprecated~~', '# Visit https://example.test', '# Visit http://example.test',
  '# **Strong**', '# A `code` heading', '# A [link](https://example.test)', '# A \\*literal',
  '# A &amp; B', '# Closing hashes #', '# Fragment #name', '####### Too deep', '# ',
  'Paragraph\n# Adjacent', '# Adjacent\nParagraph', '  # Indented', '# Title\n\n    code',
  '# Title\n\n- item', '# Title\n\n1. item', '# Title\n\n> quote', '# Title\n\n<div>html</div>',
  '[link][target]\n\n# Title\n\n[target]: https://example.test',
  '# Title\n\n```ts\n# Inside an unfinished fence', '# Title\n\n~~~\n# Inside an unfinished fence',
])('unsupported heading context keeps the existing complete prose projection: %s', body => {
  expect(mobileFileMarkdownBlocks(body, false)).toEqual(mobileMarkdownBlocks(body, false).map(block => ({ ...block, level: 0, columns: 0, rows: [] })));
});

test('heading sizes follow the pinned reference at default, larger and minimum sizes', () => {
  const functions = contractFns('mobile/file-view.contract', { max: Math.max, round: Math.round });
  expect([1, 2, 3, 4, 5, 6].map(level => functions.fileMarkdownHeadingSize(level, 16))).toEqual([21, 19, 17, 15, 15, 15]);
  expect([1, 2, 3, 4, 5, 6].map(level => functions.fileMarkdownHeadingSize(level, 24))).toEqual([32, 29, 26, 23, 23, 23]);
  expect([1, 2, 3, 4, 5, 6].map(level => functions.fileMarkdownHeadingSize(level, 8))).toEqual([16, 14, 13, 12, 12, 12]);
});

test('preview heading slicing retains non-BMP prose and heading text', () => {
  const blocks = mobileFileMarkdownBlocks('😀 Intro\n\n# Héllo 👋\n\nTail 😀', false);
  expect(blocks.filter(block => block.kind === 'heading').map(block => block.text)).toEqual(['Héllo 👋']);
  expect(blocks[0]?.text).toBe('😀 Intro\n\n');
  expect(blocks.at(-1)?.text).toBe('\nTail 😀');
});

test('plain preview prose folds soft breaks while retaining blank separators and terminal whitespace', () => {
  const body = 'One \r\nline.\r\n \r\nTwo\r\nlines. \r\n';
  expect(mobileFileMarkdownBlocks(body, false)[0]?.text).toBe('One line.\n \nTwo lines. \n');
  expect(body).toBe('One \r\nline.\r\n \r\nTwo\r\nlines. \r\n');
  expect(mobileFileMarkdownBlocks('你好 👋\n世界。\n\n`code_id`\ncontinues.', true)[0]?.text)
    .toBe('你好 👋 世界。\n\n`code_id` continues.');
});

test('prose folding keeps source-based heading identity and complete code records', () => {
  const body = '# First\n\nOne\nline.\n\n## Second\n\nMore\ntext\n\n```ts\nconst x = 1;\n\n```';
  const blocks = mobileFileMarkdownBlocks(body, false);
  expect(blocks.filter(block => block.kind === 'heading').map(block => [block.id, block.text]))
    .toEqual([['0:heading:0', 'First'], [`0:heading:${body.indexOf('## Second')}`, 'Second']]);
  expect(blocks.filter(block => block.kind === 'markdown').map(block => block.text))
    .toEqual(['\nOne line.\n\n', '\nMore text\n\n']);
  expect(blocks.find(block => block.kind === 'code'))
    .toEqual({ ...mobileMarkdownBlocks(body, false).find(block => block.kind === 'code'), level: 0, columns: 0, rows: [] });
});

test.each([
  'Hard  \nbreak', 'Hard\\\nbreak', 'Tab\t\nbreak', 'A `multiline\ncode` span',
  'A `multiline\n\ncode` span', 'A ``longer code``\nspan', 'A `unmatched\nspan',
  'Text\n===', 'Text\n---', 'Text\n***', 'Text\n1.', 'Text\n1)', 'Text\n1. item',
  'Text\n- item', 'Text\n> quote', 'Text\n    indented', 'Text\n# Adjacent heading',
  '[link][target]\n\n[target]: https://example.test',
  '[^note]: first\nsecond', 'An <em>HTML</em>\nparagraph', '<!--\nPlain\nwrapped\n-->',
  'A **bold**\nparagraph', 'A ~~strike~~\nparagraph', 'A [link](https://example.test)\nparagraph',
  'A &amp; entity\nparagraph', 'Visit https://example.test\ntoday', 'Math $x$\ncontinues',
  'One\u2028line\ncontinues', '```ts\nPlain\nwrapped', '~~~\nPlain\nwrapped',
])('unsupported prose keeps the existing complete Markdown input: %s', body => {
  expect(mobileFileMarkdownBlocks(body, false))
    .toEqual(mobileMarkdownBlocks(body, false).map(block => ({ ...block, level: 0, columns: 0, rows: [] })));
});

test('an unsupported fragment preserves other prose in the same original Markdown chunk', () => {
  const blocks = mobileFileMarkdownBlocks('Plain\nwrapped.\n\n# Heading\n\nA `span\n\nacross paragraphs`.', false);
  expect(blocks.filter(block => block.kind === 'markdown').map(block => block.text))
    .toEqual(['Plain\nwrapped.\n\n', '\nA `span\n\nacross paragraphs`.']);
});

test('file tables preserve headings, numeric values, inline code and accepted code records', () => {
  const body = '# Files\n\nIntro.\n\n| File | Count |\n| --- | --- |\n| `app.contract` | 12.5 |\n\nAfter.\n\n```sh\npwd\n\n```';
  const blocks = mobileFileMarkdownBlocks(body, false), table = blocks.find(block => block.kind === 'table');
  expect(blocks.map(block => block.kind)).toEqual(['heading', 'markdown', 'table', 'markdown', 'code']);
  expect(blocks[0]).toMatchObject({ id: '0:heading:0', level: 1, text: 'Files' });
  expect(table).toMatchObject({ id: `0:table:${body.indexOf('| File')}`, columns: 2,
    rows: [{ header: true }, { header: false }] });
  expect(table?.rows[0]?.cells.map(cell => cell.runs.map(run => run.text).join(''))).toEqual(['File', 'Count']);
  expect(table?.rows[1]?.cells[0]).toMatchObject({ numeric: false, runs: [{ kind: 'code', text: 'app.contract' }] });
  expect(table?.rows[1]?.cells[1]).toMatchObject({ numeric: true, runs: [{ kind: 'text', text: '12.5' }] });
  expect(blocks.find(block => block.kind === 'code'))
    .toEqual({ ...mobileMarkdownBlocks(body, false).find(block => block.kind === 'code'), level: 0, columns: 0, rows: [] });
  expect(new Set(blocks.map(block => block.id)).size).toBe(blocks.length);
});

test('source interval binding handles repeated text and preserves previews when code parsers disagree', () => {
  const body = 'Same\n\n```sh\nSame\n```\n\n| A |\n| - |\n| B |\n\nSame';
  const blocks = mobileFileMarkdownBlocks(body.replaceAll('\n', '\r\n'), true);
  expect(blocks.filter(block => block.kind === 'table')).toHaveLength(1);
  expect(blocks.find(block => block.kind === 'code')).toMatchObject({ id: '1', text: 'Same', language: 'sh' });
  expect(blocks.filter(block => block.kind === 'markdown').map(block => block.text.trim())).toEqual(['Same', 'Same']);
  const indented = '    code\n\n| A |\n| - |\n| B |';
  expect(mobileFileMarkdownBlocks(indented, false))
    .toEqual(mobileMarkdownBlocks(indented, false).map(block => ({ ...block, level: 0, columns: 0, rows: [] })));
});

test('an unsupported table chunk keeps its existing heading projection while other chunks gain tables', () => {
  const unsupported = '# Later\n\n| A |\n| - |\n| **bold *nested*** |';
  const blocks = mobileFileMarkdownBlocks('| Good |\n| - |\n| value |\n\n```sh\npwd\n```\n\n' + unsupported, false);
  expect(blocks.filter(block => block.kind === 'table')).toHaveLength(1);
  expect(blocks.find(block => block.kind === 'heading')).toMatchObject({ level: 1, text: 'Later' });
  expect(blocks.at(-1)).toMatchObject({ kind: 'markdown', text: '\n| A |\n| - |\n| **bold *nested*** |' });
});

test('strong table projection keeps CRLF input, fence records and partition fallback exact', () => {
  const raw = '😀 Intro\r\n\r\n| **Header** |\r\n| --- |\r\n| before **ready** `code` |\r\n\r\n```sh\r\npwd\r\n\r\n```\r\n';
  const blocks = mobileFileMarkdownBlocks(raw, false);
  expect(blocks.find(block => block.kind === 'table')).toMatchObject({ id: '0:table:10',
    rows: [{ header: true, cells: [{ runs: [{ kind: 'strong', text: 'Header' }] }] },
      { header: false, cells: [{ runs: [{ kind: 'text', text: 'before ' }, { kind: 'strong', text: 'ready' },
        { kind: 'text', text: ' ' }, { kind: 'code', text: 'code' }] }] }] });
  expect(blocks.find(block => block.kind === 'code')).toEqual({
    ...mobileMarkdownBlocks(raw, false).find(block => block.kind === 'code'), level: 0, columns: 0, rows: [],
  });
  expect(blocks.find(block => block.kind === 'code')).toMatchObject({ id: '1', text: 'pwd\n', language: 'sh' });
  for (const source of ['    indented\n\n', 'Hard\\\nbreak\n\n']) {
    const input = source + '| A |\n| --- |\n| **ready** |';
    if (source.startsWith('    ')) expect(mobileFileMarkdownBlocks(input, false))
      .toEqual(mobileMarkdownBlocks(input, false).map(block => ({ ...block, level: 0, columns: 0, rows: [] })));
    else expect(mobileFileMarkdownBlocks(input, false)[0]?.text).toBe(source);
  }
  expect(raw).toContain('\r\n');
});
