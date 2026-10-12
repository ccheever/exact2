import { describe, expect, test } from 'bun:test';
import { buildTree, compactCount, treeRows } from './timeline-tree';

const files = [
  { path: 'src/timeline/rows.ts', additions: 1, deletions: 0 }, { path: 'src/timeline/fold.ts', additions: 1, deletions: 0 },
  { path: 'docs/notes.md', additions: 3, deletions: 0 }, { path: 'README.md', additions: 1, deletions: 2 },
];
describe('changed files tree', () => {
  test('directories come first, compacted while they hold a single directory, with summed stats', () => {
    const tree = buildTree(files);
    expect(tree.map(node => [node.kind, node.name, node.additions, node.deletions])).toEqual([
      ['directory', 'docs', 3, 0], ['directory', 'src/timeline', 2, 0], ['file', 'README.md', 1, 2]]);
  });
  test('folders start collapsed; expand-all opens every folder and an override wins over it', () => {
    expect(treeRows(files, false, new Map()).map(row => row.id)).toEqual(['dir:docs', 'dir:src/timeline', 'file:README.md']);
    const open = treeRows(files, true, new Map());
    expect(open.map(row => [row.id, row.depth, row.spacer, row.icon])).toEqual([
      ['dir:docs', 0, false, ''], ['file:docs/notes.md', 1, true, 'markdown'], ['dir:src/timeline', 0, false, ''],
      ['file:src/timeline/fold.ts', 1, true, 'typescript'], ['file:src/timeline/rows.ts', 1, true, 'typescript'], ['file:README.md', 0, true, 'markdown']]);
    expect(treeRows(files, true, new Map([['docs', false]])).map(row => row.id)).toEqual(['dir:docs', 'dir:src/timeline', 'file:src/timeline/fold.ts', 'file:src/timeline/rows.ts', 'file:README.md']);
    expect(treeRows([{ path: 'a.ts', additions: 0, deletions: 0 }], false, new Map())[0]).toMatchObject({ spacer: false, plus: '+0', minus: '-0' });
  });
  test('counts compact like DiffStatLabel', () => {
    expect([999, 1000, 1250, 12_400, 1_500_000, 2_000_000_000].map(compactCount)).toEqual(['999', '1k', '1.3k', '12k', '1.5m', '2b']);
  });
});
