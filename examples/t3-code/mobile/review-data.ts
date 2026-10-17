import { mobileComposerTarget, mobileComposerTargetText, mobileComposerEditContext, mobileComposerNewTaskContext } from './composer-target';
import { mobileNewTaskDraftIsKey } from './mobile-new-task-drafts';
import { mobileExternalContextCapture, mobileExternalContextInsert, mobileExternalContextPersist, mobileExternalContextPreflight } from './composer-external-context';
import { mobileEditorDocumentMembership } from './composer-editor-persistence';
import { contextLink } from './shared/composer-editor-menu';
// T3 Code365aa87982 ReviewSheet/useReviewSections/useReviewDiffData.
// @ref llp/1109.006-review-and-files.decision.md#ownership
import { mobileClient, mobileNative } from './client';
import type { T3Client } from './shared/client';
import { arr, obj, str, type Obj } from './shared/domain';
import { ClientError, nativeFiles, type Native, type Files } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { workspaceOf } from './shared/r4-surfaces-panel';
import type { ReviewPatchFile } from './review-patch';
import { diffSource, lazyPatches, loadFilePatches, requestFiles, retryFile, unanswered, type LazyPatches, type DiffSource } from './shared/diff-lazy';
import { buildDiffReviewComment, diffReviewLines, reviewCommentContextRecord, type ReviewCommentContext } from './shared/diff-comments';
import { addReviewCommentChip, removeReviewCommentChip, localId } from './shared/composer-editor';
import { assertReviewComposerOwner, assertReviewOwner, reviewComposerNative, reviewFileAccess, reviewNative, reviewOwner } from './review-owner';
import { reviewFileRows, reviewPatch, reviewRow, reviewSuppression, type ReviewFile, type ReviewRow } from './review-model';

export interface ReviewSection { id: string; title: string; subtitle: string; selected: boolean }
export interface ReviewSnapshot {
  owner: string; revision: number; navigationRevision: number; sectionId: string; patchRequest: string; title: string; subtitle: string; sections: ReviewSection[];
  files: ReviewFile[]; rows: ReviewRow[]; loading: boolean; error: string; notice: string; raw: string;
  emptyTitle: string; emptyDetail: string; additions: number; deletions: number; selectedPath: string;
  selectionTitle: string; canComment: boolean; commentOpen: boolean; commentPath: string; commentRange: string;
  commentPreview: string; commentCount: number;
}
interface Section extends ReviewSection { diff: string | null; source: DiffSource | null; lazy: LazyPatches | null }
interface GitHeader { id: string; kind: 'working-tree' | 'branch-range'; title: string; subtitle: string }
/** Logical presentation has no read authority or retained diff/native data. */
interface Presentation {
  selected: string | null; expanded: Record<string, string[] | undefined>; viewed: Record<string, string[] | undefined>;
  headers: GitHeader[]; revision: number;
}
interface Pick { composerOwner: string; sectionId: string; path: string; anchor: number; start: number; end: number }
interface State {
  owner: string; serial: number; version: number; ui: Presentation | null; sections: Section[]; selected: string; selectedPath: string;
  navigationSerial: number; loading: boolean; error: string; canReadFiles: boolean; checked: boolean;
  revealed: Set<string>; pick: Pick | null; ranging: boolean; commentOpen: boolean;
  comments: { composerOwner: string; contextId: string; comment: ReviewCommentContext }[];
}
const states = new WeakMap<T3Client, State>();
const presentations = new WeakMap<T3Client, Map<string, Presentation>>();
function presentationOf(client: T3Client): Presentation | null {
  if (!client.environmentId || !client.threadId) return null;
  let records = presentations.get(client);
  if (!records) { records = new Map(); presentations.set(client, records); }
  const key = JSON.stringify([client.environmentId, client.threadId]);
  let record = records.get(key);
  if (!record) { record = { selected: null, expanded: {}, viewed: {}, headers: [], revision: 0 }; records.set(key, record); }
  return record;
}
function currentState(client: T3Client, state: State, serial = state.serial): boolean {
  return states.get(client) === state && reviewOwner(client) === state.owner && state.serial === serial && presentationOf(client) === state.ui;
}
function stateOf(client: T3Client): State {
  const owner = reviewOwner(client); let state = states.get(client);
  if (!state || state.owner !== owner) {
    state = { owner, serial: 0, version: 0, ui: presentationOf(client), sections: [], selected: '', selectedPath: '', navigationSerial: 0, loading: false, error: '', canReadFiles: false, checked: false,
      revealed: new Set(), pick: null, ranging: false, commentOpen: false, comments: [] };
    states.set(client, state);
  }
  return state;
}
export const EMPTY_MOBILE_REVIEW: ReviewSnapshot = { owner: '', revision: 0, navigationRevision: 0, sectionId: '', patchRequest: '', title: 'Review changes', subtitle: '', sections: [], files: [], rows: [],
  loading: false, error: '', notice: '', raw: '', emptyTitle: 'No review diffs', emptyDetail: '', additions: 0, deletions: 0, selectedPath: '',
  selectionTitle: '', canComment: false, commentOpen: false, commentPath: '', commentRange: '', commentPreview: '', commentCount: 0 };
const snapshots = new WeakMap<State, { version: number; uiRevision: number; dark: boolean; draft: string; blocked: boolean; data: ReviewSnapshot }>();
function patchRequest(section: Section | undefined): string {
  const files = section?.lazy ? unanswered(section.lazy) : [];
  return files.length ? JSON.stringify(files.map(file => file.path)) : '';
}
const errorText = (error: unknown) => error instanceof Error ? error.message : 'Review unavailable';
const fileKey = (state: State, path: string) => `${state.selected}\n${path}`;
function readySections(client: T3Client, previous: Section[]): Section[] {
  // The desktop helper deduplicates by run and includes non-ready checkpoints. Mobile
  // deriveThreadCheckpointSummaries/getReadyReviewCheckpoints retains each ready entity.
  const checkpoints = arr(client.projection.checkpoints).filter(row => row.status === 'ready'
    && typeof row.runId === 'string' && typeof row.appRunOrdinal === 'number')
    .sort((a, b) => Number(b.appRunOrdinal) - Number(a.appRunOrdinal) || str(b.capturedAt).localeCompare(str(a.capturedAt)));
  return checkpoints.map(checkpoint => {
    const ordinal = Number(checkpoint.appRunOrdinal), id = `turn:${ordinal}`, cached = previous.find(row => row.id === id);
    const count = arr(checkpoint.files).length;
    return { id, title: `Turn ${ordinal}`, subtitle: `${count} file${count === 1 ? '' : 's'} changed`, selected: false,
      diff: cached?.diff ?? null, source: null, lazy: null };
  });
}
function select(state: State, id: string) {
  const next = state.sections.find(row => row.id === id) ?? state.sections.find(row => row.id === 'git:branch-range') ?? state.sections[0];
  if (next?.id !== state.selected) { state.navigationSerial++; state.selectedPath = ''; state.pick = null; state.ranging = false; state.commentOpen = false; }
  state.selected = next?.id ?? '';
  if (next && state.ui && state.ui.selected !== next.id) { state.ui.selected = next.id; state.ui.revision++; }
}
function headerSections(state: State, loading: boolean): Section[] {
  const headers = [...(state.ui?.headers ?? [])];
  if (loading && !headers.some(row => row.id === 'git:branch-range')) headers.unshift({ id: 'git:branch-range', kind: 'branch-range', title: 'Changes', subtitle: '' });
  return headers.map(({ id, title, subtitle }) => ({ id, title, subtitle, selected: false, diff: null, source: null, lazy: null }));
}
function validIds(files: ReviewPatchFile[], ids: string[] | undefined, defaultExpanded: boolean): string[] {
  if (ids === undefined) return defaultExpanded ? files.map(file => file.id) : [];
  const current = new Set(files.map(file => file.id)); return ids.filter(id => current.has(id));
}
function sectionFiles(section: Section | undefined): { file: ReviewPatchFile; notice: string; pending: boolean; error: boolean }[] {
  if (!section) return [];
  if (!section.lazy) return reviewPatch(section.diff ?? '', section.id).files.map(file => ({ file, notice: '', pending: false, error: false }));
  return section.lazy.files.map(stat => {
    const patch = section.lazy!.patches.get(stat.path), parsed = patch?.state === 'loaded' ? reviewPatch(patch.diff, section.id).files : [];
    const file = parsed.find(row => row.path === stat.path) ?? (parsed.length === 1 ? parsed[0] : undefined);
    return { file: { ...(file ?? { path: stat.path, previous: stat.previousPath ?? '', status: stat.previousPath ? 'renamed' : 'modified', binary: false, hunks: [] }),
      id: stat.path, path: stat.path, additions: stat.additions, deletions: stat.deletions }, pending: !patch || patch.state === 'loading', error: patch?.state === 'error',
      notice: !patch || patch.state === 'loading' ? 'Loading diff…' : patch.state === 'error' ? 'Could not load diff. Select the file to retry.'
        : patch.truncated ? 'File preview exceeds the size limit. Counts include all changes.' : file ? '' : 'Could not display file preview.' };
  });
}
function commentFromPick(state: State, text: string, id: string): ReviewCommentContext | null {
  const pick = state.pick, section = state.sections.find(row => row.id === pick?.sectionId);
  const file = sectionFiles(section).find(row => row.file.path === pick?.path)?.file;
  if (!pick || !section || !file) return null;
  const lines = diffReviewLines(file), start = lines[pick.start], end = lines[pick.end];
  if (!start || !end) return null;
  const side = (line: typeof start) => line.change === 'delete' ? 'deletions' as const : 'additions' as const;
  return buildDiffReviewComment({ id, sectionId: section.id, sectionTitle: section.title, filePath: file.path, lines, text,
    range: { start: start.newLineNumber ?? start.oldLineNumber ?? 0, side: side(start), end: end.newLineNumber ?? end.oldLineNumber ?? 0, endSide: side(end) } });
}

/** Pure projection. Root can read while an awaited refresh is pending. */
export function mobileReviewSnapshot(dark = false, client: T3Client = mobileClient, active = true): ReviewSnapshot {
  if (!active) return EMPTY_MOBILE_REVIEW;
  const state = stateOf(client), cached = snapshots.get(state), blocked = !!client.pending || client.busy;
  if (cached && cached.version === state.version && cached.uiRevision === (state.ui?.revision ?? 0) && cached.dark === dark && cached.draft === `${mobileComposerTarget(client).owner}:${mobileComposerTargetText(client, mobileComposerTarget(client)) ?? ''}` && cached.blocked === blocked)
    return { ...cached.data, revision: client.revision };
  const section = state.sections.find(row => row.id === state.selected), parsed = reviewPatch(section?.diff ?? '', state.selected);
  const loaded = sectionFiles(section), files: ReviewFile[] = [], rows: ReviewRow[] = [];
  const currentFiles = loaded.map(row => row.file), expandedIds = new Set(validIds(currentFiles, state.ui?.expanded[state.selected], true));
  const viewedIds = new Set(validIds(currentFiles, state.ui?.viewed[state.selected], false));
  const composerOwner = mobileComposerTarget(client).owner;
  const comments = state.comments.filter(entry => entry.composerOwner === composerOwner && (mobileComposerTargetText(client, mobileComposerTarget(client)) ?? '').includes(`review-comment/${entry.contextId})`));
  for (const { file, notice, pending, error } of loaded) {
    const key = fileKey(state, file.path), expanded = expandedIds.has(file.id), viewed = viewedIds.has(file.id);
    files.push({ id: file.id, name: file.path.split('/').at(-1) ?? file.path, path: file.path, previousPath: file.previous,
      additions: file.additions, deletions: file.deletions, expanded, viewed, pending, error, notice });
    rows.push({ ...reviewRow(`file:${file.id}`, 'file'), markerId: file.id, path: file.path, title: file.path, detail: file.previous !== file.path ? file.previous : '',
      expanded, viewed, additions: file.additions, deletions: file.deletions, action: 'toggle' });
    if (!expanded) continue;
    if (notice) rows.push({ ...reviewRow(`notice:${file.id}`, 'notice'), path: file.path, detail: notice, action: error ? 'retry' : '' });
    if (pending || error) continue;
    const suppressed = reviewSuppression(file);
    if (suppressed === 'non-text' || suppressed === 'large' && !state.revealed.has(key)) {
      rows.push({ ...reviewRow(`suppressed:${file.id}`, 'suppressed'), path: file.path,
        title: suppressed === 'non-text' ? 'Non-text file' : 'Large diff', detail: suppressed === 'non-text' ? 'Diff preview is not available for this file format.' : 'Large diffs are not rendered by default.', action: suppressed === 'large' ? 'reveal' : '' });
      continue;
    }
    const pick = state.pick?.composerOwner === composerOwner && state.pick.sectionId === section?.id && state.pick.path === file.path ? state.pick : null;
    for (const row of reviewFileRows(file, dark, pick)) {
      rows.push({ ...row, id: JSON.stringify(['review-row', file.id, row.id]) });
      if (row.kind !== 'line') continue;
      for (const entry of comments) {
        if (entry.comment.sectionId !== state.selected || entry.comment.filePath !== file.path || entry.comment.endIndex !== row.lineIndex) continue;
        rows.push({ ...reviewRow(JSON.stringify(['review-comment', file.id, entry.contextId]), 'comment'), path: entry.contextId, filePath: file.path, title: entry.comment.rangeLabel,
          text: entry.comment.text, detail: entry.comment.diff, action: 'delete-comment' });
      }
    }
  }
  const comment = state.pick?.composerOwner === composerOwner ? commentFromPick(state, '', 'preview') : null;
  const awaitingRangeEnd = !!comment && state.ranging && state.pick?.start === state.pick?.end;
  const totals = section?.source?.files ?? files;
  const data: ReviewSnapshot = { owner: state.owner, revision: client.revision, navigationRevision: state.navigationSerial, sectionId: state.selected, patchRequest: patchRequest(section), title: section?.title ?? 'Review changes', subtitle: section?.subtitle ?? '',
    sections: state.sections.map(({ id, title, subtitle }) => ({ id, title, subtitle, selected: id === state.selected })), files, rows: rows.map(row => row.kind === 'comment' ? row : { ...row, filePath: row.path }),
    loading: state.loading, error: state.error, notice: parsed.notice, raw: !files.length && parsed.text ? parsed.text : '',
    emptyTitle: section ? 'No changes' : 'No review diffs', emptyDetail: section ? parsed.text ? parsed.rawReason : section.subtitle || 'This diff is empty.' : 'This thread has no ready turn diffs and the worktree diff is empty.',
    additions: totals.reduce((sum, file) => sum + file.additions, 0), deletions: totals.reduce((sum, file) => sum + file.deletions, 0), selectedPath: files.some(file => file.path === state.selectedPath) ? state.selectedPath : '',
    selectionTitle: awaitingRangeEnd ? 'Select range end' : comment ? `Comment on ${comment.rangeLabel}` : '',
    canComment: !!comment && !awaitingRangeEnd && !!client.threadId && !client.pending && !client.busy, commentOpen: state.commentOpen && !!comment,
    commentPath: comment?.filePath ?? '', commentRange: comment?.rangeLabel ?? '', commentPreview: comment?.diff ?? '', commentCount: comments.length };
  snapshots.set(state, { version: state.version, uiRevision: state.ui?.revision ?? 0, dark, draft: `${mobileComposerTarget(client).owner}:${mobileComposerTargetText(client, mobileComposerTarget(client)) ?? ''}`, blocked, data });
  return data;
}

/** Real shared RPC reads. A grant must resolve before cached host files become visible. */
export async function mobileReviewRead(nativeInput: Native | null | undefined, sectionId = '', dark = false, refresh = false, client: T3Client = mobileClient): Promise<ReviewSnapshot> {
  const state = stateOf(client), serial = ++state.serial;
  const current = () => currentState(client, state, serial);
  state.navigationSerial++; state.version++;
  // A known section intent is accepted synchronously; its late read cannot write
  // into a replacement strict owner or normalize a successor logical record.
  if (sectionId && state.ui && state.sections.some(row => row.id === sectionId) && current()) {
    if (state.ui.selected !== sectionId) { state.ui.selected = sectionId; state.ui.revision++; }
  }
  const preferred = sectionId || state.ui?.selected || state.selected;
  if (!nativeInput?.available || !client.ready || !client.environmentId || !client.threadId) {
    state.sections = []; state.selected = ''; state.error = 'Connect and select a thread to review its changes.'; return mobileReviewSnapshot(dark, client);
  }
  const native = reviewNative(client, letGoAware(mobileNative(nativeInput)), state.owner);
  state.loading = true; state.error = ''; state.canReadFiles = false;
  const old = state.sections;
  // These are fresh shells, including checkpoints: no cached data while the
  // current session grant is unresolved. UI headers never authorize an RPC.
  state.sections = [...readySections(client, []), ...headerSections(state, true)];
  if (current()) select(state, preferred);
  try {
    let accessError = '', canReadFiles = false;
    try { canReadFiles = await reviewFileAccess(client, native); }
    catch (error) { if (letGo(error)) throw error; accessError = errorText(error); }
    if (!current()) return mobileReviewSnapshot(dark, client);
    state.checked = true; state.canReadFiles = canReadFiles;
    if (canReadFiles && workspaceOf(client).cwd) {
      const result = await client.rpc(native, 'review.getDiffPreview', { cwd: workspaceOf(client).cwd });
      if (!current()) return mobileReviewSnapshot(dark, client);
      state.sections = readySections(client, old);
      const headers: GitHeader[] = [];
      for (const source of arr(result.sources)) {
        if (source.kind !== 'working-tree' && source.kind !== 'branch-range') continue;
        const parsed = diffSource(result, source.kind), id = `git:${source.kind}`;
        const previous = refresh ? null : old.find(row => row.id === id)?.lazy ?? null;
        const lazy = lazyPatches(parsed, false, previous);
        // Mobile keeps server order and initially requests three files.
        if (lazy && lazy !== previous) { lazy.files = parsed!.files!; lazy.requested = [0, 1, 2].filter(at => at < lazy.files.length); }
        const title = str(source.title) || (source.kind === 'branch-range' ? 'Changes' : 'Uncommitted');
        const subtitle = source.kind === 'working-tree' ? 'Staged, unstaged, and untracked files'
          : source.baseRef ? `${str(source.baseRef)} ... ${str(source.headRef) || 'HEAD'}` : 'Base branch unavailable';
        headers.push({ id, kind: source.kind, title, subtitle });
        state.sections.push({ id, title, subtitle, selected: false, diff: str(source.diff), source: parsed, lazy });
      }
      if (state.ui) { state.ui.headers = headers; state.ui.revision++; }
    } else {
      state.sections = readySections(client, old);
      if (!canReadFiles && !state.sections.length) state.error = accessError || 'This connection cannot read local diffs.';
    }
    // Empty alternatives render nothing but never erase the saved preference.
    if (!current()) return mobileReviewSnapshot(dark, client);
    select(state, state.ui?.selected || state.selected);
    const section = state.sections.find(row => row.id === state.selected);
    if (section?.id.startsWith('turn:') && (refresh || section.diff === null)) {
      const count = Number(section.id.slice(5));
      const result = await client.rpc(native, 'orchestration.getTurnDiff', { threadId: client.threadId, fromTurnCount: Math.max(0, count - 1), toTurnCount: count, ignoreWhitespace: false });
      if (current()) section.diff = str(result.diff);
    }
    if (current() && section?.lazy && section.source) await loadFilePatches(section.lazy, section.source, false, (method, payload) => client.rpc(native, method, payload));
  } catch (error) {
    if (letGo(error)) throw error;
    if (current()) {
      state.error = errorText(error);
      // A settled failure keeps real copied headers, never the loading-only
      // placeholder or an owned section's previous body/source/lazy graph.
      state.sections = [...readySections(client, []), ...(state.canReadFiles ? headerSections(state, false) : [])];
      select(state, state.ui?.selected || state.selected);
    }
  }
  finally { if (current()) { state.loading = false; state.version++; client.revision++; } }
  return mobileReviewSnapshot(dark, client);
}

/** The caller sends the snapshot owner with every action, including delayed modal Save. */
export async function mobileReviewAction(owner: string, op: string, id: string, value: string, n: number,
  nativeInput: Native | null | undefined, suppliedStorage: Files, dark = false, client: T3Client = mobileClient, routeId = '') {
  // Refuse before target/snapshot readers can default malformed cleanup metadata.
  if ((op === 'save' || op === 'delete-comment') && !mobileExternalContextPreflight(client))
    throw new ClientError('The saved draft cleanup metadata is unavailable. Keep the original draft.', 'retained');
  let message = '', navigation = ''; const actionState = stateOf(client);
  let navigationSerial = actionState.navigationSerial, navigationSection = actionState.selected;
  // Native viewport reports are read-side selection, never a navigation or retry.
  // Reject a queued report from a prior route, refresh, section or explicit file pick.
  if (op === 'viewport') {
    const unchanged = () => ({ message: '', navigation: '', data: mobileReviewSnapshot(dark, client) });
    let event: Obj;
    try { event = obj(JSON.parse(id)); } catch { return unchanged(); }
    const section = actionState.sections.find(row => row.id === actionState.selected);
    if (!routeId || event.routeId !== routeId || event.owner !== owner || owner !== actionState.owner
      || event.sectionId !== actionState.selected || event.navigationRevision !== actionState.navigationSerial
      || typeof event.path !== 'string' || event.path !== '' && !sectionFiles(section).some(row => row.file.path === event.path)) return unchanged();
    let changed = actionState.selectedPath !== event.path;
    actionState.selectedPath = event.path;
    if (event.path && section?.lazy) {
      const at = section.lazy.files.findIndex(file => file.path === event.path);
      changed = requestFiles(section.lazy, [at, at + 1, at + 2]).length > 0 || changed;
    }
    if (changed) { actionState.version++; client.revision++; }
    return unchanged();
  }
  try {
    assertReviewOwner(client, owner);
    if (!nativeInput?.available) throw new ClientError('Review is available in the native app.');
    const native = reviewNative(client, letGoAware(mobileNative(nativeInput)), owner);
    const storage = client === mobileClient ? nativeFiles(native) : suppliedStorage;
    const state = stateOf(client), section = state.sections.find(row => row.id === state.selected), key = fileKey(state, id);
    if (op === 'section' || op === 'refresh') return { message: '', navigation: '', data: await mobileReviewRead(nativeInput, op === 'section' ? id : state.selected, dark, op === 'refresh', client) };
    if (op === 'file') {
      navigationSerial = ++state.navigationSerial; navigationSection = state.selected;
      state.pick = null; state.ranging = false; state.commentOpen = false;
    }
    if (op === 'patches') {
      if (section?.id === id && section.lazy && section.source && value === patchRequest(section)) {
        await loadFilePatches(section.lazy, section.source, false, (method, payload) => client.rpc(native, method, payload));
      }
    }
    else if (op === 'file' && !id) { state.selectedPath = ''; navigation = 'top'; }
    else if (op === 'clear' || op === 'cancel') { state.pick = null; state.ranging = false; state.commentOpen = false; }
    else if (op === 'comment') {
      assertReviewComposerOwner(client, state.pick?.composerOwner ?? 'missing-comment-owner');
      state.commentOpen = !!commentFromPick(state, '', 'preview');
    }
    else if (op === 'delete-comment') {
      const entry = state.comments.find(row => row.contextId === id);
      if (!entry) throw new ClientError('That comment is no longer available.');
      assertReviewComposerOwner(client, entry.composerOwner);
      const target = mobileComposerTarget(client);
      if (target.kind === 'queued-edit') {
        const link = contextLink('review-comment', id, str(reviewCommentContextRecord(entry.comment).label));
        const text = (mobileComposerTargetText(client, target) ?? '').replace(link, '');
        await mobileComposerEditContext(client, target, text, null, id, native);
      } else if (mobileNewTaskDraftIsKey(target.key)) {
        const link = new RegExp(`\\[[^\\]\\n]{0,512}\\]\\(t3-context://v1/review-comment/${id.replace(/[^a-z0-9_-]/gi, '')}\\) ?`, 'g');
        const text = (mobileComposerTargetText(client, target) ?? '').replace(link, '');
        await mobileComposerNewTaskContext(client, target, text, undefined, id, native, storage);
      } else {
        // Pinned mobile removes an exact selected chip occurrence from its inspector.
        // The desktop helper deletes every matching ID and cannot publish owned files.
        if (mobileEditorDocumentMembership(client, target) !== 'unenrolled')
          throw new ClientError('Removing saved context from Review is not available yet. Edit the context in the composer.');
        await removeReviewCommentChip(client, reviewComposerNative(client, native, storage, owner), id);
      }
      assertReviewComposerOwner(client, target.owner);
      state.comments = state.comments.filter(row => row !== entry);
    } else if (op === 'save') {
      assertReviewComposerOwner(client, state.pick?.composerOwner ?? 'missing-comment-owner');
      if (!value.trim() || !state.commentOpen || client.busy || client.pending) throw new ClientError('Select lines and enter a comment first.');
      const comment = commentFromPick(state, value, `review-${localId()}`);
      if (!comment) throw new ClientError('The selected lines changed. Select them again.');
      const record = reviewCommentContextRecord(comment);
      const target = mobileComposerTarget(client);
      if (target.kind === 'queued-edit') {
        const text = `${mobileComposerTargetText(client, target) ?? ''}${contextLink('review-comment', str(record.contextId), str(record.label))} `;
        await mobileComposerEditContext(client, target, text, record, '', native);
      } else if (mobileNewTaskDraftIsKey(target.key)) {
        const text = `${mobileComposerTargetText(client, target) ?? ''}${contextLink('review-comment', str(record.contextId), str(record.label))} `;
        await mobileComposerNewTaskContext(client, target, text, record, '', native, storage);
      } else {
        const insertion = mobileExternalContextCapture(client, target, 'review', owner);
        if (insertion?.kind === 'owned') {
          mobileExternalContextInsert(client, insertion, contextLink('review-comment', str(record.contextId), str(record.label)), record, reviewOwner(client));
          // Acceptance is synchronous. A failed or abandoned save must not leave Submit
          // available for the same comment, or add a card to a later route on resumption.
          state.comments.push({ composerOwner: target.owner, contextId: record.contextId, comment });
          state.pick = null; state.ranging = false; state.commentOpen = false; state.version++;
          message = await mobileExternalContextPersist(client, storage);
          return { message, navigation: '', data: mobileReviewSnapshot(dark, client) };
        }
        await addReviewCommentChip(client, reviewComposerNative(client, native, storage, owner), record);
      }
      assertReviewComposerOwner(client, target.owner);
      state.comments.push({ composerOwner: target.owner, contextId: record.contextId, comment }); state.pick = null; state.ranging = false; state.commentOpen = false;
    } else {
      const loaded = sectionFiles(section), markerAction = op === 'toggle' || op === 'viewed';
      const file = loaded.find(row => (markerAction ? row.file.id : row.file.path) === id);
      if (!file) throw new ClientError('That file is no longer in this diff.');
      const currentFiles = loaded.map(row => row.file), markerId = file.file.id;
      const expanded = () => validIds(currentFiles, state.ui?.expanded[state.selected], true);
      if (markerAction) {
        if (!state.ui || !currentState(client, state)) throw new ClientError('Select a thread to change its Review presentation.');
        if (op === 'toggle') {
          const ids = expanded(); state.ui.expanded[state.selected] = ids.includes(markerId) ? ids.filter(value => value !== markerId) : [...ids, markerId];
        } else {
          const ids = validIds(currentFiles, state.ui.viewed[state.selected], false), marked = ids.includes(markerId);
          state.ui.viewed[state.selected] = marked ? ids.filter(value => value !== markerId) : [...ids, markerId];
          if (!marked) state.ui.expanded[state.selected] = expanded().filter(value => value !== markerId);
        }
        state.ui.revision++;
      }
      else if (op === 'reveal') state.revealed.add(key);
      else if (op === 'line' || op === 'extend' || op === 'range-start') {
        if (state.commentOpen && state.pick?.composerOwner === mobileComposerTarget(client).owner || !Number.isInteger(n) || !diffReviewLines(file.file)[n]) throw new ClientError('That line is unavailable.');
        const composerOwner = mobileComposerTarget(client).owner;
        const extending = op !== 'range-start' && (op === 'extend' || state.ranging) && state.pick?.composerOwner === composerOwner && state.pick.path === id && state.pick.sectionId === state.selected;
        const anchor = extending ? state.pick!.anchor : n;
        state.pick = { composerOwner, sectionId: state.selected, path: id, anchor, start: Math.min(anchor, n), end: Math.max(anchor, n) };
        state.ranging = op === 'range-start' || extending;
        // Native upstream retains a long-press anchor while taps adjust its endpoint.
        state.commentOpen = op === 'line' && !extending;
      } else if (op !== 'file' && op !== 'retry' && op !== 'visible') throw new ClientError('Unsupported review action.');
      if (['file', 'retry', 'visible', 'toggle'].includes(op)) {
        if (op === 'file') {
          state.selectedPath = id; navigation = `file:${markerId}`;
          const ids = expanded();
          if (!ids.includes(markerId) && state.ui && currentState(client, state)) { state.ui.expanded[state.selected] = [...ids, markerId]; state.ui.revision++; }
        }
        if (section?.lazy && section.source && (op !== 'toggle' || expanded().includes(markerId))) {
          const at = section.lazy.files.findIndex(row => row.path === file.file.path);
          if (op === 'retry' || file.error && (op === 'file' || op === 'toggle')) retryFile(section.lazy, id);
          requestFiles(section.lazy, [at, at + 1, at + 2]);
          if (op !== 'file') await loadFilePatches(section.lazy, section.source, false, (method, payload) => client.rpc(native, method, payload));
        }
      }
    }
  } catch (error) { if (letGo(error)) throw error; message = errorText(error); }
  finally { actionState.version++; client.revision++; }
  return { message, navigation: !message && stateOf(client) === actionState && actionState.navigationSerial === navigationSerial
    && actionState.selected === navigationSection ? navigation : '', data: mobileReviewSnapshot(dark, client) };
}
