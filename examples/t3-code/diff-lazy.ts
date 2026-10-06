// Large diffs and hidden lines in the Diff panel, adapted from T3 Code 1e2ecbd975 (MIT; see
// LICENSE-T3): apps/web/src/components/diffs/useReviewFilePatches.ts (a git preview past the
// server's 120,000-byte cap with per-file stats is read four files at a time, then four more when
// the loading boundary nears view; per-file error and truncated marks; Retry by path; the scope
// key), DiffFileStatus.tsx, DiffFileLoadingBoundary.tsx, lib/diffFileContents.ts
// (createGitDiffFileContentsLoader) and @pierre/diffs' hunk expansion (DiffHunksRenderer
// expandHunk: a hidden range of up to `expansionLineCount` 100 lines opens at once, a longer one
// 100 lines at a time from the side the reader pressed). Changes from the reference: one request at
// a time per file through the client's RPC, and the answers kept in this module instead of Effect
// atoms; Pierre's two expand arrows on a long range are one control that opens the 100 lines below
// the previous hunk.
import { arr, str, type Obj } from './domain';

export type DiffSourceKind = 'working-tree' | 'branch-range';
export type FileStat = { path: string; previousPath: string | null; additions: number; deletions: number };
/** The git source a preview answered: what its per-file and contents requests repeat. */
export type DiffSource = { kind: DiffSourceKind; cwd: string; baseRef: string | null; headRef: string | null; diffHash: string; truncated: boolean; files: FileStat[] | null };
export type FilePatch = { state: 'loading' | 'loaded' | 'error'; diff: string; truncated: boolean };
export type LazyPatches = { scope: string; files: FileStat[]; requested: number[]; patches: Map<string, FilePatch> };

/** The preview's selected source, with the fields later requests need. */
export function diffSource(result: Obj, kind: DiffSourceKind): DiffSource | null {
  const source = arr(result.sources).find(entry => entry.kind === kind);
  if (!source) return null;
  const files = Array.isArray(source.files) ? arr(source.files).map(file => ({ path: str(file.path), previousPath: typeof file.previousPath === 'string' ? file.previousPath : null,
    additions: Number(file.additions) || 0, deletions: Number(file.deletions) || 0 })) : null;
  return { kind, cwd: str(result.cwd), baseRef: typeof source.baseRef === 'string' ? source.baseRef : null, headRef: typeof source.headRef === 'string' ? source.headRef : null,
    diffHash: str(source.diffHash), truncated: source.truncated === true, files };
}

/** DiffPanel's lazySource: a truncated git preview that lists its files. Files sort as the reference's (numeric, case-insensitive). */
export function lazyPatches(source: DiffSource | null, ignoreWhitespace: boolean, previous: LazyPatches | null): LazyPatches | null {
  if (!source?.truncated || !source.files) return null;
  const scope = JSON.stringify([source.cwd, source.kind, source.diffHash, source.baseRef, ignoreWhitespace]);
  if (previous?.scope === scope) return previous;
  const files = [...source.files].sort((a, b) => a.path.localeCompare(b.path, undefined, { numeric: true, sensitivity: 'base' }));
  return { scope, files, requested: [0, 1, 2, 3].filter(index => index < files.length), patches: new Map() };
}

/** The first file still without an answer: every file before it is shown. */
export function settledFileCount(lazy: LazyPatches): number {
  const pending = lazy.files.findIndex(file => { const patch = lazy.patches.get(file.path); return !patch || patch.state === 'loading'; });
  return pending < 0 ? lazy.files.length : pending;
}
/** Adds file indices to the requested set; answers the ones that were new. */
export function requestFiles(lazy: LazyPatches, indices: number[]): number[] {
  const added = indices.filter(index => index >= 0 && index < lazy.files.length && !lazy.requested.includes(index));
  lazy.requested.push(...added);
  return added;
}
/** loadNextFiles: the four files after the settled ones. */
export function loadNextFiles(lazy: LazyPatches): number[] { const settled = settledFileCount(lazy); return requestFiles(lazy, [settled, settled + 1, settled + 2, settled + 3]); }
/** Requested files with no answer yet, in order: what to ask the server for now. */
export function unanswered(lazy: LazyPatches): FileStat[] { return lazy.requested.map(index => lazy.files[index]!).filter(file => !lazy.patches.has(file.path)); }

/** review.getDiffPreview for one file of the source. */
export function filePatchPayload(source: DiffSource, file: FileStat, ignoreWhitespace: boolean): Obj {
  return { cwd: source.cwd, ...(source.baseRef ? { baseRef: source.baseRef } : {}), ignoreWhitespace, file: { path: file.path, previousPath: file.previousPath, sourceKind: source.kind } };
}
/** One file's answer: its source's patch and truncation; an answer without that file counts as an error, as in the reference. */
export function adoptFilePatch(lazy: LazyPatches, file: FileStat, result: Obj | Error): void {
  if (result instanceof Error) { lazy.patches.set(file.path, { state: 'error', diff: '', truncated: false }); return; }
  const source = arr(result.sources).find(entry => entry.kind === JSON.parse(lazy.scope)[1]) ?? {};
  const diff = str(source.diff);
  const names = file.path.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const hasFile = new RegExp(`^diff --git .*b/${names}"?$`, 'm').test(diff);
  lazy.patches.set(file.path, hasFile ? { state: 'loaded', diff, truncated: source.truncated === true } : { state: 'error', diff: '', truncated: false });
}
/** Asks for every requested file without an answer, one request each (the reference's per-file query). */
export async function loadFilePatches(lazy: LazyPatches, source: DiffSource, ignoreWhitespace: boolean, send: (method: string, payload: Obj) => Promise<Obj>): Promise<void> {
  const files = unanswered(lazy);
  for (const file of files) lazy.patches.set(file.path, { state: 'loading', diff: '', truncated: false });
  await Promise.all(files.map(async file => {
    const result = await send('review.getDiffPreview', filePatchPayload(source, file, ignoreWhitespace)).catch((error: unknown) => error instanceof Error ? error : new Error(String(error)));
    adoptFilePatch(lazy, file, result);
  }));
}
/** Retry by path: forget the answer so the next load asks again. */
export function retryFile(lazy: LazyPatches, path: string): boolean {
  const index = lazy.files.findIndex(file => file.path === path);
  if (index < 0) return false;
  lazy.patches.delete(path);
  requestFiles(lazy, [index]);
  return true;
}
export function fileState(lazy: LazyPatches, path: string): { error: boolean; truncated: boolean } {
  const patch = lazy.patches.get(path);
  return { error: patch?.state === 'error', truncated: patch?.state === 'loaded' && patch.truncated };
}

// lib/diffFileContents.ts
export type ChangeType = 'change' | 'rename-pure' | 'rename-changed' | 'new' | 'deleted';
export type ContentsFile = { name: string; contents: string; cacheKey: string };
type ContentsSource = { cwd: string; sourceKind: DiffSourceKind; baseRef: string | null; headRef: string | null; cacheKey: string };
/** Turns the host's Git file-content RPC into the full-file loader used for hidden-line expansion. */
export function createGitDiffFileContentsLoader(getDiffFileContents: (input: Obj) => Promise<{ oldContents: string; newContents: string }>, source: ContentsSource) {
  return async (fileDiff: { type: ChangeType; name: string; prevName?: string }): Promise<{ oldFile: ContentsFile | null; newFile: ContentsFile }> => {
    const newPath = fileDiff.name, oldPath = fileDiff.prevName ?? newPath;
    const contents = await getDiffFileContents({ cwd: source.cwd, sourceKind: source.sourceKind, changeType: fileDiff.type, baseRef: source.baseRef, headRef: source.headRef, oldPath, newPath });
    const newFile = { name: newPath, contents: contents.newContents, cacheKey: `${source.cacheKey}:new:${newPath}` };
    if (fileDiff.type === 'rename-pure') return { oldFile: null, newFile };
    return { oldFile: { name: oldPath, contents: contents.oldContents, cacheKey: `${source.cacheKey}:old:${oldPath}` }, newFile };
  };
}
/** The clone's file status as Pierre's change type. */
export function changeType(status: string, hunks: number): ChangeType {
  return status === 'added' ? 'new' : status === 'deleted' ? 'deleted' : status === 'renamed' ? (hunks === 0 ? 'rename-pure' : 'rename-changed') : 'change';
}

export type FileContents = { state: 'loading' | 'loaded' | 'error'; oldContents: string; newContents: string; error: string };
const inflight = new Map<string, Promise<void>>();
/** Single flight per file: a second press while the first read is out waits for it. */
export async function loadFileContents(store: Record<string, FileContents>, key: string, read: () => Promise<{ oldFile: ContentsFile | null; newFile: ContentsFile }>): Promise<void> {
  if (store[key]?.state === 'loaded') return;
  const running = inflight.get(key);
  if (running) return running;
  store[key] = { state: 'loading', oldContents: '', newContents: '', error: '' };
  const task = read().then(files => { store[key] = { state: 'loaded', oldContents: files.oldFile?.contents ?? files.newFile.contents, newContents: files.newFile.contents, error: '' }; },
    (error: unknown) => { store[key] = { state: 'error', oldContents: '', newContents: '', error: error instanceof Error ? error.message : String(error) }; })
    .finally(() => inflight.delete(key));
  inflight.set(key, task);
  return task;
}

export const EXPANSION_LINE_COUNT = 100;
export type Expansion = { fromStart: number; fromEnd: number };
/** One press on a hidden range: all of it when it is short, else 100 more lines below the previous hunk. */
export function expandRange(current: Expansion | undefined, hidden: number): Expansion {
  const now = current ?? { fromStart: 0, fromEnd: 0 };
  const left = hidden - now.fromStart - now.fromEnd;
  if (left <= 0) return now;
  return left <= EXPANSION_LINE_COUNT ? { fromStart: now.fromStart + left, fromEnd: now.fromEnd } : { fromStart: now.fromStart + EXPANSION_LINE_COUNT, fromEnd: now.fromEnd };
}
