// Clone-authored coverage for T3 Code itemDetail.ts (the reference has no direct tests).
import { noteNow } from './composer-controls';
import { describe, expect, test } from 'bun:test';
import { toolCallLines, turnItemDetailRevision, turnItemHasDetail, turnItemNeedsDetailFetch, turnItemOutputText } from './timeline-item-detail';
import { refreshOpenTurnItemDetails, setTurnItemOpen, turnItemDetailView } from './timeline-item-fetch';
import { inspectorDetail, projectedWorkEntry } from './timeline-worklog';
import { chatLocal } from './timeline-presentation';
import type { T3Client } from './client';
import type { Obj } from './domain';
import type { Native } from './protocol';

const command = (id = 'a', extra: Obj = {}): Obj => ({ id, type: 'command_execution', input: 'printf hello', status: 'running', updatedAt: '2026-10-06T00:00:00.000Z', outputOmitted: true, ...extra });
const native = {} as Native;
function fixture(items: Obj[], rpc: (native: Native, method: string, payload: Obj) => Promise<Obj>) {
  return { environmentId: 'env', threadId: 'thread', projection: { visibleTurnItems: items.map(item => ({ sourceThreadId: 'source', sourceItemId: item.id, item })) }, rpc } as unknown as T3Client;
}

describe('toolCallLines', () => {
  test('preserves command and empty/null arguments; formats structured input', () => {
    expect(toolCallLines({ command: '  echo hi  ' }).command).toBe('echo hi');
    expect(toolCallLines({ args: { path: '', reset: null, count: 2, value: { x: 1 } } }).args).toEqual([
      ['path', '""'], ['reset', 'null'], ['count', '2'], ['value', '{"x":1}'],
    ]);
    expect(toolCallLines({ args: ['a', 'b'] }).argsText).toBe('a\nb');
  });
});
describe('turnItemNeedsDetailFetch and turnItemDetailRevision', () => {
  test('fetches omitted outputs and summarized input, once per live or final revision', () => {
    expect(turnItemNeedsDetailFetch(command())).toBe(true);
    expect(turnItemNeedsDetailFetch({ type: 'dynamic_tool', input: { summary: 'truncated', truncated: true } })).toBe(true);
    expect(turnItemNeedsDetailFetch({ type: 'file_change' })).toBe(false);
    expect(turnItemDetailRevision(command())).toBe('live');
    expect(turnItemDetailRevision(command('a', { status: 'completed' }))).toBe('2026-10-06T00:00:00.000Z');
  });
});
describe('turnItemOutputText', () => {
  test('unwraps MCP text, formats JSON, and keeps old Claude stdout/stderr', () => {
    expect(turnItemOutputText({ type: 'dynamic_tool', output: { content: [{ type: 'text', text: '{"ok":true}' }], isError: false } })).toBe('{\n  "ok": true\n}');
    expect(turnItemOutputText({ type: 'command_execution', output: '{"stdout":"out","stderr":"err","interrupted":false}' })).toBe('out\nerr');
    expect(turnItemOutputText({ type: 'command_execution', output: '  ' })).toBeNull();
    expect(turnItemOutputText({ type: 'dynamic_tool', output: { content: [{ type: 'image' }] } })).toBe('[image]');
  });
});
describe('turnItemHasDetail and inspectorDetail', () => {
  test('exit zero alone does not disclose, nonzero exits are short and search patterns/errors survive', () => {
    expect(turnItemHasDetail(command('a', { input: '', outputOmitted: false, exitCode: 0 }))).toBe(false);
    expect(turnItemHasDetail(command('a', { input: '', outputOmitted: false, exitCode: 2 }))).toBe(true);
    expect(turnItemHasDetail({ type: 'dynamic_tool', input: {}, output: 'answer' })).toBe(true);
    const inspect = (item: Obj) => inspectorDetail(projectedWorkEntry({ item }), '/work');
    expect(inspect(command('a', { exitCode: 2 })).result).toBe('exit 2');
    expect(inspect({ type: 'file_change', fileName: 'a', status: 'failed', diffStr: 'Permission denied' }).input).toBe('Permission denied');
    expect(inspect({ type: 'file_search', pattern: 'needle' }).input).toBe('needle');
  });
});
describe('on-demand turn item cache', () => {
  test('keys source thread and item independently, deduplicates, and keeps late replies in their own rows', async () => {
    const a = command('a'), b = command('b'), requests: Obj[] = [], replies = new Map<string, (value: Obj) => void>();
    const client = fixture([a, b], async (_, method, payload) => {
      expect(method).toBe('orchestration.getTurnItem'); requests.push(payload);
      return new Promise(resolve => replies.set(String(payload.itemId), resolve));
    });
    const first = chatLocal(client, native, 'item-detail', JSON.stringify(['source', 'a']), 'open');
    const duplicate = setTurnItemOpen(client, native, 'a', true);
    const second = setTurnItemOpen(client, native, 'b', true);
    expect(turnItemDetailView(client, a)).toMatchObject({ state: 'loading', text: 'Loading output…' });
    expect(requests).toEqual([{ threadId: 'source', itemId: 'a', revision: 'live' }, { threadId: 'source', itemId: 'b', revision: 'live' }]);
    replies.get('b')!({ item: { ...b, outputOmitted: false, output: 'B' } }); await second;
    replies.get('a')!({ item: { ...a, outputOmitted: false, output: 'A' } }); await Promise.all([first, duplicate]);
    expect(turnItemDetailView(client, a).text).toBe('A'); expect(turnItemDetailView(client, b).text).toBe('B');
    await setTurnItemOpen(client, native, 'a', false); await setTurnItemOpen(client, native, 'a', true);
    expect(requests).toHaveLength(2);
    client.environmentId = 'other';
    expect(turnItemDetailView(client, a).state).toBe('loading');
  });
  test('refreshes final revision and retains output while refreshing', async () => {
    const item = command(), requests: Obj[] = []; let finish!: (value: Obj) => void;
    const client = fixture([item], async (_, __, payload) => {
      requests.push(payload);
      return payload.revision === 'live' ? { item: { ...item, outputOmitted: false, output: 'partial' } } : new Promise(resolve => { finish = resolve; });
    });
    await setTurnItemOpen(client, native, 'a', true);
    item.status = 'completed'; item.updatedAt = '2026-10-06T00:01:00.000Z';
    const pending = refreshOpenTurnItemDetails(client, native);
    expect(turnItemDetailView(client, item).text).toBe('partial');
    finish({ item: { ...item, outputOmitted: false, output: 'final' } }); await pending;
    expect(turnItemDetailView(client, item).text).toBe('final');
    await refreshOpenTurnItemDetails(client, native); expect(requests).toHaveLength(2);
  });
  test('a reconnect never adopts or reuses an old connection reply', async () => {
    const item = command(); let late!: (value: Obj) => void; let calls = 0;
    const client = fixture([item], async () => ++calls === 1 ? new Promise(resolve => { late = resolve; }) : { item: { ...item, outputOmitted: false, output: 'new' } });
    client.generation = 1; client.origin = 'http://old';
    const pending = setTurnItemOpen(client, native, 'a', true);
    client.generation = 2;
    await setTurnItemOpen(client, native, 'a', true);
    late({ item: { ...item, outputOmitted: false, output: 'stale' } }); await pending;
    expect(turnItemDetailView(client, item).text).toBe('new');
    expect(calls).toBe(2);
  });
  test('source threads with the same item id keep distinct detail identity', async () => {
    const item = command(), requests: Obj[] = [];
    const client = fixture([item], async (_, __, payload) => { requests.push(payload); return { item: { ...item, outputOmitted: false, output: String(payload.threadId) } }; });
    client.projection.visibleTurnItems = ['one', 'two'].map(sourceThreadId => ({ sourceThreadId, sourceItemId: 'a', item }));
    for (const thread of ['one', 'two']) await setTurnItemOpen(client, native, JSON.stringify([thread, 'a']), true);
    expect(requests.map(request => request.threadId)).toEqual(['one', 'two']);
    expect(turnItemDetailView(client, item, 0, JSON.stringify(['one', 'a'])).text).toBe('one');
    expect(turnItemDetailView(client, item, 0, JSON.stringify(['two', 'a'])).text).toBe('two');
  });
  test('a failed refresh retains existing output and a mismatched response type is refused', async () => {
    const item = command(); let calls = 0;
    const client = fixture([item], async () => { if (++calls > 1) throw new Error('offline'); return { item: { ...item, outputOmitted: false, output: 'kept' } }; });
    await setTurnItemOpen(client, native, 'a', true);
    item.status = 'completed'; item.updatedAt = 'final';
    await refreshOpenTurnItemDetails(client, native);
    expect(turnItemDetailView(client, item)).toMatchObject({ text: 'kept', state: '' });
    const wrong = fixture([item], async () => ({ item: { ...item, type: 'user_message' } }));
    await setTurnItemOpen(wrong, native, 'a', true);
    expect(turnItemDetailView(wrong, item)).toMatchObject({ state: 'error' });
  });
  test('exposes missing, fetched-empty and RPC error states', async () => {
    for (const [reply, state, text] of [
      [{ item: null }, 'missing', 'Output is no longer available.'],
      [{ item: { ...command(), outputOmitted: false, output: '' } }, 'empty', 'No output.'],
      [null, 'error', "Couldn't load output: disconnected"],
    ] as const) {
      const item = command(); const client = fixture([item], async () => { if (!reply) throw new Error('disconnected'); return reply; });
      await setTurnItemOpen(client, native, 'a', true);
      expect(turnItemDetailView(client, item)).toMatchObject({ state, text });
    }
  });
  test('refetches after sixty seconds and drops idle entries', async () => {
    const item = command(); let calls = 0; const now = Date.now();
    const client = fixture([item], async () => { calls++; return { item: { ...item, outputOmitted: false, output: `${calls}` } }; });
    noteNow(client, now);
    await setTurnItemOpen(client, native, 'a', true, now);
    await setTurnItemOpen(client, native, 'a', false, now);
    await setTurnItemOpen(client, native, 'a', true, now + 61_000);
    expect(calls).toBe(2);
  });
});
