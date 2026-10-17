import { describe, expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import type { Native, Files } from './shared/protocol';
import { messageContext } from './shared/composer-editor';
import { mobileReviewRead, mobileReviewAction, mobileReviewSnapshot } from './review-data';
import { reviewPatch, reviewFileRows } from './review-model';

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
const marker = (f: ReturnType<typeof fixture>, path: string) => {
  const file = mobileReviewSnapshot(false, f.client).files.find(file => file.path === path);
  if (!file) throw new Error(`Missing current rendered file ${path}`);
  return file.id;
};
async function knownWorking(f: ReturnType<typeof fixture>) {
  const bootstrap = await read(f), header = bootstrap.sections.find(row => row.id === 'git:working-tree');
  expect(header?.id).toBe('git:working-tree');
  if (!header) throw new Error('Fixture prerequisite missing current Working section');
  console.log(`RETENTION_BOOTSTRAP_RECEIPT ${JSON.stringify({ scope: [f.client.environmentId, f.client.threadId],
    section: bootstrap.sectionId, header, nativeCalls: f.calls.length, calls: [...f.calls] })}`);
  // The complete cold setup receipt is retained above. Measure only the original
  // transitions below, beginning with an actual selectable Working intent.
  f.calls.length = 0;
  return read(f, header.id);
}

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
  test.each([
    { name: '401-line TypeScript body', path: 'many.ts', lines: Array.from({ length: 401 }, (_, n) => `const value${n} = ${n};`) },
    { name: 'long TypeScript line', path: 'long.ts', lines: ['x'.repeat(24_010)] },
    { name: 'PNG text body', path: 'image.png', lines: ['plain text'] },
    { name: 'ICNS text body', path: 'icon.icns', lines: ['plain text'] },
    { name: 'uppercase AAC text body', path: 'sound.AAC', lines: ['plain text'] },
  ])('native Review renders $name without a preview gate', async ({ path, lines }) => {
    const f = fixture(), diff = `diff --git a/${path} b/${path}\nnew file mode 100644\n--- /dev/null\n+++ b/${path}\n@@ -0,0 +1,${lines.length} @@\n${lines.map(line => '+' + line).join('\n')}\n`;
    f.hook(request => request.method === 'review.getDiffPreview' ? { ok: true, generation: 3,
      value: { cwd: '/repo', sources: [{ kind: 'branch-range', diff }] } } : undefined);
    const view = await read(f), rendered = view.rows.filter(row => row.kind === 'line');
    expect(rendered.map(row => row.text)).toEqual(lines);
    expect(rendered.map(row => [row.oldNumber, row.newNumber])).toEqual(lines.map((_, n) => ['', String(n + 1)]));
    expect(view.rows.filter(row => row.kind === 'hunk')).toHaveLength(1);
    expect(view.rows.filter(row => row.kind === 'notice' || row.kind === 'suppressed')).toEqual([]);
    expect(view.rows.some(row => row.action === 'reveal')).toBe(false);
    expect(f.calls.map(request => request.op === 'http' ? request.path : request.method)).toEqual(['/api/auth/session', 'review.getDiffPreview']);
    expect(f.client.draft).toBe('');
    const id = view.files[0]!.id;
    expect((await act(f, 'toggle', id)).data.rows.map(row => row.kind)).toEqual(['file']);
    expect((await act(f, 'toggle', id)).data.rows.filter(row => row.kind === 'line').map(row => row.text)).toEqual(lines);
  });
  test.each([
    { name: 'binary PNG', diff: 'diff --git a/image.png b/image.png\nBinary files a/image.png and b/image.png differ\n', notice: 'Unsupported format. Diff contents are not available.', hunks: 0 },
    { name: 'binary ICNS', diff: 'diff --git a/icon.icns b/icon.icns\nGIT binary patch\n', notice: 'Unsupported format. Diff contents are not available.', hunks: 0 },
    { name: 'binary uppercase AAC', diff: 'diff --git a/sound.AAC b/sound.AAC\nGIT binary patch\n', notice: 'Unsupported format. Diff contents are not available.', hunks: 0 },
    { name: 'unknown binary extension', diff: 'diff --git a/data.dat b/data.dat\nGIT binary patch\n', notice: '', hunks: 0 },
    { name: '100% rename', diff: 'diff --git a/old.txt b/new.txt\nsimilarity index 100%\nrename from old.txt\nrename to new.txt\n', notice: 'This file was renamed without modifications.', hunks: 0 },
    { name: '99% rename without an available body', diff: 'diff --git a/old.txt b/new.txt\nsimilarity index 99%\nrename from old.txt\nrename to new.txt\n', notice: '', hunks: 0 },
    { name: 'unified rename fallback', diff: '--- old.txt\n+++ new.txt\n', notice: 'This file was renamed without modifications.', hunks: 0 },
    { name: 'new empty text file', diff: 'diff --git a/empty.txt b/empty.txt\nnew file mode 100644\n', notice: '', hunks: 0 },
    { name: 'deleted empty text file', diff: 'diff --git a/empty.txt b/empty.txt\ndeleted file mode 100644\n', notice: '', hunks: 0 },
    { name: 'mode-only text file', diff: 'diff --git a/mode.txt b/mode.txt\nold mode 100644\nnew mode 100755\n', notice: '', hunks: 0 },
    { name: 'new-file type overrides similarity', diff: 'diff --git a/old.txt b/new.txt\nsimilarity index 100%\nnew file mode 100644\n', notice: '', hunks: 0 },
    { name: 'non-text rename priority', diff: 'diff --git a/old.png b/new.png\nsimilarity index 100%\nrename from old.png\nrename to new.png\n', notice: 'Unsupported format. Diff contents are not available.', hunks: 0 },
    { name: 'empty hunk is still a parsed row', diff: 'diff --git a/image.png b/image.png\n--- a/image.png\n+++ b/image.png\n@@ -0,0 +0,0 @@\n', notice: '', hunks: 1 },
  ])('native Review no-row notice for $name', async ({ diff, notice, hunks }) => {
    const f = fixture();
    f.hook(request => request.method === 'review.getDiffPreview' ? { ok: true, generation: 3,
      value: { cwd: '/repo', sources: [{ kind: 'branch-range', diff }] } } : undefined);
    const view = await read(f);
    expect(view.files).toHaveLength(1);
    expect(view.rows.filter(row => row.kind === 'line')).toEqual([]);
    expect(view.rows.filter(row => row.kind === 'hunk')).toHaveLength(hunks);
    expect(view.rows.filter(row => row.kind === 'notice').map(row => [row.title, row.detail, row.action])).toEqual(notice ? [['', notice, '']] : []);
    expect(view.rows.filter(row => row.kind === 'suppressed')).toEqual([]);
  });
  test.each(['truncated', 'error'])('lazy %s notices take priority over no-row format notices', async outcome => {
    const f = fixture(), path = 'image.png'; let release!: (value: unknown) => void, entered!: () => void;
    const gate = new Promise(resolve => { release = resolve; }), started = new Promise<void>(resolve => { entered = resolve; });
    const source = (diff: string, truncated: boolean) => ({ cwd: '/repo', sources: [{ kind: 'branch-range', diffHash: 'lazy', truncated,
      files: [{ path, previousPath: null, additions: 0, deletions: 0 }], diff }] });
    f.hook(request => {
      if (request.method !== 'review.getDiffPreview') return;
      if (obj(obj(request.payload).file).path) { entered(); return gate; }
      return { ok: true, generation: 3, value: source('', true) };
    });
    const reading = read(f); await started; const pending = mobileReviewSnapshot(false, f.client);
    release(outcome === 'error' ? { ok: false, generation: 3, error: { kind: 'io', message: 'Read failed' } }
      : { ok: true, generation: 3, value: source(`diff --git a/${path} b/${path}\nGIT binary patch\n`, true) });
    const settled = await reading;
    expect(pending.rows.filter(row => row.kind === 'notice').map(row => [row.detail, row.action])).toEqual([['Loading diff…', '']]);
    expect(pending.rows.filter(row => row.kind === 'line' || row.kind === 'suppressed')).toEqual([]);
    expect(settled.rows.filter(row => row.kind === 'notice').map(row => [row.detail, row.action])).toEqual(outcome === 'error'
      ? [['Could not load diff. Select the file to retry.', 'retry']]
      : [['File preview exceeds the size limit. Counts include all changes.', '']]);
    expect(settled.rows.filter(row => row.kind === 'line' || row.kind === 'suppressed')).toEqual([]);
  });
  test('raw and truncated fallback retain their pinned explanations', () => {
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
  test('a new long press replaces the pending range anchor before a tap completes it', async () => {
    const f = fixture(); await read(f); f.calls.length = 0;
    await act(f, 'range-start', 'src/a.ts', '', 0);
    const restarted = await act(f, 'range-start', 'src/a.ts', '', 1);
    expect(restarted.data.rows.filter(row => row.kind === 'line' && row.selected).map(row => row.lineIndex)).toEqual([1]);
    expect(restarted).toMatchObject({ message: '', navigation: '', data: { selectionTitle: 'Select range end', canComment: false, commentOpen: false } });
    const completed = await act(f, 'line', 'src/a.ts', '', 2);
    expect(completed.data.rows.filter(row => row.kind === 'line' && row.selected).map(row => row.lineIndex)).toEqual([1, 2]);
    expect(completed).toMatchObject({ message: '', navigation: '', data: { canComment: true, commentOpen: false } });
    expect(completed.data.files[0]).toMatchObject({ expanded: true, viewed: false });
    expect(f.calls).toEqual([]);
    expect(f.client.draft).toBe('');
  });
  test('repeated endpoint taps retain the original long-press range anchor', async () => {
    const f = fixture(); await read(f); f.calls.length = 0;
    await act(f, 'range-start', 'src/a.ts', '', 0);
    const first = await act(f, 'line', 'src/a.ts', '', 1);
    expect(first.data.rows.filter(row => row.kind === 'line' && row.selected).map(row => row.lineIndex)).toEqual([0, 1]);
    expect(first.data).toMatchObject({ canComment: true, commentOpen: false });
    const adjusted = await act(f, 'line', 'src/a.ts', '', 2);
    expect(adjusted.data.rows.filter(row => row.kind === 'line' && row.selected).map(row => row.lineIndex)).toEqual([0, 1, 2]);
    expect(adjusted.data).toMatchObject({ canComment: true, commentOpen: false });
    expect(f.calls).toEqual([]); expect(f.client.draft).toBe('');
  });
  test('range endpoints can cross and return to their anchor before commenting', async () => {
    const f = fixture(); await read(f); f.calls.length = 0;
    await act(f, 'range-start', 'src/a.ts', '', 1);
    await act(f, 'line', 'src/a.ts', '', 2);
    const crossed = await act(f, 'line', 'src/a.ts', '', 0);
    expect(crossed.data.rows.filter(row => row.kind === 'line' && row.selected).map(row => row.lineIndex)).toEqual([0, 1]);
    expect(crossed.data).toMatchObject({ canComment: true, commentOpen: false });
    expect(crossed.data.selectionTitle).toMatch(/^Comment on /);
    const atAnchor = await act(f, 'line', 'src/a.ts', '', 1);
    expect(atAnchor.data.rows.filter(row => row.kind === 'line' && row.selected).map(row => row.lineIndex)).toEqual([1]);
    expect(atAnchor.data).toMatchObject({ selectionTitle: 'Select range end', canComment: false, commentOpen: false });
    const completed = await act(f, 'line', 'src/a.ts', '', 2);
    expect(completed.data.rows.filter(row => row.kind === 'line' && row.selected).map(row => row.lineIndex)).toEqual([1, 2]);
    expect(completed.data).toMatchObject({ canComment: true, commentOpen: false });
    expect((await act(f, 'comment')).data.commentOpen).toBe(true);
    expect(f.calls).toEqual([]); expect(f.client.draft).toBe('');
  });
  test('clear, cancel, file navigation and section changes retire a retained range', async () => {
    for (const reset of ['clear', 'cancel', 'file', 'section']) {
      const f = fixture(); await read(f);
      await act(f, 'range-start', 'src/a.ts', '', 0); await act(f, 'line', 'src/a.ts', '', 1);
      const retired = await act(f, reset, reset === 'file' ? 'src/a.ts' : reset === 'section' ? 'turn:1' : '');
      expect(retired.data).toMatchObject({ selectionTitle: '', canComment: false, commentOpen: false });
      const next = await act(f, 'line', reset === 'section' ? 'turn.ts' : 'src/a.ts', '', 2);
      expect(next.data.rows.filter(row => row.kind === 'line' && row.selected).map(row => row.lineIndex)).toEqual([2]);
      expect(next.data).toMatchObject({ canComment: true, commentOpen: true });
      expect(f.client.draft).toBe('');
    }
  });
  test('a tap in another file starts an ordinary single-line comment', async () => {
    const f = fixture();
    f.hook(request => request.method === 'review.getDiffPreview' ? { ok: true, generation: 3,
      value: { cwd: '/repo', sources: [{ kind: 'branch-range', diff: patch() + patch('src/b.ts') }] } } : undefined);
    await read(f); f.calls.length = 0;
    await act(f, 'range-start', 'src/a.ts', '', 0); await act(f, 'line', 'src/a.ts', '', 1);
    const next = await act(f, 'line', 'src/b.ts', '', 2);
    expect(next.data.rows.filter(row => row.kind === 'line' && row.selected).map(row => [row.path, row.lineIndex])).toEqual([['src/b.ts', 2]]);
    expect(next.data).toMatchObject({ commentPath: 'src/b.ts', canComment: true, commentOpen: true });
    expect(f.calls).toEqual([]); expect(f.client.draft).toBe('');
  });
});


describe('Review navigator selection and viewed state', () => {
  test('select reveals a collapsed file, clears a comment range, and repeat selection can return to top', async () => {
    const f = fixture(); await read(f);
    await act(f, 'range-start', 'src/a.ts', '', 1);
    await act(f, 'toggle', marker(f, 'src/a.ts'));
    const selected = await act(f, 'file', 'src/a.ts');
    expect(selected.navigation).toBe(selected.data.rows.find(row => row.kind === 'file' && row.path === 'src/a.ts')?.id!);
    expect(selected.data).toMatchObject({ selectedPath: 'src/a.ts', canComment: false, commentOpen: false, selectionTitle: '' });
    expect(selected.data.files[0]?.expanded).toBe(true);
    const cleared = await act(f, 'file');
    expect(cleared.navigation).toBe('top'); expect(cleared.data.selectedPath).toBe('');
    expect((await act(f, 'file', 'missing.ts')).navigation).toBe('');
  });
  test('marking viewed collapses; marking unviewed preserves collapse; selection reopens without clearing viewed', async () => {
    const f = fixture(); await read(f);
    expect((await act(f, 'viewed', marker(f, 'src/a.ts'))).data.files[0]).toMatchObject({ viewed: true, expanded: false });
    expect((await act(f, 'viewed', marker(f, 'src/a.ts'))).data.files[0]).toMatchObject({ viewed: false, expanded: false });
    await act(f, 'viewed', marker(f, 'src/a.ts'));
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
  const { queuedEditThreadKey, queuedEditEndMemory } = await import('./queued-edit-state');
  const { queuedEditState } = await import('./queued-edit-memory');
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

test('a retained range cannot continue in a replacement composer owner', async () => {
  const { queuedEditThreadKey, queuedEditEndMemory } = await import('./queued-edit-state');
  const { queuedEditState } = await import('./queued-edit-memory');
  const f = fixture(); f.client.local.drafts[f.client.draftKey] = 'ordinary untouched';
  const edit = { owner: 'range-edit', session: 'range-session', draftKey: 'one:t~queued-edit~r', origin: f.client.origin,
    environmentId: 'one', threadId: 't', projectId: 'p', generation: 3, revision: 1, runId: 'r', messageId: 'm', text: 'queued',
    attachments: [], existingAttachments: [], saving: false };
  const state = queuedEditState(f.client); state.sessions.set(edit.owner, edit); state.active.set(queuedEditThreadKey('one', 't'), edit.owner);
  await read(f); f.calls.length = 0;
  await act(f, 'range-start', 'src/a.ts', '', 0); await act(f, 'line', 'src/a.ts', '', 1);
  queuedEditEndMemory(edit.owner, f.client);
  expect(mobileReviewSnapshot(false, f.client)).toMatchObject({ selectionTitle: '', canComment: false, commentOpen: false });
  const next = await act(f, 'line', 'src/a.ts', '', 2);
  expect(next.data.rows.filter(row => row.kind === 'line' && row.selected).map(row => row.lineIndex)).toEqual([2]);
  expect(next.data).toMatchObject({ canComment: true, commentOpen: true });
  expect(f.calls).toEqual([]); expect(f.client.draft).toBe('ordinary untouched');
});


test('late queued comment persistence cannot clear a replacement composer selection', async () => {
  const { queuedEditThreadKey, queuedEditEndMemory } = await import('./queued-edit-state');
  const { queuedEditState } = await import('./queued-edit-memory');
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

// Source-derived expectations from T3 Code365aa87982 reviewState,
// reviewFileVisibility, useReviewSections and useReviewDiffData. This group drives
// the clone's production reader/actions, not the primary UI or a parser ID oracle.
// Lazy metadata supplies the pinned path ID domain; marker callbacks use the
// rendered snapshot ID and navigation continues to use the actual file path.
describe('Review presentation retention baseline', () => {
  function retainedFixture(paths = ['a.ts', 'b.ts']) {
    const f = fixture();
    const answerState = { paths, version: 0, permitted: true, empty: false, failAccess: false, failPreview: false };
    let lastSources: Obj[] = [];
    const answer = (request: Obj): unknown => {
      if (request.op === 'http') {
        if (answerState.failAccess) throw new Error('Current session unavailable');
        return { ok: true, generation: f.client.generation,
          value: { authenticated: true, permissions: answerState.permitted ? ['filesystem:read'] : [], scopes: ['filesystem:read'] } };
      }
      if (request.method !== 'review.getDiffPreview') return undefined;
      if (answerState.failPreview) throw new Error('Current Git preview unavailable');
      const file = obj(obj(request.payload).file), path = String(file.path || '');
      const kinds = path ? [file.sourceKind] : ['working-tree', 'branch-range'];
      const sources: Obj[] = answerState.empty ? [] : kinds.map(kind => ({ kind,
        title: kind === 'working-tree' ? 'Local changes' : 'Branch changes', baseRef: 'release', headRef: 'feature',
        diffHash: `${f.client.environmentId}:${f.client.threadId}:${answerState.version}`, truncated: !path,
        files: answerState.paths.map(path => ({ path, previousPath: null, additions: 1, deletions: 1 })),
        diff: path ? patch(path).replace('+new\n', `+new ${answerState.version}\n`) : '' }));
      if (!path) lastSources = sources;
      return { ok: true, generation: f.client.generation, value: { cwd: '/repo', sources } };
    };
    f.hook(answer);
    return { ...f, answerState, answer, sources: () => lastSources };
  }
  type RetainedFixture = ReturnType<typeof retainedFixture>;
  type View = ReturnType<typeof mobileReviewSnapshot>;
  const files = (view: View) => view.files.map(file => ({ path: file.path, expanded: file.expanded, viewed: file.viewed }));
  function renderedId(view: View, path: string): string {
    const file = view.files.find(file => file.path === path);
    if (!file) throw new Error(`Fixture prerequisite missing rendered file ${path}`);
    return file.id;
  }
  function chooseThread(f: RetainedFixture, thread: string, environment = f.client.environmentId) {
    f.client.environmentId = environment; f.client.threadId = thread; f.client.threadEpoch++;
    if (!f.client.shell.threads.some(row => row.id === thread)) f.client.shell.threads.push({ id: thread, projectId: 'p' });
    f.client.thread!.projection.thread = { id: thread }; f.answerState.version++;
  }
  const authCalls = (f: RetainedFixture) => f.calls.filter(call => call.op === 'http' && call.path === '/api/auth/session').length;
  const previews = (f: RetainedFixture) => f.calls.filter(call => call.method === 'review.getDiffPreview' && !obj(obj(call.payload).file).path).length;
  function observed(name: string, actual: unknown, expected: unknown) {
    // One object comparison exposes every observed mismatch in a case; Bun runs
    // the whole group and this receipt preserves the passing controls as well.
    console.log(`RETENTION_OBSERVATION ${JSON.stringify({ name, actual, expected })}`);
    expect(actual).toEqual(expected);
  }
  async function inspectFreshLine(f: RetainedFixture, view: View, path: string, text: string) {
    const file = view.files.find(file => file.path === path);
    if (!file) throw new Error(`Fixture prerequisite missing inspection file ${path}`);
    const prior = { path: file.path, expanded: file.expanded, viewed: file.viewed };
    const calls = f.calls.length, opened = await act(f, 'file', prior.path);
    const freshLine = !opened.message && opened.data.rows.some(row => row.kind === 'line' && row.path === prior.path && row.text === text);
    if (!prior.expanded) await act(f, 'toggle', renderedId(opened.data, prior.path));
    const restored = mobileReviewSnapshot(false, f.client).files.find(file => file.path === prior.path);
    return { freshLine, inspection: { nativeCalls: f.calls.length - calls,
      restored: restored?.expanded === prior.expanded && restored?.viewed === prior.viewed } };
  }
  function holdAccess(f: RetainedFixture) {
    let release!: (value: unknown) => void, entered!: () => void;
    const gate = new Promise(resolve => { release = resolve; });
    const began = new Promise<void>(resolve => { entered = resolve; });
    f.hook(request => {
      if (request.op === 'http') { entered(); return gate; }
      return f.answer(request);
    });
    return { began, release: () => release({ ok: true, generation: f.client.generation,
      value: { authenticated: true, permissions: ['filesystem:read'] } }) };
  }
  const marked = [
    { path: 'a.ts', expanded: false, viewed: false },
    { path: 'b.ts', expanded: false, viewed: true },
  ];

  test('A to B to A retains section and markers while every owner reads fresh data', async () => {
    const f = retainedFixture(), first = await knownWorking(f);
    await act(f, 'toggle', renderedId(first, 'a.ts')); await act(f, 'viewed', renderedId(first, 'b.ts'));
    chooseThread(f, 'other'); const other = await read(f);
    chooseThread(f, 't'); const returned = await read(f), calls = f.calls.length;
    const preferences = { section: returned.sectionId, files: files(returned) };
    const snapshot = mobileReviewSnapshot(false, f.client);
    const inspected = await inspectFreshLine(f, returned, 'a.ts', 'new 2');
    observed('thread round trip', {
      other: { section: other.sectionId, files: files(other) },
      returned: { ...preferences, freshLine: inspected.freshLine }, inspection: inspected.inspection,
      auth: authCalls(f), previews: previews(f), snapshotIo: f.calls.length - calls, sameOwner: snapshot.owner === returned.owner,
    }, {
      other: { section: 'git:branch-range', files: [{ path: 'a.ts', expanded: true, viewed: false }, { path: 'b.ts', expanded: true, viewed: false }] },
      returned: { section: 'git:working-tree', files: marked, freshLine: true }, inspection: { nativeCalls: 0, restored: true },
      auth: 3, previews: 3, snapshotIo: 0, sameOwner: true,
    });
  });

  test('reconnect keeps presentation but retires data owner and repeats permission and patch reads', async () => {
    const f = retainedFixture(), first = await knownWorking(f);
    await act(f, 'toggle', renderedId(first, 'a.ts')); await act(f, 'viewed', renderedId(first, 'b.ts'));
    f.client.generation++; f.client.threadEpoch++; f.answerState.version++;
    const before = mobileReviewSnapshot(false, f.client), returned = await read(f);
    const preferences = { section: returned.sectionId, files: files(returned) };
    const inspected = await inspectFreshLine(f, returned, 'a.ts', 'new 1');
    observed('reconnect', {
      ownerChanged: returned.owner !== first.owner, staleBody: [before.files.length, before.rows.length, before.raw],
      ...preferences, freshLine: inspected.freshLine, inspection: inspected.inspection,
      auth: authCalls(f), previews: previews(f), patchReads: f.calls.filter(call => obj(obj(call.payload).file).path).length,
    }, { ownerChanged: true, staleBody: [0, 0, ''], section: 'git:working-tree', files: marked, freshLine: true,
      inspection: { nativeCalls: 0, restored: true }, auth: 2, previews: 2, patchReads: 4 });
  });

  test('the same thread ID in another environment has independent presentation', async () => {
    const f = retainedFixture(), first = await knownWorking(f);
    await act(f, 'viewed', renderedId(first, 'a.ts'));
    chooseThread(f, 't', 'two'); const other = await read(f);
    await act(f, 'toggle', renderedId(other, 'b.ts'));
    chooseThread(f, 't', 'one'); const returned = await read(f);
    observed('environment isolation', { other: files(other), section: returned.sectionId, returned: files(returned), auth: authCalls(f), previews: previews(f) }, {
      other: [{ path: 'a.ts', expanded: true, viewed: false }, { path: 'b.ts', expanded: true, viewed: false }],
      section: 'git:working-tree', returned: [{ path: 'a.ts', expanded: false, viewed: true }, { path: 'b.ts', expanded: true, viewed: false }], auth: 3, previews: 3,
    });
  });

  test('missing environment or thread cannot write another logical scope presentation', async () => {
    const results = [];
    for (const missing of ['environment', 'thread']) {
      const f = retainedFixture(['a.ts']), first = await knownWorking(f);
      await act(f, 'viewed', renderedId(first, 'a.ts')); f.calls.length = 0;
      if (missing === 'environment') f.client.environmentId = ''; else f.client.threadId = '';
      f.client.threadEpoch++;
      const inert = missing === 'thread' ? await read(f) : mobileReviewSnapshot(false, f.client);
      const action = await act(f, 'viewed', renderedId(first, 'a.ts'));
      const inertIo = f.calls.length;
      f.client.environmentId = 'one'; f.client.threadId = 't'; f.client.threadEpoch++;
      const returned = await read(f);
      results.push({ missing, inertFiles: inert.files.length, rejected: !!action.message, inertIo, section: returned.sectionId, files: files(returned) });
    }
    observed('missing logical scope', results, ['environment', 'thread'].map(missing => ({ missing, inertFiles: 0, rejected: true, inertIo: 0,
      section: 'git:working-tree', files: [{ path: 'a.ts', expanded: false, viewed: true }] })));
  });

  test('absent expansion includes new files and picking an already expanded file leaves it absent', async () => {
    const f = retainedFixture(['a.ts']), first = await read(f);
    const picked = await act(f, 'file', first.files[0]!.path);
    f.answerState.paths = ['a.ts', 'b.ts']; f.answerState.version++;
    const next = await read(f, '', true);
    observed('absent expansion', { navigation: picked.navigation, files: files(next) }, { navigation: 'file:a.ts',
      files: [{ path: 'a.ts', expanded: true, viewed: false }, { path: 'b.ts', expanded: true, viewed: false }] });
  });

  test('explicit empty expansion keeps a newly arriving file collapsed', async () => {
    const f = retainedFixture(['a.ts']), first = await read(f);
    await act(f, 'toggle', renderedId(first, 'a.ts'));
    f.answerState.paths = ['a.ts', 'b.ts']; f.answerState.version++;
    const next = await read(f, '', true);
    observed('explicit empty expansion', files(next), [{ path: 'a.ts', expanded: false, viewed: false }, { path: 'b.ts', expanded: false, viewed: false }]);
  });

  test('explicit nonempty expansion retains only saved IDs when another file arrives', async () => {
    const f = retainedFixture(), first = await read(f);
    await act(f, 'toggle', renderedId(first, 'a.ts'));
    f.answerState.paths = ['a.ts', 'b.ts', 'c.ts']; f.answerState.version++;
    const next = await read(f, '', true);
    observed('explicit nonempty expansion', files(next), [{ path: 'a.ts', expanded: false, viewed: false },
      { path: 'b.ts', expanded: true, viewed: false }, { path: 'c.ts', expanded: false, viewed: false }]);
  });

  test('viewed collapse makes expansion explicit; unviewed keeps collapse; path navigation reopens it', async () => {
    const f = retainedFixture(['a.ts']), first = await read(f);
    await act(f, 'viewed', renderedId(first, 'a.ts'));
    f.answerState.paths = ['a.ts', 'b.ts']; f.answerState.version++;
    const added = await read(f, '', true);
    const unviewed = await act(f, 'viewed', renderedId(added, 'a.ts'));
    const picked = await act(f, 'file', 'a.ts');
    observed('viewed expansion policy', { added: files(added), unviewed: files(unviewed.data), picked: files(picked.data), navigation: picked.navigation }, {
      added: [{ path: 'a.ts', expanded: false, viewed: true }, { path: 'b.ts', expanded: false, viewed: false }],
      unviewed: [{ path: 'a.ts', expanded: false, viewed: false }, { path: 'b.ts', expanded: false, viewed: false }],
      picked: [{ path: 'a.ts', expanded: true, viewed: false }, { path: 'b.ts', expanded: false, viewed: false }], navigation: 'file:a.ts',
    });
  });

  test('projection alone preserves a temporarily missing viewed ID until it reappears', async () => {
    const f = retainedFixture(), first = await read(f);
    await act(f, 'viewed', renderedId(first, 'a.ts'));
    f.answerState.paths = ['b.ts']; f.answerState.version++; const absent = await read(f, '', true);
    f.answerState.paths = ['a.ts', 'b.ts']; f.answerState.version++; const returned = await read(f, '', true);
    observed('projection does not prune', { absent: files(absent), returned: files(returned) }, {
      absent: [{ path: 'b.ts', expanded: true, viewed: false }],
      returned: [{ path: 'a.ts', expanded: false, viewed: true }, { path: 'b.ts', expanded: true, viewed: false }],
    });
  });

  test('a viewed action while an ID is absent prunes that ID instead of restoring its old mark', async () => {
    const f = retainedFixture(), first = await read(f);
    await act(f, 'viewed', renderedId(first, 'a.ts'));
    f.answerState.paths = ['b.ts']; f.answerState.version++; const absent = await read(f, '', true);
    await act(f, 'viewed', renderedId(absent, 'b.ts'));
    f.answerState.paths = ['a.ts', 'b.ts']; f.answerState.version++; const returned = await read(f, '', true);
    observed('action prunes viewed IDs', files(returned), [{ path: 'a.ts', expanded: false, viewed: false }, { path: 'b.ts', expanded: false, viewed: true }]);
  });

  test('identical lazy IDs in separate sections do not share expanded or viewed preferences', async () => {
    const f = retainedFixture(), first = await knownWorking(f);
    await act(f, 'toggle', renderedId(first, 'a.ts'));
    const changes = await read(f, 'git:branch-range'); await act(f, 'viewed', renderedId(changes, 'b.ts'));
    const working = await read(f, 'git:working-tree');
    observed('section isolation', { changes: files(changes), working: files(working), auth: authCalls(f) }, {
      changes: [{ path: 'a.ts', expanded: true, viewed: false }, { path: 'b.ts', expanded: true, viewed: false }],
      working: [{ path: 'a.ts', expanded: false, viewed: false }, { path: 'b.ts', expanded: true, viewed: false }], auth: 3,
    });
  });

  for (const reconnect of [false, true]) {
    test(`pending grant keeps copied Git headers and selected section with empty data, reconnect=${reconnect}`, async () => {
      const f = retainedFixture(), first = await knownWorking(f);
      f.sources()[0]!.title = 'Changed original response'; f.sources()[1]!.baseRef = 'changed-original-ref';
      if (reconnect) { f.client.generation++; f.client.threadEpoch++; }
      const held = holdAccess(f), pending = read(f); await held.began;
      let during!: View;
      try { during = mobileReviewSnapshot(false, f.client); }
      finally { held.release(); await pending; }
      observed(`pending header shells reconnect=${reconnect}`, {
        section: during.sectionId, title: during.title, subtitle: during.subtitle, loading: during.loading,
        headers: during.sections.map(({ id, title, subtitle }) => ({ id, title, subtitle })),
        body: [during.files.length, during.rows.length, during.raw, during.additions, during.deletions], auth: authCalls(f), initialOwnerChanged: during.owner !== first.owner,
      }, { section: 'git:working-tree', title: 'Local changes', subtitle: 'Staged, unstaged, and untracked files', loading: true,
        headers: [{ id: 'turn:3', title: 'Turn 3', subtitle: '0 files changed' }, { id: 'turn:1', title: 'Turn 1', subtitle: '1 file changed' },
          { id: 'git:working-tree', title: 'Local changes', subtitle: 'Staged, unstaged, and untracked files' },
          { id: 'git:branch-range', title: 'Branch changes', subtitle: 'release ... feature' }],
        body: [0, 0, '', 0, 0], auth: 2, initialOwnerChanged: reconnect,
      });
    });
  }

  for (const failure of ['denial', 'session error']) {
    test(`${failure} with a ready checkpoint commits that nonempty fallback`, async () => {
      const f = retainedFixture(); await knownWorking(f); f.calls.length = 0;
      f.answerState.permitted = false; f.answerState.failAccess = failure === 'session error';
      const denied = await read(f), duringCalls = [...f.calls];
      f.answerState.permitted = true; f.answerState.failAccess = false; const restored = await read(f);
      observed(`ready checkpoint fallback ${failure}`, {
        section: denied.sectionId, sections: denied.sections.map(row => row.id), path: denied.files[0]?.path,
        deniedPreview: duringCalls.some(call => call.method === 'review.getDiffPreview'),
        turnInput: duringCalls.find(call => call.method === 'orchestration.getTurnDiff')?.payload, restored: restored.sectionId,
      }, { section: 'turn:3', sections: ['turn:3', 'turn:1'], path: 'turn.ts', deniedPreview: false,
        turnInput: { threadId: 't', fromTurnCount: 2, toTurnCount: 3, ignoreWhitespace: false }, restored: 'turn:3' });
    });

    test(`${failure} without any alternative preserves the saved selection for later granted reads`, async () => {
      const f = retainedFixture(); f.client.thread!.projection.checkpoints = []; await knownWorking(f);
      f.answerState.permitted = false; f.answerState.failAccess = failure === 'session error'; f.calls.length = 0;
      const denied = await read(f), duringCalls = [...f.calls];
      f.answerState.permitted = true; f.answerState.failAccess = false; const restored = await read(f);
      observed(`empty list preservation ${failure}`, {
        denied: { section: denied.sectionId, headers: denied.sections.length, body: [denied.files.length, denied.rows.length, denied.raw], error: !!denied.error },
        deniedPreview: duringCalls.some(call => call.method === 'review.getDiffPreview'), restored: restored.sectionId, auth: authCalls(f), previews: previews(f),
      }, { denied: { section: '', headers: 0, body: [0, 0, ''], error: true }, deniedPreview: false,
        restored: 'git:working-tree', auth: 2, previews: 1 });
    });
  }

  test('an errored granted preview retains pending header shells but no cached file body', async () => {
    const f = retainedFixture(); f.client.thread!.projection.checkpoints = []; await knownWorking(f);
    f.client.generation++; f.client.threadEpoch++; f.answerState.failPreview = true;
    const failed = await read(f); f.answerState.failPreview = false; const restored = await read(f);
    observed('preview error headers', { section: failed.sectionId, headers: failed.sections.map(row => row.id), title: failed.title,
      body: [failed.files.length, failed.rows.length, failed.raw], error: failed.error, restored: restored.sectionId, auth: authCalls(f), previews: previews(f) }, {
      section: 'git:working-tree', headers: ['git:working-tree', 'git:branch-range'], title: 'Local changes', body: [0, 0, ''],
      error: 'Current Git preview unavailable', restored: 'git:working-tree', auth: 3, previews: 3,
    });
  });

  test('a successful empty preview replaces Git headers; the next pending grant has only the Changes placeholder', async () => {
    const f = retainedFixture(); f.client.thread!.projection.checkpoints = []; await knownWorking(f);
    f.answerState.empty = true; const empty = await read(f);
    f.client.generation++; f.client.threadEpoch++;
    const held = holdAccess(f), pending = read(f); await held.began;
    let during!: View;
    try { during = mobileReviewSnapshot(false, f.client); }
    finally { held.release(); await pending; }
    observed('successful empty replacement', {
      empty: { section: empty.sectionId, headers: empty.sections.length, files: empty.files.length, error: empty.error },
      pending: { headers: during.sections.map(({ id, title, subtitle }) => ({ id, title, subtitle })), body: [during.files.length, during.rows.length, during.raw], loading: during.loading },
      auth: authCalls(f), previews: previews(f),
    }, { empty: { section: '', headers: 0, files: 0, error: '' },
      pending: { headers: [{ id: 'git:branch-range', title: 'Changes', subtitle: '' }], body: [0, 0, ''], loading: true }, auth: 3, previews: 3 });
  });
});

// Fixed outputs witnessed through actual pinned Review/Pierre exports in marker R3.
// No runtime reference import or duplicate marker algorithm lives in these tests.
describe('Review qualified marker identities', () => {
  const vectors = [
    {"name": "single-git", "text": "diff --git a/a.ts b/a.ts\nindex 1111111..2222222 100644\n--- a/a.ts\n+++ b/a.ts\n@@ -1 +1 @@\n-old\n+new\n", "scope": "git:branch-range", "paths": ["a.ts"], "ids": ["ck1:[\"patch-file\",\"git:branch-range:98:11dtc7n:uhg5vz\",\"0\",\"0\"]"]},
    {"name": "outer-whitespace", "text": "\n  \ndiff --git a/a.ts b/a.ts\nindex 1111111..2222222 100644\n--- a/a.ts\n+++ b/a.ts\n@@ -1 +1 @@\n-old\n+new\n \n\n", "scope": "git:branch-range", "paths": ["a.ts"], "ids": ["ck1:[\"patch-file\",\"git:branch-range:98:11dtc7n:uhg5vz\",\"0\",\"0\"]"]},
    {"name": "truncation-marker", "text": "diff --git a/a.ts b/a.ts\nindex 1111111..2222222 100644\n--- a/a.ts\n+++ b/a.ts\n@@ -1 +1 @@\n-old\n+new\n\n\n[truncated]  \n", "scope": "git:branch-range", "paths": ["a.ts"], "ids": ["ck1:[\"patch-file\",\"git:branch-range:98:11dtc7n:uhg5vz\",\"0\",\"0\"]"]},
    {"name": "file-order-z-a", "text": "diff --git a/z.ts b/z.ts\nindex 1111111..2222222 100644\n--- a/z.ts\n+++ b/z.ts\n@@ -1 +1 @@\n-old\n+new\ndiff --git a/a.ts b/a.ts\nindex 1111111..2222222 100644\n--- a/a.ts\n+++ b/a.ts\n@@ -1 +1 @@\n-old\n+new\n", "scope": "git:branch-range", "paths": ["z.ts", "a.ts"], "ids": ["ck1:[\"patch-file\",\"git:branch-range:197:1ltgv45:55mrk9\",\"0\",\"0\"]", "ck1:[\"patch-file\",\"git:branch-range:197:1ltgv45:55mrk9\",\"0\",\"1\"]"]},
    {"name": "file-order-a-z", "text": "diff --git a/a.ts b/a.ts\nindex 1111111..2222222 100644\n--- a/a.ts\n+++ b/a.ts\n@@ -1 +1 @@\n-old\n+new\ndiff --git a/z.ts b/z.ts\nindex 1111111..2222222 100644\n--- a/z.ts\n+++ b/z.ts\n@@ -1 +1 @@\n-old\n+new\n", "scope": "git:branch-range", "paths": ["a.ts", "z.ts"], "ids": ["ck1:[\"patch-file\",\"git:branch-range:197:5q83zx:p5jub5\",\"0\",\"0\"]", "ck1:[\"patch-file\",\"git:branch-range:197:5q83zx:p5jub5\",\"0\",\"1\"]"]},
    {"name": "whole-patch-edit", "text": "diff --git a/z.ts b/z.ts\nindex 1111111..2222222 100644\n--- a/z.ts\n+++ b/z.ts\n@@ -1 +1 @@\n-old\n+edited\ndiff --git a/a.ts b/a.ts\nindex 1111111..2222222 100644\n--- a/a.ts\n+++ b/a.ts\n@@ -1 +1 @@\n-old\n+new\n", "scope": "git:branch-range", "paths": ["z.ts", "a.ts"], "ids": ["ck1:[\"patch-file\",\"git:branch-range:200:1xj1d3y:heytyq\",\"0\",\"0\"]", "ck1:[\"patch-file\",\"git:branch-range:200:1xj1d3y:heytyq\",\"0\",\"1\"]"]},
    {"name": "same-path-edit", "text": "diff --git a/a.ts b/a.ts\nindex 1111111..2222222 100644\n--- a/a.ts\n+++ b/a.ts\n@@ -1 +1 @@\n-old\n+edited\n", "scope": "git:branch-range", "paths": ["a.ts"], "ids": ["ck1:[\"patch-file\",\"git:branch-range:101:njnx7i:5h7xbm\",\"0\",\"0\"]"]},
    {"name": "repeated-paths", "text": "diff --git a/a.ts b/a.ts\nindex 1111111..2222222 100644\n--- a/a.ts\n+++ b/a.ts\n@@ -1 +1 @@\n-old\n+new\ndiff --git a/a.ts b/a.ts\nindex 1111111..2222222 100644\n--- a/a.ts\n+++ b/a.ts\n@@ -1 +1 @@\n-old-two\n+new-two\n", "scope": "git:branch-range", "paths": ["a.ts", "a.ts"], "ids": ["ck1:[\"patch-file\",\"git:branch-range:205:1ggzlzv:1j2dl3r\",\"0\",\"0\"]", "ck1:[\"patch-file\",\"git:branch-range:205:1ggzlzv:1j2dl3r\",\"0\",\"1\"]"]},
    {"name": "two-commits", "text": "From aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa Mon Sep 17 00:00:00 2001\nSubject: [PATCH] first\n\ndiff --git a/z.ts b/z.ts\nindex 1111111..2222222 100644\n--- a/z.ts\n+++ b/z.ts\n@@ -1 +1 @@\n-old\n+new\nFrom bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb Mon Sep 17 00:00:00 2001\nSubject: [PATCH] second\n\ndiff --git a/a.ts b/a.ts\nindex 1111111..2222222 100644\n--- a/a.ts\n+++ b/a.ts\n@@ -1 +1 @@\n-old\n+new\n", "scope": "git:branch-range", "paths": ["z.ts", "a.ts"], "ids": ["ck1:[\"patch-file\",\"git:branch-range:388:1offp2d:ze1ojt\",\"0\",\"0\"]", "ck1:[\"patch-file\",\"git:branch-range:388:1offp2d:ze1ojt\",\"1\",\"0\"]"]},
    {"name": "empty-metadata-patch-before-files", "text": "From aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa Mon Sep 17 00:00:00 2001\nSubject: [PATCH] metadata only\n\nNo supported file here.\nFrom bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb Mon Sep 17 00:00:00 2001\nSubject: [PATCH] first file\n\ndiff --git a/z.ts b/z.ts\nindex 1111111..2222222 100644\n--- a/z.ts\n+++ b/z.ts\n@@ -1 +1 @@\n-old\n+new\nFrom cccccccccccccccccccccccccccccccccccccccc Mon Sep 17 00:00:00 2001\nSubject: [PATCH] second file\n\ndiff --git a/a.ts b/a.ts\nindex 1111111..2222222 100644\n--- a/a.ts\n+++ b/a.ts\n@@ -1 +1 @@\n-old\n+new\n", "scope": "git:branch-range", "paths": ["z.ts", "a.ts"], "ids": ["ck1:[\"patch-file\",\"git:branch-range:525:11ouag4:8837rc\",\"1\",\"0\"]", "ck1:[\"patch-file\",\"git:branch-range:525:11ouag4:8837rc\",\"2\",\"0\"]"]},
    {"name": "preamble-before-commit", "text": "Unsupported preamble.\nFrom aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa Mon Sep 17 00:00:00 2001\nSubject: [PATCH] file\n\ndiff --git a/a.ts b/a.ts\nindex 1111111..2222222 100644\n--- a/a.ts\n+++ b/a.ts\n@@ -1 +1 @@\n-old\n+new\n", "scope": "git:branch-range", "paths": ["a.ts"], "ids": ["ck1:[\"patch-file\",\"git:branch-range:214:1204253:1od4a57\",\"1\",\"0\"]"]},
    {"name": "unsupported-text", "text": "This is not a supported patch.\n", "scope": "git:branch-range", "paths": [], "ids": []},
    {"name": "unified-file", "text": "--- plain.txt\n+++ plain.txt\n@@ -1 +1 @@\n-old\n+new\n", "scope": "git:branch-range", "paths": ["plain.txt"], "ids": ["ck1:[\"patch-file\",\"git:branch-range:49:1rxgru6:11rwoqi\",\"0\",\"0\"]"]},
    {"name": "skipped-unsupported-blob", "text": "--- lonely.txt\nnot a matching plus header\n--- plain.txt\n+++ plain.txt\n@@ -1 +1 @@\n-old\n+new\n", "scope": "git:branch-range", "paths": ["plain.txt"], "ids": ["ck1:[\"patch-file\",\"git:branch-range:91:1dtvdqo:1dpa49o\",\"0\",\"0\"]"]},
    {"name": "binary-file-is-accepted", "text": "diff --git a/z.bin b/z.bin\nindex 1111111..2222222 100644\nBinary files a/z.bin and b/z.bin differ\ndiff --git a/a.ts b/a.ts\nindex 1111111..2222222 100644\n--- a/a.ts\n+++ b/a.ts\n@@ -1 +1 @@\n-old\n+new\n", "scope": "git:branch-range", "paths": ["z.bin", "a.ts"], "ids": ["ck1:[\"patch-file\",\"git:branch-range:195:lh60j1:1wbecsx\",\"0\",\"0\"]", "ck1:[\"patch-file\",\"git:branch-range:195:lh60j1:1wbecsx\",\"0\",\"1\"]"]},
    {"name": "malformed-git-header-is-accepted", "text": "diff --git invalid\ndiff --git a/a.ts b/a.ts\nindex 1111111..2222222 100644\n--- a/a.ts\n+++ b/a.ts\n@@ -1 +1 @@\n-old\n+new\n", "scope": "git:branch-range", "paths": ["", "a.ts"], "ids": ["ck1:[\"patch-file\",\"git:branch-range:117:bdi1wt:jmon75\",\"0\",\"0\"]", "ck1:[\"patch-file\",\"git:branch-range:117:bdi1wt:jmon75\",\"0\",\"1\"]"]},
    {"name": "section-scope", "text": "diff --git a/a.ts b/a.ts\nindex 1111111..2222222 100644\n--- a/a.ts\n+++ b/a.ts\n@@ -1 +1 @@\n-old\n+new\n", "scope": "git:working-tree", "paths": ["a.ts"], "ids": ["ck1:[\"patch-file\",\"git:working-tree:98:11dtc7n:uhg5vz\",\"0\",\"0\"]"]},
    {"name": "turn-section-scope", "text": "diff --git a/a.ts b/a.ts\nindex 1111111..2222222 100644\n--- a/a.ts\n+++ b/a.ts\n@@ -1 +1 @@\n-old\n+new\n", "scope": "turn:4", "paths": ["a.ts"], "ids": ["ck1:[\"patch-file\",\"turn:4:98:11dtc7n:uhg5vz\",\"0\",\"0\"]"]},
    {"name": "crlf-is-not-normalized", "text": "diff --git a/a.ts b/a.ts\r\nindex 1111111..2222222 100644\r\n--- a/a.ts\r\n+++ b/a.ts\r\n@@ -1 +1 @@\r\n-old\r\n+new\r\n", "scope": "git:branch-range", "paths": ["a.ts"], "ids": ["ck1:[\"patch-file\",\"git:branch-range:104:1py1xjp:1w3maah\",\"0\",\"0\"]"]},
    {"name": "utf16-hash-domain", "text": "diff --git a/unicode.ts b/unicode.ts\nindex 1111111..2222222 100644\n--- a/unicode.ts\n+++ b/unicode.ts\n@@ -1 +1 @@\n-old\n+new \ud835\udefc caf\u00e9\n", "scope": "git:branch-range", "paths": ["unicode.ts"], "ids": ["ck1:[\"patch-file\",\"git:branch-range:130:18xoeop:cjth4l\",\"0\",\"0\"]"]},
    {"name": "empty", "text": "  \n ", "scope": "git:branch-range", "paths": [], "ids": []},
    {"name": "marker-only", "text": "\n[truncated]  \n", "scope": "git:branch-range", "paths": [], "ids": []},
  ];
  for (const vector of vectors) test(vector.name, () => {
    const parsed = reviewPatch(vector.text, vector.scope);
    expect({ paths: parsed.files.map(file => file.path), ids: parsed.files.map(file => file.id) }).toEqual({ paths: vector.paths, ids: vector.ids });
  });
});

describe('Review presentation marker and asynchronous ownership', () => {
  test('a late lazy disclosure cannot overwrite successor viewed/expanded presentation', async () => {
    const f = fixture(), paths = ['a.ts', 'b.ts', 'c.ts', 'd.ts'];
    let release!: (value: unknown) => void, entered!: () => void, held = false;
    const gate = new Promise(resolve => { release = resolve; }), began = new Promise<void>(resolve => { entered = resolve; });
    const reply = (path: string, generation = f.client.generation) => ({ ok: true, generation, value: { cwd: '/repo', sources: [
      { kind: 'branch-range', title: 'Changes', diffHash: 'held-disclosure', truncated: !path,
        files: paths.map(path => ({ path, previousPath: null, additions: 1, deletions: 1 })), diff: path ? patch(path) : '' }] } });
    f.hook(request => {
      if (request.method !== 'review.getDiffPreview') return;
      const path = String(obj(obj(request.payload).file).path || '');
      if (path === 'd.ts' && !held) { held = true; entered(); return gate; }
      return reply(path);
    });
    const first = await read(f); await act(f, 'toggle', first.files[3]!.id);
    const generation = f.client.generation, pending = act(f, 'toggle', first.files[3]!.id); await began;
    f.client.generation++; f.client.threadEpoch++;
    const successor = await read(f); await act(f, 'viewed', successor.files[3]!.id);
    release(reply('d.ts', generation)); const late = await pending;
    expect({ changedOwner: late.data.owner !== first.owner, navigation: late.navigation,
      file: late.data.files.find(file => file.path === 'd.ts') && { viewed: late.data.files[3]!.viewed, expanded: late.data.files[3]!.expanded } }).toEqual({
      changedOwner: true, navigation: '', file: { viewed: true, expanded: false } });
  });

  test('editing any ordinary file retires the whole patch markers and rejects stale disclosure callbacks', async () => {
    const f = fixture(); let diff = patch('z.ts') + patch('a.ts');
    f.hook(request => request.method === 'review.getDiffPreview' ? { ok: true, generation: f.client.generation,
      value: { cwd: '/repo', sources: [{ kind: 'branch-range', title: 'Changes', diff }] } } : undefined);
    const first = await read(f); await act(f, 'viewed', first.files[1]!.id);
    diff = diff.replace('+new', '+changed'); const changed = await read(f, '', true); f.calls.length = 0;
    const retired = await act(f, 'toggle', first.files[1]!.id), retiredViewed = await act(f, 'viewed', first.files[1]!.id);
    expect({ paths: changed.files.map(file => file.path), allRetired: changed.files.every(file => !first.files.some(old => old.id === file.id)),
      markers: retiredViewed.data.files.map(file => ({ expanded: file.expanded, viewed: file.viewed })),
      rejected: [!!retired.message, !!retiredViewed.message], native: f.calls.length }).toEqual({ paths: ['z.ts', 'a.ts'], allRetired: true,
      markers: [{ expanded: false, viewed: false }, { expanded: false, viewed: false }], rejected: [true, true], native: 0 });
  });

  test('repeated ordinary paths have independent markers and unique rows; path navigation targets an existing current header', async () => {
    const f = fixture();
    f.hook(request => request.method === 'review.getDiffPreview' ? { ok: true, generation: f.client.generation,
      value: { cwd: '/repo', sources: [{ kind: 'branch-range', diff: patch('same.ts') + patch('same.ts').replace('+new', '+second') }] } } : undefined);
    const first = await read(f), marked = await act(f, 'viewed', first.files[1]!.id);
    const navigation = await act(f, 'file', 'same.ts'), headers = navigation.data.rows.filter(row => row.kind === 'file');
    expect({ distinctIds: new Set(first.files.map(file => file.id)).size, distinctRows: new Set(first.rows.map(row => row.id)).size === first.rows.length,
      viewed: marked.data.files.map(file => file.viewed), expanded: marked.data.files.map(file => file.expanded),
      headerMarkers: headers.map(row => row.markerId), selectedPath: navigation.data.selectedPath, navigation: navigation.navigation }).toEqual({
      distinctIds: 2, distinctRows: true, viewed: [false, true], expanded: [true, false],
      headerMarkers: first.files.map(file => file.id), selectedPath: 'same.ts', navigation: headers[0]!.id });
  });

  test('identical ordinary content is section-scoped and a callback from the other section is retired', async () => {
    const f = fixture();
    f.hook(request => request.method === 'review.getDiffPreview' ? { ok: true, generation: f.client.generation,
      value: { cwd: '/repo', sources: ['working-tree', 'branch-range'].map(kind => ({ kind, diff: patch() })) } } : undefined);
    const first = await knownWorking(f); await act(f, 'viewed', first.files[0]!.id);
    const changes = await read(f, 'git:branch-range'), stale = await act(f, 'viewed', first.files[0]!.id);
    const working = await read(f, 'git:working-tree');
    expect({ different: first.files[0]!.id !== changes.files[0]!.id, stale: !!stale.message,
      otherViewed: stale.data.files[0]?.viewed, saved: { viewed: working.files[0]?.viewed, expanded: working.files[0]?.expanded } }).toEqual({
      different: true, stale: true, otherViewed: false, saved: { viewed: true, expanded: false } });
  });

  for (const superseded of ['owner', 'serial']) test(`late ${superseded} preview cannot publish headers or normalize a successor selection`, async () => {
    const f = fixture(); await knownWorking(f);
    let release!: (value: unknown) => void, entered!: () => void, held = false;
    const gate = new Promise(resolve => { release = resolve; }), began = new Promise<void>(resolve => { entered = resolve; });
    f.hook(request => {
      if (request.method !== 'review.getDiffPreview') return;
      if (!held) { held = true; entered(); return gate; }
      return { ok: true, generation: f.client.generation, value: { cwd: '/repo', sources: [
        { kind: 'working-tree', title: 'Current local', diff: '' }, { kind: 'branch-range', title: 'Current branch', diff: patch() }] } };
    });
    const generation = f.client.generation, pending = read(f, 'git:working-tree'); await began;
    if (superseded === 'owner') { f.client.generation++; f.client.threadEpoch++; }
    const current = await read(f, 'git:branch-range');
    release({ ok: true, generation, value: { cwd: '/repo', sources: [{ kind: 'working-tree', title: 'Late retired local', diff: patch('retired.ts') }] } });
    let kind = '';
    try { await pending; } catch (error) { kind = String(obj(error).kind || ''); }
    f.client.generation++; f.client.threadEpoch++;
    f.hook(request => { if (request.method === 'review.getDiffPreview') throw new Error('Observe header-only memory'); });
    const remembered = await read(f);
    expect({ current: current.sectionId, kind, remembered: remembered.sectionId, title: remembered.title,
      headers: remembered.sections.filter(row => row.id.startsWith('git:')).map(row => row.title),
      emptyBody: [remembered.files.length, remembered.rows.length, remembered.raw] }).toEqual({ current: 'git:branch-range',
      kind: superseded === 'owner' ? 'superseded' : '', remembered: 'git:branch-range', title: 'Current branch',
      headers: ['Current local', 'Current branch'], emptyBody: [0, 0, ''] });
  });
});

// R1 findings witnessed through the actual public primary factory/default export.
// Pending hook normalization is exact-source evidence, not an executed React UI.
describe('Review R2 witnessed selection and hunk recovery', () => {
  function holdGrant(f: ReturnType<typeof fixture>) {
    let release!: (value: unknown) => void, entered!: () => void;
    const gate = new Promise(resolve => { release = resolve; }), began = new Promise<void>(resolve => { entered = resolve; });
    f.hook(request => request.op === 'http' ? (entered(), gate) : undefined);
    return { began, release: () => release({ ok: true, generation: f.client.generation,
      value: { authenticated: true, permissions: ['filesystem:read'] } }) };
  }
  for (const missing of [false, true]) test(`cold pending sections normalize to Changes, missing preference=${missing}`, async () => {
    const f = fixture();
    if (missing) {
      await read(f, 'turn:1');
      f.client.thread!.projection.checkpoints = [{ runId: 'r3', appRunOrdinal: 3, status: 'ready', files: [] }];
      f.client.generation++; f.client.threadEpoch++;
    }
    const held = holdGrant(f), pending = read(f); await held.began;
    let during!: ReturnType<typeof mobileReviewSnapshot>;
    try { during = mobileReviewSnapshot(false, f.client); }
    finally { held.release(); await pending; }
    expect({ selected: during.sectionId, hasChanges: during.sections.some(row => row.id === 'git:branch-range'),
      emptyBody: [during.files.length, during.rows.length, during.raw], final: mobileReviewSnapshot(false, f.client).sectionId }).toEqual({
      selected: 'git:branch-range', hasChanges: true, emptyBody: [0, 0, ''], final: 'git:branch-range' });
  });

  test('an absent old choice that reappears in fresh sources cannot replace the live pending-normalized choice', async () => {
    const f = fixture(); f.client.thread!.projection.checkpoints = [];
    const initial = await read(f); expect(initial.sections.some(row => row.id === 'git:working-tree')).toBe(true);
    await act(f, 'section', 'git:working-tree');
    f.hook(request => request.method === 'review.getDiffPreview' ? { ok: true, generation: f.client.generation, value: { cwd: '/repo', sources: [] } } : undefined);
    const empty = await read(f); expect(empty.sections).toEqual([]);
    f.client.generation++; f.client.threadEpoch++;
    const held = holdGrant(f), pending = read(f, 'git:working-tree'); await held.began;
    let during!: ReturnType<typeof mobileReviewSnapshot>, final!: ReturnType<typeof mobileReviewSnapshot>;
    try { during = mobileReviewSnapshot(false, f.client); }
    finally { held.release(); final = await pending; }
    expect({ pendingChoice: during.sectionId, finalChoice: final.sectionId, oldChoiceReappeared: final.sections.some(row => row.id === 'git:working-tree'),
      liveChoice: final.sections.find(row => row.selected)?.id }).toEqual({ pendingChoice: 'git:branch-range', finalChoice: 'git:branch-range',
      oldChoiceReappeared: true, liveChoice: 'git:branch-range' });
  });

  for (const cached of [false, true]) test(`settled preview failure removes only loading placeholder, real Working cache=${cached}`, async () => {
    const f = fixture(); f.client.thread!.projection.checkpoints = [];
    if (cached) {
      f.hook(request => request.method === 'review.getDiffPreview' ? { ok: true, generation: f.client.generation,
        value: { cwd: '/repo', sources: [{ kind: 'working-tree', title: 'Real local header', diff: patch() }] } } : undefined);
      const initial = await read(f); expect(initial.sectionId).toBe('git:working-tree');
    }
    f.hook(request => { if (request.method === 'review.getDiffPreview') throw new Error('Witnessed preview failure'); });
    const failed = await read(f);
    expect({ headers: failed.sections.map(row => ({ id: row.id, title: row.title })), choice: failed.sectionId, loading: failed.loading,
      error: failed.error, body: [failed.files.length, failed.rows.length, failed.raw, failed.additions, failed.deletions] }).toEqual({
      headers: cached ? [{ id: 'git:working-tree', title: 'Real local header' }] : [], choice: cached ? 'git:working-tree' : '', loading: false,
      error: 'Witnessed preview failure', body: [0, 0, '', 0, 0] });
  });

  test('settled error normalizes against ready checkpoints after removing its loading-only Changes', async () => {
    const f = fixture();
    f.hook(request => { if (request.method === 'review.getDiffPreview') throw new Error('Witnessed preview failure'); });
    const failed = await read(f);
    expect({ sections: failed.sections.map(row => row.id), choice: failed.sectionId, body: [failed.files.length, failed.rows.length, failed.raw] }).toEqual({
      sections: ['turn:3', 'turn:1'], choice: 'turn:3', body: [0, 0, ''] });
  });

  for (const malformed of [false, true]) test(`unified count repair preserves following file order and marker controls, malformed=${malformed}`, () => {
    const text = `--- plain.txt\n+++ plain.txt\n@@ -${malformed ? '0,0' : '1,1'} +1,1 @@\n context\n--- next.txt\n+++ next.txt\n@@ -1,1 +1,1 @@\n next\n`;
    const parsed = reviewPatch(text, 'git:branch-range'), rows = reviewFileRows(parsed.files[0]!, false, null);
    const key = malformed ? 'git:branch-range:100:csxw51:v6dso1' : 'git:branch-range:100:16y8brx:9jmld5';
    expect({ paths: parsed.files.map(file => file.path), ids: parsed.files.map(file => file.id),
      header: rows.find(row => row.kind === 'hunk')?.text, lines: rows.filter(row => row.kind === 'line').map(row => [row.oldNumber, row.newNumber, row.text]) }).toEqual({
      paths: ['plain.txt', 'next.txt'], ids: [0, 1].map(index => `ck1:["patch-file","${key}","0","${index}"]`),
      header: '@@ -1,1 +1,1 @@', lines: [['1', '1', 'context']] });
  });

  test('mismatched nonzero-count start 0 remains a present gutter coordinate 0', () => {
    const parsed = reviewPatch('--- plain.txt\n+++ plain.txt\n@@ -0,2 +0,2 @@\n context\n', 'git:branch-range');
    const rows = reviewFileRows(parsed.files[0]!, false, null);
    expect({ starts: [parsed.files[0]!.hunks[0]!.oldStart, parsed.files[0]!.hunks[0]!.newStart],
      header: rows.find(row => row.kind === 'hunk')?.text, gutters: rows.filter(row => row.kind === 'line').map(row => [row.oldNumber, row.newNumber]) }).toEqual({
      starts: [0, 0], header: '@@ -0,1 +0,1 @@', gutters: [['0', '0']] });
  });

  test('repair of a now-empty side uses its consumed boundary and keeps that side absent', () => {
    const parsed = reviewPatch('--- plain.txt\n+++ plain.txt\n@@ -1,1 +1,1 @@\n+new\n', 'git:branch-range');
    const rows = reviewFileRows(parsed.files[0]!, false, null);
    expect({ starts: [parsed.files[0]!.hunks[0]!.oldStart, parsed.files[0]!.hunks[0]!.newStart],
      header: rows.find(row => row.kind === 'hunk')?.text, gutters: rows.filter(row => row.kind === 'line').map(row => [row.oldNumber, row.newNumber]) }).toEqual({
      starts: [0, 1], header: '@@ -0,0 +1,1 @@', gutters: [['', '1']] });
  });
});
