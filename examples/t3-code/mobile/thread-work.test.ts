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

test('command Copy uses original wire data and schema order before and after fetching output', async () => {
  const row = projected({ input: '/bin/zsh -lc pwd', output: 'withheld inline output', outputOmitted: true, exitCode: 0,
    nativeItemRef: { strength: 'strong', nativeId: 'exec-id', driver: 'codex' } });
  const client = clientFor([row]), collapsed = { ...activity, detailOpen: false, label: 'pwd' };
  const before = mobileThreadActivity(collapsed, row, client, now, false, '', 'visit');
  const configuration = JSON.parse(before.nativeWorkRow);
  expect(configuration).toMatchObject({ label: 'pwd', symbol: 'terminal', expanded: false, expandable: true });
  expect(before.nativeWorkDetail).toBe(false);
  expect(configuration.copyText.startsWith('Command\n/bin/zsh -lc pwd\n{')).toBe(true);
  const copiedItem = JSON.parse(configuration.copyText.slice(configuration.copyText.indexOf('{'))).item;
  expect(copiedItem.output).toBeUndefined(); expect(copiedItem.outputOmitted).toBe(true);
  expect(Object.keys(copiedItem.nativeItemRef)).toEqual(['driver', 'nativeId', 'strength']);
  expect(Object.keys(copiedItem).slice(-4)).toEqual(['type', 'input', 'outputOmitted', 'exitCode']);
  client.rpc = async () => ({ item: { ...row.item, output: 'actual fetched output', outputOmitted: false } });
  setTurnItemOpen(client, activity.id, true); await refreshNextOpenTurnItemDetail(client, native, now);
  const after = mobileThreadActivity({ ...activity, label: 'pwd' }, row, client, now, false, '', 'visit');
  expect(after).toMatchObject({ call: true, body: '/bin/zsh -lc pwd', output: 'actual fetched output', nativeWorkDetail: false });
  expect(JSON.parse(after.nativeWorkRow).copyText).toBe(configuration.copyText);
});

test('command Copy capitalizes its original summary and removes duplicate copy parts', () => {
  const row = projected({ title: '  pwd  ', input: 'Pwd', output: 'not copied' });
  const client = clientFor([row]);
  const configuration = JSON.parse(mobileThreadActivity({ ...activity, label: 'pwd' }, row, client, now, true, '', 'visit').nativeWorkRow);
  expect(configuration.copyText.startsWith('Pwd\n{')).toBe(true);
  expect(configuration.copiedColor).toBe('#00d492');
  expect(JSON.parse(configuration.copyText.slice(configuration.copyText.indexOf('{'))).item.title).toBe('  pwd  ');
});

test('non-expandable commands can copy while presentation stays scoped to the root route visit', () => {
  const row = projected({ input: 'pwd' }), client = clientFor([row]);
  const props = { ...activity, detailOpen: false, expandable: false, label: 'pwd' };
  const first = JSON.parse(mobileThreadActivity(props, row, client, now, false, '', 'first-visit').nativeWorkRow);
  const second = JSON.parse(mobileThreadActivity(props, row, client, now, false, '', 'second-visit').nativeWorkRow);
  expect(first.expandable).toBe(false); expect(first.copyText).toBe(second.copyText);
  expect(first.owner).not.toBe(second.owner); expect(first.routeKey).toBe('first-visit');
  expect(client.draft).toBe(''); expect(client.threadId).toBe('thread');
});

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

function forkRow(source = 'source', target = 'thread'): Obj {
  return { sourceThreadId: source, sourceItemId: 'fork-item', visibility: 'synthetic', item: { id: 'fork-item',
    threadId: target, runId: null, status: 'completed', title: 'Forked from conversation', updatedAt: time, type: 'fork',
    source: { type: 'run', threadId: source, runId: 'source-run' }, targetThreadId: target } };
}
test('fork lifecycle discloses the actual projection locally and leaves selection and unsent drafts intact', async () => {
  const row = forkRow(), client = clientFor([row]), key = '["source","fork-item"]';
  client.local.drafts[client.draftKey] = '  '; const calls: unknown[] = [];
  const bridge: Native = { available: true, watch() {}, async later(input) { calls.push(input); return { ok: true, value: {}, generation: 1 }; } };
  const collapsed = mobileThreadRows(client, now);
  expect(collapsed).toHaveLength(1);
  expect(collapsed[0]).toMatchObject({ kind: 'fork', title: '', body: '', blocks: [], showMeta: false, toggleOp: '' });
  expect(collapsed[0]!.activities).toHaveLength(1);
  expect(collapsed[0]!.activities[0]).toMatchObject({ id: key, label: 'thread', symbol: 'bolt', expanded: false, expandable: true, body: '', output: '', loading: false });
  await chatLocal(client, bridge, 'item-detail', key, 'open');
  const expanded = mobileThreadRows(client, now)[0]!.activities[0]!;
  expect(expanded.expanded).toBe(true);
  expect(JSON.parse(expanded.body)).toEqual(row);
  expect(expanded.body).toContain('\n  "visibility": "synthetic",');
  expect(mobileThreadRows(client, now)[0]!.blocks).toEqual([]);
  expect(client.threadId).toBe('thread'); expect(client.draft).toBe('  ');
  expect(calls).toEqual([]);
  expect(await refreshNextOpenTurnItemDetail(client, bridge, now)).toBe(false);
  await chatLocal(client, bridge, 'item-detail', key, 'closed');
  expect(mobileThreadRows(client, now)[0]!.activities[0]).toMatchObject({ expanded: false, body: '' });
  expect(calls).toEqual([]);
});
test('fork disclosure distinguishes inherited source identities and selected environments and threads', async () => {
  const client = clientFor([forkRow('first'), forkRow('second')]);
  await chatLocal(client, native, 'item-detail', '["first","fork-item"]', 'open');
  const shown = () => mobileThreadRows(client, now).flatMap(row => row.activities);
  expect(shown().map(row => [row.id, row.expanded])).toEqual([['["first","fork-item"]', true], ['["second","fork-item"]', false]]);
  client.threadId = 'another-thread'; expect(shown().every(row => !row.expanded)).toBe(true);
  client.threadId = 'thread'; client.environmentId = 'another-env'; expect(shown().every(row => !row.expanded)).toBe(true);
});
test('fork detail retains actual values with source schema field order after native dictionary transport', () => {
  const row = forkRow(), original = obj(row.item);
  row.item = { targetThreadId: original.targetThreadId, source: { runId: 'source-run', threadId: 'source', type: 'run' },
    type: 'fork', updatedAt: time, title: 'Forked from conversation', status: 'completed', runId: null, threadId: 'thread', id: 'fork-item' };
  const client = clientFor([row]); setTurnItemOpen(client, '["source","fork-item"]', true);
  const text = mobileThreadRows(client, now)[0]!.activities[0]!.body;
  expect(JSON.parse(text)).toEqual(row);
  expect(text.indexOf('"id":')).toBeLessThan(text.indexOf('"threadId":'));
  expect(text.indexOf('"updatedAt":')).toBeLessThan(text.indexOf('"type": "fork"'));
  expect(text).toContain('"source": {\n      "type": "run",\n      "threadId": "source",\n      "runId": "source-run"\n    }');
  expect(text).not.toContain('"nodeId"'); expect(text).not.toContain('"providerThreadId"');
});
test('fork long-press copies the source summary, target and exact full detail while disclosure stays local', async () => {
  const row = forkRow(), client = clientFor([row]), key = '["source","fork-item"]';
  client.local.drafts[client.draftKey] = '  ';
  const shown = () => mobileThreadRows(client, now, false, 'visit-one')[0]!.activities[0]!;
  const collapsed = shown(), configuration = JSON.parse(collapsed.nativeWorkRow);
  expect(collapsed.body).toBe(''); expect(configuration).toMatchObject({ id: key, routeKey: 'visit-one', label: 'thread', expanded: false, copiedColor: '#009966' });
  expect(configuration.copyText.split('\n').slice(0, 2)).toEqual(['Forked from conversation', 'thread']);
  expect(JSON.parse(configuration.copyText.slice('Forked from conversation\nthread\n'.length))).toEqual(row);
  await chatLocal(client, native, 'item-detail', key, 'open');
  const expanded = shown(), openedConfiguration = JSON.parse(expanded.nativeWorkRow);
  expect(openedConfiguration.expanded).toBe(true); expect(openedConfiguration.owner).toBe(configuration.owner);
  expect(openedConfiguration.copyText).toBe(`Forked from conversation\nthread\n${expanded.body}`);
  expect(client.threadId).toBe('thread'); expect(client.draft).toBe('  '); expect(client.pending).toBeUndefined();
});
test('fork copy uses the pinned title capitalization and untitled fallback', () => {
  const row = forkRow(), client = clientFor([row]);
  const copied = () => JSON.parse(mobileThreadRows(client, now, false, 'visit')[0]!.activities[0]!.nativeWorkRow).copyText as string;
  obj(row.item).title = '  forked from conversation  '; expect(copied().startsWith('Forked from conversation\nthread\n')).toBe(true);
  obj(row.item).title = ' '; expect(copied().startsWith('Thread forked\nthread\n')).toBe(true);
});
test('native copy ownership changes with a route visit or live server scope, while disclosure and theme preserve it', () => {
  const client = clientFor([forkRow()]);
  const configuration = (route = 'visit-one', dark = false) => JSON.parse(mobileThreadRows(client, now, dark, route)[0]!.activities[0]!.nativeWorkRow);
  const initial = configuration(); expect(configuration('visit-two').owner).not.toBe(initial.owner);
  const dark = configuration('visit-one', true); expect(dark.owner).toBe(initial.owner); expect(dark.copiedColor).toBe('#00d492');
  client.generation++; expect(configuration().owner).not.toBe(initial.owner);
  const reconnected = configuration(); client.threadEpoch++; expect(configuration().owner).not.toBe(reconnected.owner);
  const current = configuration(); client.environmentId = 'other-env'; expect(configuration().owner).not.toBe(current.owner);
  expect(mobileThreadActivity(activity, projected({ type: 'reasoning' }), client, now, false, '').nativeWorkRow).toBe('');
});


test('failure row copy uses the source title, actual message and projected wire JSON', () => {
  const row = projected({ type: 'error', status: 'failed', title: '  provider error  ',
    failure: { retryable: null, code: 'other', message: 'Provider stopped unexpectedly', class: 'provider_error' } });
  const client = clientFor([row]), result = mobileThreadRows(client, now, false, 'failure-visit').flatMap(row => row.activities)[0]!;
  const config = JSON.parse(result.nativeWorkRow);
  expect(result).toMatchObject({ label: 'Provider error', expanded: false, expandable: false, detail: 'Provider stopped unexpectedly' });
  expect(config).toMatchObject({ id: '["source","item"]', routeKey: 'failure-visit', label: 'Provider error', expanded: false, copiedColor: '#009966' });
  expect(config.copyText).toStartWith('Provider error\nProvider stopped unexpectedly\n{\n');
  const json = config.copyText.slice(config.copyText.indexOf('{'));
  expect(JSON.parse(json)).toEqual(row);
  expect(json).toContain('"failure": {\n      "class": "provider_error",\n      "message": "Provider stopped unexpectedly",\n      "code": "other",\n      "retryable": null\n    }');
  expect(json.indexOf('"id":')).toBeLessThan(json.indexOf('"threadId":'));
  expect(json.indexOf('"updatedAt":')).toBeLessThan(json.indexOf('"type": "error"'));
  expect(json).not.toContain('"resetAt"'); expect(json).not.toContain('"retry":');
});

test('usage row copies the original failure while its visible warning keeps the reset-time label', () => {
  const row = projected({ type: 'error', status: 'failed', title: '', failure: {
    class: 'usage_limit', message: 'Quota exhausted', code: null, retryable: true, resetAt: '2026-10-07T15:00:00Z' } });
  const client = clientFor([row]);
  const result = mobileThreadActivity(activity, row, client, now, true, '', 'usage-visit'), config = JSON.parse(result.nativeWorkRow);
  expect(result.warning).toBe(true); expect(result.label).toStartWith('Usage limit reached. Retry after ');
  expect(config.label).toBe(result.label); expect(config.copiedColor).toBe('#00d492');
  expect(config.copyText).toStartWith('Usage limit reached\nQuota exhausted\n{');
  expect(config.copyText).not.toContain('Retry after');
  expect(JSON.parse(config.copyText.slice(config.copyText.indexOf('{')))).toEqual(row);
});

test('failure copy stays on the captured source item and route without detail or retry dispatch', () => {
  const rows = ['first', 'second'].map(source => projected({ type: 'error', status: 'failed', title: 'transport error',
    failure: { class: 'transport_error', message: source, code: null, retryable: null },
    retry: { retryDelayMs: 500, maxAttempts: 3, attempt: 1 } }, source));
  const client = clientFor(rows);
  const config = (row: Obj, route = 'one') => JSON.parse(mobileThreadActivity({ ...activity, id: JSON.stringify([row.sourceThreadId, row.sourceItemId]) },
    row, client, now, false, '', route).nativeWorkRow);
  const first = config(rows[0]!), second = config(rows[1]!);
  expect(first.owner).not.toBe(second.owner); expect(first.copyText).toContain('Transport error\nfirst\n');
  expect(second.copyText).toContain('Transport error\nsecond\n'); expect(config(rows[0]!, 'two').owner).not.toBe(first.owner);
  client.generation++; expect(config(rows[0]!).owner).not.toBe(first.owner);
  const before = config(rows[0]!); client.threadEpoch++; expect(config(rows[0]!).owner).not.toBe(before.owner);
  expect(config(rows[0]!).copyText).toContain('"retry": {\n      "attempt": 1,\n      "maxAttempts": 3,\n      "retryDelayMs": 500\n    }');
  expect(client.pending).toBeUndefined();
  expect(mobileThreadActivity(activity, projected({ type: 'error', status: 'completed' }), client, now, false, '', 'one').nativeWorkRow).toBe('');
});
