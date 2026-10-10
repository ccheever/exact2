import { describe, expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import type { Native, Files } from './shared/protocol';
import { messageContext } from './shared/composer-editor';
import { mobileReviewRead, mobileReviewAction, mobileReviewSnapshot } from './review-data';
import { reviewPatch, reviewSuppression } from './review-model';

const patch = (path = 'src/a.ts') => `diff --git a/${path} b/${path}\n--- a/${path}\n+++ b/${path}\n@@ -1,2 +1,2 @@\n const before = 1;\n-old\n+new\n`;
function fixture() {
  const client = new T3Client(); client.origin = 'https://example.test'; client.environmentId = 'one'; client.projectId = 'p'; client.threadId = 't';
  client.connection = 'connected'; client.configLive = true; client.shellLive = true; client.threadLive = true; client.generation = 3;
  client.shell.projects = [{ id: 'p', title: 'Project', workspaceRoot: '/repo' }]; client.shell.threads = [{ id: 't', projectId: 'p' }];
  client.thread = { sequence: 1, hasMore: false, historyCursor: null, latestLocalTurnOrdinal: null,
    projection: { thread: { id: 't' }, checkpoints: [
      { runId: 'r1', appRunOrdinal: 1, status: 'ready', files: [{ path: 'a' }] },
      { runId: 'r2', appRunOrdinal: 2, status: 'missing', files: [] },
      { runId: 'r3', appRunOrdinal: 3, status: 'ready', files: [] }], runs: [] } };
  const calls: Obj[] = []; let hook: ((request: Obj) => unknown) | undefined;
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request); const custom = hook?.(request);
    if (custom !== undefined) return await custom;
    const value = request.op === 'http' ? { authenticated: true, permissions: ['filesystem:read'] }
      : request.method === 'review.getDiffPreview' ? { cwd: '/repo', sources: [{ kind: 'working-tree', title: 'Uncommitted', diff: '' }, { kind: 'branch-range', title: 'Changes', baseRef: 'main', headRef: 'feature', diffHash: 'a', diff: patch() }] }
      : request.method === 'orchestration.getTurnDiff' ? { diff: patch('turn.ts') } : {};
    return { ok: true, generation: client.generation, value };
  } };
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new ArrayBuffer(0); }, async atomicWriteFile() {} } };
  return { client, calls, native, storage, hook(fn: typeof hook) { hook = fn; } };
}
const read = (f: ReturnType<typeof fixture>, section = '', refresh = false) => mobileReviewRead(f.native, section, false, refresh, f.client);
const act = (f: ReturnType<typeof fixture>, op: string, id = '', value = '', n = 0, owner = mobileReviewSnapshot(false, f.client).owner) => mobileReviewAction(owner, op, id, value, n, f.native, f.storage, false, f.client);

describe('mobile review source rules and real shared RPC ownership', () => {
  test('Changes default, ready checkpoints only, source labels, and turn whitespace retained', async () => {
    const f = fixture(), view = await read(f);
    expect(view.sections.map(row => row.id)).toEqual(['turn:3', 'turn:1', 'git:working-tree', 'git:branch-range']);
    expect(view.sectionId).toBe('git:branch-range'); expect(view.subtitle).toBe('main ... feature');
    expect(view.files.map(row => row.path)).toEqual(['src/a.ts']); expect(view.rows.filter(row => row.kind === 'line')).toHaveLength(3);
    expect(view.additions).toBe(1); expect(view.deletions).toBe(1);
    expect((await read(f, 'turn:1')).files[0]?.path).toBe('turn.ts');
    expect(f.calls.find(call => call.method === 'orchestration.getTurnDiff')?.payload).toEqual({ threadId: 't', fromTurnCount: 0, toTurnCount: 1, ignoreWhitespace: false });
  });
  test('explicit permission denial removes cached host diffs, still permits ready turn RPC', async () => {
    const f = fixture(); await read(f);
    f.hook(request => request.op === 'http' ? { ok: true, generation: 3, value: { authenticated: true, permissions: [], scopes: ['filesystem:read'] } } : undefined);
    f.calls.length = 0; const denied = await read(f);
    expect(denied.sections.map(row => row.id)).toEqual(['turn:3', 'turn:1']);
    expect(denied.files[0]?.path).toBe('turn.ts'); expect(f.calls.some(call => call.method === 'review.getDiffPreview')).toBe(false);
  });
  test('three lazy files keep server order; visible file requests next two; counts use metadata', async () => {
    const f = fixture(), paths = ['z.ts', 'a.ts', 'm.ts', 'b.ts', 'q.ts'];
    f.hook(request => {
      if (request.method !== 'review.getDiffPreview') return;
      const file = obj(obj(request.payload).file);
      return { ok: true, generation: 3, value: { cwd: '/repo', sources: [{ kind: 'branch-range', title: 'Changes', diffHash: 'large', truncated: !file.path,
        files: paths.map(path => ({ path, previousPath: null, additions: 10, deletions: 2 })), diff: file.path ? patch(String(file.path)) : patch('z.ts') }] } };
    });
    const view = await read(f); expect(view.files.map(file => file.path)).toEqual(paths); expect(view.additions).toBe(50);
    expect(f.calls.filter(call => obj(obj(call.payload).file).path).map(call => obj(obj(call.payload).file).path)).toEqual(paths.slice(0, 3));
    await act(f, 'visible', 'm.ts');
    expect(f.calls.filter(call => obj(obj(call.payload).file).path).map(call => obj(obj(call.payload).file).path)).toEqual(paths);
  });
  test('stale response cannot replace newly selected workspace or mutate its draft', async () => {
    const f = fixture(); let release!: (value: unknown) => void;
    const gate = new Promise(resolve => { release = resolve; }); let started!: () => void;
    const begun = new Promise<void>(resolve => { started = resolve; });
    f.hook(request => { if (request.method === 'review.getDiffPreview') { started(); return gate; } });
    const pending = read(f); await begun; f.client.threadId = 'other'; f.client.threadEpoch++;
    release({ ok: true, generation: 3, value: { cwd: '/repo', sources: [{ kind: 'branch-range', diff: patch('old.ts') }] } });
    await expect(pending).rejects.toMatchObject({ kind: 'superseded' });
    expect(mobileReviewSnapshot(false, f.client).error).toBe('');
  });
  test('line comment carries real selected patch into the shared outbound message context', async () => {
    const f = fixture(); const view = await read(f);
    await act(f, 'range-start', 'src/a.ts', '', 1); await act(f, 'extend', 'src/a.ts', '', 2); await act(f, 'comment');
    expect((await act(f, 'save', '', 'Please explain this change')).message).toBe('');
    expect(f.client.draft).toContain('t3-context://v1/review-comment/');
    const records = obj(messageContext(f.client, f.client.draft)).records as Obj[];
    expect(records).toHaveLength(1); expect(records[0]).toMatchObject({ sectionId: 'git:branch-range', filePath: 'src/a.ts', text: 'Please explain this change', diff: '@@ -2,1 +2,1 @@\n-old\n+new' });
    expect(f.calls.some(call => call.op === 'editorInsert' || call.method === 'orchestration.dispatchCommand')).toBe(false);
    expect((await act(f, 'delete-comment', String(records[0]!.contextId))).message).toBe(''); expect(f.client.draft.trim()).toBe('');
    f.client.threadId = 'other'; await expect(act(f, 'save', '', 'Wrong thread', 0, view.owner)).rejects.toMatchObject({ kind: 'superseded' });
    expect(f.client.draft).toBe('');
  });
  test('large/non-text suppression and raw/truncated fallback use pinned boundaries', () => {
    const file = reviewPatch(patch()).files[0]!;
    expect(reviewSuppression(file)).toBe(''); expect(reviewSuppression({ ...file, path: 'image.png' })).toBe('non-text');
    file.hunks[0]!.lines = Array.from({ length: 401 }, (_, n) => ({ kind: 'addition', text: 'x', old: 0, next: n + 1 }));
    expect(reviewSuppression(file)).toBe('large');
    expect(reviewPatch('unexpected\n[truncated]').notice).toContain('server size cap');
    expect(reviewPatch('unexpected').rawReason).toBe('Unsupported diff format. Showing raw patch.');
  });
  test('inactive snapshot skips projection, unchanged active snapshot retains row/token objects', async () => {
    const f = fixture(); const first = await read(f); f.client.revision++;
    expect(mobileReviewSnapshot(false, f.client).rows).toBe(first.rows);
    expect(mobileReviewSnapshot(false, f.client, false).rows).toEqual([]);
    await act(f, 'line', 'src/a.ts', '', 0);
    expect(mobileReviewSnapshot(false, f.client).commentOpen).toBe(true);
    await act(f, 'cancel');
    await act(f, 'range-start', 'src/a.ts', '', 1);
    expect(mobileReviewSnapshot(false, f.client)).toMatchObject({ selectionTitle: 'Select range end', commentOpen: false, canComment: false });
    await act(f, 'line', 'src/a.ts', '', 2);
    expect(mobileReviewSnapshot(false, f.client)).toMatchObject({ commentOpen: false, canComment: true });
  });
});


describe('Review navigator selection and viewed state', () => {
  test('select reveals a collapsed file, clears a comment range, and repeat selection can return to top', async () => {
    const f = fixture(); await read(f);
    await act(f, 'range-start', 'src/a.ts', '', 1);
    await act(f, 'toggle', 'src/a.ts');
    const selected = await act(f, 'file', 'src/a.ts');
    expect(selected.navigation).toBe('file:src/a.ts');
    expect(selected.data).toMatchObject({ selectedPath: 'src/a.ts', canComment: false, commentOpen: false, selectionTitle: '' });
    expect(selected.data.files[0]?.expanded).toBe(true);
    const cleared = await act(f, 'file');
    expect(cleared.navigation).toBe('top'); expect(cleared.data.selectedPath).toBe('');
    expect((await act(f, 'file', 'missing.ts')).navigation).toBe('');
  });
  test('marking viewed collapses; marking unviewed preserves collapse; selection reopens without clearing viewed', async () => {
    const f = fixture(); await read(f);
    expect((await act(f, 'viewed', 'src/a.ts')).data.files[0]).toMatchObject({ viewed: true, expanded: false });
    expect((await act(f, 'viewed', 'src/a.ts')).data.files[0]).toMatchObject({ viewed: false, expanded: false });
    await act(f, 'viewed', 'src/a.ts');
    expect((await act(f, 'file', 'src/a.ts')).data.files[0]).toMatchObject({ viewed: true, expanded: true });
    expect((await act(f, 'section', 'turn:1')).data.selectedPath).toBe('');
  });
  test('lazy navigation returns immediately and later patches cannot override return-to-top selection', async () => {
    const f = fixture(), paths = ['a.ts', 'b.ts', 'c.ts', 'd.ts'];
    let release!: (value: unknown) => void, entered!: () => void;
    const gate = new Promise(resolve => { release = resolve; });
    const began = new Promise<void>(resolve => { entered = resolve; });
    const reply = (file: string) => ({ ok: true, generation: 3, value: { cwd: '/repo', sources: [{ kind: 'branch-range', title: 'Changes', diffHash: 'navigation',
      truncated: !file, files: paths.map(path => ({ path, previousPath: null, additions: 1, deletions: 1 })), diff: patch(file || 'a.ts') }] } });
    f.hook(request => {
      if (request.method !== 'review.getDiffPreview') return;
      const file = String(obj(obj(request.payload).file).path || '');
      if (file === 'd.ts') { entered(); return gate; }
      return reply(file);
    });
    await read(f);
    const selected = await act(f, 'file', 'd.ts');
    expect(selected.navigation).toBe('file:d.ts');
    expect(selected.data.patchRequest).toBe(JSON.stringify(['d.ts']));
    expect(f.calls.some(call => obj(obj(call.payload).file).path === 'd.ts')).toBe(false);
    const pending = act(f, 'patches', selected.data.sectionId, selected.data.patchRequest); await began;
    expect((await act(f, 'file')).navigation).toBe('top');
    release(reply('d.ts'));
    const stale = await pending;
    expect(stale.navigation).toBe(''); expect(stale.data.selectedPath).toBe('');
  });
});


test('queued comment selection cannot retarget ordinary content after edit ends; diffs remain readable', async () => {
  const { queuedEditState, queuedEditThreadKey, queuedEditEndMemory } = await import('./queued-edit-state');
  const f = fixture(); f.client.local.drafts[f.client.draftKey] = 'ordinary untouched';
  const edit = { owner: 'comment-edit', session: 'comment-session', draftKey: 'one:t~queued-edit~r', origin: f.client.origin,
    environmentId: 'one', threadId: 't', projectId: 'p', generation: 3, revision: 1, runId: 'r', messageId: 'm', text: 'queued',
    attachments: [], existingAttachments: [], saving: false };
  const state = queuedEditState(f.client); state.sessions.set(edit.owner, edit); state.active.set(queuedEditThreadKey('one', 't'), edit.owner);
  const view = await read(f); await act(f, 'range-start', 'src/a.ts', '', 1); await act(f, 'extend', 'src/a.ts', '', 2); await act(f, 'comment');
  queuedEditEndMemory(edit.owner, f.client);
  const after = mobileReviewSnapshot(false, f.client);
  expect(after.owner).toBe(view.owner); expect(after.files).toHaveLength(1); expect(after.commentOpen).toBe(false);
  await expect(act(f, 'save', '', 'Old queued comment', 0, view.owner)).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.client.draft).toBe('ordinary untouched');
  await act(f, 'line', 'src/a.ts', '', 1);
  expect((await act(f, 'save', '', 'New ordinary comment')).message).toBe('');
  expect(f.client.draft).toContain('t3-context://v1/review-comment/');
});


test('late queued comment persistence cannot clear a replacement composer selection', async () => {
  const { queuedEditState, queuedEditThreadKey, queuedEditEndMemory } = await import('./queued-edit-state');
  const f = fixture(); const state = queuedEditState(f.client), key = queuedEditThreadKey('one', 't');
  const edit = { owner: 'prior-comment', session: 'prior-session', draftKey: 'one:t~queued-edit~r', origin: f.client.origin,
    environmentId: 'one', threadId: 't', projectId: 'p', generation: 3, revision: 1, runId: 'r', messageId: 'm', text: 'queued',
    attachments: [], existingAttachments: [], saving: false };
  state.sessions.set(edit.owner, edit); state.active.set(key, edit.owner);
  await read(f); await act(f, 'line', 'src/a.ts', '', 1);
  let release!: (value: unknown) => void, started!: () => void;
  const gate = new Promise(resolve => { release = resolve; }), began = new Promise<void>(resolve => { started = resolve; });
  f.hook(request => { if (request.op === 'mobileQueuedEdit') { started(); return gate; } });
  const pending = act(f, 'save', '', 'Old comment'); await began;
  queuedEditEndMemory(edit.owner, f.client);
  const next = { ...edit, owner: 'next-comment', session: 'next-session' }; state.sessions.set(next.owner, next); state.active.set(key, next.owner);
  await act(f, 'line', 'src/a.ts', '', 2);
  expect(mobileReviewSnapshot(false, f.client).commentOpen).toBe(true);
  release({ ok: true, generation: 3, value: {} });
  await expect(pending).rejects.toMatchObject({ kind: 'superseded' });
  expect(mobileReviewSnapshot(false, f.client).commentOpen).toBe(true);
  expect(state.sessions.get(next.owner)?.text).toBe('queued'); expect(f.client.draft).toBe('');
});


test('manual viewport follows files and top without navigating or altering comment selection', async () => {
  const f = fixture(); const first = await read(f);
  await act(f, 'line', 'src/a.ts', '', 1);
  const report = (path: string, extra: Obj = {}, route = 'review-1') => mobileReviewAction(first.owner, 'viewport', JSON.stringify({
    owner: first.owner, sectionId: first.sectionId, navigationRevision: first.navigationRevision, routeId: 'review-1', path, ...extra,
  }), '', 0, f.native, f.storage, false, f.client, route);
  f.calls.length = 0;
  const file = await report('src/a.ts');
  expect(file.data.selectedPath).toBe('src/a.ts'); expect(file.navigation).toBe('');
  expect(file.data.commentOpen).toBe(true); expect(file.data.commentRange).toBeTruthy();
  const revision = f.client.revision;
  await report('src/a.ts'); expect(f.client.revision).toBe(revision);
  for (const extra of [{ owner: 'prior' }, { sectionId: 'prior' }, { navigationRevision: first.navigationRevision - 1 }, { routeId: 'prior' }, { path: 'missing.ts' }]) {
    expect((await report('', extra)).data.selectedPath).toBe('src/a.ts');
  }
  expect((await report('', {}, 'another-route')).data.selectedPath).toBe('src/a.ts');
  expect((await report('')).data.selectedPath).toBe('');
  expect(f.calls).toHaveLength(0);
  await act(f, 'file', 'src/a.ts');
  expect((await report('')).data.selectedPath).toBe('src/a.ts');
  const afterPick = mobileReviewSnapshot(false, f.client);
  await read(f, '', true);
  const afterRefresh = f.client.revision;
  expect((await report('', { navigationRevision: afterPick.navigationRevision })).data.selectedPath).toBe('src/a.ts');
  expect(f.client.revision).toBe(afterRefresh);
  expect(f.client.revision).toBeGreaterThan(revision);
});

test('viewport queues the current file and next two without awaiting reads or retrying failed patches', async () => {
  const f = fixture(), paths = ['a.ts', 'b.ts', 'c.ts', 'd.ts', 'e.ts', 'f.ts'];
  f.hook(request => {
    if (request.method !== 'review.getDiffPreview') return;
    const path = String(obj(obj(request.payload).file).path || '');
    return { ok: true, generation: 3, value: { cwd: '/repo', sources: [{ kind: 'branch-range', title: 'Changes', diffHash: 'viewport',
      truncated: !path, files: paths.map(path => ({ path, previousPath: null, additions: 1, deletions: 1 })), diff: path === 'b.ts' ? '' : patch(path || 'a.ts') }] } };
  });
  const first = await read(f);
  expect(first.files.find(file => file.path === 'b.ts')?.error).toBe(true);
  const report = (path: string) => mobileReviewAction(first.owner, 'viewport', JSON.stringify({ owner: first.owner, sectionId: first.sectionId,
    navigationRevision: first.navigationRevision, routeId: 'review-2', path }), '', 0, f.native, f.storage, false, f.client, 'review-2');
  f.calls.length = 0;
  const selected = await report('d.ts');
  expect(selected.data.selectedPath).toBe('d.ts'); expect(selected.navigation).toBe('');
  expect(selected.data.patchRequest).toBe(JSON.stringify(['d.ts', 'e.ts', 'f.ts']));
  expect(f.calls).toHaveLength(0);
  await report('b.ts');
  expect(mobileReviewSnapshot(false, f.client).files.find(file => file.path === 'b.ts')?.error).toBe(true);
  await act(f, 'patches', selected.data.sectionId, selected.data.patchRequest);
  expect(f.calls.filter(call => obj(obj(call.payload).file).path).map(call => obj(obj(call.payload).file).path)).toEqual(paths.slice(3));
  expect(mobileReviewSnapshot(false, f.client).selectedPath).toBe('b.ts');
});

test('comment row keeps its diff file identity independently from its delete action context', async () => {
  const f = fixture(); await read(f); await act(f, 'line', 'src/a.ts', '', 1); await act(f, 'save', '', 'Review this');
  const rows = mobileReviewSnapshot(false, f.client).rows;
  expect(rows.every(row => row.filePath === 'src/a.ts')).toBe(true);
  const comment = rows.find(row => row.kind === 'comment');
  expect(comment?.path).not.toBe(comment?.filePath);
});
