import { observeComposer, acceptComposerEdit } from '../support/composer-field-fixture';
// The Diff panel's tree, large-diff loading, hidden-line expansion and line comments, driven through
// T3Client.command as the Contract drives them (diff-review.ts), against a scripted server.
import { describe, test, expect } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { snapshot } from '../../src/client/presentation';
import { obj, type Obj } from '../../src/client/domain';
import type { Native, Files } from '../../src/client/protocol';
import { citationField } from '../timeline/citation-field.fixture';
import { composerChipCurrent } from '../../src/composer/composer-chip-bridge';
import { collectAssistantCitations } from '../../src/timeline/diff-citations';

const filePatch = (path: string, extra = '') => `diff --git a/${path} b/${path}\n--- a/${path}\n+++ b/${path}\n@@ -1,2 +1,3 @@\n one\n+added in ${path}\n two\n${extra}`;
const stats = Array.from({ length: 10 }, (_, index) => ({ path: index < 6 ? `src/ui/f${index}.ts` : `docs/d${index}.md`, previousPath: null, additions: 1, deletions: 0 }));
const smallPatch = 'diff --git a/a.ts b/a.ts\n--- a/a.ts\n+++ b/a.ts\n@@ -3,2 +3,3 @@\n three\n+inserted\n four\n@@ -9,1 +10,1 @@\n-nine\n+NINE\n';
const contents = { oldContents: 'one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\n', newContents: 'one\ntwo\nthree\ninserted\nfour\nfive\nsix\nseven\neight\nNINE\nten\n' };

function harness(large: boolean, preview = { patch: smallPatch, path: 'a.ts' }) {
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
          : { kind: 'branch-range', diff: preview.patch, truncated: false, diffHash: `h2:${preview.path}`, baseRef: 'main', headRef: 'feat', files: [{ path: preview.path, previousPath: null, additions: 2, deletions: 1 }] }] } };
    }
    if (request.op === 'request' && request.method === 'review.getDiffFileContents') return { ok: true, generation: 1, value: contents };
    if (request.op === 'editorInsert') return { ok: true, generation: 1, value: { applied: true } };
    if (request.op === 'editorEdit') return { ok: true, generation: 1, value: { applied: acceptComposerEdit(client, request) } };
    return { ok: true, generation: 1, value: {} };
  } };
  const storage: Files = { fs: { async mkdir() {}, async readFile() { throw new Error('missing'); }, async atomicWriteFile() {} } };
  const client = new T3Client();
  Object.assign(client, { available: true, generation: 1, connection: 'connected', environmentId: 'env', projectId: 'p1', threadId: 't1',
    configLive: true, shellLive: true, threadLive: true, scopes: ['orchestration:read', 'orchestration:operate', 'filesystem:read'],
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
    expect(gaps.map(item => [item.label, item.expandable])).toEqual([['2 unmodified lines', true], ['4 unmodified lines', true], ['More unchanged context may be available', true]]);
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
    // A click on number 3, then a Shift-click on 5 (the gutter's drags): the Diff panel's viewer has no
    // onLineSelectionEnd (AnnotatableCodeView), so the lines stay selected and no draft opens.
    await command('diffreview', 'drag:additions', 'a.ts', 3); await command('diffreview', 'end', 'a.ts', 3);
    await command('diffreview', 'drag:additions:shift', 'a.ts', 5); await command('diffreview', 'end', 'a.ts', 5);
    let view = snapshot(client);
    expect(view.diffItems.filter(item => item.kind === 'line').map(item => item.selected)).toEqual([true, true, true, false, false]);
    expect(view.diffCommentOpen).toBe(false);
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
    client.local.drafts[client.draftKey] = `Please look ${insert.text}`; observeComposer(client, client.draft);
    expect(snapshot(client).diffItems.find(item => item.kind === 'note')).toMatchObject({ text: 'Why this order?', entry: id });
    const sent = (await import('../../src/composer/composer-editor')).messageContext(client, client.draft);
    expect(sent).toMatchObject({ version: 1, records: [{ kind: 'review-comment', contextId: id, label: 'a.ts 3 to 5', sectionId: 'branch', sectionTitle: 'Changes', filePath: 'a.ts',
      startIndex: 0, endIndex: 2, rangeLabel: '3 to 5', text: 'Why this order?', diff: '@@ -3,2 +3,3 @@\n three\n+inserted\n four', fenceLanguage: 'diff' }] });
    await command('diffreview', 'delete', id);
    expect(calls.filter(call => call.op === 'editorEdit').at(-1)).toMatchObject({ all: true, text: 'Please look ' });
    expect(snapshot(client).diffItems.some(item => item.kind === 'note')).toBe(false);
  });
});

describe('orphaned drafts (AnnotatableCodeView)', () => {
  test('a draft whose file a refresh dropped is released, and the gutter opens a new one', async () => {
    const preview = { patch: smallPatch, path: 'a.ts' };
    const { client, command } = harness(false, preview);
    await command('diff'); await command('diffreview', 'reveal', 'a.ts');
    await command('diffreview', 'comment:additions', 'a.ts', 4);
    expect(snapshot(client).diffCommentOpen).toBe(true);
    preview.patch = smallPatch.replaceAll('a.ts', 'b.ts'); preview.path = 'b.ts';
    await command('diff-refresh');
    let view = snapshot(client);
    expect([view.diffCommentOpen, client.diffState.draft]).toEqual([false, null]);
    await command('diffreview', 'reveal', 'b.ts');
    await command('diffreview', 'comment:additions', 'b.ts', 4);
    view = snapshot(client);
    expect([view.diffCommentOpen, view.diffItems.find(item => item.kind === 'draft')?.path]).toEqual([true, 'b.ts']);
  });
});

test('a draft whose file came back as another change (modified, now added) is released too', async () => {
  const preview = { patch: smallPatch, path: 'a.ts' };
  const { client, command } = harness(false, preview);
  await command('diff'); await command('diffreview', 'reveal', 'a.ts');
  await command('diffreview', 'comment:additions', 'a.ts', 4);
  expect(client.diffState.draft).toMatchObject({ path: 'a.ts', previous: 'a.ts', status: 'modified' });
  preview.patch = 'diff --git a/a.ts b/a.ts\nnew file mode 100644\n--- /dev/null\n+++ b/a.ts\n@@ -0,0 +1,2 @@\n+one\n+two\n';
  await command('diff-refresh');
  expect([snapshot(client).diffCommentOpen, client.diffState.draft]).toEqual([false, null]);
});

describe('the gutter\'s drags in the Diff panel', () => {
  test('a drag on the numbers selects the lines it crosses; a drag from the "+" opens the draft on its range', async () => {
    const { client, command } = harness(false);
    await command('diff'); await command('diffreview', 'reveal', 'a.ts');
    await command('diffreview', 'drag:additions', 'a.ts', 3);
    await command('diffreview', 'to', 'dl:additions:4:a.ts', 0);
    await command('diffreview', 'to', 'dl:additions:5:a.ts', 0);
    await command('diffreview', 'end', 'a.ts', 3);
    let view = snapshot(client);
    expect([view.diffItems.filter(item => item.kind === 'line').map(item => item.selected), view.diffCommentOpen]).toEqual([[true, true, true, false, false], false]);
    // A press on the one selected line that never moves clears it.
    await command('diffreview', 'drag:additions', 'a.ts', 9); await command('diffreview', 'end', 'a.ts', 9);
    await command('diffreview', 'drag:additions', 'a.ts', 9); await command('diffreview', 'end', 'a.ts', 9);
    expect(snapshot(client).diffItems.filter(item => item.kind === 'line').some(item => item.selected)).toBe(false);
    await command('diffreview', 'gutter:additions', 'a.ts', 3);
    await command('diffreview', 'to', 'dl:additions:5:a.ts', 0);
    await command('diffreview', 'end', 'a.ts', 3);
    view = snapshot(client);
    const draft = view.diffItems.findIndex(item => item.kind === 'draft');
    expect([view.diffCommentOpen, view.diffItems[draft]?.label]).toEqual([true, '3 to 5']);
  });
});

describe('the "+" pinned to a selection (placeUtilityFromSelection; the review of #413)', () => {
  const lines = (client: T3Client) => snapshot(client).diffItems.filter(item => item.kind === 'line');
  const pins = (client: T3Client) => lines(client).filter(item => item.pin).map(item => `${item.side}:${item.line}`);
  test('a selection pins the "+" to its bottom line, and it follows the drag; without one it follows the hover', async () => {
    const { client, command } = harness(false);
    await command('diff'); await command('diffreview', 'reveal', 'a.ts');
    expect([lines(client).some(item => item.pinned), pins(client)]).toEqual([false, []]);
    await command('diffreview', 'drag:additions', 'a.ts', 3);
    expect([lines(client).every(item => item.pinned), pins(client)]).toEqual([true, ['additions:3']]);
    await command('diffreview', 'to', 'dl:additions:5:a.ts', 0);
    expect(pins(client)).toEqual(['additions:5']);
    await command('diffreview', 'end', 'a.ts', 3);
    expect([pins(client), snapshot(client).diffCommentOpen]).toEqual([['additions:5'], false]);
    // A click on the one selected line clears it: the "+" follows the hover again.
    await command('diffreview', 'drag:additions', 'a.ts', 10); await command('diffreview', 'end', 'a.ts', 10);
    await command('diffreview', 'drag:additions', 'a.ts', 10); await command('diffreview', 'end', 'a.ts', 10);
    expect([lines(client).some(item => item.pinned), pins(client)]).toEqual([false, []]);
  });
  test('a click on the "+" opens one draft on the selection, whichever of its press and release runs first', async () => {
    // Numbers 3 to 5 dragged (they stay selected, no draft), then the "+": on line 5, the selection's bottom where it is
    // drawn, or on line 10 (a press no pointer could make now: before the pin, the "+" under the pointer there opened the
    // draft on 3 to 5 or on line 10 alone, by which of its two sends ran first).
    for (const line of [5, 10]) for (const order of [['gutter', 'end', 'comment'], ['gutter', 'comment', 'end'], ['comment']]) {
      const { client, command } = harness(false);
      await command('diff'); await command('diffreview', 'reveal', 'a.ts');
      await command('diffreview', 'drag:additions', 'a.ts', 3); await command('diffreview', 'to', 'dl:additions:5:a.ts', 0); await command('diffreview', 'end', 'a.ts', 3);
      for (const step of order) {
        if (step === 'gutter') await command('diffreview', 'gutter:additions', 'a.ts', line);
        if (step === 'end') await command('diffreview', 'end', 'a.ts', line);
        if (step === 'comment') await command('diffreview', 'comment:additions', 'a.ts', line);
      }
      const view = snapshot(client);
      expect([line, order.join(','), view.diffItems.filter(item => item.kind === 'draft').map(item => item.label)]).toEqual([line, order.join(','), ['3 to 5']]);
    }
  });
  test('a "+" drag whose press arrives before its release comments on the range it was dragged over', async () => {
    const { client, command } = harness(false);
    await command('diff'); await command('diffreview', 'reveal', 'a.ts');
    await command('diffreview', 'gutter:additions', 'a.ts', 3);
    await command('diffreview', 'to', 'dl:additions:5:a.ts', 0);
    await command('diffreview', 'comment:additions', 'a.ts', 3);
    await command('diffreview', 'end', 'a.ts', 3);
    expect(snapshot(client).diffItems.filter(item => item.kind === 'draft').map(item => item.label)).toEqual(['3 to 5']);
  });
});

describe('what sits under a selected line is painted selected (Pierre\'s renderSelection)', () => {
  test('the draft under its range, and a saved comment once its line is selected again', async () => {
    const { client, command, calls } = harness(false);
    await command('diff'); await command('diffreview', 'reveal', 'a.ts');
    const notes = () => snapshot(client).diffItems.filter(item => item.kind === 'note' || item.kind === 'draft').map(item => `${item.kind}${item.selected ? '*' : ''}`);
    await command('diffreview', 'gutter:additions', 'a.ts', 3); await command('diffreview', 'to', 'dl:additions:5:a.ts', 0); await command('diffreview', 'end', 'a.ts', 3);
    expect(notes()).toEqual(['draft*']);
    await command('diffreview', 'save', 'Why this order?');
    // The saved comment lives as long as its chip does in the composer's text.
    client.local.drafts[client.draftKey] = String(calls.filter(call => call.op === 'editorInsert').at(-1)!.text);
    expect(notes()).toEqual(['note']);
    await command('diffreview', 'drag:additions', 'a.ts', 3); await command('diffreview', 'to', 'dl:additions:4:a.ts', 0); await command('diffreview', 'end', 'a.ts', 3);
    expect(notes()).toEqual(['note']);
    await command('diffreview', 'drag:additions', 'a.ts', 4); await command('diffreview', 'to', 'dl:additions:5:a.ts', 0); await command('diffreview', 'end', 'a.ts', 4);
    expect(notes()).toEqual(['note*']);
  });
});

describe('Cite', () => {
  test('a selection across answer blocks inserts once and opens through independent ACK', async () => {
    const { client, command, calls, native } = harness(false);
    const state = citationField('', 0, 0, client, { ...native, later: native.later.bind(native) });
    native.later = state.native.later;
    const stream = 'First paragraph of the answer.\nSecond block with more words.', waiting = () => state.next();
    const pending = command('diffreview', 'cite:["t1","turn-item:provider:codex:native-item:a1"]', `23-30,31-37|412,180.5,540,22|${stream}`);
    await pending;
    const insertion = waiting();
    const citations = collectAssistantCitations(insertion.text);
    expect(citations).toHaveLength(1);
    expect(citations[0]?.citation).toEqual({ version: 1, environmentId: 'env', threadId: 't1', messageId: 'message:provider:codex:native-item:a1',
      text: 'answer.\nSecond', start: 23, end: 37, prefix: 'First paragraph of the ', suffix: ' block with more words.' });
    expect(insertion.citation).toBeGreaterThan(0);
    state.input(insertion);
    expect(composerChipCurrent(client)).toBeNull();
    expect((await state.ack(insertion)).applied).toBe(true);
    expect(composerChipCurrent(client)).toMatchObject({ kind: 'citation', open: true, anchor: 'source', anchorFrame: [412, 180.5, 540, 22] });
    await pending;
    expect(collectAssistantCitations(state.current().value)).toHaveLength(1);
    expect(calls.filter(call => String(call.op).startsWith('editor'))).toEqual([]);
  });

  test('shared input refusal opens nothing and does not append a fallback quote', async () => {
    const { client, command, calls, native } = harness(false);
    const state = citationField('kept', 4, 4, client, { ...native, later: native.later.bind(native) });
    native.later = state.native.later;
    const waiting = () => state.next();
    const pending = command('diffreview', 'cite:["t1","a1"]', '0-5|10,20,80,18|Hello world');
    await pending;
    const insertion = waiting();
    expect((await state.ack(insertion, false)).applied).toBe(false);
    await pending;
    expect(state.current().value).toBe('kept');
    expect(composerChipCurrent(client)).toBeNull();
    expect(calls.filter(call => String(call.op).startsWith('editor'))).toEqual([]);
  });
});
