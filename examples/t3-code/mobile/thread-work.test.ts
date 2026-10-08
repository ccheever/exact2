// @ref llp/1109.005-composer-and-transcript.decision.md#work-log-detail-rows
import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { arr, obj, type Activity, type Obj } from './shared/domain';
import type { Files, Native } from './shared/protocol';
import { noteNow } from './shared/composer-controls';
import { chatCommand } from './shared/chat-commands';
import { setTurnItemOpen, refreshNextOpenTurnItemDetail } from './shared/timeline-item-fetch';
import { mobileThreadActivity } from './thread-work';
import { mobileThreadRows } from './thread';
import { chatLocal } from './shared/timeline-presentation';
import { mobileThreadColors } from './design';
const now = Date.parse('2026-10-07T12:00:00Z');
const time = '2026-10-07T12:00:00Z';
const activity: Activity = { id: '["source","item"]', label: 'Tool', body: '', output: '', result: '', icon: 'terminal', timestamp: time, failed: false, expandable: true, detailOpen: true };
const projected = (item: Obj, source = 'source') => ({ sourceThreadId: source, sourceItemId: 'item', visibility: 'inherited', item: { id: 'item', type: 'command_execution', threadId: source, runId: 'run', startedAt: time, updatedAt: time, status: 'completed', ...item } });
function clientFor(rows: Obj[]) {
  const client = new T3Client(); Object.assign(client, { origin: 'https://tools.test', environmentId: 'env', threadId: 'thread', projectId: 'project',
    connection: 'connected', generation: 1, configLive: true, shellLive: true, threadLive: true, scopes: ['orchestration:operate'] });
  client.config = { environment: { capabilities: { serverResolvedCommandContext: true } } };
  noteNow(client, now);
  client.thread = { sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: 1,
    projection: { thread: { id: 'thread', projectId: 'project' }, runs: [], attempts: [], nodes: [], checkpoints: [], subagents: [], runtimeRequests: [],
      visibleTurnItems: rows, turnItems: rows.map(row => obj(row.item)) } };
  return client;
}
const native: Native = { available: true, watch() {}, async later() { return { ok: true, value: {}, generation: 1 }; } };
const files: Files = { fs: { async mkdir() {}, async readFile() { return new ArrayBuffer(0); }, async atomicWriteFile() {} } };

test('failed provider errors show their actual message despite being non-expandable', () => {
  const row = projected({ type: 'error', status: 'failed', failure: { class: 'provider', message: 'Provider stopped unexpectedly' } });
  const client = clientFor([row]);
  const result = mobileThreadRows(client, now).flatMap(row => row.activities)[0]!;
  expect(result).toMatchObject({ prominentError: true, expandable: false, expanded: false, warning: false, detail: 'Provider stopped unexpectedly' });
  expect(result.timestamp).not.toBe('');
});
test('usage failures show the source reset-time label without a destructive message', () => {
  const row = projected({ type: 'error', status: 'failed', failure: { class: 'usage_limit', message: 'Internal provider text', resetAt: '2026-10-07T15:00:00Z' } });
  const result = mobileThreadActivity(activity, row, clientFor([row]), now, false, '');
  expect(result.warning).toBe(true); expect(result.label.startsWith('Usage limit reached. Retry after ')).toBe(true);
  expect(result.detail).toBe('');
  expect(mobileThreadColors('light').warningForeground).toBe('#bb4d00');
});
test('Retry exists only while the real run retains failed workspace preparation', () => {
  const row = projected({ type: 'error', status: 'failed', failure: { code: 'workspace_preparation_failed', message: 'Setup failed' } });
  const client = clientFor([row]); client.projection.runs = [{ id: 'run', status: 'failed', workspacePreparation: {} }];
  expect(mobileThreadActivity(activity, row, client, now, false, '')).toMatchObject({ retryRunId: 'run', retryDisabled: false });
  client.busy = true; expect(mobileThreadActivity(activity, row, client, now, false, '').retryDisabled).toBe(true);
  client.projection.runs = [{ id: 'run', status: 'running', workspacePreparation: {} }];
  expect(mobileThreadActivity(activity, row, client, now, false, '').retryRunId).toBe('');
});
test('existing retry command dispatches the actual captured run/thread payload', async () => {
  const client = clientFor([]), calls: Obj[] = [];
  const source: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    return { ok: true, generation: 1, value: request.op === 'ids' ? ['command-id'] : {} };
  } };
  await chatCommand(client, source, files, 'retry-preparation', '', 'run');
  expect(calls.find(call => call.method === 'orchestration.dispatchCommand')?.payload).toEqual({ type: 'prepared-run.retry', commandId: 'command-id', threadId: 'thread', runId: 'run' });
});
test('search calls preserve named arguments and separate real search results', () => {
  const file = projected({ type: 'file_search', pattern: 'needle', results: [{ fileName: 'main.ts', line: 12, preview: 'needle()' }] });
  expect(mobileThreadActivity(activity, file, clientFor([file]), now, false, '')).toMatchObject({ call: true, body: 'pattern needle', output: 'main.ts:12\nneedle()' });
  const web = projected({ type: 'web_search', patterns: ['one', 'two'], results: [{ title: 'Source', url: 'https://example.com', snippet: 'Result' }] });
  expect(mobileThreadActivity(activity, web, clientFor([web]), now, false, '')).toMatchObject({ call: true, body: 'query one, two', output: 'Source\nhttps://example.com\nResult' });
});
test('tool input retains empty and null arguments and nonzero command exit is separate', () => {
  const row = projected({ type: 'dynamic_tool', toolName: 'unknown', input: { clear: '', reset: null, count: 2 } });
  expect(mobileThreadActivity(activity, row, clientFor([row]), now, false, '')).toMatchObject({ body: 'clear ""\nreset null\ncount 2', call: true });
  const command = projected({ input: ' echo failed ', exitCode: 3 });
  expect(mobileThreadActivity(activity, command, clientFor([command]), now, false, '')).toMatchObject({ body: 'echo failed', result: 'exit 3' });
});
test('expanded inline command output survives without a detail fetch and retains shared legacy formatting', () => {
  for (const output of ['hello from command', JSON.stringify({ stdout: 'hello from command', stderr: '', interrupted: false })]) {
    const row = projected({ input: 'echo hello', output, exitCode: 0 });
    const client = clientFor([row]);
    expect(mobileThreadActivity(activity, row, client, now, false, '').output).toBe('hello from command');
    expect(mobileThreadActivity({ ...activity, detailOpen: false }, row, client, now, false, '').output).toBe('');
  }
});
test('inline dynamic-tool and read output use the actual shared result rather than a fetched-item-only branch', () => {
  for (const toolName of ['unknown', 'Read']) {
    const row = projected({ type: 'dynamic_tool', toolName, input: { file_path: '/repo/a.ts' }, output: 'actual file contents' });
    expect(mobileThreadActivity(activity, row, clientFor([row]), now, false, '').output).toBe('actual file contents');
  }
  const row = projected({ type: 'notification' });
  expect(mobileThreadActivity({ ...activity, output: 'shared activity output' }, row, clientFor([row]), now, false, '').output).toBe('shared activity output');
});
test('source-thread detail cache owns output, including duplicate item IDs', async () => {
  const rows = ['first', 'second'].map(source => projected({ input: 'cat file', outputOmitted: true }, source));
  const client = clientFor(rows), requests: Obj[] = [];
  client.rpc = async (_native, method, payload) => {
    requests.push({ method, ...payload });
    return { item: { ...obj(rows.find(row => row.sourceThreadId === payload.threadId)?.item), outputOmitted: false, output: String(payload.threadId) } };
  };
  for (const row of rows) {
    const id = JSON.stringify([row.sourceThreadId, row.sourceItemId]);
    const props = { ...activity, id };
    expect(mobileThreadActivity(props, row, client, now, false, '').output).toBe('Loading output…');
    setTurnItemOpen(client, id, true); await refreshNextOpenTurnItemDetail(client, native, now);
    expect(mobileThreadActivity(props, row, client, now, false, '').output).toBe(row.sourceThreadId);
  }
  expect(requests.map(request => request.threadId)).toEqual(['first', 'second']);
});
test('read rows retain paths and missing detail has the exact source unavailable text', async () => {
  const row = projected({ type: 'dynamic_tool', toolName: 'Read', input: { file_path: '/repo/a.ts' }, outputOmitted: true });
  const client = clientFor([row]); client.rpc = async () => ({ item: null });
  setTurnItemOpen(client, activity.id, true); await refreshNextOpenTurnItemDetail(client, native, now);
  expect(mobileThreadActivity(activity, row, client, now, false, '')).toMatchObject({ call: false, body: '/repo/a.ts', output: 'Output is no longer available.' });
});

test('answered question preview and history preserve shared parsing and source union order', () => {
  const row = projected({ type: 'user_input_request', questionAnswer: { requestId: 'request',
    questionTextById: { second: 'Second question?', first: 'First question?' },
    answers: { first: ['One', { answers: ['Two', ''] }], answerOnly: 'Standalone answer' },
    attachmentsByQuestionId: { fileOnly: [{ id: 'image', name: 'actual.png', type: 'image' }] } } });
  const result = mobileThreadActivity(activity, row, clientFor([row]), now, false, '');
  expect(result.answerPreview).toBe('One, Two · Standalone answer'); expect(result.hasAnswer).toBe(true);
  expect(result.answerHistory).toMatchObject([
    { id: 'second', question: 'Second question?', answer: '', files: [] },
    { id: 'first', question: 'First question?', answer: 'One, Two', files: [] },
    { id: 'answerOnly', question: '', answer: 'Standalone answer', files: [] },
    { id: 'fileOnly', question: '', answer: '', files: [{ name: 'actual.png', image: true, url: '' }] },
  ]);
  const collapsed = mobileThreadActivity({ ...activity, detailOpen: false }, row, clientFor([row]), now, false, '');
  expect(collapsed.answerPreview).toBe(result.answerPreview); expect(collapsed.answerHistory).toEqual([]);
});
test('answer previews retain file-only and unanswered source fallbacks without inventing file actions', () => {
  const answer = { questionTextById: { q: '  Which\nfile? ' }, answers: {}, attachmentsByQuestionId: { q: [{ id: 'a', name: 'source.txt' }] } };
  const row = projected({ type: 'user_input_request', questionAnswer: answer });
  expect(mobileThreadActivity(activity, row, clientFor([row]), now, false, '')).toMatchObject({ answerPreview: 'source.txt', hasAnswer: true,
    answerHistory: [{ files: [{ name: 'source.txt', image: false, url: '' }] }] });
  answer.attachmentsByQuestionId.q = [];
  expect(mobileThreadActivity(activity, row, clientFor([row]), now, false, '')).toMatchObject({ answerPreview: 'Which file?', hasAnswer: false });
  const ordinary = projected({ type: 'command_execution', output: 'result' });
  expect(mobileThreadActivity(activity, ordinary, clientFor([ordinary]), now, false, '')).toMatchObject({ answerPreview: '', hasAnswer: false, answerHistory: [], output: 'result' });
});
test('actual transcript keeps answered history scoped to inherited source item identities', async () => {
  const rows = ['first', 'second'].map(source => projected({ type: 'user_input_request', questionAnswer: {
    requestId: source, questionTextById: { q: 'Choice?' }, answers: { q: source }, attachmentsByQuestionId: {} } }, source));
  const client = clientFor(rows);
  // The real feed first discloses its run fold, then the multi-item work group.
  for (let depth = 0; depth < 2; depth++) for (const shown of mobileThreadRows(client, now)) if (shown.toggleOp && !shown.expanded) await chatLocal(client, native, shown.toggleOp.replace('chatlocal:', ''), shown.toggleId, '');
  for (const row of rows) setTurnItemOpen(client, JSON.stringify([row.sourceThreadId, row.sourceItemId]), true);
  const shown = mobileThreadRows(client, now).flatMap(row => row.activities);
  expect(shown.map(row => [row.id, row.answerPreview, row.answerHistory[0]?.answer])).toEqual([
    ['["first","item"]', 'first', 'first'], ['["second","item"]', 'second', 'second'],
  ]);
});
test('expanded reasoning uses existing Markdown/code blocks while commands remain literal', async () => {
  const markdown = '**Important**\n\n- first\n\n```typescript\nconst n = 1;\n```';
  const row = projected({ type: 'reasoning', text: markdown }); const client = clientFor([row]);
  for (const shown of mobileThreadRows(client, now)) if (shown.toggleOp && !shown.expanded) await chatLocal(client, native, shown.toggleOp.replace('chatlocal:', ''), shown.toggleId, '');
  setTurnItemOpen(client, activity.id, true);
  const shown = mobileThreadRows(client, now).flatMap(row => row.activities)[0]!;
  expect(shown.reasoningBlocks.map(block => block.kind)).toEqual(['markdown', 'code']);
  expect(shown.reasoningBlocks[0]!.text).toContain('**Important**');
  expect(shown.reasoningBlocks[1]!.tokens.map(token => token.text).join('')).toBe('const n = 1;');
  setTurnItemOpen(client, activity.id, false);
  expect(mobileThreadRows(client, now).flatMap(row => row.activities)[0]!.reasoningBlocks).toEqual([]);
  const command = projected({ input: 'echo markdown', output: markdown }); const commands = clientFor([command]);
  for (const shown of mobileThreadRows(commands, now)) if (shown.toggleOp && !shown.expanded) await chatLocal(commands, native, shown.toggleOp.replace('chatlocal:', ''), shown.toggleId, '');
  setTurnItemOpen(commands, activity.id, true);
  expect(mobileThreadRows(commands, now).flatMap(row => row.activities)[0]).toMatchObject({ reasoningBlocks: [], output: markdown });
});
