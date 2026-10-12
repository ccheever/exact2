// Ported from T3 Code 1e2ecbd975 apps/web/src/components/diffs/diffFileTree.logic.test.ts (MIT; see LICENSE-T3).
// "diff tree reading order" reads the rows diffTreeRows lays out instead of Pierre's shadow HTML.
// "buildDiffFileTreeUpdates" is n/a: the clone keeps collapsed folders by path (diff-tree.ts header).
import { describe, expect, it } from 'bun:test';
import { allDirectoriesExpanded, collectDirectoryPaths, diffFileTreeEntries, diffTreeRows } from './diff-tree';

describe('diffFileTreeEntries', () => {
  it("maps each change type to its git status under the file's current path", () => {
    expect(diffFileTreeEntries([
      { path: 'a/src/a.ts', status: 'added' }, { path: 'src/b.ts', status: 'deleted' }, { path: 'src/c.ts', status: 'renamed' },
      { path: 'src/d.ts', status: 'renamed' }, { path: 'README.md', status: 'modified' },
    ])).toEqual([
      { path: 'a/src/a.ts', status: 'added' }, { path: 'src/b.ts', status: 'deleted' }, { path: 'src/c.ts', status: 'renamed' },
      { path: 'src/d.ts', status: 'renamed' }, { path: 'README.md', status: 'modified' },
    ]);
  });
  it('folds a file-to-symlink type change into one modified entry', () => {
    expect(diffFileTreeEntries([
      { path: 'CLAUDE.md', status: 'modified' }, { path: 'AGENTS.md', status: 'deleted' }, { path: 'AGENTS.md', status: 'added' }, { path: 'docs/new.md', status: 'added' },
    ])).toEqual([{ path: 'CLAUDE.md', status: 'modified' }, { path: 'AGENTS.md', status: 'modified' }, { path: 'docs/new.md', status: 'added' }]);
  });
});

describe('collectDirectoryPaths', () => {
  it("lists every ancestor once, parents first, with Pierre's trailing slash", () => {
    expect(collectDirectoryPaths(['apps/web/src/a.ts', 'apps/web/b.ts', 'README.md'])).toEqual(['apps/', 'apps/web/', 'apps/web/src/']);
  });
});

describe('diff tree reading order', () => {
  it('places folders and files where their first diff appears', () => {
    const paths = ['apps/mobile/src/state/shell.ts', 'apps/mobile/src/features/threads/route.ts', 'apps/mobile/src/features/threads/screen.tsx'];
    const rows = diffTreeRows(paths.map(path => ({ path, status: 'modified' as const })), new Set(), '');
    expect(rows.map(row => row.path)).toEqual([
      'apps/mobile/src/', 'apps/mobile/src/state/', 'apps/mobile/src/state/shell.ts',
      'apps/mobile/src/features/threads/', 'apps/mobile/src/features/threads/route.ts', 'apps/mobile/src/features/threads/screen.tsx',
    ]);
    expect(rows.map(row => [row.name, row.depth])).toEqual([['apps/mobile/src', 0], ['state', 1], ['shell.ts', 2], ['features/threads', 1], ['route.ts', 2], ['screen.tsx', 2]]);
  });
  it('a file before a folder in the diff stays first; a closed folder hides its files', () => {
    const entries = diffFileTreeEntries([{ path: 'README.md', status: 'deleted' }, { path: 'src/a.ts', status: 'modified' }, { path: 'src/ui/b.ts', status: 'added' }]);
    expect(diffTreeRows(entries, new Set(), 'src/a.ts').map(row => [row.path, row.letter, row.selected]))
      .toEqual([['README.md', 'D', false], ['src/', '', false], ['src/a.ts', 'M', true], ['src/ui/', '', false], ['src/ui/b.ts', 'A', false]]);
    const closed = new Set(['src/ui/']);
    expect(diffTreeRows(entries, closed, '').map(row => [row.path, row.expanded])).toEqual([['README.md', false], ['src/', true], ['src/a.ts', false], ['src/ui/', false]]);
    expect(allDirectoriesExpanded(entries.map(entry => entry.path), closed)).toBe(false);
    expect(allDirectoriesExpanded(entries.map(entry => entry.path), new Set())).toBe(true);
  });
});
