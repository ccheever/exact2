import { expect, test } from 'bun:test';
import { mobileThreadHistoryClear, mobileThreadHistoryCommit, mobileThreadHistoryRead, mobileThreadHistoryTicket } from './mobile-thread-history-store';
import { ClientError, type Native } from './shared/protocol';
import { obj } from './shared/domain';

const identity = { environmentId: 'env', threadId: 'thread' };
const row = (key: string, ordinal: number, text = key) => ({ key, ordinal, payload: JSON.stringify({ id: key, text }) });
const success = (value: unknown) => ({ ok: true, generation: 1, value });

function peer() {
  const calls: Record<string, unknown>[] = [], state = { token: 'token-1' };
  const native: Native = { available: true, watch() {}, async later(raw) {
    const request = obj(raw); calls.push(request);
    switch (request.action) {
      case 'historyTicket': return success({ token: state.token });
      case 'historyCommit': return success({ committed: request.token === state.token, stale: request.token !== state.token });
      case 'historyRead': return success({ revision: 4, beforeCursor: 'before', afterCursor: null, hasMore: true, items: [row('a', 1)] });
      case 'historyClear': state.token = 'token-2'; return success({ removed: 2 });
      default: throw new Error('Unexpected history action');
    }
  } };
  return { native, calls, state };
}

test('range requests use the existing cache operation and preserve opaque boundaries', async () => {
  const f = peer(), token = await mobileThreadHistoryTicket(f.native, identity);
  expect(token).toBe('token-1');
  expect(await mobileThreadHistoryCommit(f.native, identity, token, 4, 'before', null, true, [row('a', 1)])).toBe(true);
  expect(await mobileThreadHistoryRead(f.native, identity, 'older', 8, { key: 'b', ordinal: 2 })).toEqual({ revision: 4,
    beforeCursor: 'before', afterCursor: null, hasMore: true, items: [row('a', 1)] });
  expect(f.calls.map(call => call.action)).toEqual(['historyTicket', 'historyCommit', 'historyRead']);
  expect(f.calls[1]).toMatchObject({ op: 'mobileClientCache', environmentId: 'env', threadId: 'thread', revision: 4,
    beforeCursor: 'before', afterCursor: null, hasMore: true });
  expect(f.calls[2]).toMatchObject({ direction: 'older', limit: 8, anchor: { key: 'b', ordinal: 2 } });
});

test('stale commit is a normal false result and clear advances the owner token', async () => {
  const f = peer(), token = await mobileThreadHistoryTicket(f.native, identity);
  f.state.token = 'new-token';
  expect(await mobileThreadHistoryCommit(f.native, identity, token, 1, null, null, false, [])).toBe(false);
  expect(await mobileThreadHistoryClear(f.native, identity)).toBe(2);
  expect(await mobileThreadHistoryTicket(f.native, identity)).toBe('token-2');
});

test('range validation rejects malformed items, cursors, limits and oversized commits before native', async () => {
  const f = peer(), token = await mobileThreadHistoryTicket(f.native, identity), before = f.calls.length;
  await expect(mobileThreadHistoryCommit(f.native, identity, token, 1, null, null, true, [{ ...row('x', 1), payload: '[]' }])).rejects.toBeInstanceOf(ClientError);
  await expect(mobileThreadHistoryCommit(f.native, identity, token, 1, null, null, true, [row('x', 1), row('x', 2)])).rejects.toBeInstanceOf(ClientError);
  await expect(mobileThreadHistoryCommit(f.native, identity, token, 1, null, null, true, [row('y', 2), row('x', 1)])).rejects.toBeInstanceOf(ClientError);
  await expect(mobileThreadHistoryRead(f.native, identity, 'older', 0)).rejects.toBeInstanceOf(ClientError);
  await expect(mobileThreadHistoryRead(f.native, identity, 'sideways' as never, 1)).rejects.toBeInstanceOf(ClientError);
  await expect(mobileThreadHistoryCommit(f.native, identity, token, 1, ' bad ', null, true, [])).rejects.toBeInstanceOf(ClientError);
  await expect(mobileThreadHistoryCommit(f.native, identity, token, 1, '', null, true, [])).rejects.toBeInstanceOf(ClientError);
  await expect(mobileThreadHistoryRead(f.native, { ...identity, threadId: 'bad\0thread' }, 'older', 1)).rejects.toBeInstanceOf(ClientError);
  expect(f.calls.length).toBe(before);
});
