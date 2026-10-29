import { describe, test, expect } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { snapshot } from '../../src/client/presentation';
import { parsePatch, wordMarks, diffSnapshot, turnSummaries, hunkRows, ROW_LIMIT } from '../../src/diff/diff';
import { highlightDeferrals, resetHighlights } from '../../src/highlight/highlights';
import { obj, type Obj } from '../../src/client/domain';
import type { Native, Files } from '../../src/client/protocol';
import { settleHighlights } from '../support/highlights-fixture';

const turnPatch = 'diff --git a/fixture-result.md b/fixture-result.md\nindex 1..2 100644\n--- a/fixture-result.md\n+++ b/fixture-result.md\n@@ -1,3 +1,3 @@\n # Exact T3 fixture\n \n-Completed run 35c7a0ce.\n+Completed run 151596fd.\n';
const treePatch = 'diff --git a/README.md b/README.md\ndeleted file mode 100644\n--- a/README.md\n+++ /dev/null\n@@ -1,2 +0,0 @@\n-# T3\n-Isolated\ndiff --git a/src/new.ts b/src/new.ts\nnew file mode 100644\n--- /dev/null\n+++ b/src/new.ts\n@@ -0,0 +1 @@\n+export {}\ndiff --git a/a.ts b/a.ts\n--- a/a.ts\n+++ b/a.ts\n@@ -1,2 +1,2 @@\n one\n-two\n+TWO\n@@ -40,2 +40,3 @@\n forty\n+fortyone\n end\n';

function harness(turn = turnPatch) {
  const calls: Obj[] = [];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    if (request.op === 'request' && request.method === 'orchestration.getTurnDiff') return { ok: true, generation: 1, value: { diff: turn } };
    if (request.op === 'request' && request.method === 'review.getDiffPreview') return { ok: true, generation: 1, value: { cwd: '/repo', sources: [
      { kind: 'working-tree', diff: '', truncated: true }, { kind: 'branch-range', diff: treePatch, truncated: false }] } };
    return { ok: true, generation: 1, value: {} };
  } };
  let saved = '';
  const storage: Files = { fs: { async mkdir() {}, async readFile() { if (!saved) throw new Error('missing'); return new TextEncoder().encode(saved).buffer; },
    async atomicWriteFile(_path, bytes) { saved = new TextDecoder().decode(bytes); } } };
  const client = new T3Client();
  Object.assign(client, { available: true, generation: 1, connection: 'connected', environmentId: 'env', projectId: 'p1', threadId: 't1',
    configLive: true, shellLive: true, threadLive: true, scopes: ['orchestration:read', 'orchestration:operate', 'filesystem:read'],
    config: { environment: { capabilities: { serverResolvedCommandContext: true } } } });
  client.shell.projects = [{ id: 'p1', title: 'Fixture', workspaceRoot: '/repo' }];
  client.thread = { projection: { thread: { id: 't1' }, runtimeRequests: [], turnItems: [], runs: [{ id: 'r1', completedAt: '2026-10-03T14:15:00Z' }, { id: 'r2', completedAt: '2026-10-03T14:20:00Z' }],
    checkpoints: [{ id: 'c1', status: 'ready', runId: 'r1', appRunOrdinal: 1 }, { id: 'c2', status: 'ready', runId: 'r2', appRunOrdinal: 2 }] }, sequence: 0, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null };
  const command = (op: string, id = '', value = '', n = 0) => client.command(op, id, value, n, native, storage);
  return { client, command, calls };
}
const requests = (calls: Obj[]) => calls.filter(call => call.op === 'request');

describe('diff panel', () => {
  test('a workspace outside git shows the reference\'s notice, not an empty diff or an error (DiffPanel !isGitRepo)', async () => {
    const { client, command, calls } = harness();
    const { watchVcsStatus, vcsStatusEvent } = await import('../../src/details/shell-vcs');
    void watchVcsStatus(client, { available: true, watch() {}, later: async () => ({ ok: true, generation: 1, value: {} }) }, '/repo', 1000);
    vcsStatusEvent(client, { subscriptionId: '1-99', value: { _tag: 'snapshot', local: { isRepo: false }, remote: null } });
    await command('diff');
    expect(requests(calls).filter(call => call.method === 'review.getDiffPreview')).toHaveLength(0);
    expect(snapshot(client)).toMatchObject({ diffError: '', diffEmpty: true, diffEmptyLabel: 'Turn diffs are unavailable because this project is not a git repository.' });
    // ChatView: the Diff surface is offered for Git repositories only.
    expect((await import('../../src/panel/panel-surfaces')).availability(client).diff).toBe(false);
  });
  test('parses statuses, numbered hunks and hidden ranges from git patches', () => {
    const files = parsePatch(treePatch);
    expect(files.map(file => [file.path, file.status, file.additions, file.deletions])).toEqual([['a.ts', 'modified', 2, 1], ['README.md', 'deleted', 0, 2], ['src/new.ts', 'added', 1, 0]]);
    expect(files[0]!.hunks[1]!.lines.map(line => [line.kind, line.old, line.next])).toEqual([['context', 40, 40], ['addition', 0, 41], ['context', 41, 42]]);
  });
  test('word marks highlight only the changed middle of a replaced line', () => {
    expect(wordMarks('Completed run 35c7a0ce-6e97.', 'Completed run 151596fd-5dc0.')).toEqual([
      [{ id: '0', text: 'Completed run ', mark: false, syntax: '' }, { id: '1', text: '35c7a0ce-6e97', mark: true, syntax: '' }, { id: '2', text: '.', mark: false, syntax: '' }],
      [{ id: '0', text: 'Completed run ', mark: false, syntax: '' }, { id: '1', text: '151596fd-5dc0', mark: true, syntax: '' }, { id: '2', text: '.', mark: false, syntax: '' }]]);
    expect(wordMarks('alpha', 'beta')[0]).toEqual([{ id: '0', text: 'alpha', mark: false, syntax: '' }]);
  });
  test('the header toggle opens Changes (the branch); checkpoints open their own turn diff', async () => {
    const { client, command, calls } = harness();
    await command('diff');
    expect(requests(calls).at(-1)).toMatchObject({ method: 'review.getDiffPreview', payload: { cwd: '/repo', ignoreWhitespace: true } });
    expect(snapshot(client)).toMatchObject({ diffOpen: true, diffScopeLabel: 'Changes', diffCanRefresh: true, diffAdditions: 3, diffDeletions: 3, diffAllCollapsed: true });
    expect(snapshot(client).diffItems.map(item => item.kind)).toEqual(['file', 'file', 'file']);
    await command('checkpoint-diff', 'fixture-result.md', '', 1);
    expect(requests(calls).at(-1)).toMatchObject({ method: 'orchestration.getTurnDiff', payload: { threadId: 't1', fromTurnCount: 0, toTurnCount: 1, ignoreWhitespace: true } });
    // The named file opens (revealDiffFile), and the list is asked to scroll to it.
    const opened = snapshot(client);
    expect(opened).toMatchObject({ diffScopeLabel: 'Turn 1', diffLatestSelected: false, diffCanRefresh: false, diffAllCollapsed: false, diffReveal: 1, diffRevealTarget: 'file:fixture-result.md' });
    expect(opened.diffItems.map(item => [item.kind, item.tone, item.number])).toEqual([['file', '', ''], ['line', 'context', '1'], ['line', 'context', '2'], ['line', 'deletion', '3'], ['line', 'addition', '3'], ['pad', '', '']]);
    // The served panel marks changed words with Pierre's "word-alt" despite DiffPanel's `lineDiffType: "none"` (9c161462c9 oracle).
    expect(opened.diffItems[3]!.segments.map((segment: Obj) => segment.mark)).toEqual([false, true, false]);
    expect(opened.diffItems[1]!.gutter).toBe(33.3);
    // The lines paint plain until the lexers answer (highlights.ts); then the heading is `title` (bold, synWeight).
    expect(opened.diffItems[1]!.segments.map((segment: Obj) => segment.syntax)).toEqual(['']);
    settleHighlights();
    expect(snapshot(client).diffItems[1]!.segments.map((segment: Obj) => segment.syntax)).toEqual(['title']);
  });
  test('a file change’s Open diff (turn-diff) opens the panel on that run’s turn and file (onOpenTurnDiff)', async () => {
    const { client, command, calls } = harness();
    expect(snapshot(client).diffOpen).toBe(false);
    await command('turn-diff', 'fixture-result.md', 'r1', 0);
    expect(requests(calls).at(-1)).toMatchObject({ method: 'orchestration.getTurnDiff', payload: { threadId: 't1', fromTurnCount: 0, toTurnCount: 1 } });
    expect(snapshot(client)).toMatchObject({ diffOpen: true, diffScope: 'turn:r1', diffScopeLabel: 'Turn 1', diffLatestSelected: false, diffError: '' });
    expect(client.diffState.selections['env:t1']).toEqual({ kind: 'turn', runId: 'r1', filePath: 'fixture-result.md', baseRef: null, reveal: 1 });
    // A change with no run asks for nothing and says so in the panel.
    const count = requests(calls).length;
    await command('turn-diff', 'fixture-result.md', '', 0);
    expect(requests(calls).length).toBe(count);
    expect(snapshot(client)).toMatchObject({ diffOpen: true, diffError: 'That turn is no longer available.' });
  });
  test('scope, whitespace, layout, wrap, tree and collapse are real and stateful', async () => {
    const { client, command, calls } = harness();
    await command('diff');
    await command('diff-scope', '', 'latest');
    expect(snapshot(client)).toMatchObject({ diffScopeLabel: 'Latest turn', diffLatestSelected: true });
    expect(requests(calls).at(-1)).toMatchObject({ payload: { fromTurnCount: 1, toTurnCount: 2 } });
    expect(snapshot(client).diffTurns.map(turn => [turn.label, turn.selected])).toEqual([['Turn 2', true], ['Turn 1', false]]);
    await command('diff-whitespace');
    expect(requests(calls).at(-1)).toMatchObject({ payload: { ignoreWhitespace: false } });
    await command('diff-view', 'expand-all');
    await command('diff-view', 'layout', 'split');
    const split = snapshot(client);
    expect(split.diffItems.filter(item => item.kind === 'split').map(item => [item.leftTone, item.leftNumber, item.rightTone, item.rightNumber]))
      .toEqual([['context', '1', 'context', '1'], ['context', '2', 'context', '2'], ['deletion', '3', 'addition', '3']]);
    await command('diff-view', 'wrap'); await command('diff-view', 'tree');
    expect(snapshot(client)).toMatchObject({ diffWrap: false, diffTree: true, diffLayout: 'split', diffAllCollapsed: false });
    await command('diff-scope', '', 'unstaged');
    expect(snapshot(client)).toMatchObject({ diffScopeLabel: 'Uncommitted', diffTruncated: true, diffEmpty: true, diffEmptyLabel: 'No net changes in this selection.' });
    await command('close-diff');
    await command('diff-refresh');
    expect(snapshot(client).diffOpen).toBe(false);
    await command('diff');
    await command('diff-scope', '', 'turn:gone');
    expect(snapshot(client).diffError).toBe('That turn is no longer available.');
  });
  test('hunks past the row limit only count: their sides ask the lexers for nothing', async () => {
    const hunk = (k: number) => [`@@ -${k * 10 + 1},7 +${k * 10 + 1},7 @@`, ` a${k}`, ` b${k}`, ` c${k}`, `-let old${k} = ${k};`, `+let new${k} = ${k};`, ` d${k}`, ` e${k}`, ` f${k}`].join('\n');
    const { client, command } = harness(`diff --git a/rows.ts b/rows.ts\n--- a/rows.ts\n+++ b/rows.ts\n${Array.from({ length: 600 }, (_, k) => hunk(k)).join('\n')}\n`);
    await command('checkpoint-diff', 'rows.ts', '', 1);
    const deferrals = highlightDeferrals;
    expect(snapshot(client).diffTruncated).toBe(true);
    // Eight rows a hunk: the first 500 fill the 4,000 rows, and only their two sides ask for tokens.
    expect(highlightDeferrals - deferrals).toBe(2 * ROW_LIMIT / 8);
    resetHighlights();
  });
  test('the hidden range before a later hunk is labeled, and turns sort newest first', async () => {
    const { client, command } = harness();
    await command('diff'); await command('diff-view', 'reveal', 'a.ts');
    const gaps = snapshot(client).diffItems.filter(item => item.kind === 'gap');
    // Past the last hunk the count is not known before the contents load (Pierre's "unknown" separator).
    expect(gaps.map(item => item.label)).toEqual(['37 unmodified lines', 'More unchanged context may be available']);
    expect(turnSummaries(client.projection).map(turn => turn.count)).toEqual([2, 1]);
    expect(diffSnapshot(client, 0).diffFiles.map(file => file.letter)).toEqual(['M', 'D', 'A']);
  });
});

test('a replaced line pair projects word-alt segments on both sides, stacked and split', () => {
  const [file] = parsePatch(['diff --git a/README.md b/README.md', '--- a/README.md', '+++ b/README.md', '@@ -1,3 +1,3 @@', ' # agent-edits', ' ',
    '-A fixture repository.', '+A fixture repository, edited by the ACP stand-in.', ''].join('\n'));
  const marked = (segments: { text: string; mark: boolean }[]) => segments.filter(segment => segment.mark).map(segment => segment.text).join('|');
  const { stacked, split } = hunkRows(file!.hunks[0]!, file!.path, true);
  expect(stacked.map(row => [row.tone, marked(row.segments)])).toEqual([['context', ''], ['context', ''], ['deletion', ''], ['addition', ', edited by the ACP stand-in']]);
  expect([marked(split[2]!.left.segments), marked(split[2]!.right.segments)]).toEqual(['', ', edited by the ACP stand-in']);
  expect(split[2]!.right.segments.map(segment => segment.text).join('')).toBe('A fixture repository, edited by the ACP stand-in.');
  // Pierre's maxLineDiffLength: a pair with a line over 1000 characters is not marked.
  const [long] = parsePatch(['diff --git a/a.txt b/a.txt', '--- a/a.txt', '+++ b/a.txt', '@@ -1 +1 @@', `-${'a'.repeat(1001)}`, `+${'a'.repeat(1001)}b`, ''].join('\n'));
  expect(hunkRows(long!.hunks[0]!, long!.path, true).stacked.map(row => marked(row.segments))).toEqual(['', '']);
  // Without `marks` (the pull request Code tab) the rows keep no marks.
  expect(hunkRows(file!.hunks[0]!, file!.path).stacked.map(row => marked(row.segments))).toEqual(['', '', '', '']);
});

test('a Markdown list marker paints punctuation grey; the heading keeps its ink, bold', () => {
  // @pierre/highlights' markdown lexer: a heading is `title` (weight 700, synWeight), a list marker punctuation.
  const rowsOf = (patch: string[]) => {
    const [file] = parsePatch(patch.join('\n'));
    hunkRows(file!.hunks[0]!, file!.path);
    settleHighlights();
    return hunkRows(file!.hunks[0]!, file!.path).stacked.map(row => row.segments.filter(segment => segment.text.trim() !== '').map(segment => [segment.text, segment.syntax]));
  };
  expect(rowsOf(['diff --git a/notes/plan.md b/notes/plan.md', '--- a/notes/plan.md', '+++ b/notes/plan.md', '@@ -0,0 +1,5 @@',
    '+# Plan', '+1. Write the outline.', '+  - Who reviews', '+3 is a number.', '+#hashtag', ''])).toEqual([[['# Plan', 'title']], [['1. ', 'punct'], ['Write the outline.', '']],
    [['- ', 'punct'], ['Who reviews', '']], [['3 is a number.', '']], [['#hashtag', '']]]);
  // Outside Markdown a `#` line is code; inside a fenced block it is the fence language's comment, not a heading.
  expect(rowsOf(['diff --git a/run.sh b/run.sh', '--- a/run.sh', '+++ b/run.sh', '@@ -0,0 +1,1 @@', '+# comment', ''])).toEqual([[['# comment', 'com']]]);
  expect(rowsOf(['diff --git a/README.md b/README.md', '--- a/README.md', '+++ b/README.md', '@@ -0,0 +1,3 @@', '+```sh', '+# install the tools', '+```', ''])[1])
    .toEqual([['# install the tools', 'com']]);
});
