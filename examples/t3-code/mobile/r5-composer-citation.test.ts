import { describe, expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { loadCitedHistory, revealCitation } from './shared/r5-composer-citation';
import type { Native } from './shared/protocol';
import { obj } from './shared/domain';

const native: Native = { available: true, watch() {}, async later(request) {
  return { ok: true, generation: 0, value: obj(request).op === 'timelineJump' ? {} : {} };
} };

function fixture() {
  const client = new T3Client();
  client.generation = 0;
  client.threadId = 'thread';
  client.shell.threads = [{ id: 'thread' }];
  client.thread = { sequence: 1, historyCursor: 'page-1', hasMore: true, latestLocalTurnOrdinal: 1,
    projection: { thread: { id: 'thread' }, visibleTurnItems: [{ sourceThreadId: 'thread', sourceItemId: 'current', item: { id: 'current', messageId: 'current', type: 'user_message', text: 'Current' } }] } };
  const requested: string[] = [], jumps: unknown[] = [], pages = [
    [{ id: 'older', type: 'user_message', text: 'Older' }],
    [{ id: 'target', type: 'assistant_message', text: 'Cited response' }],
  ];
  const original = native.later;
  native.later = async request => {
    if (obj(request).op === 'timelineJump') jumps.push(request);
    return original(request);
  };
  client.history = (async (_bridge, _visit, _current, through) => {
    requested.push(String(through));
    const page = pages[requested.length - 1] ?? [];
    const prior = client.thread!;
    client.thread = { ...prior, historyCursor: requested.length < pages.length ? `page-${requested.length + 1}` : null,
      hasMore: requested.length < pages.length,
      projection: { ...prior.projection, visibleTurnItems: [...page.map(item => ({ sourceThreadId: 'thread', sourceItemId: item.id, item })), ...obj(prior.projection).visibleTurnItems as unknown[]] } };
  }) as T3Client['history'];
  return { client, requested, jumps };
}

function rows(client: T3Client) {
  return (Array.isArray(client.projection.visibleTurnItems) ? client.projection.visibleTurnItems : []).map(row => ({
    id: JSON.stringify([String(obj(row).sourceThreadId), String(obj(row).sourceItemId)]),
    kind: obj(obj(row).item).type === 'assistant_message' ? 'assistant' : 'user', body: String(obj(obj(row).item).text ?? ''),
  }));
}

describe('mobile cited-response history', () => {
  test('pages filtered history toward a cited response before placing it', async () => {
    const f = fixture();
    await revealCitation(f.client, native, 'thread', 'message:target', () => rows(f.client));
    expect(f.requested).toEqual(['message:target', 'message:target']);
    expect(f.jumps).toHaveLength(1);
    expect(obj(f.jumps[0]).id).toBe('cite:["thread","target"]');
  });

  test('a route change aborts targeted history without placing a stale response', async () => {
    const f = fixture();
    f.client.history = (async () => { f.client.threadId = 'other-thread'; }) as T3Client['history'];
    await revealCitation(f.client, native, 'thread', 'message:target', () => rows(f.client));
    expect(f.jumps).toHaveLength(0);
  });

  test('an unavailable target reports the reference warning outcome', async () => {
    const f = fixture();
    const result = await loadCitedHistory(f.client, native, 'thread', 'message:missing');
    expect(result).toMatchObject({ title: 'The cited response is unavailable' });
  });
});
