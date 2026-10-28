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
    .toEqual({ ...mobileMarkdownBlocks(body, false).find(block => block.kind === 'code'), level: 0 });
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
  expect(mobileFileMarkdownBlocks(body, false)).toEqual(mobileMarkdownBlocks(body, false).map(block => ({ ...block, level: 0 })));
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
