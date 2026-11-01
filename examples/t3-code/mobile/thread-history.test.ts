import { describe, expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import { ClientError, type Files, type Native } from './shared/protocol';
import { letGoAware } from './shared/let-go';
import { mobileThreadHistory } from './thread-history';
import { chatLocal } from './shared/timeline-presentation';
import { mobileThreadRows } from './thread';
import { contractComponent } from '../tests/support/contract-action-fixture';

function fixture() {
  const client = new T3Client();
  Object.assign(client, { origin: 'https://history.test', environmentId: 'env', generation: 1, threadId: 'thread', threadEpoch: 2,
    connection: 'connected', configLive: true, shellLive: true, threadLive: true });
  client.thread = { sequence: 1, historyCursor: 'older', hasMore: true, latestLocalTurnOrdinal: 1,
    projection: { thread: { id: 'thread' }, turnItems: [], visibleTurnItems: [], runs: [], attempts: [] } };
  const requests: { input: Obj; resolve(value: unknown): void; reject(error: unknown): void }[] = [];
  const native: Native = { available: true, watch() {}, later(input) {
    return new Promise((resolve, reject) => requests.push({ input: obj(input), resolve, reject }));
  } };
  const page = (index: number, text = 'Older content', extra: Obj = {}) => {
    const request = requests[index]!;
    request.resolve({ ok: true, generation: request.input.generation, value: {
      snapshotSequence: 1, nextCursor: null, hasMoreHistory: false,
      items: [{ position: 0, visibility: 'local', sourceThreadId: client.threadId, sourceItemId: 'older-item',
        item: { id: 'older-item', threadId: client.threadId, type: 'assistant_message', text, streaming: false, messageId: 'older-message',
          runId: null, nodeId: null, ordinal: 0, status: 'completed', startedAt: '2026-10-30T00:00:00Z', updatedAt: '2026-10-30T00:00:00Z' } }],
      ...extra,
    } });
  };
  return { client, native, requests, page };
}

describe('older-history request ownership', () => {
  test('same-owner reads deduplicate, merge complete content and release their loading flag', async () => {
    const f = fixture(), pending = f.client.history(f.native);
    expect(f.client.historyLoading).toBe(true);
    expect(await f.client.history(f.native)).toBeNull(); expect(f.requests).toHaveLength(1);
    expect(f.requests[0]!.input).toMatchObject({ op: 'http', generation: 1,
      path: '/api/orchestration/threads/thread/history?cursor=older' });
    f.page(0); expect(await pending).toEqual(['["thread","older-item"]']);
    expect(f.client.historyLoading).toBe(false);
    expect(f.client.historyRevision).toBe(1);
    expect(f.client.thread).toMatchObject({ hasMore: false, historyCursor: null });
    expect(f.client.projection.visibleTurnItems.map((row: Obj) => obj(row.item).text)).toEqual(['Older content']);
  });

  test('an accepted page with no admitted older rows returns an empty array', async () => {
    const f = fixture(), pending = f.client.history(f.native);
    f.page(0, '', { items: [] });
    expect(await pending).toEqual([]);
    expect(f.client.historyLoading).toBe(false);
  });

  test('a nonlocal command retires the token even when the history identity stays unchanged', async () => {
    const f = fixture(), old = f.client.history(f.native).catch(error => error);
    const files: Files = { fs: { async mkdir() {}, async readFile() { return new ArrayBuffer(0); }, async atomicWriteFile() {} } };
    const native: Native = { ...f.native, async later() { return { ok: true, generation: 1, value: {} }; } };
    await f.client.command('stop', '', '', 0, native, files);
    expect(f.client.historyLoading).toBe(false);
    expect(f.client.thread).toMatchObject({ historyCursor: 'older', hasMore: true });
    const current = f.client.history(f.native);
    expect(f.requests).toHaveLength(2);
    f.page(0, 'Retired page');
    expect(await old).toMatchObject({ kind: 'superseded' });
    expect(f.client.historyLoading).toBe(true);
    f.page(1, 'Current page'); await current;
    expect(f.client.projection.visibleTurnItems.map((row: Obj) => obj(row.item).text)).toEqual(['Current page']);
  });

  for (const field of ['origin', 'environmentId', 'generation', 'threadId', 'threadEpoch', 'cursor']) {
    for (const outcome of ['success', 'failure']) test(`${field} retires old ${outcome} and its finally cannot clear the newer load`, async () => {
      const f = fixture(), old = f.client.history(f.native).catch(error => error);
      if (field === 'cursor') f.client.thread!.historyCursor = 'different';
      else if (field === 'generation') f.client.generation++;
      else if (field === 'threadEpoch') f.client.threadEpoch++;
      else if (field === 'origin') f.client.origin = 'https://other.test';
      else if (field === 'environmentId') f.client.environmentId = 'other-env';
      else f.client.threadId = 'other-thread';
      const current = f.client.history(f.native);
      expect(f.requests).toHaveLength(2);
      if (outcome === 'success') f.page(0, 'Stale content');
      else f.requests[0]!.reject(new Error('Stale failure'));
      expect(await old).toMatchObject({ kind: 'superseded' });
      expect(f.client.historyLoading).toBe(true);
      expect(f.client.projection.visibleTurnItems).toEqual([]);
      f.page(1, 'Current content'); await current;
      expect(f.client.historyLoading).toBe(false);
      expect(f.client.projection.visibleTurnItems.map((row: Obj) => obj(row.item).text)).toEqual(['Current content']);
    });
  }

  test('current failure remains retryable and native let-go stays superseded', async () => {
    const f = fixture(), failed = f.client.history(f.native).catch(error => error);
    f.requests[0]!.reject(new ClientError('Offline', 'transport'));
    expect(await failed).toMatchObject({ message: 'Offline', kind: 'transport' });
    expect(f.client.historyLoading).toBe(false);
    const aborted = f.client.history(letGoAware(f.native)).catch(error => error);
    f.requests[1]!.reject({ name: 'FetchError', kind: 'Aborted' });
    expect(await aborted).toMatchObject({ kind: 'superseded' });
    expect(f.client.historyLoading).toBe(false);
    const retry = f.client.history(f.native); f.page(2); await retry;
    expect(f.client.thread!.hasMore).toBe(false);
  });

  test('same-thread revisit retires a descriptor and releases its captured native transfer', async () => {
    const f = fixture(), releases: Obj[] = [];
    const native: Native = { ...f.native, later(input) {
      if (obj(input).op === 'releaseChunk') { releases.push(obj(input)); return Promise.resolve({ ok: true, generation: 1, value: {} }); }
      return f.native.later(input);
    } };
    let visit = 'first';
    const old = f.client.history(native, 'first', () => visit === 'first').catch(error => error);
    visit = 'second';
    const current = f.client.history(native, 'second', () => visit === 'second');
    f.requests[0]!.resolve({ ok: true, generation: 1, value: { _nativeTransfer: { id: 'retired', parts: 1 } } });
    expect(await old).toMatchObject({ kind: 'superseded' });
    expect(releases).toEqual([{ op: 'releaseChunk', id: 'retired', generation: 1 }]);
    expect(f.client.historyLoading).toBe(true);
    f.page(1); await current;
    expect(f.requests).toHaveLength(2);
  });
});

describe('automatic earlier activity', () => {
  const view = (f: ReturnType<typeof fixture>, route = 'visit') => mobileThreadHistory(f.client, route, true);
  const load = (f: ReturnType<typeof fixture>, key = view(f).historyRequest, retry = false) =>
    chatLocal(f.client, f.native, retry ? 'history-retry' : 'history-auto', key, '');

  test('reachstart plus sentinel deduplicate and stale queued keys cannot fetch another page', async () => {
    const f = fixture(), key = view(f).historyRequest, pending = load(f, key);
    await load(f, key); expect(f.requests).toHaveLength(1);
    f.page(0, 'Older content', { hasMoreHistory: true, nextCursor: 'page-2' }); await pending;
    expect(view(f).historyRequest).not.toBe(key);
    await load(f, key); expect(f.requests).toHaveLength(1);
    const next = load(f); f.page(1, 'Oldest content', { items: [] }); await next;
    expect(view(f)).toMatchObject({ historyRequest: '', historyLoading: false, historyError: '' });
    expect(f.client.projection.visibleTurnItems.map((row: Obj) => obj(row.item).text)).toEqual(['Older content']);
  });

  test('empty and filtered pages continue with new keys, then stop after eight pages without older content', async () => {
    const f = fixture();
    for (let index = 0; index < 8; index++) {
      const key = view(f).historyRequest, pending = load(f, key);
      const live = { sourceThreadId: 'thread', sourceItemId: `live-${index}`, visibility: 'local',
        item: { id: `live-${index}`, type: 'assistant_message', text: 'Unrelated live append', messageId: `live-${index}`, streaming: true, runId: null } };
      f.client.projection.visibleTurnItems.push(live);
      f.page(index, '   ', { hasMoreHistory: true, nextCursor: `older-${index}`,
        ...(index % 2 === 0 ? { items: [] } : {}) });
      await pending;
      expect(view(f).historyRequest).not.toBe(key);
    }
    expect(view(f).historyError).toContain('several pages');
    await load(f); expect(f.requests).toHaveLength(8);
    const retry = load(f, view(f).historyRequest, true); f.page(8, 'Recovered', { items: [] }); await retry;
    expect(view(f).historyError).toBe('');
  });

  test('repeated boundaries and short cursor cycles stop automatic reads while retry stays available', async () => {
    for (const cursors of [['older'], ['two', 'older'], [null]]) {
      const f = fixture();
      for (const nextCursor of cursors) {
        const pending = load(f); f.page(f.requests.length - 1, 'Page', { items: [], hasMoreHistory: true, nextCursor }); await pending;
      }
      expect(view(f).historyError).not.toBe('');
      expect(view(f).historyRequest).not.toBe('');
      const count = f.requests.length; await load(f); expect(f.requests).toHaveLength(count);
      const retry = load(f, view(f).historyRequest, true); f.page(count); await retry;
      expect(view(f).historyError).toBe('');
    }
  });

  test('older tools admitted into an existing collapsed fold count as progress and remain accessible', async () => {
    const f = fixture(), at = '2026-10-30T00:00:00Z';
    const tool = (id: string) => ({ position: 0, visibility: 'local', sourceThreadId: 'thread', sourceItemId: id,
      item: { id, threadId: 'thread', type: 'command_execution', input: id, output: 'Done', exitCode: 0,
        runId: 'run', nodeId: null, ordinal: 0, status: 'completed', startedAt: at, updatedAt: at } });
    f.client.projection.runs = [{ id: 'run', status: 'completed', startedAt: at, completedAt: at }];
    f.client.projection.visibleTurnItems = [tool('current')];
    f.client.projection.turnItems = [tool('current').item];
    const initial = mobileThreadRows(f.client, 0);
    expect(initial).toMatchObject([{ kind: 'work', expanded: false, toggleId: 'run' }]);
    for (let index = 0; index < 9; index++) {
      const pending = load(f);
      f.page(index, '', { items: [tool(`older-${index}`)], hasMoreHistory: true, nextCursor: `page-${index}` });
      await pending;
      expect(view(f).historyError).toBe('');
      expect(mobileThreadRows(f.client, 0).map(row => row.id)).toEqual(initial.map(row => row.id));
    }
    await chatLocal(f.client, f.native, 'fold', 'run', '');
    const group = mobileThreadRows(f.client, 0).find(row => row.kind === 'group')!;
    expect(group).toMatchObject({ toggleOp: 'chatlocal:group', expanded: false });
    await chatLocal(f.client, f.native, 'group', group.toggleId, '');
    const commands = mobileThreadRows(f.client, 0).flatMap(row => row.activities.map(activity => activity.body));
    expect(commands).toEqual(['older-8', 'older-7', 'older-6', 'older-5', 'older-4', 'older-3', 'older-2', 'older-1', 'older-0', 'current']);
  });

  test('departure suppresses old errors and returning to the same visit gives a fresh observation key', async () => {
    const f = fixture(), first = view(f).historyRequest;
    const pending = load(f, first).catch(error => error);
    mobileThreadHistory(f.client, '', false);
    const returned = view(f).historyRequest;
    expect(returned).not.toBe(first);
    const current = load(f, returned);
    f.requests[0]!.reject(new Error('Old route failure'));
    expect(await pending).toMatchObject({ kind: 'superseded' });
    expect(view(f)).toMatchObject({ historyLoading: true, historyError: '' });
    f.page(1); await current;
    expect(view(f).historyError).toBe('');
  });

  for (const outcome of ['success', 'failure']) test(`Stop re-arms the same cursor before retired ${outcome} settles`, async () => {
    const f = fixture(), key = view(f).historyRequest, old = load(f, key).catch(error => error);
    const files: Files = { fs: { async mkdir() {}, async readFile() { return new ArrayBuffer(0); }, async atomicWriteFile() {} } };
    f.client.scopes = ['orchestration:operate'];
    f.client.config = { environment: { capabilities: { serverResolvedCommandContext: true } } };
    f.client.projection.runs = [{ id: 'run', status: 'running' }];
    const commands: Obj[] = [];
    const native: Native = { ...f.native, async later(input) {
      const request = obj(input);
      if (request.method === 'orchestration.dispatchCommand') commands.push(obj(request.payload));
      return { ok: true, generation: 1, value: request.op === 'ids' ? ['stop-command'] : {} };
    } };
    expect(view(f).historyLoading).toBe(true);
    expect((await f.client.command('stop', '', '', 0, native, files)).message).toBe('');
    expect(commands).toEqual([{ type: 'run.interrupt', commandId: 'stop-command', threadId: 'thread', runId: 'run', holdQueue: true }]);
    expect(view(f)).toMatchObject({ historyLoading: false, historyError: '' });
    expect(view(f).historyRequest).not.toBe(key);
    await load(f, key); expect(f.requests).toHaveLength(1);
    const current = load(f);
    expect(f.requests).toHaveLength(2);
    if (outcome === 'success') f.page(0, 'Retired page');
    else f.requests[0]!.reject(new Error('Retired failure'));
    expect(await old).toMatchObject({ kind: 'superseded' });
    expect(view(f)).toMatchObject({ historyLoading: true, historyError: '' });
    expect(f.client.historyLoading).toBe(true);
    expect(f.client.projection.visibleTurnItems).toEqual([]);
    f.page(1, 'Current page'); await current;
    expect(view(f)).toMatchObject({ historyLoading: false, historyError: '' });
    expect(f.client.projection.visibleTurnItems.map((row: Obj) => obj(row.item).text)).toEqual(['Current page']);
  });

  test('same-cursor native cancellation re-arms automatically without publishing an error', async () => {
    const f = fixture(), key = view(f).historyRequest, pending = load(f, key).catch(error => error);
    f.requests[0]!.reject(new ClientError('Cancelled', 'superseded'));
    expect(await pending).toMatchObject({ kind: 'superseded' });
    expect(view(f).historyError).toBe('');
    expect(view(f).historyRequest).not.toBe(key);
    const retry = load(f); f.page(1); await retry;
    expect(view(f).historyRequest).toBe('');
  });
});

test('authored paging gates inactive views, rejects stale observations and isolates history from Stop', () => {
  const feed = contractComponent('mobile/thread.contract', 'ThreadFeed');
  const history = contractComponent('mobile/thread.contract', 'ThreadHistory');
  const root = contractComponent('mobile/app.contract', 'App');
  const calls: unknown[][] = [], data = { hasMore: true, historyRequest: 'key', historyLoading: false, historyError: '' };
  const scope = { data, active: true, action: (...args: unknown[]) => calls.push(args) };
  feed.call('loadEarlier', {}, { ...scope, active: false }, []);
  history.call('observe', {}, scope, ['old-key', { isIntersecting: true }]);
  history.call('observe', {}, scope, ['key', { isIntersecting: false }]);
  expect(calls).toEqual([]);
  feed.call('loadEarlier', {}, scope, []);
  history.call('observe', {}, scope, ['key', { isIntersecting: true }]);
  expect(calls).toEqual([['chatlocal:history-auto', 'key', ''], ['chatlocal:history-auto', 'key', '']]);
  data.historyError = 'Offline'; feed.call('loadEarlier', {}, scope, []); history.call('retry', {}, scope, []);
  expect(calls.at(-1)).toEqual(['chatlocal:history-retry', 'key', '']);
  const sent: string[] = [], rootScope = { data: { environmentId: 'env', threadId: 'thread' }, nav: {},
    top: () => ({ id: 7, name: 'thread', params: { threadEnvironment: 'env', threadId: 'thread' } }),
    wallTime: { epochAtZero: 0 }, performanceNow: () => 1, taskGuardOwner: '', taskGuardRoute: '',
    pending: () => true, changed: {}, threadSubmissionChanged: {}, threadHistoryChanged: {},
    threadSubmission: { ordinary: true }, command: (...args: unknown[]) => args,
    sent: (mutation: string) => sent.push(mutation) };
  root.call('threadAction', { lastOperation: '' }, rootScope, ['chatlocal:history-auto', 'key', '']);
  root.call('threadAction', { lastOperation: '' }, rootScope, ['stop', '', '']);
  expect(sent).toEqual(['threadHistoryChanged', 'changed']);
  const anchorCalls: unknown[][] = [], anchorState = { historyAnchorKey: '', historyAnchorOffset: 0, historyAnchorRoute: '', historyAnchorEnvironment: '', historyAnchorThread: '', historyAnchorRevision: 0 };
  const anchorScope = { ...rootScope, threadData: { historyRevision: 0, environmentId: 'env', threadId: 'thread' }, scrollIntoView: (...args: unknown[]) => anchorCalls.push(args) };
  root.call('threadHistoryScroll', anchorState, anchorScope, [0, 12, { firstVisibleKey: 'turn-03', firstVisibleOffset: 42 }]);
  expect(anchorState).toMatchObject({ historyAnchorKey: 'turn-03', historyAnchorOffset: 42, historyAnchorRoute: '7', historyAnchorEnvironment: 'env', historyAnchorThread: 'thread', historyAnchorRevision: 0 });
  anchorScope.threadData.historyRevision = 1;
  root.call('didThreadHistoryChanged', anchorState, anchorScope, []);
  expect(anchorCalls).toEqual([['thread-feed', 'turn-03', { offset: 42, behavior: 'instant' }]]);
  anchorScope.threadData.threadId = 'other-thread';
  anchorScope.threadData.historyRevision = 2;
  root.call('didThreadHistoryChanged', anchorState, anchorScope, []);
  expect(anchorCalls).toHaveLength(1);
});

test('history chrome describes active loading and disables retry while offline', () => {
  const history = contractComponent('mobile/thread.contract', 'ThreadHistory');
  const data = { hasMore: true, historyRequest: 'key', historyLoading: false, historyError: '' };
  const scope = { data, active: true };
  expect(history.drawn('t3-archive-spinner', {}, scope)).toBe(false);
  expect(history.drawn('intersect=observe(key)', {}, scope)).toBe(true);
  expect(history.drawn('testId="thread-history-load-earlier"', {}, scope)).toBe(true);
  data.historyRequest = '';
  expect(history.drawn('t3-archive-spinner', {}, scope)).toBe(false);
  expect(history.drawn('testId="thread-history-load-earlier"', {}, scope)).toBe(false);
  data.historyError = 'Offline';
  expect(history.drawn('testId="thread-history-load-earlier"', {}, scope)).toBe(false);
  expect(history.attribute('testId="thread-history-retry"', 'disabled', {}, scope)).toBe(true);
  data.historyRequest = 'key';
  expect(history.attribute('testId="thread-history-retry"', 'disabled', {}, scope)).toBe(false);
  data.historyError = ''; data.historyLoading = true;
  expect(history.drawn('t3-archive-spinner', {}, scope)).toBe(true);
  expect(history.drawn('testId="thread-history-load-earlier"', {}, scope)).toBe(true);
  expect(history.attribute('testId="thread-history-load-earlier"', 'disabled', {}, scope)).toBe(true);
});
