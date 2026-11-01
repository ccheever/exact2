// ChatMarkdown's code block label and its tables, read from markdown.contract.
import { describe, expect, test } from 'bun:test';
import { contractFn } from '../support/contract-fn-fixture';
import { appPath } from '../support/app-files';

describe('code block label (extractFenceLanguage)', () => {
  test('a fence with no language reads "text"; a language reads itself; a file icon stands for its language', async () => {
    const label = await contractFn('markdown.contract', 'citeCodeLabel', ['b', 'code']);
    const block = (href: string) => ({ kind: 'code', text: 'plain', href });
    expect(label(block(''), [])).toBe('text');
    expect(label(block('bash'), [])).toBe('bash');
    expect(label(block('ts'), [{ code: 'plain', icon: 'typescript' }])).toBe('');
  });
  test('the toolbar shows that label', async () => {
    const source = await Bun.file(appPath('markdown.contract')).text();
    expect(source).toContain('text (block.href != "" ? block.href : "text") font-family="Menlo, Consolas, \\"Liberation Mono\\", monospace"');
  });
});

describe('plain-text fences and rules (index.css)', () => {
  test('a text, txt, plain or plaintext fence paints its code in the prose colour (#27144)', async () => {
    const plain = await contractFn('markdown.contract', 'plainFence', ['language']);
    expect(['', 'text', 'TXT', 'Plain', 'plaintext', 'ts', 'bash', 'texts'].map(language => plain(language))).toEqual([true, true, true, true, true, false, false, false]);
    const source = await Bun.file(appPath('markdown.contract')).text();
    expect(source).toContain('derive ink = plainFence(block.href) ? prose : "light-dark(#000000, #fafafa)"');
  });
  test('a rule in dark mode takes the muted foreground, not the border colour (#22367)', async () => {
    const source = await Bun.file(appPath('markdown.contract')).text();
    expect(source).toContain('if block.kind == "rule"\n      box width="100%" height=1 margin-top="0.5rem" margin-bottom="0.5rem" background-color="light-dark(#e4e4e7, #a1a1a1)"');
  });
});

// MarkdownTable at v0.1.5 (#26449): no "Expand table cells" toggle; every cell, a header's too, wraps at word
// boundaries within its column, so a table opens at its size.
describe('tables open at their size', () => {
  test('the footer holds only Copy table, and no cell keeps to one line', async () => {
    const source = await Bun.file(appPath('markdown.contract')).text();
    const table = source.slice(source.indexOf('component MarkdownTable\n'), source.indexOf('// Whether a cell draws run by run:'));
    expect(table).not.toContain('Expand table cells');
    expect(table).toContain('TableCopyButton(');
    expect(table).not.toContain('line-clamp=1');
    expect(table).toContain('nowrap=false');
    expect(table).toContain('overflow-wrap="break-word"');
  });
});

describe('strikethrough (`<del>`)', () => {
  test('a struck run draws its line through its find spans, as CSS propagates a decoration to nested text', async () => {
    const markdown = await Bun.file(appPath('markdown.contract')).text();
    const find = await Bun.file(appPath('thread-find.contract')).text();
    expect(markdown.match(/text-decoration-line=\(run\.strike \? "line-through" : "none"\)/g)!.length).toBeGreaterThan(10);
    expect(markdown.match(/FindParts\(parts=run\.findParts\)/g)!.length).toBeGreaterThan(10);
    expect(markdown).not.toContain('strike=');
    expect(find.slice(find.indexOf('component FindParts'), find.indexOf('component FindToken'))).not.toContain('text-decoration');
  });
});
