// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/timeline-tree.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// The changed-files card's tree, adapted from T3 Code (MIT, see LICENSE-T3):
// apps/web/src/lib/turnDiffTree.ts (directories compacted while they hold one
// directory) and components/chat/ChangedFilesTree.tsx (all folders collapsed
// until "Expand all folders"; a folder's own toggle overrides that until the
// all-folders toggle flips again).
import { fileIconToken } from './timeline-files';

export interface TreeFile { path: string; additions: number; deletions: number }
interface DirNode { kind: 'directory'; name: string; path: string; additions: number; deletions: number; children: Node[] }
interface FileNode { kind: 'file'; name: string; path: string; additions: number; deletions: number }
type Node = DirNode | FileNode;
interface Mutable { name: string; path: string; additions: number; deletions: number; directories: Map<string, Mutable>; files: FileNode[] }

const byName = (a: { name: string }, b: { name: string }) => a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: 'base' });

function compact(node: DirNode): DirNode {
  let current: DirNode = { ...node, children: node.children.map(child => child.kind === 'directory' ? compact(child) : child) };
  while (current.children.length === 1 && current.children[0]!.kind === 'directory') {
    const only = current.children[0] as DirNode;
    current = { kind: 'directory', name: `${current.name}/${only.name}`, path: only.path, additions: only.additions, deletions: only.deletions, children: only.children };
  }
  return current;
}
function nodes(directory: Mutable): Node[] {
  const directories = [...directory.directories.values()].sort(byName).map(child => compact({ kind: 'directory', name: child.name, path: child.path,
    additions: child.additions, deletions: child.deletions, children: nodes(child) }));
  return [...directories, ...[...directory.files].sort(byName)];
}
/** buildTurnDiffTree: directories first, then files, each sorted by name. */
export function buildTree(files: readonly TreeFile[]): Node[] {
  const root: Mutable = { name: '', path: '', additions: 0, deletions: 0, directories: new Map(), files: [] };
  for (const file of files) {
    const segments = file.path.replace(/\\/g, '/').split('/').filter(Boolean);
    const name = segments[segments.length - 1];
    if (!name) continue;
    const ancestors = [root];
    let current = root;
    for (const segment of segments.slice(0, -1)) {
      let next = current.directories.get(segment);
      if (!next) {
        next = { name: segment, path: current.path ? `${current.path}/${segment}` : segment, additions: 0, deletions: 0, directories: new Map(), files: [] };
        current.directories.set(segment, next);
      }
      current = next; ancestors.push(current);
    }
    current.files.push({ kind: 'file', name, path: segments.join('/'), additions: file.additions, deletions: file.deletions });
    for (const ancestor of ancestors) { ancestor.additions += file.additions; ancestor.deletions += file.deletions; }
  }
  return nodes(root);
}

/** formatCompactDiffCount: 999, 1.2k, 12k, 1.5m. */
export function compactCount(value: number): string {
  const short = (scaled: number) => scaled < 10 ? scaled.toFixed(1).replace(/\.0$/, '') : String(Math.round(scaled));
  if (value < 1000) return String(value);
  if (value < 1_000_000) return `${short(value / 1000)}k`;
  if (value < 1_000_000_000) return `${short(value / 1_000_000)}m`;
  return `${short(value / 1_000_000_000)}b`;
}

export interface TreeRow { id: string; path: string; additions: number; deletions: number; kind: string; name: string; depth: number;
  expanded: boolean; icon: string; spacer: boolean; plus: string; minus: string }

/**
 * The visible rows of the tree: a folder shows its children when expanded,
 * where expanded is its own override or else the card's all-folders state.
 */
export function treeRows(files: readonly TreeFile[], allExpanded: boolean, overrides: ReadonlyMap<string, boolean>): TreeRow[] {
  const tree = buildTree(files), hasDirectories = tree.some(node => node.kind === 'directory');
  const rows: TreeRow[] = [];
  const walk = (node: Node, depth: number) => {
    const base = { path: node.path, additions: node.additions, deletions: node.deletions, name: node.name, depth,
      plus: `+${compactCount(node.additions)}`, minus: `-${compactCount(node.deletions)}` };
    if (node.kind === 'directory') {
      const expanded = overrides.get(node.path) ?? allExpanded;
      rows.push({ ...base, id: `dir:${node.path}`, kind: 'directory', expanded, icon: '', spacer: false });
      if (expanded) node.children.forEach(child => walk(child, depth + 1));
      return;
    }
    rows.push({ ...base, id: `file:${node.path}`, kind: 'file', expanded: false, icon: fileIconToken(node.path), spacer: hasDirectories || depth > 0 });
  };
  tree.forEach(node => walk(node, 0));
  return rows;
}
