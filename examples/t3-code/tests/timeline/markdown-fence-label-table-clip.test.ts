// ChatMarkdown's code block label and table expand toggle, read from markdown.contract's functions.
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

describe('table expand toggle (tableHasClippedCells)', () => {
  test('a cell is clipped only when its unwrapped content is wider than its content box', async () => {
    const clipped = await contractFn('markdown.contract', 'cellClipped', ['natural', 'box']);
    // The comparison's table at 1280×840: its widest cell is 517 px in a 744 px column (493 px inside its padding).
    expect(clipped(470, 493)).toBeFalse();
    // Past the 24rem cap in a roomy table still fits (the oracle's td max-width only caps the column's request).
    expect(clipped(450, 476)).toBeFalse();
    expect(clipped(493.6, 493)).toBeFalse();
    // A table wider than the column: the 24rem-capped cell cuts its 1,586 px text.
    expect(clipped(1586, 559)).toBeTrue();
    expect(clipped(526, 360)).toBeTrue();
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
