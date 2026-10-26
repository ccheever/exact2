import { describe, expect, test } from 'bun:test';
import {
  ack, addReaction, admit, deleteMessage, editMessage, gateway, history, limiterInit, makeNonce,
  me, onResponse, pageDone, pageRequest, pagerInit, removeReaction, routeKey, sendMessage, typing,
  type Limiter, type RestRequest,
} from './rest.ts';
import { cmpId, msFromSnowflake, snowflakeFromMs, type NonceState } from './ids.ts';

const ctx = { origin: 'http://127.0.0.1:8767/', headers: { Authorization: 'fixture-token' } };
const base = 'http://127.0.0.1:8767/api/v9';
const headers = (values: Record<string, string>) => (name: string) =>
  Object.entries(values).find(([key]) => key.toLowerCase() === name.toLowerCase())?.[1] ?? null;
function response(l: Limiter, req: RestRequest, values: Record<string, string>, nowMs = 0, status = 200, body: unknown = {}) {
  return onResponse(l, req, status, headers(values), body, nowMs);
}
function quota(bucket: string, remaining: number, after = '1', limit = '2') {
  return { 'X-RateLimit-Bucket': bucket, 'X-RateLimit-Limit': limit,
    'X-RateLimit-Remaining': String(remaining), 'X-RateLimit-Reset-After': after };
}

describe('snowflakes', () => {
  test('comparison keeps the low bits and compares different decimal lengths', () => {
    expect(cmpId('9', '10')).toBeLessThan(0);
    expect(cmpId('999999999999999998', '999999999999999999')).toBeLessThan(0);
    expect(cmpId('999999999999999999', '999999999999999999')).toBe(0);
    expect(cmpId('1000000000000000000', '999999999999999999')).toBeGreaterThan(0);
  });
  test('timestamp extraction is exact at a low-bit boundary and over seeded times', () => {
    let seed = 0x1234abcd;
    for (let i = 0; i < 500; i++) {
      seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
      const ms = 1420070400000 + seed * 1000;
      const first = snowflakeFromMs(ms);
      expect(msFromSnowflake(first)).toBe(ms);
      expect(msFromSnowflake((BigInt(first) + 4194303n).toString())).toBe(ms);
      expect(makeNonce(ms, { lastMs: null, sequence: 0 })).toBe(first);
    }
    expect(snowflakeFromMs(1420070400000)).toBe('0');
    expect(() => snowflakeFromMs(0)).toThrow();
    expect(() => msFromSnowflake('nope')).toThrow();
  });
  test('1,000 sends in one millisecond have distinct increasing nonces', () => {
    const state: NonceState = { lastMs: null, sequence: 0 };
    const nowMs = 1792627200000;
    const nonces = Array.from({ length: 1000 }, () => makeNonce(nowMs, state));
    expect(new Set(nonces).size).toBe(1000);
    for (let i = 0; i < nonces.length; i++) {
      expect(msFromSnowflake(nonces[i])).toBe(nowMs);
      expect(BigInt(nonces[i]) - BigInt(snowflakeFromMs(nowMs))).toBe(BigInt(i));
      if (i) expect(cmpId(nonces[i - 1], nonces[i])).toBeLessThan(0);
    }
  });
  test('nonces stay increasing across clock rollback and sequence overflow', () => {
    const nowMs = 1792627200000;
    const state: NonceState = { lastMs: null, sequence: 0 };
    let previous = makeNonce(nowMs, state);
    for (const time of [nowMs - 1000, nowMs - 500, nowMs, nowMs + 1]) {
      const nonce = makeNonce(time, state);
      expect(cmpId(nonce, previous)).toBeGreaterThan(0);
      previous = nonce;
    }
    const full: NonceState = { lastMs: nowMs, sequence: 4194303 };
    const overflow = makeNonce(nowMs - 1, full);
    expect(overflow).toBe(snowflakeFromMs(nowMs + 1));
    expect(cmpId(makeNonce(nowMs, full), overflow)).toBeGreaterThan(0);
    const beforeInvalid = { ...state };
    for (const time of [0, NaN, Infinity, nowMs + 0.5]) {
      expect(() => makeNonce(time, state)).toThrow();
      expect(state).toEqual(beforeInvalid);
    }
  });
  test('a retry reuses its pending send nonce while a new send gets another', () => {
    const state: NonceState = { lastMs: null, sequence: 0 };
    const nowMs = 1792627200000;
    const pending = { content: 'hello', nonce: makeNonce(nowMs, state) };
    const first = sendMessage(ctx, '123', pending);
    const next = sendMessage(ctx, '123', { content: 'next', nonce: makeNonce(nowMs, state) });
    const retry = sendMessage(ctx, '123', pending);
    expect(retry.body).toBe(first.body);
    expect(JSON.parse(next.body!).nonce).not.toBe(pending.nonce);
  });
});

describe('request builders', () => {
  test('history uses v9, one encoded cursor and no more than 50 messages', () => {
    expect(history(ctx, '123')).toEqual({ method: 'GET', url: `${base}/channels/123/messages?limit=50`,
      headers: ctx.headers, route: 'GET /channels/123/messages' });
    for (const cursor of ['before', 'after', 'around']) {
      expect(history(ctx, '123', { [cursor]: '456' }, 25).url).toBe(`${base}/channels/123/messages?limit=25&${cursor}=456`);
    }
    expect(history(ctx, '123', { before: '4 5', limit: 2 }).url).toEndWith('limit=2&before=4%205');
    expect(() => history(ctx, '123', {}, 51)).toThrow();
    expect(() => history(ctx, '123', { before: '1', after: '2' })).toThrow();
  });
  test('send supplies the client defaults and optional reply reference', () => {
    const req = sendMessage(ctx, '123', { content: 'hello', nonce: '456' });
    expect(req.method).toBe('POST');
    expect(req.url).toBe(`${base}/channels/123/messages`);
    expect(JSON.parse(req.body!)).toEqual({ content: 'hello', nonce: '456', tts: false, flags: 0, enforce_nonce: true });
    expect(req.headers['Content-Type']).toBe('application/json');
    expect(ctx.headers).toEqual({ Authorization: 'fixture-token' });
    const reply = sendMessage(ctx, '123', { content: 'reply', nonce: '789', replyTo: { messageId: '456', guildId: '99' } });
    expect(JSON.parse(reply.body!).message_reference).toEqual({ message_id: '456', channel_id: '123', guild_id: '99' });
    expect(JSON.parse(sendMessage(ctx, '123', { content: '', nonce: '8', replyTo: { messageId: '7', channelId: '6' } }).body!)
      .message_reference).toEqual({ message_id: '7', channel_id: '6' });
    // A reply pings its original's author unless told not to, as Discord's @ OFF sends it.
    expect(JSON.parse(reply.body!)).not.toHaveProperty('allowed_mentions');
    expect(JSON.parse(sendMessage(ctx, '123', { content: 'quiet', nonce: '9', replyTo: { messageId: '456', mention: false } }).body!)
      .allowed_mentions).toEqual({ parse: ['users', 'roles', 'everyone'], replied_user: false });
  });
  test('edit, delete, ack, typing, gateway and me have exact methods and bodies', () => {
    const cases: [RestRequest, string, string, unknown][] = [
      [editMessage(ctx, '1', '2', 'edited'), 'PATCH', '/channels/1/messages/2', { content: 'edited' }],
      [deleteMessage(ctx, '1', '2'), 'DELETE', '/channels/1/messages/2', undefined],
      [ack(ctx, '1', '2', { token: null, lastViewed: 20000 }), 'POST', '/channels/1/messages/2/ack', { token: null, last_viewed: 20000 }],
      [typing(ctx, '1'), 'POST', '/channels/1/typing', undefined],
      [gateway(ctx), 'GET', '/gateway', undefined],
      [me(ctx), 'GET', '/users/@me', undefined],
    ];
    for (const [req, method, path, body] of cases) {
      expect(req.method).toBe(method);
      expect(req.url).toBe(base + path);
      expect(req.body === undefined ? undefined : JSON.parse(req.body)).toEqual(body);
      expect(req.headers.Authorization).toBe('fixture-token');
    }
  });
  test('Unicode and custom reactions are encoded as a whole path segment', () => {
    for (const emoji of ['👍🏽', 'party:123', 'a/b?#']) {
      for (const [builder, method] of [[addReaction, 'PUT'], [removeReaction, 'DELETE']] as const) {
        const req = builder(ctx, '1', '2', emoji);
        expect(req.method).toBe(method);
        expect(req.url).toBe(`${base}/channels/1/messages/2/reactions/${encodeURIComponent(emoji)}/@me`);
        expect(req.body).toBeUndefined();
      }
    }
  });
  test('route keys retain major parameters and normalize other ids, queries and emoji', () => {
    expect(routeKey('get', '/api/v9/channels/123/messages/456?x=7')).toBe('GET /channels/123/messages/:id');
    expect(routeKey('GET', '/guilds/123/members/456')).toBe('GET /guilds/123/members/:id');
    expect(routeKey('POST', '/webhooks/123/secret-token/messages/456')).toBe('POST /webhooks/123/secret-token/messages/:id');
    expect(routeKey('DELETE', '/channels/123/messages/456/reactions/party%3A999/@me'))
      .toBe('DELETE /channels/123/messages/:id/reactions/:emoji/:user');
    expect(routeKey('DELETE', '/channels/123/messages/789/reactions/x/999'))
      .toBe(routeKey('DELETE', '/channels/123/messages/456/reactions/y/@me'));
    expect(routeKey('GET', '/channels/124/messages')).not.toBe(routeKey('GET', '/channels/123/messages'));
    expect(routeKey('POST', '/webhooks/123/another-token')).not.toBe(routeKey('POST', '/webhooks/123/secret-token'));
  });
});

describe('limiter', () => {
  test('unknown routes admit one in flight, released even when a fetch fails', () => {
    const l = limiterInit(), req = history(ctx, '1'), other = history(ctx, '2');
    expect(admit(l, req, 0)).toEqual({ ok: true });
    expect(admit(l, history(ctx, '1', { before: '9' }), 0)).toEqual({ ok: false, waitMs: Infinity });
    expect(admit(l, other, 0)).toEqual({ ok: true });
    response(l, req, {}, 0, 0);
    expect(admit(l, req, 0)).toEqual({ ok: true });
  });
  test('remaining zero waits until reset and Reset-After wins over absolute clock skew', () => {
    const l = limiterInit(), req = history(ctx, '1');
    admit(l, req, 100);
    response(l, req, { ...quota('a', 0, '1.25'), 'X-RateLimit-Reset': '999999' }, 100);
    expect(admit(l, req, 101)).toEqual({ ok: false, waitMs: 1249 });
    expect(admit(l, req, 1349)).toEqual({ ok: false, waitMs: 1 });
    expect(admit(l, req, 1350)).toEqual({ ok: true });
    expect(admit(l, req, 1350)).toEqual({ ok: true });
    expect(admit(l, req, 1350)).toEqual({ ok: false, waitMs: Infinity });
  });
  test('absolute Reset is used when Reset-After is absent', () => {
    const l = limiterInit(), req = history(ctx, '1');
    admit(l, req, 1000);
    response(l, req, { 'X-RateLimit-Limit': '1', 'X-RateLimit-Remaining': '0', 'X-RateLimit-Reset': '2.5' }, 1000);
    expect(admit(l, req, 2499)).toEqual({ ok: false, waitMs: 1 });
    expect(admit(l, req, 2500)).toEqual({ ok: true });
  });
  test('two routes share their discovered bucket but different major parameters do not', () => {
    const l = limiterInit(), read = history(ctx, '1'), send = typing(ctx, '1'), other = history(ctx, '2');
    admit(l, read, 0); response(l, read, quota('shared', 1));
    admit(l, send, 0); response(l, send, quota('shared', 0));
    expect(admit(l, read, 0)).toEqual({ ok: false, waitMs: 1000 });
    expect(admit(l, send, 0)).toEqual({ ok: false, waitMs: 1000 });
    expect(admit(l, other, 0)).toEqual({ ok: true });
    response(l, other, quota('shared', 1));
    expect(admit(l, other, 0)).toEqual({ ok: true });
  });
  test('outstanding reservations survive discovery and stale remaining headers cannot reopen the bucket', () => {
    const l = limiterInit(), a = history(ctx, '1'), b = typing(ctx, '1');
    admit(l, a, 0); response(l, a, quota('shared', 2, '1', '3'));
    admit(l, b, 0); response(l, b, quota('shared', 2, '1', '3'));
    expect(admit(l, a, 0)).toEqual({ ok: true });
    expect(admit(l, b, 0)).toEqual({ ok: true });
    response(l, b, quota('shared', 0, '1', '3'));
    response(l, a, quota('shared', 1, '1', '3'));
    expect(admit(l, a, 0)).toEqual({ ok: false, waitMs: 1000 });
  });
  test('a fractional retry_after blocks its bucket, including another mapped route', () => {
    const l = limiterInit(), a = history(ctx, '1'), b = typing(ctx, '1');
    admit(l, a, 0); response(l, a, quota('a', 1));
    admit(l, b, 0); response(l, b, quota('a', 1));
    admit(l, a, 0);
    expect(response(l, a, { 'X-RateLimit-Scope': 'shared' }, 10, 429, { retry_after: 1.75, global: false }))
      .toEqual({ retryAfterMs: 1750, global: false });
    expect(admit(l, b, 1000)).toEqual({ ok: false, waitMs: 760 });
    expect(admit(l, b, 1760)).toEqual({ ok: true });
  });
  test('global 429 blocks all routes through either the body or the headers', () => {
    for (const globalHeaders of [false, true]) {
      const l = limiterInit(), a = history(ctx, '1'), b = me(ctx);
      admit(l, a, 0);
      response(l, a, globalHeaders ? { 'X-RateLimit-Global': 'true', 'Retry-After': '2.25' } : {}, 100, 429,
        globalHeaders ? {} : { global: true, retry_after: 2.25 });
      expect(admit(l, b, 2349)).toEqual({ ok: false, waitMs: 1 });
      expect(admit(l, b, 2350)).toEqual({ ok: true });
    }
  });
});

test('pager serializes each channel, refuses duplicate queries and frees successful or failed requests', () => {
  const p = pagerInit();
  expect(pageRequest(p, ctx, '1', { before: '9' }).request).not.toBeNull();
  expect(pageRequest(p, ctx, '1', { before: '9', limit: 50 })).toMatchObject({ request: null, reason: 'duplicate-query' });
  expect(pageRequest(p, ctx, '1', { before: '8' })).toMatchObject({ request: null, reason: 'channel-busy' });
  expect(pageRequest(p, ctx, '2', {}).request).not.toBeNull();
  pageDone(p, '1', { before: '8' });
  expect(pageRequest(p, ctx, '1', { before: '9' }).request).toBeNull();
  pageDone(p, '1', { before: '9' });
  expect(pageRequest(p, ctx, '1', { before: '9' }).request).not.toBeNull();
  pageDone(p, '2');
  expect(pageRequest(p, ctx, '2', { after: '10' }).request).not.toBeNull();
});
