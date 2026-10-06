// The Diff panel's tree, large-diff loading, hidden-line expansion and line comments, driven through
// T3Client.command as the Contract drives them (diff-review.ts), against a scripted server.
import { describe, test, expect } from 'bun:test';
import { T3Client } from './client';
import { snapshot } from './presentation';
import { obj, type Obj } from './domain';
import type { Native, Files } from './protocol';

const filePatch = (path: string, extra = '') => `diff --git a/${path} b/${path}\n--- a/${path}\n+++ b/${path}\n@@ -1,2 +1,3 @@\n one\n+added in ${path}\n two\n${extra}`;
const stats = Array.from({ length: 10 }, (_, index) => ({ path: index < 6 ? `src/ui/f${index}.ts` : `docs/d${index}.md`, previousPath: null, additions: 1, deletions: 0 }));
const smallPatch = 'diff --git a/a.ts b/a.ts\n--- a/a.ts\n+++ b/a.ts\n@@ -3,2 +3,3 @@\n three\n+inserted\n four\n@@ -9,1 +10,1 @@\n-nine\n+NINE\n';
const contents = { oldContents: 'one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\n', newContents: 'one\ntwo\nthree\ninserted\nfour\nfive\nsix\nseven\neight\nNINE\nten\n' };

function harness(large: boolean) {
  const calls: Obj[] = [];
  const failOnce = new Set(['src/ui/f2.ts']);
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    const payload = obj(request.payload);
    if (request.op === 'request' && request.method === 'review.getDiffPreview') {
      const file = obj(payload.file);
      if (typeof file.path === 'string') {
        if (failOnce.delete(file.path)) return { ok: false, generation: 1, error: { message: 'boom', kind: 'Server' } };
        return { ok: true, generation: 1, value: { cwd: '/repo', sources: [{ kind: 'branch-range', diff: filePatch(file.path), truncated: false, diffHash: 'h1', baseRef: 'main', headRef: 'feat' }] } };
      }
      return { ok: true, generation: 1, value: { cwd: '/repo', sources: [{ kind: 'working-tree', diff: '', truncated: false, diffHash: 'w' },
        large ? { kind: 'branch-range', diff: '', truncated: true, diffHash: 'h1', baseRef: 'main', headRef: 'feat', files: stats }
          : { kind: 'branch-range', diff: smallPatch, truncated: false, diffHash: 'h2', baseRef: 'main', headRef: 'feat', files: [{ path: 'a.ts', previousPath: null, additions: 2, deletions: 1 }] }] } };
    }
    if (request.op === 'request' && request.method === 'review.getDiffFileContents') return { ok: true, generation: 1, value: contents };
    if (request.op === 'editorInsert' || request.op === 'editorEdit') return { ok: true, generation: 1, value: { applied: true } };
    return { ok: true, generation: 1, value: {} };
  } };
  const storage: Files = { fs: { async mkdir() {}, async readFile() { throw new Error('missing'); }, async atomicWriteFile() {} } };
  const client = new T3Client();
  Object.assign(client, { available: true, generation: 1, connection: 'connected', environmentId: 'env', projectId: 'p1', threadId: 't1',
    configLive: true, shellLive: true, threadLive: true, scopes: ['orchestration:read', 'orchestration:operate'],
    config: { environment: { capabilities: { serverResolvedCommandContext: true } } } });
  client.shell.projects = [{ id: 'p1', title: 'Fixture', workspaceRoot: '/repo' }];
  client.thread = { projection: { thread: { id: 't1' }, runtimeRequests: [], turnItems: [], runs: [], checkpoints: [] }, sequence: 0, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null };
  const command = (op: string, id = '', value = '', n = 0) => client.command(op, id, value, n, native, storage);
  const fileAsks = () => calls.filter(call => call.method === 'review.getDiffPreview' && obj(obj(call.payload).file).path).map(call => obj(obj(call.payload).file).path);
  return { client, command, calls, native, fileAsks };
}

describe('diff review engine', () => {
  test('a large preview reads four files at a time, shows ghosts, stats, Retry and a reveal of a later file', async () => {
    const { client, command, fileAsks } = harness(true);
    await command('diff');
    expect(fileAsks()).toEqual(['docs/d6.md', 'docs/d7.md', 'docs/d8.md', 'docs/d9.md']);
    let view = snapshot(client);
    expect(view).toMatchObject({ diffTruncated: false, diffAdditions: 10, diffDeletions: 0 });
    expect(view.diffItems.map(item => item.kind)).toEqual(['file', 'file', 'file', 'file', 'ghost', 'ghost', 'ghost', 'ghost']);
    await command('diffreview', 'next');
    expect(fileAsks().slice(4)).toEqual(['src/ui/f0.ts', 'src/ui/f1.ts', 'src/ui/f2.ts', 'src/ui/f3.ts']);
    view = snapshot(client);
    const failed = view.diffItems.find(item => item.path === 'src/ui/f2.ts')!;
    expect(failed).toMatchObject({ kind: 'file', error: true, unavailable: true, expanded: false });
    expect(view.diffItems.filter(item => item.kind === 'ghost')).toHaveLength(2);
    await command('diffreview', 'retry', 'src/ui/f2.ts');
    expect(snapshot(client).diffItems.find(item => item.path === 'src/ui/f2.ts')).toMatchObject({ error: false, unavailable: false });
    // The tree lists every file, folders flattened and open; a click on an unread file asks for it and opens it.
    expect(snapshot(client).diffTreeRows.map(row => row.name)).toEqual(['docs', 'd6.md', 'd7.md', 'd8.md', 'd9.md', 'src/ui', 'f0.ts', 'f1.ts', 'f2.ts', 'f3.ts', 'f4.ts', 'f5.ts']);
    await command('diffreview', 'reveal', 'src/ui/f5.ts');
    expect(fileAsks().at(-1)).toBe('src/ui/f5.ts');
    view = snapshot(client);
    expect(view.diffItems.find(item => item.path === 'src/ui/f5.ts' && item.kind === 'file')).toMatchObject({ expanded: true });
    expect(view.diffTreeRows.find(row => row.path === 'src/ui/f5.ts')).toMatchObject({ selected: true });
    await command('diff-view', 'folders', 'collapse');
    expect(snapshot(client)).toMatchObject({ diffTreeAllOpen: false });
    expect(snapshot(client).diffTreeRows.map(row => row.name)).toEqual(['docs', 'src/ui']);
    await command('diff-view', 'folder', 'src/ui/');
    expect(snapshot(client).diffTreeRows.map(row => row.name)).toContain('f0.ts');
  });

  test('a separator opens hidden lines from the file contents, once per file', async () => {
    const { client, command, calls } = harness(false);
    await command('diff'); await command('diffreview', 'reveal', 'a.ts');
    let gaps = snapshot(client).diffItems.filter(item => item.kind === 'gap');
    expect(gaps.map(item => [item.label, item.expandable])).toEqual([['2 unmodified lines', true], ['4 unmodified lines', true]]);
    await command('diffreview', 'expand', 'a.ts', 1);
    expect(calls.filter(call => call.method === 'review.getDiffFileContents').map(call => call.payload)).toEqual([
      { cwd: '/repo', sourceKind: 'branch-range', changeType: 'change', baseRef: 'main', headRef: 'feat', oldPath: 'a.ts', newPath: 'a.ts' }]);
    const view = snapshot(client);
    gaps = view.diffItems.filter(item => item.kind === 'gap');
    expect(gaps.map(item => item.label)).toEqual(['2 unmodified lines', '1 unmodified line']);
    expect(view.diffItems.filter(item => item.kind === 'line').map(item => [item.tone, item.number])).toEqual([
      ['context', '3'], ['addition', '4'], ['context', '5'], ['context', '6'], ['context', '7'], ['context', '8'], ['context', '9'], ['deletion', '9'], ['addition', '10']]);
    await command('diffreview', 'expand', 'a.ts', 0);
    expect(calls.filter(call => call.method === 'review.getDiffFileContents')).toHaveLength(1);
  });

  test('a line comment becomes a review-comment chip with the reference record; deleting it removes the chip', async () => {
    const { client, command, calls } = harness(false);
    await command('diff'); await command('diffreview', 'reveal', 'a.ts');
    await command('diffreview', 'line:additions', 'a.ts', 3);
    await command('diffreview', 'line:additions:shift', 'a.ts', 5);
    let view = snapshot(client);
    expect(view.diffItems.filter(item => item.kind === 'line').map(item => item.selected)).toEqual([true, true, true, false, false]);
    await command('diffreview', 'comment:additions', 'a.ts', 4);
    view = snapshot(client);
    expect(view.diffCommentOpen).toBe(true);
    const draft = view.diffItems.findIndex(item => item.kind === 'draft');
    expect(view.diffItems[draft]).toMatchObject({ label: '3 to 5' });
    expect(view.diffItems[draft - 1]).toMatchObject({ kind: 'line', number: '5' });
    // A mixed range keeps no side mark (reviewCommentContextLabel: "a.ts 3 to 5"; a range of additions only reads "L4 to L5").
    await command('diffreview', 'save', ' Why this order? ');
    const insert = calls.filter(call => call.op === 'editorInsert').at(-1)!;
    const id = /review-comment\/([^)]+)\)/.exec(String(insert.text))![1]!;
    expect(insert.text).toBe(`[a.ts 3 to 5](t3-context://v1/review-comment/${id}) `);
    client.local.drafts[client.draftKey] = `Please look ${insert.text}`;
    expect(snapshot(client).diffItems.find(item => item.kind === 'note')).toMatchObject({ text: 'Why this order?', entry: id });
    const sent = (await import('./composer-editor')).messageContext(client, client.draft);
    expect(sent).toMatchObject({ version: 1, records: [{ kind: 'review-comment', contextId: id, label: 'a.ts 3 to 5', sectionId: 'branch', sectionTitle: 'Changes', filePath: 'a.ts',
      startIndex: 0, endIndex: 2, rangeLabel: '3 to 5', text: 'Why this order?', diff: '@@ -3,2 +3,3 @@\n three\n+inserted\n four', fenceLanguage: 'diff' }] });
    await command('diffreview', 'delete', id);
    expect(calls.filter(call => call.op === 'editorEdit').at(-1)).toMatchObject({ all: true, text: 'Please look ' });
    expect(snapshot(client).diffItems.some(item => item.kind === 'note')).toBe(false);
  });
});
