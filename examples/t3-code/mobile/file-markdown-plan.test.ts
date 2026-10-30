import { expect, test } from 'bun:test';
import { fileMarkdownPlan, type FileMarkdownPart } from './shared/file-markdown-parser';
import { mobileMarkdownBlocks } from './markdown-blocks';

type Table = Extract<FileMarkdownPart, { kind: 'table' }>;
const table = '| Name | Value |\n| --- | --- |\n| first | 42 |';
function planFor(source: string): readonly FileMarkdownPart[] {
  return fileMarkdownPlan(source, mobileMarkdownBlocks(source, false));
}
function tablesFor(source: string): Table[] {
  return planFor(source).filter(part => part.kind === 'table');
}
function values(table: Table): string[][] {
  return table.rows.map(row => row.cells.map(cell => cell.runs.map(run => run.text).join('')));
}

test('tables expose decoded text and code runs, escaped pipes and header state', () => {
  const source = '| Name | Value |\n| :--- | ---: |\n| a &amp; b \\| c | `a \\| b` |\n| pre `code` post | &#49;&#50; |';
  const [parsed] = tablesFor(source);
  if (!parsed) throw new Error('Expected one table');
  expect([parsed.idSuffix, parsed.columns]).toEqual([':table:0', 2]);
  expect(parsed.rows.map(row => row.header)).toEqual([true, false, false]);
  expect(values(parsed)).toEqual([['Name', 'Value'], ['a & b | c', 'a | b'], ['pre code post', '12']]);
  expect(parsed.rows[1]?.cells[1]?.runs.map(run => [run.kind, run.text])).toEqual([['code', 'a | b']]);
  expect(parsed.rows[2]?.cells[0]?.runs.map(run => [run.kind, run.text]))
    .toEqual([['text', 'pre '], ['code', 'code'], ['text', ' post']]);
  expect(parsed.rows[2]?.cells[1]?.numeric).toBe(true);
  expect(Object.keys(parsed).sort()).toEqual(['blockIndex', 'columns', 'idSuffix', 'kind', 'rows']);
});

test('single-column tables can use an aligned delimiter without pipe characters', () => {
  const source = 'Name\n:---\nValue', [parsed] = tablesFor(source);
  if (!parsed) throw new Error('Expected one table');
  expect(parsed).toMatchObject({ idSuffix: ':table:0', columns: 1 });
  expect(values(parsed)).toEqual([['Name'], ['Value']]);
});

test('body rows follow the header column count, retaining empty cells and dropping excess cells', () => {
  const source = '| A | B | C |\n| --- | --- | --- |\n| only |\n| extra | kept | last | ignored |\n| | | |';
  const [parsed] = tablesFor(source);
  if (!parsed) throw new Error('Expected one table');
  expect(values(parsed)).toEqual([['A', 'B', 'C'], ['only', '', ''], ['extra', 'kept', 'last'], ['', '', '']]);
  expect(parsed.rows.every(row => row.cells.length === 3)).toBe(true);
  expect(parsed.rows[1]?.cells[2]).toEqual({ id: '1:2', numeric: false, runs: [] });
  expect(parsed.rows[3]?.cells.every(cell => !cell.numeric && cell.runs.length === 0)).toBe(true);
});

test('numeric flags require a whole numeric cell and preserve decoded cell text', () => {
  const contents = ['0', '-12.5', '+.75', '1,024', '2e3', '5%', '`42`', '  1  2  ', 'v2', '12 ms', 'NaN', 'Infinity', '1,2', '$12', '', '16 KiB', '0.001 ms', '3.5GHz', '12 files'];
  const source = '| Value |\n| --- |\n' + contents.map(content => `| ${content} |`).join('\n');
  const [parsed] = tablesFor(source);
  if (!parsed) throw new Error('Expected one table');
  expect(parsed.rows.slice(1).map(row => row.cells[0]?.numeric))
    .toEqual([true, true, true, true, true, true, true, false, false, true, false, false, false, false, false, true, true, true, false]);
  expect(values(parsed)[8]).toEqual(['1  2']);
});

test.each(['**', '__'])('text-only strong uses decoded header/body runs with %s delimiters', delimiter => {
  const source = `😀 Intro\n\n| ${delimiter}Header${delimiter} | Value |\n| --- | --- |\n| ${delimiter}ready${delimiter} | ${delimiter}&#49;&#50;${delimiter} ms |`;
  const [parsed] = tablesFor(source);
  expect(parsed?.idSuffix).toBe(':table:10');
  expect(parsed?.rows[0]?.cells[0]?.runs).toEqual([{ id: '0:0:0', kind: 'strong', text: 'Header' }]);
  expect(parsed?.rows[1]?.cells[0]?.runs).toEqual([{ id: '1:0:0', kind: 'strong', text: 'ready' }]);
  expect(parsed?.rows[1]?.cells[1]).toEqual({ id: '1:1', numeric: true, runs: [
    { id: '1:1:0', kind: 'strong', text: '12' }, { id: '1:1:1', kind: 'text', text: ' ms' },
  ] });
});

test('mixed and adjacent strong preserve outer-child IDs and plain/code objects', () => {
  const [parsed] = tablesFor('| A |\n| --- |\n| pre **one**__two__ `code` post |');
  expect(parsed?.rows[1]?.cells[0]?.runs).toEqual([
    { id: '1:0:0', kind: 'text', text: 'pre ' },
    { id: '1:0:1', kind: 'strong', text: 'one' },
    { id: '1:0:2', kind: 'strong', text: 'two' },
    { id: '1:0:3', kind: 'text', text: ' ' },
    { id: '1:0:4', kind: 'code', text: 'code' },
    { id: '1:0:5', kind: 'text', text: ' post' },
  ]);
  expect(tablesFor('| A |\n| --- |\n| pre `code` post |')[0]?.rows[1]?.cells[0]?.runs).toEqual([
    { id: '1:0:0', kind: 'text', text: 'pre ' }, { id: '1:0:1', kind: 'code', text: 'code' },
    { id: '1:0:2', kind: 'text', text: ' post' },
  ]);
});

test.each([
  ['**a \\* b &amp; 😀 \\| c**', 'strong', 'a * b & 😀 | c'],
  ['__世界 &#x1F44B;__', 'strong', '世界 👋'],
  ['\\*\\*ready\\*\\*', 'text', '**ready**'], ['**ready', 'text', '**ready'],
  ['** ready **', 'text', '** ready **'],
])('strong admission follows actual decoded parser text: %s', (cell, kind, text) => {
  expect(tablesFor(`| A |\n| --- |\n| ${cell} |`)[0]?.rows[1]?.cells[0]?.runs)
    .toEqual([{ id: '1:0:0', kind, text }]);
});

test('final instructions preserve original heading/table suffixes and code ordinals', () => {
  const before = `# Title\n\nBefore.\n\n${table}\n\n`, fence = '```TypeScript extra\nconst x = 1;\n\n```\n';
  const after = `\nAfter.\n\n${table}\n`, source = before + fence + after;
  const parts = planFor(source);
  expect(parts.map(part => [part.kind, part.blockIndex, 'idSuffix' in part ? part.idSuffix : ''])).toEqual([
    ['heading', 0, ':heading:0'], ['markdown', 0, ':text:8'], ['table', 0, ':table:18'],
    ['code', 1, ''], ['markdown', 2, ''], ['table', 2, ':table:9'],
  ]);
  expect(parts.filter(part => part.kind === 'markdown').map(part => part.text)).toEqual(['\nBefore.\n\n', '\nAfter.\n\n']);
  expect(parts.find(part => part.kind === 'code')).toEqual({ kind: 'code', blockIndex: 1 });
  expect(parts.filter(part => part.kind === 'table').map(values))
    .toEqual([[['Name', 'Value'], ['first', '42']], [['Name', 'Value'], ['first', '42']]]);
});

test('empty documents and adjacent accepted code retain total rendering plans', () => {
  expect(planFor('')).toEqual([]);
  expect(planFor('```\n\n```\n\n \t\n~~~sh\necho done\n~~~\n')).toEqual([
    { kind: 'code', blockIndex: 0 }, { kind: 'code', blockIndex: 1 },
  ]);
});

test.each(['    indented', '```js\nopen fence'])('partition disagreement retains original prose: %s', code => {
  const source = table + '\n\n' + code;
  expect(planFor(source)).toEqual([{ kind: 'markdown', blockIndex: 0, idSuffix: '', text: source }]);
});

test('a mismatch anywhere disables all parser projection but retains legacy plain folding', () => {
  const source = `One\nline\n\n~~~\ncode\n~~~\n\n${table}`;
  const blocks = mobileMarkdownBlocks(source, false);
  for (const changed of [blocks.map(block => block.kind === 'code' ? { ...block, text: 'wrong' } : block),
    blocks.map(block => block.kind === 'code' ? { ...block, language: 'wrong' } : block), blocks.slice(0, 2),
    blocks.map((block, index) => index === 2 ? { ...block, text: 'wrong' } : block)]) {
    const parts = fileMarkdownPlan(source, changed);
    expect(parts[0]).toEqual({ kind: 'markdown', blockIndex: 0, idSuffix: '', text: 'One line\n\n' });
    expect(parts.some(part => part.kind === 'table')).toBe(false);
    expect(parts[1]).toEqual({ kind: 'code', blockIndex: 1 });
  }
});

test.each([
  '**bold *nested***', '*a _nested_ b*', '~~struck~~', '[link](/docs)', '![alt](/image)', '<em>html</em>',
  'https://example.test', '<https://example.test>', '**a __nested__ b**', '**a `code` b**',
  '**[link](/x)**', '**<br>**', '**~~deleted~~**', '**![image](/x)**', '***ready***',
  '___ready___', '**ready** *a `nested` b*', '**ready** ~~later~~',
  '*a **nested** b*', '*[link](/x)*', '*![image](/x)*', '*~~deleted~~*', '*<br>*',
])('unsupported cells retain all tables in the chunk and independent headings: %s', cell => {
  const original = `# Heading\n\n${table}\n\n| A |\n| --- |\n| ${cell} |`;
  const barrier = '\n\n```\nbarrier\n```\n\n';
  const parts = planFor(original + barrier + table);
  if (cell.includes('<')) expect(parts[0]).toEqual({ kind: 'markdown', blockIndex: 0, idSuffix: '', text: original + '\n\n' });
  else {
    expect(parts[0]).toEqual({ kind: 'heading', blockIndex: 0, idSuffix: ':heading:0', text: 'Heading', level: 1 });
    expect(parts[1]).toEqual({ kind: 'markdown', blockIndex: 0, idSuffix: ':text:10', text: original.slice(10) + '\n\n' });
  }
  expect(tablesFor(original)).toEqual([]);
  expect(parts.filter(part => part.kind === 'table').map(part => [part.blockIndex, part.idSuffix])).toEqual([[2, ':table:1']]);
});

test('all excess cells are validated before header-width truncation', () => {
  expect(tablesFor('| A |\n| --- |\n| **kept** | __valid extra__ |').map(values)).toEqual([[['A'], ['kept']]]);
  expect(tablesFor('| A |\n| --- |\n| **kept** | **a `refused`** |')).toEqual([]);
});

test.each(['[ref]: /docs', '[link][ref]', '![alt][ref]', 'Footnote[^note].', '> [nested]: /docs'])
('full-document reference context refuses complete chunks across code boundaries: %s', context => {
  const barrier = '\n\n~~~\nbarrier\n~~~\n\n';
  const source = `${context}\n\n${table}${barrier}[ref]: /docs\n\n[^note]: Note.\n\n${table}${barrier}${table}`;
  expect(tablesFor(source).map(part => [part.blockIndex, part.idSuffix])).toEqual([[4, ':table:1']]);
});

test('nested tables remain Markdown while a top-level table is independently extracted', () => {
  const nested = '> | Nested | Table |\n> | --- | --- |\n> | a | b |\n\n';
  const parts = planFor(nested + table);
  expect(parts[0]).toEqual({ kind: 'markdown', blockIndex: 0, idSuffix: '', text: nested });
  expect(tablesFor(nested + table).map(part => part.idSuffix)).toEqual([`:table:${nested.length}`]);
});

test.each(['*', '_'])('text-only emphasis retains decoded header/body runs with %s delimiters', delimiter => {
  const source = `😀 Intro\n\n| ${delimiter}Header${delimiter} | Value |\n| --- | --- |\n| ${delimiter}ready${delimiter} | ${delimiter}&#49;&#50;${delimiter} ms |`;
  const [parsed] = tablesFor(source);
  expect(parsed?.idSuffix).toBe(':table:10');
  expect(parsed?.rows[0]?.cells[0]?.runs).toEqual([{ id: '0:0:0', kind: 'emphasis', text: 'Header' }]);
  expect(parsed?.rows[1]?.cells[0]?.runs).toEqual([{ id: '1:0:0', kind: 'emphasis', text: 'ready' }]);
  expect(parsed?.rows[1]?.cells[1]).toEqual({ id: '1:1', numeric: true, runs: [
    { id: '1:1:0', kind: 'emphasis', text: '12' }, { id: '1:1:1', kind: 'text', text: ' ms' },
  ] });
});

test('adjacent emphasis, strong, plain and code retain their outer-child identities', () => {
  const [parsed] = tablesFor('| A | B |\n| --- | --- |\n| pre *one*_two_ **bold** `code` post | v*2* |');
  expect(parsed?.rows[1]?.cells[0]?.runs).toEqual([
    { id: '1:0:0', kind: 'text', text: 'pre ' },
    { id: '1:0:1', kind: 'emphasis', text: 'one' },
    { id: '1:0:2', kind: 'emphasis', text: 'two' },
    { id: '1:0:3', kind: 'text', text: ' ' },
    { id: '1:0:4', kind: 'strong', text: 'bold' },
    { id: '1:0:5', kind: 'text', text: ' ' },
    { id: '1:0:6', kind: 'code', text: 'code' },
    { id: '1:0:7', kind: 'text', text: ' post' },
  ]);
  expect(parsed?.rows[1]?.cells[1]?.numeric).toBe(false);
});

test.each([
  ['*a \\* b &amp; 😀 \\| c*', 'emphasis', 'a * b & 😀 | c'],
  ['_世界 &#x1F44B;_', 'emphasis', '世界 👋'],
  ['\\*ready\\*', 'text', '*ready*'], ['*ready', 'text', '*ready'], ['* ready *', 'text', '* ready *'],
])('emphasis follows authoritative decoded text: %s', (cell, kind, text) => {
  expect(tablesFor(`| A |\n| --- |\n| ${cell} |`)[0]?.rows[1]?.cells[0]?.runs)
    .toEqual([{ id: '1:0:0', kind, text }]);
});

test('emphasis validates excess cells before truncation and retains whole-segment fallback', () => {
  const valid = '| *A* |\n| --- |\n| *kept* | _extra_ |';
  expect(tablesFor(valid).map(values)).toEqual([[['A'], ['kept']]]);
  const refused = valid + '\n\n| A |\n| --- |\n| *kept* | *a **nested** b* |';
  expect(tablesFor(refused)).toEqual([]);
  const parts = planFor(refused + '\n\n```sh\npwd\n\n```\n\n' + valid);
  expect(parts.filter(part => part.kind === 'table').map(part => part.blockIndex)).toEqual([2]);
  expect(parts[0]).toEqual({ kind: 'markdown', blockIndex: 0, idSuffix: '', text: refused + '\n\n' });
  for (const context of ['[ref]: /docs', '[label][ref]\n\n[ref]: /docs', '![alt][ref]\n\n[ref]: /docs', 'Footnote[^note].\n\n[^note]: Note']) {
    const source = valid + '\n\n' + context;
    expect(tablesFor(source)).toEqual([]);
  }
  expect(tablesFor(valid + '\n\n```js\nopen')).toEqual([]);
});
