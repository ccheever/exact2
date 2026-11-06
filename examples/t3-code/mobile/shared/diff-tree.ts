// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/diff-tree.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// The Diff panel's file tree, adapted from T3 Code 1e2ecbd975 (MIT; see LICENSE-T3):
// apps/web/src/components/diffs/diffFileTree.logic.ts (diffFileTreeEntries, collectDirectoryPaths,
// diffFileTreePositions, compareDiffFileTreeEntries) and DiffFileTree.tsx, which hands those to
// @pierre/trees with `initialExpansion: "open"`, `flattenEmptyDirectories: true` and the
// reading-order comparator. Pierre draws the tree in the browser; here `diffTreeRows` lays the
// same tree out as the flat rows Contract draws (diff-tree.contract). Changes from the reference:
// entries come from the clone's parsed files (`DiffFileModel.status`) or the server's per-file
// stats, not Pierre's FileDiffMetadata; `buildDiffFileTreeUpdates` is not needed because the
// collapsed folders are kept by path in DiffState and survive a new slice or a refresh.

export type DiffTreeStatus = 'added' | 'deleted' | 'renamed' | 'modified';
/** One changed file as the tree shows it: its current path and how it changed. */
export interface DiffFileTreeEntry { readonly path: string; readonly status: DiffTreeStatus }

/**
 * Keeps the diff's own order. A path appears once: a type change (regular file to symlink) is a
 * deletion plus an addition of the same path, and the tree shows the surviving file as modified.
 */
export function diffFileTreeEntries(files: ReadonlyArray<{ path: string; status: DiffTreeStatus }>): DiffFileTreeEntry[] {
  const statusByPath = new Map<string, DiffTreeStatus>();
  for (const file of files) {
    const previous = statusByPath.get(file.path);
    statusByPath.set(file.path, previous === undefined || previous === file.status ? file.status : 'modified');
  }
  return [...statusByPath].map(([path, status]) => ({ path, status }));
}

/** Every directory on the way to each file, with Pierre's trailing slash, parents first. */
export function collectDirectoryPaths(paths: ReadonlyArray<string>): string[] {
  const directories = new Set<string>();
  for (const path of paths) {
    let directory = '';
    for (const segment of path.split('/').slice(0, -1)) { directory += `${segment}/`; directories.add(directory); }
  }
  return [...directories];
}

/** A folder takes the position of its first file in the diff. */
export function diffFileTreePositions(paths: ReadonlyArray<string>): Map<string, number> {
  const positions = new Map<string, number>();
  paths.forEach((path, index) => {
    positions.set(path, index);
    let directory = '';
    for (const segment of path.split('/').slice(0, -1)) {
      directory += `${segment}/`;
      if (!positions.has(directory)) positions.set(directory, index);
    }
  });
  return positions;
}

export function compareDiffFileTreeEntries(getPositions: () => ReadonlyMap<string, number>) {
  return (left: { path: string; depth: number }, right: { path: string; depth: number }): number => {
    const positions = getPositions();
    return (positions.get(left.path) ?? Number.MAX_SAFE_INTEGER) - (positions.get(right.path) ?? Number.MAX_SAFE_INTEGER)
      || left.depth - right.depth || left.path.localeCompare(right.path);
  };
}

export type DiffTreeRow = {
  id: string; kind: 'folder' | 'file'; path: string; name: string; depth: number; status: string; letter: string;
  markdown: boolean; expanded: boolean; selected: boolean;
};
type Node = { path: string; name: string; depth: number; file?: DiffFileTreeEntry; children: Map<string, Node> };
const LETTER: Record<string, string> = { modified: 'M', added: 'A', deleted: 'D', renamed: 'R' };

/**
 * The visible rows: folders first open (`initialExpansion: "open"`) unless the reader closed
 * them, a chain of folders that each hold only one folder drawn as one row
 * (`flattenEmptyDirectories`, "apps/mobile/src"), siblings in reading order.
 * `collapsed` holds directory paths with their trailing slash; a flattened row is keyed by its
 * deepest folder, as Pierre's flattened row is.
 */
export function diffTreeRows(entries: ReadonlyArray<DiffFileTreeEntry>, collapsed: ReadonlySet<string>, selectedPath: string): DiffTreeRow[] {
  const root: Node = { path: '', name: '', depth: 0, children: new Map() };
  for (const entry of entries) {
    let node = root, directory = '';
    const segments = entry.path.split('/');
    segments.forEach((segment, index) => {
      const last = index === segments.length - 1;
      const path = last ? entry.path : (directory += `${segment}/`);
      const key = last ? `f:${segment}` : `d:${segment}`;
      let child = node.children.get(key);
      if (!child) { child = { path, name: segment, depth: 0, children: new Map() }; node.children.set(key, child); }
      if (last) child.file = entry;
      node = child;
    });
  }
  const positions = diffFileTreePositions(entries.map(entry => entry.path));
  const compare = compareDiffFileTreeEntries(() => positions);
  const rows: DiffTreeRow[] = [];
  const visit = (node: Node, depth: number) => {
    const children = [...node.children.values()].sort((a, b) => compare({ path: a.path, depth }, { path: b.path, depth }));
    for (let child of children) {
      if (child.file) {
        const path = child.file.path;
        rows.push({ id: `file:${path}`, kind: 'file', path, name: child.name, depth, status: child.file.status, letter: LETTER[child.file.status] ?? 'M',
          markdown: /\.(md|mdx|markdown)$/i.test(path), expanded: false, selected: path === selectedPath });
        continue;
      }
      let name = child.name;
      while (child.children.size === 1) {
        const only = [...child.children.values()][0]!;
        if (only.file) break;
        name = `${name}/${only.name}`;
        child = only;
      }
      const expanded = !collapsed.has(child.path);
      rows.push({ id: `dir:${child.path}`, kind: 'folder', path: child.path, name, depth, status: '', letter: '', markdown: false, expanded, selected: false });
      if (expanded) visit(child, depth + 1);
    }
  };
  visit(root, 0);
  return rows;
}

/** The folders to open so a file's row shows (DiffFileTree's reveal: every ancestor expanded). */
export function ancestorDirectories(path: string): string[] { return collectDirectoryPaths([path]); }
/** Pierre's areAllDirectoriesExpanded over the diff's directories. */
export function allDirectoriesExpanded(paths: ReadonlyArray<string>, collapsed: ReadonlySet<string>): boolean {
  return collectDirectoryPaths(paths).every(directory => !collapsed.has(directory));
}
