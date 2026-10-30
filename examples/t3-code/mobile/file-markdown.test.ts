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
  'A ~~strike~~\nparagraph', 'A [link](https://example.test)\nparagraph',
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
});


test.each([
  ['Before **ready**\nafter text', 'Before **ready** after text'],
  ['Before *ready* and `code`\nafter text', 'Before *ready* and `code` after text'],
  ['A **bold**\nparagraph', 'A **bold** paragraph'],
])('formatted preview paragraphs join certified soft breaks: %s', (source, text) => {
  expect(mobileFileMarkdownBlocks(source, false)).toEqual([
    { id: '0', kind: 'markdown', text, language: '', tokens: [], level: 0, columns: 0, rows: [] },
  ]);
});

test.each([
  ['A **one\ntwo** end', 'A **one two** end'],
  ['A **one \ntwo** end', 'A **one two** end'],
  ['A\n**one**\n*two* end', 'A **one** *two* end'],
  ['**one**\n__two__ and *three\nfour*', '**one** __two__ and *three four*'],
  ['A **one *two\nthree* four**\nend', 'A **one *two three* four** end'],
  ['A ***one\ntwo***\nend', 'A ***one two*** end'],
  ['A **one** \n`a b`\nend', 'A **one** `a b` end'],
  ['A ` one ` and **two**\nend', 'A ` one ` and **two** end'],
  ['A **one**  \nB\nC', 'A **one**  \nB C'],
  ['A **one**   \nB\nC', 'A **one**   \nB C'],
  ['A\\\nB **two**\nC', 'A\\\nB **two** C'],
  ['A **one**\nB\\\nC\nD', 'A **one** B\\\nC D'],
  ['A **one**\nB\n \nC *two*\nD  \n', 'A **one** B\n \nC *two* D  \n'],
])('traits, hard-break barriers and terminal whitespace survive soft folding: %s', (source, text) => {
  expect(mobileFileMarkdownBlocks(source, true).map(block => [block.id, block.kind, block.text]))
    .toEqual([['0', 'markdown', text]]);
});

test.each([
  'A `multi\nline` span', 'A ``multiple`` span', 'A `unmatched span',
  'A `span\n\nacross paragraphs`', 'A \\*literal', 'A &amp; entity', 'A &#49; entity',
  '[link](/docs)', '![image](/image)', '<https://example.test>', 'https://example.test',
  '~~deleted~~', '- list', '1. list', '> quoted', '<b>HTML</b>', '<!-- hidden -->',
  'Setext\n===', '# **Unsupported heading**', '# Adjacent\nparagraph',
  '    indented', 'A\n indented', 'A\tB', 'A\u2028B', 'A\u2029B',
  '[unresolved]', 'A $math', 'A @mention', 'A #fragment', 'Text\n1.',
  '| A |\n| --- |\n| *refused* |',
])('unsupported siblings veto new folding throughout the complete prose chunk: %s', sibling => {
  const source = `Before **ready**\nafter text\n\n${sibling}\n\nLast *ready*\nafter text`;
  expect(mobileFileMarkdownBlocks(source, false))
    .toEqual(mobileMarkdownBlocks(source, false).map(block => ({ ...block, level: 0, columns: 0, rows: [] })));
});

test('full-document definitions and references veto their chunks while a later ordinary chunk folds', () => {
  const barrier = '\n\n~~~txt\nSENTINEL\nkeep newline\n\n~~~\n\n';
  const first = 'Before **ready**\nafter [link][ref] and [^note].';
  const second = '[ref]: /docs\n\n[^note]: Note.\n\nOther *ready*\nafter text';
  const third = 'Last **ready**\nafter text';
  const source = first + barrier + second + barrier + third;
  const blocks = mobileFileMarkdownBlocks(source, false);
  expect(blocks.filter(block => block.kind === 'markdown').map(block => block.text))
    .toEqual([first + '\n\n', '\n' + second + '\n\n', '\nLast **ready** after text']);
  expect(blocks.filter(block => block.kind === 'code'))
    .toEqual(mobileMarkdownBlocks(source, false).filter(block => block.kind === 'code').map(block => ({ ...block, level: 0, columns: 0, rows: [] })));
});

test('original UTF-16 cuts and repeated content retain every ID and complete highlighted code object', () => {
  const prose = '😀 **ready**\n世界 👋', table = '| **Header** | Value |\n| --- | --- |\n| `item` | **42** |';
  const chunk = `# Heading 👋\n\n${prose}\n\n${table}\n\n${prose}\n\n`;
  const fence = '```ts\nconst 👋 = "世界";\n\n```\n', source = chunk + fence + '\n' + chunk;
  const raw = source.replaceAll('\n', '\r\n'), blocks = mobileFileMarkdownBlocks(raw, true);
  expect(blocks.map(block => block.id)).toEqual([
    '0:heading:0', '0:text:13', `0:table:${chunk.indexOf('| **Header')}`, `0:text:${chunk.indexOf(table) + table.length}`,
    '1', '2:heading:1', '2:text:14', `2:table:${chunk.indexOf('| **Header') + 1}`, `2:text:${chunk.indexOf(table) + table.length + 1}`,
  ]);
  expect(blocks.filter(block => block.kind === 'markdown').map(block => block.text))
    .toEqual(['\n😀 **ready** 世界 👋\n\n', '\n\n😀 **ready** 世界 👋\n\n', '\n😀 **ready** 世界 👋\n\n', '\n\n😀 **ready** 世界 👋\n\n']);
  expect(blocks.find(block => block.kind === 'code'))
    .toEqual({ ...mobileMarkdownBlocks(raw, true).find(block => block.kind === 'code'), level: 0, columns: 0, rows: [] });
  expect(blocks.filter(block => block.kind === 'table').map(block => block.rows[1]?.cells[1]?.runs))
    .toEqual([[{ id: '1:1:0', kind: 'strong', text: '42' }], [{ id: '1:1:0', kind: 'strong', text: '42' }]]);
});

test('refused tables and unsupported headings retain independently admitted projections', () => {
  const table = '| A |\n| --- |\n| value |';
  const source = `# Heading\n\nBefore **ready**\nafter text\n\n${table}\n\n# **Refused**\n\nLast *ready*\nafter text`;
  const blocks = mobileFileMarkdownBlocks(source, false);
  expect(blocks.map(block => block.kind)).toEqual(['heading', 'markdown', 'table', 'markdown']);
  expect(blocks[1]?.text).toBe('\nBefore **ready**\nafter text\n\n');
  expect(blocks.at(-1)?.text).toBe('\n\n# **Refused**\n\nLast *ready*\nafter text');
});
