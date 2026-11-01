// @ref examples/t3-code/mobile/docs/1130.005-composer-and-transcript.decision.md#work-log-detail-rows
import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { arr, obj, type Activity, type Obj } from './shared/domain';
import type { Files, Native } from './shared/protocol';
import { noteNow } from './shared/composer-controls';
import { chatCommand } from './shared/chat-commands';
import { setTurnItemOpen, refreshNextOpenTurnItemDetail } from './shared/timeline-item-fetch';
import { mobileThreadActivity } from './thread-work';
import { mobileThreadRows } from './thread';
import { chatLocal, transcriptRows } from './shared/timeline-presentation';
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

test('recognized browser tools keep their globe through shared and mobile row projection', async () => {
  for (const toolName of ['preview_status', 'preview_open', 'unknown']) {
    const row = projected({ type: 'dynamic_tool', toolName: `mcp__t3-code__${toolName}`, input: { url: 'https://example.com' } });
    const client = clientFor([row]);
    for (const shown of mobileThreadRows(client, now)) if (shown.toggleOp && !shown.expanded) await chatLocal(client, native, shown.toggleOp.replace('chatlocal:', ''), shown.toggleId, '');
    const shared = transcriptRows(client).flatMap(message => message.activities ?? []).find(value => value.id === activity.id)!;
    expect(shared.icon).toBe(toolName === 'unknown' ? 'wrench' : 'browser');
    for (const dark of [false, true]) {
      const shown = mobileThreadRows(client, now, dark).flatMap(message => message.activities).find(value => value.id === activity.id)!;
      expect(shown).toMatchObject({ symbol: toolName === 'unknown' ? 'wrench' : 'globe', iconURL: '', nativeWorkRow: '', nativeWorkDetail: false, nativeCommandDetail: false });
      const image = mobileThreadActivity({ ...shared, iconLight: 'https://example.com/light.png', iconDark: 'https://example.com/dark.png' }, row, client, now, dark, '', 'visit');
      expect(image.iconURL).toBe(`https://example.com/${dark ? 'dark' : 'light'}.png`);
      expect(image.nativeWorkRow).toBe('');
    }
  }
});

test('native command paragraphs appear only for an expanded command call', () => {
  const row = projected({ input: '/bin/zsh -lc pwd', output: 'actual directory' }), client = clientFor([row]);
  expect(mobileThreadActivity(activity, row, client, now, false, '', 'visit')).toMatchObject({
    nativeCommandDetail: true, nativeWorkDetail: false, body: '/bin/zsh -lc pwd', output: 'actual directory', result: '',
  });
  expect(mobileThreadActivity({ ...activity, detailOpen: false }, row, client, now, false, '', 'visit')).toMatchObject({
    nativeCommandDetail: false, body: '', output: '',
  });
  for (const item of [{ type: 'dynamic_tool', toolName: 'Read', input: { file_path: '/repo/a.ts' } }, { type: 'dynamic_tool', toolName: 'unknown', input: { value: 1 } },
    { type: 'reasoning', text: 'Actual reasoning' }, { type: 'error', status: 'failed' }]) {
    const other = projected(item);
    expect(mobileThreadActivity(activity, other, clientFor([other]), now, false, '', 'visit').nativeCommandDetail).toBe(false);
  }
});

test('native command paragraphs preserve actual loading, missing, empty and error output states', async () => {
  for (const state of ['missing', 'empty', 'error']) {
    const row = projected({ input: 'pwd', outputOmitted: true }), client = clientFor([row]);
    const shown = () => mobileThreadActivity(activity, row, client, now, false, '', 'visit');
    expect(shown()).toMatchObject({ nativeCommandDetail: true, body: 'pwd', output: 'Loading output…', loading: true });
    client.rpc = async () => {
      if (state === 'error') throw new Error('Actual output read failed');
      return { item: state === 'missing' ? null : { ...obj(row.item), outputOmitted: false, output: '' } };
    };
    setTurnItemOpen(client, activity.id, true); await refreshNextOpenTurnItemDetail(client, native, now);
    expect(shown()).toMatchObject({ nativeCommandDetail: true, body: 'pwd', loading: false,
      output: state === 'missing' ? 'Output is no longer available.' : state === 'empty' ? 'No output.' : "Couldn't load output: Actual output read failed" });
  }
});

test('fetched command paragraphs stay separate from original header copy, exit annotation and draft ownership', async () => {
  const row = projected({ input: 'original command', outputOmitted: true }), client = clientFor([row]);
  client.local.drafts[client.draftKey] = '  '; const before = JSON.stringify(client.projection);
  const shown = () => mobileThreadActivity(activity, row, client, now, false, '', 'visit');
  const copy = JSON.parse(shown().nativeWorkRow).copyText;
  const output = Array.from({ length: 30 }, (_, index) => `line ${index + 1}`).join('\n') + '\n';
  client.rpc = async () => ({ item: { ...obj(row.item), input: 'actual fetched command', outputOmitted: false, output, exitCode: 7 } });
  setTurnItemOpen(client, activity.id, true); await refreshNextOpenTurnItemDetail(client, native, now);
  expect(shown()).toMatchObject({ nativeCommandDetail: true, body: 'actual fetched command', output, result: 'exit 7' });
  expect(JSON.parse(shown().nativeWorkRow).copyText).toBe(copy);
  expect(copy).not.toContain('actual fetched command'); expect(copy).not.toContain('line 30');
  expect(JSON.stringify(client.projection)).toBe(before); expect(client.threadId).toBe('thread'); expect(client.draft).toBe('  ');
});

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
test('web search headers copy original patterns and results without changing detail ownership', async () => {
  const web = projected({ type: 'web_search', results: [{ snippet: 'Result', url: 'https://example.com', title: 'Source' },
    { url: 'https://other.test' }], patterns: ['  one  ', 'two'] });
  const client = clientFor([web]), requests: string[] = [];
  client.scopes = []; client.local.drafts[client.draftKey] = '  ';
  client.rpc = async (_native, method) => { requests.push(method); return {}; };
  const before = JSON.stringify(client.projection), props = { ...activity, icon: 'globe', label: 'Visible web row' };
  const collapsed = mobileThreadActivity({ ...props, detailOpen: false }, web, client, now, false, '', 'visit');
  expect(collapsed.nativeWorkRow).not.toBe('');
  const configuration = JSON.parse(collapsed.nativeWorkRow), expected = `Searched the web\n  one  , two\n${JSON.stringify({
    visibility: 'inherited', sourceThreadId: 'source', sourceItemId: 'item', item: { id: 'item', threadId: 'source', runId: 'run',
      status: 'completed', startedAt: time, updatedAt: time, type: 'web_search', patterns: ['  one  ', 'two'],
      results: [{ title: 'Source', url: 'https://example.com', snippet: 'Result' }, { url: 'https://other.test' }] },
  }, null, 2)}`;
  expect(configuration).toMatchObject({ copyText: expected, label: 'Visible web row', symbol: 'globe', expanded: false, expandable: true });
  expect(collapsed).toMatchObject({ nativeCommandDetail: false, nativeWorkDetail: false, body: '', output: '' });
  setTurnItemOpen(client, activity.id, true);
  expect(await refreshNextOpenTurnItemDetail(client, native, now)).toBe(false);
  const expanded = mobileThreadActivity(props, web, client, now, false, '', 'visit');
  expect(JSON.parse(expanded.nativeWorkRow)).toMatchObject({ copyText: expected, owner: configuration.owner, expanded: true });
  expect(expanded).toMatchObject({ nativeCommandDetail: false, nativeWorkDetail: false, body: 'query   one  , two',
    output: 'Source\nhttps://example.com\nResult\n\nhttps://other.test' });
  const otherVisit = JSON.parse(mobileThreadActivity(props, web, client, now, false, '', 'other-visit').nativeWorkRow);
  expect(otherVisit.owner).not.toBe(configuration.owner); expect(otherVisit.copyText).toBe(expected);
  expect(requests).toEqual([]); expect(client.draft).toBe('  '); expect(client.threadId).toBe('thread');
  expect(JSON.stringify(client.projection)).toBe(before);
  for (const dark of [false, true]) {
    const image = mobileThreadActivity({ ...props, iconLight: 'https://example.com/light.png', iconDark: 'https://example.com/dark.png' }, web, client, now, dark, '', 'visit');
    expect(image.nativeWorkImage).toBe(true); expect(image.iconURL).toBe(`https://example.com/${dark ? 'dark' : 'light'}.png`);
    expect(JSON.parse(image.nativeWorkRow).copyText).toBe(expected);
  }
  const titled = projected({ type: 'web_search', title: '  searched docs  ', patterns: ['Searched docs'] });
  const titledCopy = JSON.parse(mobileThreadActivity(props, titled, clientFor([titled]), now, false, '', 'visit').nativeWorkRow).copyText;
  expect(titledCopy.startsWith('Searched docs\n{')).toBe(true);
  expect(JSON.parse(titledCopy.slice(titledCopy.indexOf('{'))).item.title).toBe('  searched docs  ');
  const absent = projected({ type: 'web_search' });
  const absentCopy = JSON.parse(mobileThreadActivity({ ...props, expandable: false }, absent, clientFor([absent]), now, false, '', 'visit').nativeWorkRow);
  expect(absentCopy.expandable).toBe(false); expect(absentCopy.copyText.startsWith('Searched the web\n{')).toBe(true);
  const absentItem = JSON.parse(absentCopy.copyText.slice(absentCopy.copyText.indexOf('{'))).item;
  expect(Object.hasOwn(absentItem, 'patterns')).toBe(false); expect(Object.hasOwn(absentItem, 'results')).toBe(false);
  const empty = projected({ type: 'web_search', patterns: [], results: [] });
  const emptyCopy = JSON.parse(mobileThreadActivity(props, empty, clientFor([empty]), now, false, '', 'visit').nativeWorkRow).copyText;
  expect(emptyCopy.startsWith('Searched the web\n{')).toBe(true);
  expect(JSON.parse(emptyCopy.slice(emptyCopy.indexOf('{'))).item).toMatchObject({ patterns: [], results: [] });
  const separators = projected({ type: 'web_search', patterns: ['', ''] });
  const separatorCopy = JSON.parse(mobileThreadActivity(props, separators, clientFor([separators]), now, false, '', 'visit').nativeWorkRow).copyText;
  expect(separatorCopy.startsWith('Searched the web\n, \n{')).toBe(true);
});
test('file search headers copy original results without taking command detail ownership', async () => {
  const file = projected({ type: 'file_search', results: [{ preview: 'needle()', column: 3, line: 12, fileName: 'main.ts' },
    { preview: 'needleAgain()', fileName: 'other.ts' }], pattern: 'needle' });
  const client = clientFor([file]), requests: string[] = [];
  client.scopes = []; client.local.drafts[client.draftKey] = '  ';
  client.rpc = async (_native, method) => { requests.push(method); return {}; };
  const before = JSON.stringify(client.projection), props = { ...activity, icon: 'search', label: 'Visible search row' };
  const collapsed = mobileThreadActivity({ ...props, detailOpen: false }, file, client, now, false, '', 'visit');
  expect(collapsed.nativeWorkRow).not.toBe('');
  const configuration = JSON.parse(collapsed.nativeWorkRow), expected = `Searched needle\nneedle\n${JSON.stringify({
    visibility: 'inherited', sourceThreadId: 'source', sourceItemId: 'item', item: { id: 'item', threadId: 'source', runId: 'run',
      status: 'completed', startedAt: time, updatedAt: time, type: 'file_search', pattern: 'needle',
      results: [{ fileName: 'main.ts', line: 12, column: 3, preview: 'needle()' }, { fileName: 'other.ts', preview: 'needleAgain()' }] },
  }, null, 2)}`;
  expect(configuration).toMatchObject({ copyText: expected, label: 'Visible search row', symbol: 'magnifyingglass', expanded: false, expandable: true });
  expect(collapsed).toMatchObject({ nativeCommandDetail: false, nativeWorkDetail: false, body: '', output: '' });
  setTurnItemOpen(client, activity.id, true);
  expect(await refreshNextOpenTurnItemDetail(client, native, now)).toBe(false);
  const expanded = mobileThreadActivity(props, file, client, now, false, '', 'visit');
  expect(JSON.parse(expanded.nativeWorkRow)).toMatchObject({ copyText: expected, owner: configuration.owner, expanded: true });
  expect(expanded).toMatchObject({ nativeCommandDetail: false, nativeWorkDetail: false, body: 'pattern needle', output: 'main.ts:12\nneedle()\nother.ts\nneedleAgain()' });
  const otherVisit = JSON.parse(mobileThreadActivity(props, file, client, now, false, '', 'other-visit').nativeWorkRow);
  expect(otherVisit.owner).not.toBe(configuration.owner); expect(otherVisit.copyText).toBe(expected);
  expect(requests).toEqual([]); expect(client.draft).toBe('  '); expect(client.threadId).toBe('thread');
  expect(JSON.stringify(client.projection)).toBe(before);
  for (const dark of [false, true]) {
    const image = mobileThreadActivity({ ...props, iconLight: 'https://example.com/light.png', iconDark: 'https://example.com/dark.png' }, file, client, now, dark, '', 'visit');
    expect(image.nativeWorkImage).toBe(true); expect(image.iconURL).toBe(`https://example.com/${dark ? 'dark' : 'light'}.png`);
    expect(JSON.parse(image.nativeWorkRow).copyText).toBe(expected);
  }
  const titled = projected({ type: 'file_search', title: '  exact search  ', pattern: 'Exact search' });
  const titledCopy = JSON.parse(mobileThreadActivity(props, titled, clientFor([titled]), now, false, '', 'visit').nativeWorkRow).copyText;
  expect(titledCopy.startsWith('Exact search\n{')).toBe(true);
  expect(JSON.parse(titledCopy.slice(titledCopy.indexOf('{'))).item.title).toBe('  exact search  ');
  const empty = projected({ type: 'file_search' });
  const emptyCopy = JSON.parse(mobileThreadActivity({ ...props, expandable: false }, empty, clientFor([empty]), now, false, '', 'visit').nativeWorkRow);
  expect(emptyCopy.expandable).toBe(false); expect(emptyCopy.copyText.startsWith('Searched files\n{')).toBe(true);
  expect(Object.hasOwn(JSON.parse(emptyCopy.copyText.slice(emptyCopy.copyText.indexOf('{'))).item, 'results')).toBe(false);
  const blank = projected({ type: 'file_search', pattern: '', results: [] });
  const blankCopy = JSON.parse(mobileThreadActivity(props, blank, clientFor([blank]), now, false, '', 'visit').nativeWorkRow).copyText;
  expect(blankCopy.startsWith('Searched files\n{')).toBe(true);
  expect(JSON.parse(blankCopy.slice(blankCopy.indexOf('{'))).item).toMatchObject({ pattern: '', results: [] });
  const spaced = projected({ type: 'file_search', pattern: '  needle  ' });
  const spacedCopy = JSON.parse(mobileThreadActivity(props, spaced, clientFor([spaced]), now, false, '', 'visit').nativeWorkRow).copyText;
  expect(spacedCopy.startsWith('Searched needle\n  needle  \n{')).toBe(true);
  expect(JSON.parse(spacedCopy.slice(spacedCopy.indexOf('{'))).item.pattern).toBe('  needle  ');
  const nullable = projected({ type: 'file_search', pattern: null, results: null });
  const nullCopy = JSON.parse(mobileThreadActivity(props, nullable, clientFor([nullable]), now, false, '', 'visit').nativeWorkRow).copyText;
  expect(JSON.parse(nullCopy.slice(nullCopy.indexOf('{'))).item).toMatchObject({ pattern: null, results: null });
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

test('untitled read headers copy original paths without taking detail ownership', async () => {
  const row = projected({ type: 'dynamic_tool', toolName: 'Read', title: null, input: { file_path: '/repo/a.ts' }, outputOmitted: true });
  const client = clientFor([row]), requests: Obj[] = [];
  client.scopes = []; client.local.drafts[client.draftKey] = '  ';
  client.rpc = async (_native, method, payload) => {
    requests.push({ method, ...payload });
    return { item: { ...obj(row.item), input: { file_path: '/fetched/b.ts' }, outputOmitted: false, output: 'Fetched file body' } };
  };
  const before = JSON.stringify(client.projection), props = { ...activity, icon: 'file-text', label: 'Visible read row' };
  const collapsed = mobileThreadActivity({ ...props, detailOpen: false }, row, client, now, false, '', 'visit');
  expect(collapsed.nativeWorkRow).not.toBe('');
  const configuration = JSON.parse(collapsed.nativeWorkRow), expected = 'Read /repo/a.ts\n/repo/a.ts';
  expect(configuration).toMatchObject({ copyText: expected, label: 'Visible read row', symbol: 'doc.text', expanded: false, expandable: true });
  expect(collapsed).toMatchObject({ nativeCommandDetail: false, nativeWorkDetail: false, call: false, output: '' });
  const staticRow = mobileThreadActivity({ ...props, expandable: false }, row, client, now, false, '', 'visit');
  expect(JSON.parse(staticRow.nativeWorkRow)).toMatchObject({ copyText: expected, expandable: false });
  expect(requests).toEqual([]);
  setTurnItemOpen(client, activity.id, true);
  expect(await refreshNextOpenTurnItemDetail(client, native, now)).toBe(true);
  const expanded = mobileThreadActivity(props, row, client, now, false, '', 'visit');
  expect(JSON.parse(expanded.nativeWorkRow)).toMatchObject({ copyText: expected, owner: configuration.owner, expanded: true });
  expect(expanded).toMatchObject({ nativeCommandDetail: false, nativeWorkDetail: false, call: false, body: '/repo/a.ts', output: 'Fetched file body' });
  const otherVisit = JSON.parse(mobileThreadActivity(props, row, client, now, false, '', 'other-visit').nativeWorkRow);
  expect(otherVisit.owner).not.toBe(configuration.owner); expect(otherVisit.copyText).toBe(expected);
  expect(requests).toEqual([{ method: 'orchestration.getTurnItem', threadId: 'source', itemId: 'item', revision: time }]);
  expect(client.draft).toBe('  '); expect(client.threadId).toBe('thread'); expect(JSON.stringify(client.projection)).toBe(before);
});

test('read header Copy preserves relative paths and existing collection bounds', () => {
  const row = projected({ type: 'dynamic_tool', toolName: 'ReadFile', title: ' \n ', input: {
    file_path: ' ./src/a.ts ', locations: [{ path: './src/a.ts' }, { path: 'src/b.ts' }],
  } });
  expect(JSON.parse(mobileThreadActivity(activity, row, clientFor([row]), now, false, '', 'visit').nativeWorkRow).copyText)
    .toBe('Read ./src/a.ts\n./src/a.ts\nsrc/b.ts');
  const array = projected({ type: 'dynamic_tool', toolName: 'read_file', fileName: 'root.ts',
    input: [{ fileName: 'last.ts', file_path: 'middle.ts', path: 'first.ts' }] });
  expect(JSON.parse(mobileThreadActivity(activity, array, clientFor([array]), now, false, '', 'visit').nativeWorkRow).copyText)
    .toBe('Read first.ts\nroot.ts\nfirst.ts\nmiddle.ts\nlast.ts');
  const bounded = projected({ type: 'dynamic_tool', toolName: 'Read', input: { locations: Array.from({ length: 10 }, (_, index) => ({ path: `p${index}.ts` })) } });
  expect(JSON.parse(mobileThreadActivity(activity, bounded, clientFor([bounded]), now, false, '', 'visit').nativeWorkRow).copyText)
    .toBe('Read p0.ts\np0.ts\np1.ts\np2.ts\np3.ts\np4.ts\np5.ts\np6.ts\np7.ts');
  const deep = projected({ type: 'dynamic_tool', toolName: 'Read', input: { data: { data: { data: {
    path: 'at-limit.ts', data: { path: 'too-deep.ts' },
  } } } } });
  expect(JSON.parse(mobileThreadActivity(activity, deep, clientFor([deep]), now, false, '', 'visit').nativeWorkRow).copyText)
    .toBe('Read at-limit.ts\nat-limit.ts');
  const empty = projected({ type: 'dynamic_tool', toolName: 'Read', input: {}, output: { path: '/output-only.ts' } });
  expect(JSON.parse(mobileThreadActivity(activity, empty, clientFor([empty]), now, false, '', 'visit').nativeWorkRow).copyText).toBe('Read file');
});

for (const { name, item, expected } of [
  { name: 'trimmed title', item: { title: ' \tread source\n ', input: { file_path: '/repo/a.ts' } }, expected: 'Read source\n/repo/a.ts' },
  { name: 'static T3 title while running', item: { title: 't3_thread_read', status: 'running', input: { file_path: '/repo/a.ts' } }, expected: 'Read a T3 thread\n/repo/a.ts' },
  { name: 'completed MCP title alias', item: { title: '  mcp__t3-code__t3_thread_read completed  ', status: 'failed', input: { file_path: '/repo/a.ts' } }, expected: 'Read a T3 thread\n/repo/a.ts' },
  { name: 'OpenCode2 title alias', item: { title: 't3-code-thread_id_t3_thread_read', input: { file_path: '/repo/a.ts' } }, expected: 'Read a T3 thread\n/repo/a.ts' },
  { name: 'unrecognized foreign MCP title', item: { title: 'mcp__other__t3_thread_read', input: { file_path: '/repo/a.ts' } }, expected: 'Mcp__other__t3_thread_read\n/repo/a.ts' },
  { name: 'original collected fields', item: { title: 'inspect source', fileName: 'root.ts', input: { path: 'input.ts' },
    result: { path: 'result.ts' }, data: { path: 'data.ts' }, changes: [{ path: 'change.ts' }, { path: 'input.ts' }] },
    expected: 'Inspect source\nroot.ts\ninput.ts\nresult.ts\ndata.ts\nchange.ts' },
  { name: 'duplicate summary and path', item: { title: '/repo/a.ts', input: { file_path: '/repo/a.ts' } }, expected: '/repo/a.ts' },
  { name: 'empty path list', item: { title: 'read source', input: {}, output: { path: '/output-only.ts' } }, expected: 'Read source' },
]) test(`titled Read header Copy uses ${name} from the original item`, () => {
  const row = projected({ type: 'dynamic_tool', toolName: 'Read', ...item }), client = clientFor([row]);
  const shown = mobileThreadActivity({ ...activity, label: 'Grouped display label', detailOpen: false }, row, client, now, false, '', 'visit');
  expect(shown.nativeWorkRow).not.toBe('');
  expect(JSON.parse(shown.nativeWorkRow)).toMatchObject({ copyText: expected, label: 'Grouped display label', expanded: false });
  expect(shown).toMatchObject({ nativeWorkImage: false, nativeWorkDetail: false, nativeCommandDetail: false, call: false, output: '' });
});

test('titled Read Copy keeps original paths and route ownership through disclosure and fetching', async () => {
  const row = projected({ type: 'dynamic_tool', toolName: 'Read', title: ' read source ', input: { file_path: '/repo/a.ts' }, outputOmitted: true });
  const client = clientFor([row]), requests: Obj[] = [], before = JSON.stringify(client.projection);
  client.scopes = []; client.local.drafts[client.draftKey] = '  ';
  client.rpc = async (_native, method, payload) => {
    requests.push({ method, ...payload });
    return { item: { ...obj(row.item), title: 'Fetched title', input: { file_path: '/fetched/b.ts' }, outputOmitted: false, output: 'Fetched file body' } };
  };
  const props = { ...activity, label: 'Grouped display label', icon: 'file-text', iconLight: 'https://example.com/light.png', iconDark: 'https://example.com/dark.png' };
  const collapsed = mobileThreadActivity({ ...props, detailOpen: false }, row, client, now, false, '', 'visit');
  expect(collapsed.nativeWorkRow).not.toBe('');
  const initial = JSON.parse(collapsed.nativeWorkRow), expected = 'Read source\n/repo/a.ts';
  expect(initial).toMatchObject({ copyText: expected, label: 'Grouped display label', symbol: 'doc.text', expanded: false, expandable: true });
  expect(collapsed).toMatchObject({ nativeWorkImage: true, iconURL: props.iconLight, nativeWorkDetail: false, nativeCommandDetail: false, call: false, output: '' });
  expect(JSON.parse(mobileThreadActivity({ ...props, expandable: false }, row, client, now, false, '', 'visit').nativeWorkRow)).toMatchObject({ copyText: expected, expandable: false });
  expect(requests).toEqual([]);
  await chatLocal(client, native, 'item-detail', activity.id, 'open');
  expect(await refreshNextOpenTurnItemDetail(client, native, now)).toBe(true);
  const expanded = mobileThreadActivity(props, row, client, now, true, '', 'visit');
  expect(JSON.parse(expanded.nativeWorkRow)).toMatchObject({ copyText: expected, owner: initial.owner, expanded: true });
  expect(expanded).toMatchObject({ nativeWorkImage: true, iconURL: props.iconDark, nativeWorkDetail: false, nativeCommandDetail: false,
    call: false, body: '/repo/a.ts', output: 'Fetched file body' });
  const otherVisit = JSON.parse(mobileThreadActivity(props, row, client, now, true, '', 'other-visit').nativeWorkRow);
  expect(otherVisit.owner).not.toBe(initial.owner); expect(otherVisit.copyText).toBe(expected);
  client.generation++;
  const otherGeneration = JSON.parse(mobileThreadActivity(props, row, client, now, true, '', 'visit').nativeWorkRow);
  expect(otherGeneration.owner).not.toBe(initial.owner); expect(otherGeneration.copyText).toBe(expected);
  await chatLocal(client, native, 'item-detail', activity.id, 'closed');
  expect(mobileThreadActivity({ ...props, detailOpen: false }, row, client, now, false, '', 'visit').expanded).toBe(false);
  expect(requests).toEqual([{ method: 'orchestration.getTurnItem', threadId: 'source', itemId: 'item', revision: time }]);
  expect(client.draft).toBe('  '); expect(client.threadId).toBe('thread'); expect(JSON.stringify(client.projection)).toBe(before);
});

test('read header Copy leaves other dynamic rows on their existing path', () => {
  for (const fields of [{ toolName: 'unknown', kind: 'read' },
    { toolName: 'unknown', viewedImagePath: '/repo/image.png' }, { toolName: 'Skill', input: { skill: 'full-send' } },
    { toolName: 'cua_repl.js', input: { title: 'Read source' } }, { toolName: 'mcp__t3-code__t3_thread_read' },
    { toolName: 'mcp__t3-code__t3_thread_read', title: 't3_pending_request_read' },
    { type: 'notification', toolName: 'Read' }]) {
    const row = projected({ type: 'dynamic_tool', input: { file_path: '/repo/a.ts' }, ...fields });
    expect(mobileThreadActivity(activity, row, clientFor([row]), now, false, '', 'visit')).toMatchObject({ nativeWorkRow: '', nativeWorkImage: false });
  }
  const row = projected({ type: 'dynamic_tool', toolName: 'Read', input: { file_path: '/repo/a.ts' } });
  for (const dark of [false, true]) {
    const image = mobileThreadActivity({ ...activity, iconLight: 'https://example.com/light.png', iconDark: 'https://example.com/dark.png' }, row, clientFor([row]), now, dark, '', 'visit');
    expect(image.nativeWorkImage).toBe(true); expect(image.iconURL).toBe(`https://example.com/${dark ? 'dark' : 'light'}.png`);
    expect(JSON.parse(image.nativeWorkRow).copyText).toBe('Read /repo/a.ts\n/repo/a.ts');
  }
});

test('image work headers preserve theme fallbacks and all symbol-header Copy bytes', () => {
  const sources = [
    { iconLight: 'https://example.com/light.png', iconDark: 'https://example.com/dark.png' },
    { iconLight: 'https://example.com/light.png' }, { iconDark: 'https://example.com/dark.png' },
    { iconLight: '', iconDark: 'https://example.com/dark.png' },
    { iconLight: 'https://example.com/light.png', iconDark: '' }, {},
  ];
  for (const item of [{ type: 'file_search', title: '  find source  ', pattern: '  needle  ', results: [{ fileName: 'main.ts', line: 2, preview: 'needle' }] },
    { type: 'web_search', title: '  find docs  ', patterns: ['', '  needle  '], results: [{ title: 'Docs', url: 'https://example.com' }] },
    { type: 'dynamic_tool', toolName: 'Read', input: { file_path: './main.ts' } },
    { type: 'dynamic_tool', toolName: 'Read', title: 'read source', input: { file_path: './main.ts' } }]) {
    const row = projected(item), client = clientFor([row]);
    for (const dark of [false, true]) for (const source of sources) {
      const props = { ...activity, detailOpen: false }, plain = mobileThreadActivity(props, row, client, now, dark, '', 'visit');
      const image = mobileThreadActivity({ ...props, ...source }, row, client, now, dark, '', 'visit');
      const url = dark ? source.iconDark ?? source.iconLight ?? '' : source.iconLight ?? source.iconDark ?? '';
      expect(image).toMatchObject({ iconURL: url, nativeWorkImage: url !== '', nativeWorkDetail: false, nativeCommandDetail: false });
      expect(JSON.parse(image.nativeWorkRow)).toEqual(JSON.parse(plain.nativeWorkRow));
      expect(plain.nativeWorkImage).toBe(false);
    }
  }
});

test('image work headers retain disclosure, original content and route ownership', async () => {
  for (const item of [{ type: 'file_search', pattern: 'old', results: [{ fileName: 'old.ts' }] },
    { type: 'web_search', patterns: ['old'], results: [{ url: 'https://example.com/old' }] },
    { type: 'dynamic_tool', toolName: 'Read', input: { file_path: 'old.ts' }, outputOmitted: true },
    { type: 'dynamic_tool', toolName: 'Read', title: 'read source', input: { file_path: 'old.ts' }, outputOmitted: true }]) {
    const row = projected(item), client = clientFor([row]), before = JSON.stringify(client.projection), requests: Obj[] = [];
    client.local.drafts[client.draftKey] = '  ';
    client.rpc = async (_native, method, payload) => {
      requests.push({ method, ...payload });
      return { item: { ...obj(row.item), input: { file_path: 'fetched.ts' }, outputOmitted: false, output: 'Fetched body' } };
    };
    const collapsed = mobileThreadActivity({ ...activity, detailOpen: false, iconLight: 'https://example.com/light.png' }, row, client, now, false, '', 'visit');
    const initial = JSON.parse(collapsed.nativeWorkRow);
    expect(collapsed).toMatchObject({ nativeWorkImage: true, expandable: true, expanded: false, body: item.type === 'dynamic_tool' ? 'old.ts' : '', output: '' });
    await chatLocal(client, native, 'item-detail', activity.id, 'open');
    const didFetch = await refreshNextOpenTurnItemDetail(client, native, now);
    const expanded = mobileThreadActivity({ ...activity, iconLight: 'https://example.com/light.png' }, row, client, now, false, '', 'visit');
    expect(didFetch).toBe(item.type === 'dynamic_tool');
    expect(expanded).toMatchObject({ nativeWorkImage: true, expanded: true, nativeWorkDetail: false, nativeCommandDetail: false });
    expect(JSON.parse(expanded.nativeWorkRow)).toEqual({ ...initial, expanded: true });
    if (item.type === 'dynamic_tool') expect(expanded).toMatchObject({ body: 'old.ts', output: 'Fetched body' });
    await chatLocal(client, native, 'item-detail', activity.id, 'closed');
    expect(mobileThreadActivity({ ...activity, detailOpen: false, iconLight: 'https://example.com/light.png' }, row, client, now, false, '', 'visit').expanded).toBe(false);
    expect(JSON.parse(mobileThreadActivity({ ...activity, iconLight: 'https://example.com/light.png' }, row, client, now, false, '', 'other-visit').nativeWorkRow).owner).not.toBe(initial.owner);
    client.generation++;
    const next = JSON.parse(mobileThreadActivity({ ...activity, iconLight: 'https://example.com/light.png' }, row, client, now, false, '', 'visit').nativeWorkRow);
    expect(next.owner).not.toBe(initial.owner); expect(next.copyText).toBe(initial.copyText);
    const changed = { ...row, item: { ...obj(row.item), title: item.type === 'dynamic_tool' ? null : 'new title', pattern: 'new', patterns: ['new'], input: { file_path: 'new.ts' } } };
    // A same-row update keeps the owner but supplies new Copy content, which cancels a native hold.
    const revised = JSON.parse(mobileThreadActivity(activity, changed, client, now, false, '', 'visit').nativeWorkRow);
    expect(revised.owner).toBe(next.owner); expect(revised.copyText).not.toBe(initial.copyText);
    const otherRow = JSON.parse(mobileThreadActivity({ ...activity, id: '["source","other"]' },
      { ...row, sourceItemId: 'other' }, client, now, false, '', 'visit').nativeWorkRow);
    expect(otherRow.owner).not.toBe(next.owner);
    expect(requests).toHaveLength(item.type === 'dynamic_tool' ? 1 : 0);
    expect(client.draft).toBe('  '); expect(client.threadId).toBe('thread'); expect(JSON.stringify(client.projection)).toBe(before);
  }
});

test('image composition does not widen existing command, Fork or generic dynamic headers', () => {
  for (const item of [{ type: 'command_execution', input: 'pwd' }, { type: 'fork', targetThreadId: 'child' },
    { type: 'dynamic_tool', toolName: 'mcp__t3-code__preview_open', input: {} }]) {
    const row = projected(item), client = clientFor([row]);
    const plain = mobileThreadActivity(activity, row, client, now, false, '', 'visit');
    const image = mobileThreadActivity({ ...activity, iconLight: 'https://example.com/light.png' }, row, client, now, false, '', 'visit');
    expect(image.nativeWorkImage).toBe(false); expect(image.nativeWorkRow).toBe(plain.nativeWorkRow);
    expect(image.nativeCommandDetail).toBe(plain.nativeCommandDetail); expect(image.nativeWorkDetail).toBe(plain.nativeWorkDetail);
    expect(image.iconURL).toBe('https://example.com/light.png');
  }
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
  expect(mobileThreadRows(client, now).flatMap(row => row.activities)[0]!.reasoningBlocks).toBe(shown.reasoningBlocks);
  const saved = structuredClone(shown.reasoningBlocks);
  row.item.text = markdown + '\n\nAdditional reasoning.';
  const updated = mobileThreadRows(client, now).flatMap(row => row.activities)[0]!;
  expect(updated.reasoningBlocks).not.toBe(shown.reasoningBlocks);
  expect(updated.reasoningBlocks.at(-1)!.text).toContain('Additional reasoning.');
  expect(shown.reasoningBlocks).toEqual(saved);
  setTurnItemOpen(client, activity.id, false);
  expect(mobileThreadRows(client, now).flatMap(row => row.activities)[0]!.reasoningBlocks).toEqual([]);
  setTurnItemOpen(client, activity.id, true);
  const reopened = mobileThreadRows(client, now).flatMap(row => row.activities)[0]!;
  expect(reopened.reasoningBlocks).toEqual(updated.reasoningBlocks);
  expect(reopened.reasoningBlocks).not.toBe(updated.reasoningBlocks);
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
