// The client whole against the fixture over real sockets: the gateway feeding the store, REST
// pages through the limiter and the pager, sends confirmed by the gateway.
import { afterAll, afterEach, beforeEach, beforeAll, expect, test } from 'bun:test';
import { identity, restHeaders } from '../discord/client.ts';
import * as rest from '../discord/rest.ts';
import { identity as clientIdentity } from '../discord/client.ts';
import {
  addPending, applyDispatch, applyHistoryPage, channel as storeChannel, channelVersion, createStore, latestWindow,
  sidebarVersion, storeSeq, typingUsers, unread, windowAround, windowBefore, type Store,
} from '../discord/store.ts';
import { control, drive, restCall, type Drive } from './drive.ts';
import { startFixture } from './server.ts';

let fixture: ReturnType<typeof startFixture>;
let d: Drive;
let store: Store;
let applyMs = 0;
let ctx: rest.RestContext;
const nonces: rest.NonceState = { lastMs: null, sequence: 0 };
beforeAll(() => {
  // subscribeAll: these tests read full guild events, which Discord sends only for a guild the
  // client has subscribed to; the passive case below uses a fixture of its own.
  fixture = startFixture({ port: 0, heartbeatInterval: 5_000, subscribeAll: true,
    rateLimits: { messages: { limit: 3, windowMs: 600 } } });
  const id = identity('ios', { launchId: 'store-e2e' });
  ctx = { origin: fixture.origin, headers: restHeaders(id, 'fixture-token') };
});
afterAll(async () => { await fixture.close(); });
beforeEach(async () => {
  store = createStore();
  applyMs = 0;
  d = drive({
    gatewayUrl: fixture.gatewayUrl,
    onChange: change => {
      if (change.kind !== 'dispatch') return;
      const start = performance.now();
      applyDispatch(store, change.t, change.d, Date.now());
      applyMs += performance.now() - start;
    },
  });
  await d.until(c => c.some(x => x.kind === 'dispatch' && x.t === 'GUILD_CREATE'), 'login');
});
afterEach(async () => {
  d.close();
  await control(fixture.origin, 'reset');
});

const general = () => fixture.world.generalChannelId;
const ids = (rows: { id: string | null }[]) => rows.map(r => r.id);
const worldIds = () => fixture.world.messages[general()].map(m => m.id);

async function page(channelId: string, query: rest.HistoryQuery, limiter = rest.limiterInit()) {
  const reply = await restCall(limiter, rest.history(ctx, channelId, query));
  expect(reply.status).toBe(200);
  applyHistoryPage(store, channelId, { ...query, limit: 50 }, reply.body);
  return reply.body as { id: string }[];
}

test('paging back from the newest builds one run that reaches the channel start', async () => {
  const limiter = rest.limiterInit();
  let pager = rest.pagerInit();
  const channel = general();
  const started = performance.now();
  let query: rest.HistoryQuery = {};
  let requests = 0;
  for (;;) {
    const next = rest.pageRequest(pager, ctx, channel, query);
    expect(next.request).not.toBeNull();
    // A second ask for the same channel while one is in flight is refused.
    expect(rest.pageRequest(pager, ctx, channel, { before: '1' }).request).toBeNull();
    const reply = await restCall(limiter, next.request!);
    pager = rest.pageDone(pager, channel, query);
    requests++;
    applyHistoryPage(store, channel, { ...query, limit: 50 }, reply.body);
    if (reply.body.length < 50) break;
    // Newest first: the page's last row is its oldest. The store names it as the next cursor,
    // which is what the view's reachstart will ask for.
    const oldest = reply.body.at(-1).id;
    const edge = windowBefore(store, channel, oldest, 50);
    expect(edge.rows).toEqual([]);
    expect(edge.fetchBefore).toBe(oldest);
    query = { before: edge.fetchBefore! };
    if (requests > 250) throw new Error('paging did not reach the start');
  }
  const elapsed = performance.now() - started;
  const all = worldIds();
  console.log(`paged ${all.length} messages in ${requests} requests, ${elapsed.toFixed(0)} ms`);
  expect(requests).toBe(Math.ceil(all.length / 50) + (all.length % 50 === 0 ? 1 : 0));
  const latest = latestWindow(store, channel, 200);
  expect(ids(latest.rows)).toEqual(all.slice(-200));
  expect(latest.reachesNewest).toBe(true);
  const middle = windowAround(store, channel, all[5000], 101);
  expect(ids(middle.rows)).toEqual(all.slice(4950, 5051));
  const start = windowBefore(store, channel, all[10], 200);
  expect(ids(start.rows)).toEqual(all.slice(0, 10));
  expect(start.reachesStart).toBe(true);
});

test('a jump far back is its own run; paging forward from it meets the live tail and merges', async () => {
  const channel = general();
  const all = worldIds();
  await page(channel, {});
  await page(channel, { around: all[3000] });
  const jump = windowAround(store, channel, all[3000], 51);
  expect(ids(jump.rows)).toContain(all[3000]);
  expect(jump.reachesNewest).toBe(false);
  expect(jump.moreAfter).toBe(true);
  // The live tail does not see the far run.
  expect(latestWindow(store, channel, 200).rows.length).toBe(50);
  let cursor = jump.rows.at(-1)!.id!;
  let pages = 0;
  for (;;) {
    const rows = await page(channel, { after: cursor });
    pages++;
    const w = windowAround(store, channel, all[3000], 51);
    if (w.reachesNewest || latestWindow(store, channel, 200).rows.length === 200) break;
    cursor = rows.reduce((a, r) => (r.id.length > a.length || (r.id.length === a.length && r.id > a) ? r.id : a), cursor);
    if (pages > 200) throw new Error('never met the tail');
  }
  // Merged: a window around the jump target and the latest window are one run.
  const tail = latestWindow(store, channel, 200);
  expect(ids(tail.rows)).toEqual(all.slice(-200));
  const back = windowBefore(store, channel, all.at(-1)!, 200);
  expect(back.rows.length).toBe(200);
  expect(ids(windowAround(store, channel, all[3000], 21).rows)).toEqual(all.slice(2990, 3011));
});

test('a reconnect that must identify keeps the cached history; the next latest page joins it', async () => {
  const channel = general();
  const all = worldIds();
  await page(channel, {});
  await page(channel, { before: all.at(-50)! });
  expect(latestWindow(store, channel, 200).rows.length).toBe(100);
  const readies = d.dispatches('READY').length;
  await control(fixture.origin, 'close', { code: 4009 });
  await d.until(c => c.some(x => x.kind === 'status' && x.phase === 'backoff'), 'backoff');
  // Sent while no session exists: a fresh identify does not replay these.
  const { ids: missed } = await control(fixture.origin, 'burst', { channel_id: channel, count: 3 });
  await d.until(() => d.dispatches('READY').length > readies, 'the second READY', 15_000);
  await d.until(c => c.filter(x => x.kind === 'dispatch' && x.t === 'GUILD_CREATE').length >= 2, 'the lazy guild again');
  const stale = latestWindow(store, channel, 200);
  expect(stale.rows).toEqual([]);
  expect(stale.fetchLatest).toBe(true);
  // The rows are still cached in their run.
  expect(ids(windowAround(store, channel, all.at(-60)!, 11).rows)).toEqual(all.slice(-65, -54));
  await page(channel, {});
  const joined = latestWindow(store, channel, 200);
  expect(ids(joined.rows)).toEqual([...all, ...missed].slice(-103));
  expect(joined.reachesNewest).toBe(true);
});

test('a send shows at once and the gateway confirmation replaces it in place', async () => {
  const channel = general();
  await page(channel, {});
  const nonce = rest.makeNonce(Date.now(), nonces);
  addPending(store, channel, { nonce, content: 'hello from the test', nowMs: Date.now() });
  const before = latestWindow(store, channel, 200);
  expect(before.rows.at(-1)).toMatchObject({ key: nonce, status: 'pending', id: null });
  const index = before.rows.length - 1;
  const reply = await restCall(rest.limiterInit(), rest.sendMessage(ctx, channel, { content: 'hello from the test', nonce }));
  expect(reply.status).toBe(200);
  await d.until(c => c.some(x => x.kind === 'dispatch' && x.t === 'MESSAGE_CREATE' && x.d.nonce === nonce), 'confirmation');
  const after = latestWindow(store, channel, 200);
  expect(after.rows.length).toBe(before.rows.length);
  expect(after.rows[index]).toMatchObject({ key: nonce, status: 'sent', id: reply.body.id, content: 'hello from the test' });
  expect(after.rows.filter(r => r.key === nonce)).toHaveLength(1);
  expect(unread(store, channel).unread).toBe(false);
});

test('edits, deletes, reactions, typing and acks reach the store; other channels keep their version', async () => {
  const channel = general();
  const other = fixture.world.guilds[0].channels.find(c => c.type === 0 && c.id !== channel)!.id;
  await page(channel, {});
  const rows = latestWindow(store, channel, 50).rows;
  const otherVersion = channelVersion(store, other);
  const sidebar = sidebarVersion(store);
  const [a, b, c] = [rows[10].id!, rows[20].id!, rows[30].id!];
  const n = d.dispatches().length;
  await control(fixture.origin, 'edit', { channel_id: channel, message_id: a, content: 'edited by the fixture' });
  await control(fixture.origin, 'delete', { channel_id: channel, message_id: b });
  await control(fixture.origin, 'react', { channel_id: channel, message_id: c, emoji: '🎉', user_id: fixture.world.users[3].id, add: true });
  await control(fixture.origin, 'typing', { channel_id: channel, user_id: fixture.world.users[4].id });
  await d.until(() => d.dispatches().length >= n + 4, 'four dispatches');
  const now = latestWindow(store, channel, 50).rows;
  expect(now.find(r => r.id === a)!.content).toBe('edited by the fixture');
  expect(now.some(r => r.id === b)).toBe(false);
  expect(now.find(r => r.id === c)!.reactions.find((r: any) => r.emoji.name === '🎉').count).toBe(1);
  expect(typingUsers(store, channel, Date.now()).map(t => t.user.id)).toEqual([fixture.world.users[4].id]);
  expect(channelVersion(store, other)).toBe(otherVersion);
  expect(sidebarVersion(store)).toBe(sidebar);
  const last = now.at(-1)!.id!;
  await restCall(rest.limiterInit(), rest.ack(ctx, channel, last, { token: null, lastViewed: 0 }));
  await d.until(c2 => c2.some(x => x.kind === 'dispatch' && x.t === 'MESSAGE_ACK'), 'MESSAGE_ACK');
  expect(unread(store, channel)).toMatchObject({ unread: false, mentionCount: 0, lastReadId: last });
});

test('the limiter spaces sends inside the bucket and honours a forced 429', async () => {
  const channel = general();
  const limiter = rest.limiterInit();
  const statuses: number[] = [];
  const started = Date.now();
  for (let i = 0; i < 4; i++) {
    const r = await restCall(limiter, rest.sendMessage(ctx, channel, { content: `spaced ${i}`, nonce: rest.makeNonce(Date.now(), nonces) }));
    statuses.push(r.status);
  }
  // Three per 600 ms: the fourth waited for the reset rather than drawing a 429.
  expect(statuses).toEqual([200, 200, 200, 200]);
  expect(Date.now() - started).toBeGreaterThanOrEqual(400);
  await control(fixture.origin, 'rate-limit', { route: `/channels/${channel}/typing`, count: 1, retry_after: 0.25 });
  const typingLimiter = rest.limiterInit();
  const first = await restCall(typingLimiter, rest.typing(ctx, channel));
  expect(first.status).toBe(429);
  expect(first.retryAfterMs).toBe(250);
  const blocked = rest.admit(typingLimiter, rest.typing(ctx, channel), Date.now());
  expect(blocked.ok).toBe(false);
  const retried = Date.now();
  const second = await restCall(typingLimiter, rest.typing(ctx, channel));
  expect(second.status).toBe(204);
  expect(Date.now() - retried).toBeGreaterThanOrEqual(150);
});

test('a burst of 1,000 MESSAGE_CREATEs over the gateway lands in the store in order', async () => {
  const channel = general();
  await page(channel, {});
  const n = d.dispatches('MESSAGE_CREATE').length;
  applyMs = 0;
  const started = performance.now();
  const { ids: burst } = await control(fixture.origin, 'burst', { channel_id: channel, count: 1000 });
  await d.until(() => d.dispatches('MESSAGE_CREATE').length >= n + 1000, 'the burst', 20_000);
  const elapsed = performance.now() - started;
  console.log(`1,000 MESSAGE_CREATEs over the gateway: ${elapsed.toFixed(0)} ms end to end, ${applyMs.toFixed(1)} ms in the store`);
  expect(applyMs).toBeLessThan(500);
  const tail = latestWindow(store, channel, 200);
  expect(ids(tail.rows)).toEqual(burst.slice(-200));
  expect(tail.reachesNewest).toBe(true);
  // The burst joined the run the latest page made: no gap between the old tail and the burst.
  expect(ids(windowBefore(store, channel, burst[0], 50).rows)).toEqual(worldIds().slice(-1050, -1000));
});

test('a late page keeps the edit and reaction the gateway delivered while it was in flight', async () => {
  const channel = general();
  await page(channel, {});
  const since = storeSeq(store);
  const reply = await restCall(rest.limiterInit(), rest.history(ctx, channel, {}));
  // The page is in hand but not yet applied: the gateway moves on meanwhile.
  const target = reply.body[5].id;
  const n = d.dispatches().length;
  await control(fixture.origin, 'edit', { channel_id: channel, message_id: target, content: 'edited in flight' });
  await control(fixture.origin, 'react', { channel_id: channel, message_id: target, emoji: '🧪', user_id: fixture.world.users[3].id, add: true });
  await d.until(() => d.dispatches().length >= n + 2, 'edit and reaction');
  applyHistoryPage(store, channel, { limit: 50 }, reply.body, { since });
  const row = latestWindow(store, channel, 50).rows.find(r => r.id === target)!;
  expect(row.content).toBe('edited in flight');
  expect(row.reactions.some((r: any) => r.emoji.name === '🧪')).toBe(true);
});

test('reactions from several people arrive as one MESSAGE_REACTION_ADD_MANY and count each', async () => {
  const channel = general();
  await page(channel, {});
  const target = latestWindow(store, channel, 50).rows[3].id!;
  const users = [3, 4, 5].map(i => fixture.world.users[i].id);
  await control(fixture.origin, 'react-many', { channel_id: channel, message_id: target, emoji: '🪴', user_ids: users });
  await d.until(c => c.some(x => x.kind === 'dispatch' && x.t === 'MESSAGE_REACTION_ADD_MANY'), 'the batch');
  const row = latestWindow(store, channel, 50).rows.find(r => r.id === target)!;
  expect(row.reactions.find((r: any) => r.emoji.name === '🪴')).toMatchObject({ count: 3, me: false });
});

test('a repeated nonce returns the first message within five minutes and a new one after', async () => {
  const channel = general();
  const nonce = rest.makeNonce(Date.now(), nonces);
  const limiter = rest.limiterInit();
  const send = () => restCall(limiter, rest.sendMessage(ctx, channel, { content: 'once', nonce }));
  const first = await send();
  const again = await send();
  expect(again.body.id).toBe(first.body.id);
  await control(fixture.origin, 'clock', { advance_ms: 6 * 60_000 });
  const later = await send();
  expect(later.status).toBe(200);
  expect(later.body.id).not.toBe(first.body.id);
});

test('an unsubscribed guild sends passive updates, V2 to every host the client identifies as; the sidebar learns of them', async () => {
  const passive = startFixture({ port: 0, heartbeatInterval: 5_000 });
  const drives: Drive[] = [];
  try {
    for (const [host, version] of [['ios', 'V2'], ['web', 'V2']] as const) {
      const own = createStore();
      const pd = drive({ gatewayUrl: passive.gatewayUrl, host, onChange: change => {
        if (change.kind === 'dispatch') applyDispatch(own, change.t, change.d, Date.now());
      } });
      drives.push(pd);
      await pd.until(c => c.some(x => x.kind === 'dispatch' && x.t === 'GUILD_CREATE'), `${host} login`);
      expect(pd.sent[0].d.capabilities).toBe(clientIdentity(host, { launchId: 'x' }).capabilities);
      const channel = passive.world.guilds[1].channels.find(c => c.type === 0)!.id;
      const before = sidebarVersion(own);
      const { ids: sent } = await control(passive.origin, 'burst', { channel_id: channel, count: 1 });
      await pd.until(c => c.some(x => x.kind === 'dispatch' && x.t === `PASSIVE_UPDATE_${version}`), `PASSIVE_UPDATE_${version}`);
      expect(pd.dispatches('MESSAGE_CREATE')).toHaveLength(0);
      expect(storeChannel(own, channel)!.last_message_id).toBe(sent[0]);
      expect(unread(own, channel).unread).toBe(true);
      expect(sidebarVersion(own)).toBeGreaterThan(before);
      pd.close();
      await control(passive.origin, 'reset');
    }
  } finally {
    for (const pd of drives) pd.close();
    await passive.close();
  }
});
