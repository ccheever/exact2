import { expect, test } from 'bun:test';
import { fileMarkdownPlan, type FileMarkdownPlanSegment, type FileTableSlice } from './shared/file-markdown-parser';

const table = '| Name | Value |\n| --- | --- |\n| first | 42 |';

function planFor(source: string): FileMarkdownPlanSegment[] {
  const plan = fileMarkdownPlan(source);
  if (plan === null) throw new Error('Expected an admitted file Markdown plan');
  return plan;
}

function tablesFor(source: string): FileTableSlice[] {
  return planFor(source).flatMap(segment => segment.kind === 'prose' ? segment.tables : []);
}

function values(table: FileTableSlice): string[][] {
  return table.rows.map(row => row.cells.map(cell => cell.runs.map(run => run.text).join('')));
}

test.each([
  '', ' \n\t\n', '# Plain\n\nText', '```md\n| A | B |\n| --- | --- |\n| x | y |\n```',
  '> | A | B |\n> | --- | --- |\n> | x | y |',
  '- | A | B |\n  | --- | --- |\n  | x | y |',
])('a document without a top-level table has no file plan: %s', source => {
  expect(fileMarkdownPlan(source)).toBeNull();
});

test('tables expose decoded text and code runs, escaped pipes and header state', () => {
  const source = '| Name | Value |\n| :--- | ---: |\n| a &amp; b \\| c | `a \\| b` |\n| pre `code` post | &#49;&#50; |';
  const [parsed] = tablesFor(source);
  if (!parsed) throw new Error('Expected one table');
  expect([parsed.start, parsed.end, parsed.columns]).toEqual([0, source.length, 2]);
  expect(parsed.rows.map(row => row.header)).toEqual([true, false, false]);
  expect(values(parsed)).toEqual([['Name', 'Value'], ['a & b | c', 'a | b'], ['pre code post', '12']]);
  expect(parsed.rows[1]?.cells[1]?.runs.map(run => [run.kind, run.text])).toEqual([['code', 'a | b']]);
  expect(parsed.rows[2]?.cells[0]?.runs.map(run => [run.kind, run.text]))
    .toEqual([['text', 'pre '], ['code', 'code'], ['text', ' post']]);
  expect(parsed.rows[2]?.cells[1]?.numeric).toBe(true);
  expect(Object.keys(parsed).sort()).toEqual(['columns', 'end', 'rows', 'start']);
});

test('single-column tables can use an aligned delimiter without pipe characters', () => {
  const source = 'Name\n:---\nValue', [parsed] = tablesFor(source);
  if (!parsed) throw new Error('Expected one table');
  expect(parsed).toMatchObject({ start: 0, end: source.length, columns: 1 });
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

test('code and prose intervals preserve complete source slices and consume only one code-ending newline', () => {
  const before = `Before\n\n${table}\n\n`, code = '```TypeScript extra\nconst x = 1;\n\n```\n', after = `\nAfter\n\n${table}\n`;
  const source = before + code + after;
  const plan = planFor(source);
  expect(plan.map(segment => [segment.kind, segment.start, segment.end])).toEqual([
    ['prose', 0, before.length], ['code', before.length, before.length + code.length],
    ['prose', before.length + code.length, source.length],
  ]);
  expect(plan.map(segment => source.slice(segment.start, segment.end))).toEqual([before, code, after]);
  expect(plan[1]).toEqual({ kind: 'code', start: before.length, end: before.length + code.length, value: 'const x = 1;\n', language: 'TypeScript' });
  expect(tablesFor(source).map(parsed => source.slice(parsed.start, parsed.end))).toEqual([table, table]);
});

test('whitespace-only prose is omitted around adjacent code nodes without losing source offsets', () => {
  const first = '```\n\n```\n', gap = '\n \t\n', second = '~~~sh\necho done\n~~~\n';
  const source = first + gap + second + '\n' + table;
  const plan = planFor(source);
  expect(plan.map(segment => segment.kind)).toEqual(['code', 'code', 'prose']);
  expect(plan[0]).toEqual({ kind: 'code', start: 0, end: first.length, value: '', language: '' });
  expect(plan[1]).toEqual({ kind: 'code', start: first.length + gap.length, end: first.length + gap.length + second.length, value: 'echo done', language: 'sh' });
  expect(plan.at(-1)?.start).toBe(first.length + gap.length + second.length);
});

test.each([
  ['    indented', 'indented', ''],
  ['```js\nopen fence', 'open fence', 'js'],
  ['```js\nclosed fence\n```', 'closed fence', 'js'],
])('top-level Remark code retains its own partition for caller agreement: %s', (code, value, language) => {
  const before = table + '\n\n', source = before + code;
  expect(planFor(source).at(-1)).toEqual({ kind: 'code', start: before.length, end: source.length, value, language });
});

test('LF-normalized preview input keeps UTF-16 positions and leaves original CRLF source unchanged', () => {
  const raw = `😀 Intro\r\n\r\n${table.replace(/\n/g, '\r\n')}\r\n\r\n~~~js\r\nconst 👋 = "世界";\r\n~~~\r\n`;
  const source = raw.replace(/\r\n?/g, '\n');
  expect(fileMarkdownPlan(raw)).toBeNull();
  const plan = planFor(source), [parsed] = tablesFor(source);
  expect(parsed?.start).toBe('😀 Intro\n\n'.length);
  expect(parsed && source.slice(parsed.start, parsed.end)).toBe(table);
  expect(plan.at(-1)).toEqual({ kind: 'code', start: '😀 Intro\n\n'.length + table.length + 2, end: source.length, value: 'const 👋 = "世界";', language: 'js' });
  expect(raw).toContain('\r\n');
  expect(raw).not.toBe(source);
});

test('repeated tables and prose retain distinct positions and stable ordinal keys', () => {
  const repeated = `Same\n\n${table}`, separator = '\n\n---\n\n', source = repeated + separator + repeated;
  const parsed = tablesFor(source);
  expect(parsed.map(item => [item.start, item.end])).toEqual([
    ['Same\n\n'.length, repeated.length], [repeated.length + separator.length + 'Same\n\n'.length, source.length],
  ]);
  expect(parsed.map(values)).toEqual([[['Name', 'Value'], ['first', '42']], [['Name', 'Value'], ['first', '42']]]);
  expect(fileMarkdownPlan(source)).toEqual(fileMarkdownPlan(source));
  for (const item of parsed) {
    expect(new Set(item.rows.map(row => row.id)).size).toBe(item.rows.length);
    for (const row of item.rows) {
      expect(new Set(row.cells.map(cell => cell.id)).size).toBe(row.cells.length);
      for (const cell of row.cells) expect(new Set(cell.runs.map(run => run.id)).size).toBe(cell.runs.length);
    }
  }
});

test.each([
  '**bold**', '_emphasis_', '~~struck~~', '[link](/docs)', '![alt](/image)', '<em>html</em>',
  'https://example.test', '<https://example.test>',
])('an unsupported table inline preserves every table in its original prose segment: %s', cell => {
  const unsupported = `| A | B |\n| --- | --- |\n| ${cell} | plain |`, original = `# Heading\n\n${table}\n\n${unsupported}`;
  expect(fileMarkdownPlan(original)).toBeNull();
  const separator = '\n\n```\nkeep code\n```\n\n', source = original + separator + table;
  const plan = planFor(source);
  expect(plan[0]).toEqual({ kind: 'prose', start: 0, end: original.length + 2, tables: [] });
  expect(tablesFor(source).map(item => [item.start, item.end])).toEqual([[original.length + separator.length, source.length]]);
});

test.each([
  `[ref]: /docs\n\n${table}`,
  `[link][ref]\n\n${table}\n\n[ref]: /docs`,
  `![alt][ref]\n\n${table}\n\n[ref]: /image`,
  `Footnote[^note].\n\n${table}\n\n[^note]: Note.`,
  `> [nested]: /docs\n\n${table}`,
])('definitions, references and footnotes preserve their entire original prose segment: %s', original => {
  expect(fileMarkdownPlan(original)).toBeNull();
  const separator = '\n\n~~~\nbarrier\n~~~\n\n', source = original + separator + table;
  expect(planFor(source)[0]).toEqual({ kind: 'prose', start: 0, end: original.length + 2, tables: [] });
  expect(tablesFor(source).map(item => source.slice(item.start, item.end))).toEqual([table]);
});

test('full-document parsing identifies a reference whose definition follows a code boundary', () => {
  const first = `[link][ref]\n\n${table}`, second = `[ref]: /docs\n\n${table}`, barrier = '\n\n```\ncode\n```\n\n';
  const source = first + barrier + second + barrier + table, plan = planFor(source);
  expect(plan.filter(segment => segment.kind === 'prose').map(segment => segment.tables.length)).toEqual([0, 0, 1]);
  expect(tablesFor(source).map(item => item.start)).toEqual([first.length + barrier.length + second.length + barrier.length]);
});

test('nested tables remain in prose while only a top-level table is extracted', () => {
  const nested = '> | Nested | Table |\n> | --- | --- |\n> | a | b |\n\n';
  const source = nested + table, plan = planFor(source);
  expect(plan).toHaveLength(1);
  expect(plan[0]?.start).toBe(0);
  expect(tablesFor(source).map(item => [item.start, item.end])).toEqual([[nested.length, source.length]]);
});
