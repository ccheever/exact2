import { describe, expect, test } from 'bun:test';
import {
  addPending, applyDispatch, applyHistoryPage, channel, channelVersion, createStore, dms, expireTyping,
  failPending, firstUnread, guildChannels, guilds, latestWindow, latestEditable, removePending, sidebarVersion, storeSeq, typingUsers, unread,
  user, windowAfter, windowAround, windowBefore, type DiscordObject, type MessageWindow, type Store,
} from './store.ts';
import { snowflakeFromMs } from './ids.ts';

const me = { id: '1', username: 'me' };
const other = { id: '2', username: 'friend' };
const cid = '1000';
function msg(id: number | string, extra: DiscordObject = {}): DiscordObject {
  return { id: String(id), channel_id: cid, guild_id: '10', author: other, content: `message ${id}`,
    timestamp: '2026-10-22T12:00:00.000Z', attachments: [], embeds: [], mentions: [], ...extra };
}
function page(start: number, end: number): DiscordObject[] {
  return Array.from({ length: end - start + 1 }, (_, i) => msg(end - i));
}
function ids(window: MessageWindow): (string | null)[] { return window.rows.map(row => row.id); }
function keys(s: Store): string[] { return latestWindow(s, cid).rows.map(row => row.key); }
function seeded(lastId = '100'): Store {
  const s = createStore();
  applyDispatch(s, 'READY', { user: me, users: [other], guilds: [{ id: '10', name: 'server', channels: [
    { id: cid, type: 0, last_message_id: lastId }, { id: '2000', type: 0, last_message_id: '80' },
  ] }], read_state: [{ id: cid, last_message_id: lastId, mention_count: 0 }] }, 0);
  return s;
}
function send(s: Store, id: number | string, extra: DiscordObject = {}) {
  return applyDispatch(s, 'MESSAGE_CREATE', msg(id, extra), 100);
}
function event(s: Store, type: string, data: DiscordObject) {
  return applyDispatch(s, type, { channel_id: cid, ...data }, 100);
}
function readyAgain(s: Store, lastId: string | null) {
  return applyDispatch(s, 'READY', { user: me, guilds: [{ id: '10', name: 'reconnected', channels: [
    { id: cid, type: 0, last_message_id: lastId },
  ] }], read_state: [{ id: cid, last_message_id: '99', mention_count: 2 }] }, 200);
}

describe('last own message for editing', () => {
  test('skips pending sends, other authors, forwards, system messages and webhooks; accepts replies', () => {
    const s = seeded('7');
    applyHistoryPage(s, cid, { limit: 50 }, [msg(7, { author: me, message_snapshots: [{ message: { content: 'forwarded' } }] }),
      msg(6), msg(5, { author: me, webhook_id: 'hook' }),
      msg(4, { author: me, type: 6 }), msg(3, { author: me, type: 19, content: '**reply**' }),
      msg(2, { author: me }), msg(1)]);
    addPending(s, cid, { nonce: 'pending', content: 'not sent', nowMs: Date.now() });
    expect(latestEditable(s, cid).row).toMatchObject({ id: '3', content: '**reply**' });
    failPending(s, cid, 'pending', 'offline');
    expect(latestEditable(s, cid).row?.id).toBe('3');
    event(s, 'MESSAGE_DELETE', { id: '3' });
    expect(latestEditable(s, cid).row?.id).toBe('2');
  });

  test('requests gaps from the live tail instead of choosing a disconnected older own message', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { around: '20', limit: 3 }, [msg(21), msg(20, { author: me }), msg(19)]);
    expect(latestEditable(s, cid)).toEqual({ row: null, need: 'latest', cursor: '' });
    applyHistoryPage(s, cid, { limit: 5 }, page(96, 100));
    expect(latestEditable(s, cid)).toEqual({ row: null, need: 'before', cursor: '96' });
    applyHistoryPage(s, cid, { before: '96', limit: 5 }, [msg(95), msg(94, { author: me }), ...page(91, 93)]);
    expect(latestEditable(s, cid).row?.id).toBe('94');
    readyAgain(s, '103');
    expect(latestEditable(s, cid)).toEqual({ row: null, need: 'latest', cursor: '' });
    applyHistoryPage(s, cid, { limit: 5 }, page(99, 103));
    expect(latestEditable(s, cid)).toEqual({ row: null, need: 'before', cursor: '99' });
  });

  test('looks beyond a display window and stops at a confirmed beginning with no own message', () => {
    const s = seeded('299');
    applyHistoryPage(s, cid, { limit: 50 }, page(250, 299));
    for (let end = 249; end >= 99; end -= 50) {
      applyHistoryPage(s, cid, { before: String(end + 1), limit: 50 }, page(end - 49, end));
    }
    applyHistoryPage(s, cid, { before: '50', limit: 50 }, [...page(2, 49), msg(1, { author: me })]);
    expect(latestWindow(s, cid, 200).rows.every(r => r.author.id !== me.id)).toBe(true);
    expect(latestEditable(s, cid).row?.id).toBe('1');
    event(s, 'MESSAGE_DELETE', { id: '1' });
    expect(latestEditable(s, cid)).toEqual({ row: null, need: '', cursor: '' });
    const empty = seeded();
    applyHistoryPage(empty, cid, { limit: 50 }, [], { since: storeSeq(empty) });
    expect(latestEditable(empty, cid)).toEqual({ row: null, need: '', cursor: '' });
  });
});

describe('READY and sidebar', () => {
  test('a read state of the number 0 is read as the id "0": nothing read', () => {
    const s = createStore();
    applyDispatch(s, 'READY', { user: me, guilds: [{ id: '10', name: 'server', channels: [{ id: cid, type: 0, last_message_id: '100' }] }],
      read_state: [{ id: cid, last_message_id: 0, mention_count: 0 }] }, 0);
    expect(unread(s, cid)).toEqual({ unread: true, mentionCount: 0, lastReadId: '0' });
  });
  for (const wrapped of [false, true]) test(`READY accepts ${wrapped ? 'properties, recipient_ids and read-state entries' : 'flat guilds, recipients and read-state arrays'}`, () => {
    const s = createStore();
    const properties = { name: 'guild', icon: 'icon' };
    applyDispatch(s, 'READY', {
      user: me, users: wrapped ? [other] : [],
      guilds: [{ id: '10', ...(wrapped ? { properties } : properties),
        channels: [{ id: cid, type: 0, name: 'chat', last_message_id: '100' }] }],
      private_channels: [{ id: '20', type: 1, last_message_id: '90',
        ...(wrapped ? { recipient_ids: ['2'] } : { recipients: [other] }) }],
      read_state: wrapped ? { entries: [{ id: cid, last_message_id: '99', mention_count: 3 }], partial: false, version: 1 }
        : [{ id: cid, last_message_id: '99', mention_count: 3 }],
    }, 0);
    expect(s.meId).toBe('1');
    expect(guilds(s)).toEqual([{ id: '10', name: 'guild', icon: 'icon' }]);
    expect(channel(s, cid)?.guild_id).toBe('10');
    expect(dms(s)[0].recipients).toEqual([other]);
    expect(user(s, '2')).toEqual(other);
    expect(unread(s, cid)).toEqual({ unread: true, mentionCount: 3, lastReadId: '99' });
    expect(event(s, 'MESSAGE_ACK', { message_id: '100' })).toEqual({ channels: [cid], sidebar: true });
    expect(unread(s, cid)).toEqual({ unread: false, mentionCount: 0, lastReadId: '100' });
  });
  test('an unavailable guild is retained and filled by GUILD_CREATE', () => {
    const s = createStore();
    applyDispatch(s, 'READY', { user: me, guilds: [{ id: '10', unavailable: true }] }, 0);
    expect(guilds(s)).toEqual([{ id: '10', unavailable: true }]);
    applyDispatch(s, 'GUILD_CREATE', { id: '10', properties: { name: 'back' }, channels: [{ id: cid, type: 0 }] }, 0);
    expect(guilds(s)).toEqual([{ id: '10', unavailable: false, name: 'back' }]);
    expect(guildChannels(s, '10')[0].channels[0].id).toBe(cid);
    applyDispatch(s, 'GUILD_DELETE', { id: '10', unavailable: true }, 0);
    expect(channel(s, cid)).not.toBeNull();
    applyDispatch(s, 'GUILD_DELETE', { id: '10' }, 0);
    expect(guilds(s)).toEqual([]);
    expect(channel(s, cid)).toBeNull();
  });
  test('categories and children use position then snowflake order, with uncategorized first', () => {
    const s = createStore();
    applyDispatch(s, 'GUILD_CREATE', { id: '10', channels: [
      { id: '12', type: 4, position: 2 }, { id: '9', type: 4, position: 2 },
      { id: '7', type: 4, position: 1 }, { id: '40', type: 0, position: 0 },
      { id: '20', type: 0, position: 1, parent_id: '7' },
      { id: '19', type: 5, position: 1, parent_id: '7' },
      { id: '18', type: 0, position: 0, parent_id: '7' },
      { id: '41', type: 0, parent_id: 'missing' },
    ] }, 0);
    expect(guildChannels(s, '10').map(group => [group.category?.id ?? null, group.channels.map(c => c.id)]))
      .toEqual([[null, ['40', '41']], ['7', ['18', '19', '20']], ['9', []], ['12', []]]);
    applyDispatch(s, 'CHANNEL_UPDATE', { id: '20', parent_id: '12' }, 0);
    expect(guildChannels(s, '10').at(-1)?.channels[0].id).toBe('20');
    expect(channel(s, '20')?.type).toBe(0);
    applyDispatch(s, 'CHANNEL_DELETE', { id: '20' }, 0);
    expect(channel(s, '20')).toBeNull();
  });
  test('DMs sort by the full last-message id and update after live traffic', () => {
    const s = createStore({ meId: '1' });
    for (const [id, last] of [['20', '999999999999999999'], ['21', '1000000000000000000'], ['22', null]]) {
      applyDispatch(s, 'CHANNEL_CREATE', { id, type: 1, recipients: [other], last_message_id: last }, 0);
    }
    expect(dms(s).map(c => c.id)).toEqual(['21', '20', '22']);
    send(s, '1000000000000000001', { channel_id: '22', guild_id: undefined });
    expect(dms(s).map(c => c.id)).toEqual(['22', '21', '20']);
  });
  test('supplemental READY augments users and guilds without clearing cached history', () => {
    const s = seeded();
    send(s, '101');
    applyDispatch(s, 'READY_SUPPLEMENTAL', { guilds: [{ id: '10', properties: { icon: 'new' } }],
      merged_members: [[{ user: { id: '2', global_name: 'Friendly' } }]] }, 0);
    expect(guilds(s)[0].name).toBe('server');
    expect(user(s, '2')?.global_name).toBe('Friendly');
    expect(ids(latestWindow(s, cid))).toEqual(['101']);
    applyDispatch(s, 'READY', { user: { id: '3' }, guilds: [] }, 0);
    expect(ids(latestWindow(s, cid))).toEqual([]);
    expect(guilds(s)).toEqual([]);
    expect(s.meId).toBe('3');
  });
  test('READY after a gap retains runs, pending sends and tombstones, then a latest page bridges the gap', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { limit: 5 }, page(96, 100));
    applyHistoryPage(s, cid, { around: '20', limit: 3 }, page(19, 21));
    event(s, 'MESSAGE_DELETE', { id: '99' });
    addPending(s, cid, { nonce: 'pending', content: 'still sending', nowMs: 100 });
    addPending(s, cid, { nonce: 'failed', content: 'retry me', nowMs: 100 });
    failPending(s, cid, 'failed', 'offline');
    event(s, 'TYPING_START', { user_id: other.id });
    const before = latestWindow(s, cid).rows;
    const version = channelVersion(s, cid), side = sidebarVersion(s);
    expect(readyAgain(s, '103')).toMatchObject({ sidebar: true });
    expect(channelVersion(s, cid)).toBe(version + 1);
    expect(sidebarVersion(s)).toBe(side + 1);
    expect(windowAround(s, cid, '100').rows).toEqual(before.filter(row => row.id !== null));
    expect(windowAround(s, cid, '100')).toMatchObject({ reachesNewest: false, moreAfter: true, fetchAfter: '100' });
    expect(ids(windowAround(s, cid, '20'))).toEqual(['19', '20', '21']);
    expect(keys(s)).toEqual(['pending', 'failed']);
    expect(latestWindow(s, cid)).toMatchObject({ fetchLatest: true });
    expect(typingUsers(s, cid, 200)).toEqual([]);
    expect(user(s, other.id)).toBeNull();
    expect(channel(s, '2000')).toBeNull();
    expect(guilds(s)).toEqual([{ id: '10', name: 'reconnected' }]);
    expect(unread(s, cid)).toEqual({ unread: true, lastReadId: '99', mentionCount: 2 });
    applyHistoryPage(s, cid, { limit: 5 }, page(99, 103));
    expect(keys(s)).toEqual(['96', '97', '98', '100', '101', '102', '103', 'pending', 'failed']);
    expect(latestWindow(s, cid)).toMatchObject({ reachesNewest: true, fetchAfter: null, fetchLatest: false });
    expect(latestWindow(s, cid).rows.find(row => row.key === 'failed')).toMatchObject({ status: 'failed', error: 'offline' });
    send(s, '104', { author: me, nonce: 'pending' });
    expect(keys(s)).toEqual(['96', '97', '98', '100', '101', '102', '103', 'pending', 'failed']);
    expect(latestWindow(s, cid).rows.find(row => row.key === 'pending')).toMatchObject({ id: '104', status: 'sent' });
    expect(removePending(s, cid, 'failed')).toEqual({ channels: [cid], sidebar: false });
  });
  test('READY keeps an up-to-date live run, using actual ids despite local echo display order', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { limit: 2 }, page(99, 100));
    addPending(s, cid, { nonce: 'mine', content: 'hello', nowMs: 100 });
    send(s, '101');
    send(s, '102', { author: me, nonce: 'mine' });
    for (const latest of ['102', '101']) {
      readyAgain(s, latest);
      expect(ids(latestWindow(s, cid))).toEqual(['99', '100', '102', '101']);
      expect(latestWindow(s, cid)).toMatchObject({ reachesNewest: true, fetchLatest: false });
    }
  });
  test('a latest page after READY stays separate until it meets retained history', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { limit: 3 }, page(98, 100));
    readyAgain(s, '200');
    applyHistoryPage(s, cid, { limit: 3 }, page(198, 200));
    expect(ids(latestWindow(s, cid))).toEqual(['198', '199', '200']);
    expect(ids(windowAround(s, cid, '99'))).toEqual(['98', '99', '100']);
    expect(windowAround(s, cid, '99').reachesNewest).toBe(false);
    expect(windowAround(s, cid, '150').rows).toEqual([]);
  });
  test('READY does not mistake an uncached cursor or a pending row for the newest cached message', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { after: '110', limit: 5 }, []);
    addPending(s, cid, { nonce: 'mine', content: 'hello', nowMs: 100 });
    readyAgain(s, '110');
    expect(keys(s)).toEqual(['mine']);
    expect(latestWindow(s, cid)).toMatchObject({ fetchLatest: true });
    expect(windowAround(s, cid, 'mine')).toMatchObject({ reachesNewest: true });
    expect(windowAround(s, cid, 'mine').rows[0]).toMatchObject({ status: 'pending', key: 'mine' });
  });
  test('READY clears typing and advances versions even for a channel absent from its metadata', () => {
    const s = seeded();
    applyHistoryPage(s, '3000', { around: '20', limit: 3 }, page(19, 21));
    applyDispatch(s, 'TYPING_START', { channel_id: '3000', user_id: other.id }, 100);
    const version = channelVersion(s, '3000');
    expect(readyAgain(s, '100').channels).toContain('3000');
    expect(channelVersion(s, '3000')).toBe(version + 1);
    expect(typingUsers(s, '3000', 200)).toEqual([]);
    expect(ids(windowAround(s, '3000', '20'))).toEqual(['19', '20', '21']);
  });
  test('mention counts, unread updates and acknowledgements change the sidebar when needed', () => {
    const s = seeded();
    send(s, '101', { mentions: [me] });
    expect(unread(s, cid)).toEqual({ unread: true, mentionCount: 1, lastReadId: '100' });
    send(s, '101', { mentions: [me] });
    expect(unread(s, cid).mentionCount).toBe(1);
    send(s, '102', { mention_everyone: true });
    expect(unread(s, cid).mentionCount).toBe(2);
    event(s, 'CHANNEL_UNREAD_UPDATE', { channel_unreads: [{ channel_id: cid, last_message_id: '101', mention_count: 7 }] });
    expect(unread(s, cid)).toEqual({ unread: true, mentionCount: 7, lastReadId: '101' });
    event(s, 'MESSAGE_ACK', { message_id: '102' });
    expect(unread(s, cid).unread).toBe(false);
    expect(unread(s, cid).mentionCount).toBe(0);
  });
});

describe('contiguous history runs', () => {
  test('latest and older pages join at the cursor and a short older page proves the start', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { limit: 5 }, page(96, 100));
    expect(latestWindow(s, cid)).toMatchObject({ moreBefore: true, moreAfter: false, reachesStart: false, reachesNewest: true });
    applyHistoryPage(s, cid, { before: '96', limit: 5 }, page(91, 95));
    expect(ids(latestWindow(s, cid))).toEqual(Array.from({ length: 10 }, (_, i) => String(91 + i)));
    applyHistoryPage(s, cid, { before: '91', limit: 5 }, page(89, 90));
    expect(latestWindow(s, cid)).toMatchObject({ moreBefore: false, moreAfter: false, reachesStart: true, reachesNewest: true });
    expect(ids(latestWindow(s, cid)).at(0)).toBe('89');
  });
  test('a short latest page, including an empty channel, proves both endpoints', () => {
    for (const messages of [page(99, 100), []]) {
      const s = createStore();
      applyHistoryPage(s, cid, { limit: 5 }, messages, { since: storeSeq(s) });
      expect(latestWindow(s, cid)).toMatchObject({ reachesStart: true, reachesNewest: true, moreBefore: false, moreAfter: false });
    }
  });
  test('an around page remains a separate run and windows never cross its gap', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { limit: 5 }, page(96, 100));
    applyHistoryPage(s, cid, { around: '42', limit: 5 }, page(40, 44));
    expect(ids(latestWindow(s, cid))).toEqual(['96', '97', '98', '99', '100']);
    expect(ids(windowAround(s, cid, '42'))).toEqual(['40', '41', '42', '43', '44']);
    expect(windowAround(s, cid, '42')).toMatchObject({ moreBefore: true, moreAfter: true, reachesStart: false, reachesNewest: false });
    expect(windowAround(s, cid, '70').rows).toEqual([]);
    expect(windowBefore(s, cid, '96').rows).toEqual([]);
    expect(windowAfter(s, cid, '44').rows).toEqual([]);
    applyHistoryPage(s, cid, { around: '20', limit: 5 }, [msg('20')]);
    expect(windowAround(s, cid, '20')).toMatchObject({ reachesStart: false, reachesNewest: false });
  });
  test('runs that meet at an exclusive page cursor merge transitively', () => {
    const s = seeded('12');
    applyHistoryPage(s, cid, { limit: 3 }, page(10, 12));
    applyHistoryPage(s, cid, { around: '2', limit: 3 }, page(1, 3));
    applyHistoryPage(s, cid, { after: '3', limit: 3 }, page(4, 6));
    applyHistoryPage(s, cid, { after: '6', limit: 3 }, page(7, 9));
    expect(ids(windowAround(s, cid, '2'))).toHaveLength(9);
    expect(ids(latestWindow(s, cid))).toEqual(['10', '11', '12']);
    applyHistoryPage(s, cid, { before: '10', limit: 3 }, page(7, 9));
    expect(ids(latestWindow(s, cid))).toEqual(Array.from({ length: 12 }, (_, i) => String(i + 1)));
  });
  test('overlapping pages merge once per id, even with duplicated response rows', () => {
    const s = seeded('10');
    applyHistoryPage(s, cid, { limit: 5 }, page(6, 10));
    applyHistoryPage(s, cid, { around: '5', limit: 5 }, [msg(7), msg(6), msg(5), msg(5), msg(4)]);
    expect(ids(latestWindow(s, cid))).toEqual(['4', '5', '6', '7', '8', '9', '10']);
    applyHistoryPage(s, cid, { before: '6', limit: 5 }, page(1, 5));
    expect(ids(latestWindow(s, cid))).toEqual(Array.from({ length: 10 }, (_, i) => String(i + 1)));
  });
  test('an after page can connect a historical run to the live tail', () => {
    const s = seeded('10');
    applyHistoryPage(s, cid, { limit: 3 }, page(8, 10));
    applyHistoryPage(s, cid, { around: '2', limit: 3 }, page(1, 3));
    send(s, '11');
    applyHistoryPage(s, cid, { after: '3', limit: 5 }, page(4, 8));
    expect(ids(windowAround(s, cid, '2'))).toEqual(Array.from({ length: 11 }, (_, i) => String(i + 1)));
    expect(windowAround(s, cid, '2').reachesNewest).toBe(true);
  });
  test('empty directional pages set their endpoint without removing cached rows', () => {
    const s = createStore();
    applyHistoryPage(s, cid, { around: '2', limit: 3 }, page(1, 3));
    applyHistoryPage(s, cid, { before: '1', limit: 3 }, []);
    expect(windowAround(s, cid, '2').reachesStart).toBe(true);
    applyHistoryPage(s, cid, { after: '3', limit: 3 }, []);
    expect(ids(latestWindow(s, cid))).toEqual(['1', '2', '3']);
    expect(latestWindow(s, cid)).toMatchObject({ reachesStart: true, reachesNewest: true });
  });
  test('late latest-page completion joins messages received since its request', () => {
    const s = seeded('100');
    const since = storeSeq(s);
    send(s, '101');
    applyHistoryPage(s, cid, { limit: 5 }, page(96, 100), { since });
    expect(ids(latestWindow(s, cid))).toEqual(['96', '97', '98', '99', '100', '101']);
  });
  test('seeded overlapping pages preserve the same sorted set under different arrival orders', () => {
    let seed = 0xa53c201;
    for (let trial = 0; trial < 16; trial++) {
      const starts = [1, 10, 19, 28, 37, 46, 55, 64, 73, 82, 91];
      for (let i = starts.length - 1; i > 0; i--) {
        seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
        const j = seed % (i + 1);
        [starts[i], starts[j]] = [starts[j], starts[i]];
      }
      const s = seeded();
      applyHistoryPage(s, cid, { limit: 10 }, page(91, 100));
      for (const start of starts) applyHistoryPage(s, cid, { around: String(start + 4), limit: 10 }, page(start, start + 9));
      expect(ids(latestWindow(s, cid, 200))).toEqual(Array.from({ length: 100 }, (_, i) => String(i + 1)));
    }
  });
});

describe('bounded windows', () => {
  test('a middle run exposes fetch cursors only where the window touches its edges', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { around: '20', limit: 5 }, page(18, 22));
    expect(windowAround(s, cid, '20')).toMatchObject({ fetchBefore: '18', fetchAfter: '22', fetchLatest: false });
    expect(windowAround(s, cid, '20', 3)).toMatchObject({ fetchBefore: null, fetchAfter: null });
    expect(windowBefore(s, cid, '20', 2)).toMatchObject({ fetchBefore: '18', fetchAfter: null });
    expect(windowAfter(s, cid, '20', 2)).toMatchObject({ fetchBefore: null, fetchAfter: '22' });
    expect(windowBefore(s, cid, '18')).toMatchObject({ rows: [], fetchBefore: '18', fetchAfter: null });
    expect(windowAfter(s, cid, '22')).toMatchObject({ rows: [], fetchBefore: null, fetchAfter: '22' });
  });
  test('live and start-reaching runs suppress fetches beyond known channel ends', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { limit: 5 }, page(96, 100));
    expect(latestWindow(s, cid)).toMatchObject({ fetchBefore: '96', fetchAfter: null, fetchLatest: false });
    expect(latestWindow(s, cid, 3)).toMatchObject({ moreBefore: true, fetchBefore: null, fetchAfter: null });
    const beginning = seeded();
    applyHistoryPage(beginning, cid, { before: '4', limit: 5 }, page(1, 3));
    expect(windowAround(beginning, cid, '2')).toMatchObject({ reachesStart: true, fetchBefore: null, fetchAfter: '3' });
    applyHistoryPage(s, cid, { before: '96', limit: 5 }, page(94, 95));
    expect(latestWindow(s, cid)).toMatchObject({ reachesStart: true, reachesNewest: true, fetchBefore: null, fetchAfter: null });
  });
  test('a full page after id 0 reaches the channel start (a channel never read opens there)', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { after: '0', limit: 3 }, page(1, 3));
    expect(windowAround(s, cid, '0')).toMatchObject({ reachesStart: true, moreBefore: false, fetchBefore: null, fetchAfter: '3' });
  });
  test('fetch cursors include uncached endpoint messages and make the next page meet the run', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { after: '20', limit: 3 }, page(21, 23));
    const older = windowAround(s, cid, '22');
    expect(older.fetchBefore).toBe('21');
    applyHistoryPage(s, cid, { before: older.fetchBefore!, limit: 3 }, page(18, 20));
    expect(ids(windowAround(s, cid, '22'))).toEqual(['18', '19', '20', '21', '22', '23']);
    applyHistoryPage(s, cid, { before: '80', limit: 3 }, page(77, 79));
    const newer = windowAround(s, cid, '78');
    expect(newer.fetchAfter).toBe('79');
    applyHistoryPage(s, cid, { after: newer.fetchAfter!, limit: 3 }, page(80, 82));
    expect(ids(windowAround(s, cid, '78'))).toEqual(['77', '78', '79', '80', '81', '82']);
    event(s, 'MESSAGE_DELETE_BULK', { ids: ['77', '82'] });
    expect(windowAround(s, cid, '79')).toMatchObject({ fetchBefore: '78', fetchAfter: '81' });
  });
  test('paging follows the confirmed id boundary even when the last displayed row has an older id', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { limit: 1 }, [msg(100)]);
    addPending(s, cid, { nonce: 'mine', content: 'hello', nowMs: 100 });
    send(s, '101');
    send(s, '103', { author: me, nonce: 'mine' });
    readyAgain(s, '110');
    expect(windowAround(s, cid, '100')).toMatchObject({ fetchBefore: '100', fetchAfter: '103' });
    event(s, 'MESSAGE_DELETE', { id: '103' });
    expect(windowAround(s, cid, '100').fetchAfter).toBe('101');
  });
  test('an unloaded or pending-only live tail explicitly requests latest, while a loaded empty channel does not', () => {
    const s = seeded();
    expect(latestWindow(s, cid)).toMatchObject({ fetchBefore: null, fetchAfter: null, fetchLatest: true });
    expect(windowAround(s, cid, '20')).toMatchObject({ fetchBefore: null, fetchAfter: null, fetchLatest: false });
    addPending(s, cid, { nonce: 'mine', content: 'hello', nowMs: 100 });
    expect(latestWindow(s, cid)).toMatchObject({ fetchBefore: null, fetchAfter: null, fetchLatest: true });
    removePending(s, cid, 'mine');
    expect(latestWindow(s, cid)).toMatchObject({ rows: [], fetchBefore: null, fetchLatest: true });
    applyHistoryPage(s, cid, { limit: 5 }, [], { since: storeSeq(s) });
    expect(latestWindow(s, cid)).toMatchObject({ rows: [], fetchBefore: null, fetchAfter: null, fetchLatest: false });
  });
  test('around, before and after select only their requested rows with boundary flags', () => {
    const s = createStore();
    applyHistoryPage(s, cid, { limit: 20 }, page(1, 10));
    expect(ids(windowAround(s, cid, '5', 5))).toEqual(['3', '4', '5', '6', '7']);
    expect(windowAround(s, cid, '5', 5)).toMatchObject({ moreBefore: true, moreAfter: true });
    expect(ids(windowBefore(s, cid, '5', 3))).toEqual(['2', '3', '4']);
    expect(ids(windowAfter(s, cid, '5', 3))).toEqual(['6', '7', '8']);
    expect(windowBefore(s, cid, '4', 10)).toMatchObject({ moreBefore: false, moreAfter: true, reachesStart: true });
    expect(windowAfter(s, cid, '8', 10)).toMatchObject({ moreBefore: true, moreAfter: false, reachesNewest: true });
    expect(windowAround(s, cid, '1', 10)).toMatchObject({ reachesStart: true, reachesNewest: true });
    event(s, 'MESSAGE_DELETE', { id: '5' });
    expect(ids(windowBefore(s, cid, '5', 2))).toEqual(['3', '4']);
    expect(ids(windowAfter(s, cid, '5', 2))).toEqual(['6', '7']);
  });
  test('latest and cursor windows clamp to 300 without returning the rest of history', () => {
    const s = createStore();
    applyHistoryPage(s, cid, { limit: 50 }, page(951, 1000));
    for (let end = 950; end >= 50; end -= 50) {
      applyHistoryPage(s, cid, { before: String(end + 1), limit: 50 }, page(end - 49, end));
    }
    expect(ids(latestWindow(s, cid, 10000))).toEqual(Array.from({ length: 300 }, (_, i) => String(701 + i)));
    expect(windowAround(s, cid, '500', 10000).rows).toHaveLength(300);
    expect(windowBefore(s, cid, '500', 10000).rows).toHaveLength(300);
    expect(windowAfter(s, cid, '500', 10000).rows).toHaveLength(300);
    expect(latestWindow(s, cid, 10000)).toMatchObject({ moreBefore: true, moreAfter: false });
    expect(() => latestWindow(s, cid, 0)).toThrow();
  });
  test('unloaded windows are fetchable and an old run alone does not pretend to be the live tail', () => {
    const s = seeded();
    expect(latestWindow(s, cid)).toMatchObject({ rows: [], moreBefore: true, moreAfter: true });
    applyHistoryPage(s, cid, { around: '20', limit: 3 }, page(19, 21));
    expect(latestWindow(s, cid).rows).toEqual([]);
    expect(ids(windowAround(s, cid, '20'))).toEqual(['19', '20', '21']);
  });
  test('an empty around response does not claim the gaps between cached messages', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { around: '70', limit: 3 }, []);
    applyHistoryPage(s, cid, { limit: 5 }, [msg(100), msg(98), msg(96)]);
    expect(ids(windowBefore(s, cid, '99', 2))).toEqual(['96', '98']);
    expect(ids(windowAfter(s, cid, '99', 2))).toEqual(['100']);
  });
});

describe('message mutations', () => {
  test('own messages keep a read guild channel read without invalidating the sidebar', () => {
    const s = seeded();
    const side = sidebarVersion(s);
    expect(send(s, '101', { author: me })).toEqual({ channels: [cid], sidebar: false });
    expect(unread(s, cid)).toEqual({ unread: false, mentionCount: 0, lastReadId: '101' });
    expect(sidebarVersion(s)).toBe(side);
  });
  test('own MESSAGE_CREATE advances read state without clearing mentions or moving another channel version', () => {
    const s = seeded();
    send(s, '101', { mentions: [me] });
    const side = sidebarVersion(s), version = channelVersion(s, cid), otherVersion = channelVersion(s, '2000');
    expect(send(s, '102', { author: me })).toEqual({ channels: [cid], sidebar: true });
    expect(unread(s, cid)).toEqual({ unread: false, mentionCount: 1, lastReadId: '102' });
    expect(sidebarVersion(s)).toBe(side + 1);
    expect(channelVersion(s, cid)).toBe(version + 1);
    expect(channelVersion(s, '2000')).toBe(otherVersion);
    send(s, '99', { author: me });
    expect(unread(s, cid)).toEqual({ unread: false, mentionCount: 1, lastReadId: '102' });
    send(s, '103');
    expect(unread(s, cid).unread).toBe(true);
    send(s, '102', { author: me });
    expect(unread(s, cid)).toEqual({ unread: true, mentionCount: 1, lastReadId: '102' });
  });
  test('partial edits, deletes and bulk deletes reach non-newest runs', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { limit: 3 }, page(98, 100));
    applyHistoryPage(s, cid, { around: '20', limit: 5 }, page(18, 22));
    event(s, 'MESSAGE_UPDATE', { id: '20', embeds: [{ title: 'arrived' }] });
    expect(windowAround(s, cid, '20').rows.find(r => r.id === '20')).toMatchObject({ content: 'message 20', embeds: [{ title: 'arrived' }] });
    event(s, 'MESSAGE_DELETE', { id: '19' });
    event(s, 'MESSAGE_DELETE_BULK', { ids: ['18', '21', '99'] });
    expect(ids(windowAround(s, cid, '20'))).toEqual(['20', '22']);
    expect(ids(latestWindow(s, cid))).toEqual(['98', '100']);
  });
  test('tombstones prevent resurrection by late pages, CREATE and UPDATE', () => {
    const s = seeded();
    event(s, 'MESSAGE_DELETE', { id: '99' });
    event(s, 'MESSAGE_DELETE_BULK', { ids: ['98', '97'] });
    applyHistoryPage(s, cid, { limit: 5 }, page(96, 100));
    send(s, '99');
    event(s, 'MESSAGE_UPDATE', { id: '98', content: 'late' });
    expect(ids(latestWindow(s, cid))).toEqual(['96', '100']);
    applyHistoryPage(s, cid, { before: '98', limit: 5 }, page(93, 97));
    expect(ids(latestWindow(s, cid))).toEqual(['93', '94', '95', '96', '100']);
  });
  test('UPDATE before CREATE is ignored, and later partial UPDATE preserves content and author', () => {
    const s = seeded();
    expect(event(s, 'MESSAGE_UPDATE', { id: '101', embeds: [{ title: 'early' }] })).toEqual({ channels: [], sidebar: false });
    expect(latestWindow(s, cid).rows).toEqual([]);
    send(s, '101');
    event(s, 'MESSAGE_UPDATE', { id: '101', embeds: [{ title: 'late' }], edited_timestamp: '2026-10-22T12:01:00.000Z' });
    expect(latestWindow(s, cid).rows[0]).toMatchObject({ content: 'message 101', author: other,
      embeds: [{ title: 'late' }], edited_timestamp: '2026-10-22T12:01:00.000Z', key: '101' });
  });
  test('poll votes keep each answer\'s count and the reader\'s choice, and a page does not undo them', () => {
    const s = seeded();
    const poll = { question: { text: 'Lunch?' }, answers: [{ answer_id: 1 }, { answer_id: 2 }] };
    const vote = (type: string, userId: string, answer: number) => event(s, type, { message_id: '20', user_id: userId, answer_id: answer });
    const counts = () => windowAround(s, cid, '20').rows.find(r => r.id === '20')!.poll.results.answer_counts;
    // Counts Discord left out stay unknown: a vote does not start them from zero.
    applyHistoryPage(s, cid, { around: '20', limit: 3 }, page(19, 21).map(m => m.id === '20' ? { ...m, poll } : m));
    expect(event(s, 'MESSAGE_POLL_VOTE_ADD', { message_id: '20', user_id: '2', answer_id: 1 })).toMatchObject({ channels: [] });
    // The reader's own vote is kept meanwhile, so it can be taken back, and a page with the counts replaces it.
    const before = storeSeq(s);
    vote('MESSAGE_POLL_VOTE_ADD', '1', 2);
    expect(windowAround(s, cid, '20').rows.find(r => r.id === '20')!.poll.results).toEqual({ is_finalized: false, partial: true,
      answer_counts: [{ id: 2, count: 1, me_voted: true }] });
    const known = { is_finalized: false, answer_counts: [{ id: 2, count: 4, me_voted: true }] };
    applyHistoryPage(s, cid, { around: '20', limit: 3 }, page(19, 21).map(m => m.id === '20' ? { ...m, poll: { ...poll, results: known } } : m), { since: before });
    expect(counts()).toEqual(known.answer_counts);
    const empty = { ...poll, results: { is_finalized: false, answer_counts: [] } };
    applyHistoryPage(s, cid, { around: '20', limit: 3 }, page(19, 21).map(m => m.id === '20' ? { ...m, poll: empty } : m));
    const since = storeSeq(s);
    vote('MESSAGE_POLL_VOTE_ADD', '2', 1);
    vote('MESSAGE_POLL_VOTE_ADD', '1', 1);
    vote('MESSAGE_POLL_VOTE_ADD', '1', 1);
    expect(counts()).toEqual([{ id: 1, count: 2, me_voted: true }]);
    vote('MESSAGE_POLL_VOTE_REMOVE', '1', 1);
    vote('MESSAGE_POLL_VOTE_REMOVE', '1', 1);
    vote('MESSAGE_POLL_VOTE_ADD', '3', 2);
    expect(counts()).toEqual([{ id: 1, count: 1, me_voted: false }, { id: 2, count: 1, me_voted: false }]);
    applyHistoryPage(s, cid, { around: '20', limit: 3 }, page(19, 21).map(m => m.id === '20' ? { ...m, poll } : m), { since });
    expect(counts()).toEqual([{ id: 1, count: 1, me_voted: false }, { id: 2, count: 1, me_voted: false }]);
    expect(vote('MESSAGE_POLL_VOTE_ADD', '2', 1)).toMatchObject({ channels: [cid] });
    // A poll Discord sends without its results keeps the counts already known.
    applyHistoryPage(s, cid, { around: '20', limit: 3 }, page(19, 21).map(m => m.id === '20' ? { ...m, poll } : m));
    expect(counts()).toEqual([{ id: 1, count: 2, me_voted: false }, { id: 2, count: 1, me_voted: false }]);
    // A reaction after a page was asked for does not keep that page's newer poll out, nor a vote its reactions.
    const later = storeSeq(s);
    event(s, 'MESSAGE_REACTION_ADD', { message_id: '20', user_id: '2', emoji: { id: null, name: '👍' } });
    const tally = { is_finalized: true, answer_counts: [{ id: 1, count: 7, me_voted: false }] };
    applyHistoryPage(s, cid, { around: '20', limit: 3 }, page(19, 21).map(m => m.id === '20' ? { ...m, poll: { ...poll, results: tally } } : m), { since: later });
    expect(counts()).toEqual(tally.answer_counts);
    // An update that leaves the counts out is not news about them: a page asked before it still lands.
    const asked = storeSeq(s);
    event(s, 'MESSAGE_UPDATE', { id: '20', poll });
    const final = { is_finalized: true, answer_counts: [{ id: 1, count: 8, me_voted: false }] };
    applyHistoryPage(s, cid, { around: '20', limit: 3 }, page(19, 21).map(m => m.id === '20' ? { ...m, poll: { ...poll, results: final } } : m), { since: asked });
    expect(counts()).toEqual(final.answer_counts);
  });
  test('reaction add/remove, remove-emoji and remove-all maintain count and me', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { around: '20', limit: 3 }, page(19, 21));
    const emoji = { id: null, name: '👍' };
    const custom = { id: '99', name: 'party' };
    const react = (type: string, userId: string, value = emoji) => event(s, type, { message_id: '20', user_id: userId, emoji: value });
    const reactions = () => windowAround(s, cid, '20').rows.find(r => r.id === '20')!.reactions;
    react('MESSAGE_REACTION_ADD', '2');
    react('MESSAGE_REACTION_ADD', '1');
    expect(reactions()).toEqual([{ emoji, count: 2, me: true }]);
    react('MESSAGE_REACTION_REMOVE', '2');
    expect(reactions()).toEqual([{ emoji, count: 1, me: true }]);
    react('MESSAGE_REACTION_REMOVE', '1');
    expect(reactions()).toEqual([]);
    react('MESSAGE_REACTION_ADD', '1');
    react('MESSAGE_REACTION_ADD', '3', custom as any);
    react('MESSAGE_REACTION_REMOVE_EMOJI', '3', { ...custom, name: 'renamed' } as any);
    expect(reactions()).toEqual([{ emoji, count: 1, me: true }]);
    event(s, 'MESSAGE_REACTION_REMOVE_ALL', { message_id: '20' });
    expect(reactions()).toEqual([]);
    expect(event(s, 'MESSAGE_REACTION_ADD', { message_id: 'missing', user_id: '2', emoji })).toEqual({ channels: [], sidebar: false });
  });
  test('typing lasts exactly ten seconds and MESSAGE_CREATE clears its author', () => {
    const s = seeded();
    applyDispatch(s, 'TYPING_START', { channel_id: cid, user_id: '2', member: { user: other } }, 500);
    expect(typingUsers(s, cid, 10499)).toEqual([{ user: other, expiresAtMs: 10500 }]);
    expect(typingUsers(s, cid, 10500)).toEqual([]);
    const version = channelVersion(s, cid);
    expect(expireTyping(s, 10500)).toBe(1);
    expect(channelVersion(s, cid)).toBe(version);
    expect(expireTyping(s, 10500)).toBe(0);
    event(s, 'TYPING_START', { user_id: '2' });
    expect(typingUsers(s, cid, 100)).toHaveLength(1);
    send(s, '101');
    expect(typingUsers(s, cid, 100)).toEqual([]);
    expect(event(s, 'TYPING_START', { user_id: '1' })).toEqual({ channels: [], sidebar: false });
  });
});

describe('local echo', () => {
  for (const fromPage of [false, true]) test(`${fromPage ? 'history' : 'gateway'} confirmation reads through the send and retains mentions`, () => {
    const s = seeded();
    send(s, '101', { mentions: [me] });
    addPending(s, cid, { nonce: 'mine', content: 'hello', nowMs: 100 });
    expect(unread(s, cid)).toEqual({ unread: true, mentionCount: 1, lastReadId: '100' });
    const side = sidebarVersion(s), version = channelVersion(s, cid);
    const message = msg('102', { author: me, nonce: 'mine' });
    const changed = fromPage ? applyHistoryPage(s, cid, { limit: 3 }, [message, msg(101), msg(100)])
      : applyDispatch(s, 'MESSAGE_CREATE', message, 200);
    expect(changed).toEqual({ channels: [cid], sidebar: true });
    expect(sidebarVersion(s)).toBe(side + 1);
    expect(channelVersion(s, cid)).toBe(version + 1);
    expect(unread(s, cid)).toEqual({ unread: false, mentionCount: 1, lastReadId: '102' });
    expect(latestWindow(s, cid).rows.find(row => row.key === 'mine')).toMatchObject({ id: '102', status: 'sent' });
  });
  test('an ordinary history page does not mark old own messages read', () => {
    const s = seeded('90');
    send(s, '101');
    applyHistoryPage(s, cid, { limit: 5 }, [msg(101), msg(100, { author: me })]);
    expect(unread(s, cid)).toEqual({ unread: true, mentionCount: 0, lastReadId: '90' });
  });
  test('confirmation replaces the pending row with the same key and position', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { limit: 3 }, page(98, 100));
    addPending(s, cid, { nonce: 'nonce', content: 'hello', nowMs: 100, replyTo: { messageId: '99', guildId: '10' } });
    const before = keys(s);
    expect(latestWindow(s, cid).rows.at(-1)).toMatchObject({ key: 'nonce', status: 'pending', content: 'hello', id: null,
      message_reference: { message_id: '99', channel_id: cid, guild_id: '10' } });
    send(s, '101', { author: me, nonce: 'nonce', content: 'hello' });
    expect(keys(s)).toEqual(before);
    expect(latestWindow(s, cid).rows.at(-1)).toMatchObject({ id: '101', key: 'nonce', status: 'sent' });
    expect(removePending(s, cid, 'nonce')).toEqual({ channels: [], sidebar: false });
  });
  test('intervening traffic and multiple sends retain their visible order through confirmation and paging', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { limit: 1 }, [msg('100')]);
    addPending(s, cid, { nonce: 'first', content: 'one', nowMs: 100 });
    send(s, '101');
    addPending(s, cid, { nonce: 'second', content: 'two', nowMs: 101 });
    send(s, '102');
    const before = ['100', 'first', '101', 'second', '102'];
    expect(keys(s)).toEqual(before);
    send(s, '103', { author: me, nonce: 'first' });
    send(s, '104', { author: me, nonce: 'second' });
    expect(keys(s)).toEqual(before);
    expect(ids(latestWindow(s, cid))).toEqual(['100', '103', '101', '104', '102']);
    expect(ids(windowBefore(s, cid, '103', 1))).toEqual(['100']);
    expect(ids(windowAfter(s, cid, '103', 1))).toEqual(['101']);
    applyHistoryPage(s, cid, { limit: 5 }, [msg(104, { author: me, nonce: 'second' }),
      msg(103, { author: me, nonce: 'first' }), ...page(100, 102)]);
    expect(keys(s)).toEqual(before);
    send(s, '105');
    expect(keys(s)).toEqual([...before, '105']);
  });
  test('a failed send remains visible and can be removed or confirmed late', () => {
    const s = seeded();
    addPending(s, cid, { nonce: 'first', content: 'one', nowMs: 100 });
    failPending(s, cid, 'first', 'offline');
    expect(latestWindow(s, cid).rows[0]).toMatchObject({ status: 'failed', error: 'offline', key: 'first' });
    send(s, '101', { author: me, nonce: 'first' });
    expect(latestWindow(s, cid).rows[0].error).toBeUndefined();
    expect(latestWindow(s, cid).rows[0].status).toBe('sent');
    addPending(s, cid, { nonce: 'second', content: 'two', nowMs: 101 });
    failPending(s, cid, 'second', 'offline');
    removePending(s, cid, 'second');
    expect(keys(s)).toEqual(['first']);
  });
  test('another user cannot confirm a local nonce and a history page can confirm it', () => {
    const s = seeded();
    addPending(s, cid, { nonce: 'nonce', content: 'mine', nowMs: 100 });
    send(s, '101', { nonce: 'nonce' });
    expect(latestWindow(s, cid).rows[0].status).toBe('pending');
    applyHistoryPage(s, cid, { limit: 5 }, [msg(102, { author: me, nonce: 'nonce' }), msg(101), msg(100)]);
    expect(keys(s)).toEqual(['100', 'nonce', '101']);
    expect(latestWindow(s, cid).rows[1].status).toBe('sent');
  });
  test('an around-page confirmation joins an otherwise empty local-echo run', () => {
    const s = createStore({ meId: '1' });
    addPending(s, cid, { nonce: 'nonce', content: 'mine', nowMs: 100 });
    applyHistoryPage(s, cid, { around: '101', limit: 3 }, [msg(101, { author: me, nonce: 'nonce' }), msg(100)]);
    expect(keys(s)).toEqual(['100', 'nonce']);
    expect(ids(windowAround(s, cid, '101'))).toEqual(['100', '101']);
    send(s, '102');
    expect(ids(latestWindow(s, cid))).toEqual(['100', '101', '102']);
    event(s, 'MESSAGE_DELETE', { id: '101' });
    expect(ids(latestWindow(s, cid))).toEqual(['100', '102']);
  });
  test('sending before history loads keeps older rows before the echo and new arrivals after it', () => {
    const s = createStore({ meId: '1' });
    addPending(s, cid, { nonce: 'first', content: 'mine', nowMs: 100 });
    addPending(s, cid, { nonce: 'second', content: 'also mine', nowMs: 100 });
    send(s, '101');
    applyHistoryPage(s, cid, { limit: 5 }, [msg(101), msg(100)]);
    expect(keys(s)).toEqual(['100', 'first', 'second', '101']);
    send(s, '102', { author: me, nonce: 'second' });
    send(s, '103', { author: me, nonce: 'first' });
    expect(keys(s)).toEqual(['100', 'first', 'second', '101']);
    const empty = createStore();
    addPending(empty, cid, { nonce: 'last', content: 'mine', nowMs: 100 });
    applyHistoryPage(empty, cid, { limit: 5 }, page(99, 100));
    expect(keys(empty)).toEqual(['99', '100', 'last']);
  });
});

describe('first unread', () => {
  test('finds the first message after a cached or deleted read cursor, stopping at run boundaries', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { limit: 5 }, page(96, 100));
    applyHistoryPage(s, cid, { around: '20', limit: 5 }, page(18, 22));
    event(s, 'MESSAGE_ACK', { message_id: '20' });
    expect(firstUnread(s, cid)).toBe('21');
    event(s, 'MESSAGE_DELETE_BULK', { ids: ['20', '21'] });
    expect(firstUnread(s, cid)).toBe('22');
    event(s, 'MESSAGE_ACK', { message_id: '22' });
    expect(firstUnread(s, cid)).toBeNull();
    event(s, 'MESSAGE_ACK', { message_id: '70' });
    expect(firstUnread(s, cid)).toBeNull();
    event(s, 'MESSAGE_ACK', { message_id: '99' });
    expect(firstUnread(s, cid)).toBe('100');
    event(s, 'MESSAGE_ACK', { message_id: '100' });
    expect(firstUnread(s, cid)).toBeNull();
  });
  test('an exclusive page cursor locates its run even when that cursor is not a row', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { after: '20', limit: 3 }, page(21, 23));
    event(s, 'MESSAGE_ACK', { message_id: '20' });
    expect(firstUnread(s, cid)).toBe('21');
  });
  test('no read cursor requires the cached channel start; pending sends are never unread messages', () => {
    const s = seeded();
    event(s, 'CHANNEL_UNREAD_UPDATE', { last_message_id: null });
    expect(firstUnread(s, cid)).toBeNull();
    applyHistoryPage(s, cid, { limit: 5 }, page(96, 100));
    expect(firstUnread(s, cid)).toBeNull();
    applyHistoryPage(s, cid, { before: '96', limit: 5 }, page(94, 95));
    expect(firstUnread(s, cid)).toBe('94');
    event(s, 'MESSAGE_ACK', { message_id: '93' });
    expect(firstUnread(s, cid)).toBe('94');
    event(s, 'MESSAGE_DELETE_BULK', { ids: ['94', '95', '96', '97', '98', '99', '100'] });
    addPending(s, cid, { nonce: 'mine', content: 'hello', nowMs: 100 });
    expect(firstUnread(s, cid)).toBeNull();
    expect(firstUnread(s, 'missing')).toBeNull();
  });
  test('uses server ids when local echo order differs, including a confirmation displayed before the read cursor', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { limit: 1 }, [msg(100)]);
    addPending(s, cid, { nonce: 'mine', content: 'hello', nowMs: 100 });
    send(s, '101');
    send(s, '102');
    send(s, '103', { author: me, nonce: 'mine' });
    expect(ids(latestWindow(s, cid))).toEqual(['100', '103', '101', '102']);
    event(s, 'MESSAGE_ACK', { message_id: '100' });
    expect(firstUnread(s, cid)).toBe('101');
    event(s, 'MESSAGE_ACK', { message_id: '102' });
    expect(firstUnread(s, cid)).toBe('103');
  });
});

test('versions invalidate only affected channels and sidebar-visible changes', () => {
  const s = seeded();
  const b = channelVersion(s, '2000');
  const side = sidebarVersion(s);
  applyHistoryPage(s, cid, { limit: 3 }, page(98, 100));
  expect(sidebarVersion(s)).toBe(side);
  event(s, 'MESSAGE_UPDATE', { id: '99', content: 'edited' });
  event(s, 'TYPING_START', { user_id: '2' });
  event(s, 'MESSAGE_REACTION_ADD', { message_id: '99', user_id: '2', emoji: { name: '👍', id: null } });
  expect(sidebarVersion(s)).toBe(side);
  expect(send(s, '101')).toEqual({ channels: [cid], sidebar: true });
  const unreadSide = sidebarVersion(s);
  expect(send(s, '102')).toEqual({ channels: [cid], sidebar: false });
  expect(sidebarVersion(s)).toBe(unreadSide);
  expect(channelVersion(s, '2000')).toBe(b);
  expect(event(s, 'UNKNOWN_EVENT', {})).toEqual({ channels: [], sidebar: false });
  const a = channelVersion(s, cid);
  event(s, 'MESSAGE_UPDATE', { id: 'missing', content: 'nothing' });
  expect(channelVersion(s, cid)).toBe(a);
});

test('a known user\'s new name moves the identity revision; a new user or the same name does not', () => {
  const s = seeded();
  const before = s.identityRevision;
  send(s, '101', { author: { id: '3', username: 'newcomer' } });
  send(s, '102');
  expect(s.identityRevision).toBe(before);
  send(s, '103', { author: { ...other, global_name: 'Friendly' } });
  expect(s.identityRevision).toBe(before + 1);
});

test('a known user\'s new name moves the version of a DM with them, and of no other channel', () => {
  const s = seeded();
  applyDispatch(s, 'CHANNEL_CREATE', { id: '4000', type: 1, recipients: [other], last_message_id: '90' }, 0);
  const dm = channelVersion(s, '4000'), elsewhere = channelVersion(s, '2000');
  send(s, '101', { author: { ...other, global_name: 'Friendly' } });
  expect(channelVersion(s, '4000')).toBe(dm + 1);
  expect(channelVersion(s, '2000')).toBe(elsewhere);
  expect(channel(s, '4000')?.recipients?.[0]?.global_name).toBe('Friendly');
});

test('a thousand real-shaped MESSAGE_CREATEs append within 500 ms', () => {
  const s = seeded('0');
  let seed = 0x512a291;
  const messages = Array.from({ length: 1000 }, (_, i) => {
    seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
    const id = snowflakeFromMs(1792670400000 + i);
    return msg(id, { author: { id: String(2 + seed % 20), username: `user-${seed % 20}`, avatar: null },
      content: `hello **${i}** <@1>\nhttps://example.invalid/${seed}`, mentions: i % 17 === 0 ? [me] : [],
      attachments: [{ id, filename: 'photo.png', url: `http://127.0.0.1/photo/${id}`, width: 640, height: 480, size: 12345 }],
      embeds: [{ type: 'rich', title: `card ${i}`, image: { url: 'http://127.0.0.1/card.png', width: 320, height: 200 } }] });
  });
  const start = performance.now();
  for (const message of messages) applyDispatch(s, 'MESSAGE_CREATE', message, 1000);
  const elapsed = performance.now() - start;
  console.log(`Excord store: 1,000 MESSAGE_CREATEs in ${elapsed.toFixed(2)} ms`);
  expect(elapsed).toBeLessThan(500);
  expect(latestWindow(s, cid, 200).rows).toHaveLength(200);
  expect(latestWindow(s, cid).rows.at(-1)?.id).toBe(messages.at(-1)!.id);
  expect(windowAround(s, cid, messages[0].id, 3).rows[0].attachments[0]).toMatchObject({ width: 640, height: 480 });
});

describe('review regressions', () => {
  for (const dispatch of ['reaction', 'embed', 'reaction after READY']) {
    test(`a newer page edit survives an in-flight ${dispatch}`, () => {
      const s = seeded();
      applyHistoryPage(s, cid, { limit: 1 }, [msg(100, { content: 'original', edited_timestamp: null, reactions: [] })]);
      if (dispatch === 'reaction after READY') {
        readyAgain(s, '100');
        expect(latestWindow(s, cid).stale).toBe(true);
      }
      const since = storeSeq(s);
      const emoji = { id: null, name: '👍' };
      if (dispatch === 'embed') event(s, 'MESSAGE_UPDATE', { id: '100', embeds: [{ title: 'in flight' }] });
      else event(s, 'MESSAGE_REACTION_ADD', { message_id: '100', user_id: me.id, emoji });
      const edited = msg(100, { content: 'edited', edited_timestamp: '2026-10-22T12:02:00Z',
        embeds: [{ title: 'edited embed' }], reactions: [] });
      applyHistoryPage(s, cid, { limit: 1 }, [edited], { since });
      expect(latestWindow(s, cid).rows[0]).toMatchObject({ content: 'edited', edited_timestamp: edited.edited_timestamp,
        embeds: edited.embeds, reactions: dispatch === 'embed' ? [] : [{ emoji, count: 1, me: true }] });
      expect(latestWindow(s, cid).stale).toBe(false);
    });
  }

  for (const timestamp of [null, '2026-10-22T12:02:00.000002Z']) {
    for (const dispatch of ['reaction', 'embed']) test(`equal edit timestamps ${timestamp} protect only the ${dispatch} group`, () => {
      const s = seeded();
      const emoji = { id: null, name: '👍' };
      applyHistoryPage(s, cid, { limit: 1 }, [msg(100, { content: 'cached', edited_timestamp: timestamp,
        embeds: [{ title: 'cached' }], reactions: [], pinned: false })]);
      const since = storeSeq(s);
      if (dispatch === 'embed') event(s, 'MESSAGE_UPDATE', { id: '100', embeds: [{ title: 'in flight' }] });
      else event(s, 'MESSAGE_REACTION_ADD', { message_id: '100', user_id: me.id, emoji });
      const snapshot = msg(100, { content: 'snapshot', edited_timestamp: timestamp?.replace('Z', '+00:00') ?? null,
        embeds: [{ title: 'snapshot' }], reactions: [{ emoji, count: 3, me: false }], pinned: true });
      applyHistoryPage(s, cid, { limit: 1 }, [snapshot], { since });
      expect(latestWindow(s, cid).rows[0]).toMatchObject({
        content: dispatch === 'embed' ? 'cached' : 'snapshot',
        embeds: [{ title: dispatch === 'embed' ? 'in flight' : 'snapshot' }], pinned: true,
        reactions: [{ emoji, count: dispatch === 'reaction' ? 1 : 3, me: dispatch === 'reaction' }],
      });
      applyHistoryPage(s, cid, { limit: 1 }, [snapshot], { since: storeSeq(s) });
      expect(latestWindow(s, cid).rows[0]).toMatchObject(snapshot);
    });
  }

  test('a newer cached edit protects every content field from an older page', () => {
    const s = seeded();
    const edited = msg(100, { content: 'edited', edited_timestamp: '2026-10-22T12:02:00.000002Z',
      embeds: [{ title: 'edited' }], attachments: [{ id: '4' }], mentions: [me], mention_roles: ['5'],
      mention_everyone: true, flags: 4, pinned: false });
    applyHistoryPage(s, cid, { limit: 1 }, [edited]);
    applyHistoryPage(s, cid, { limit: 1 }, [msg(100, { edited_timestamp: '2026-10-22T12:02:00.000001Z',
      mention_roles: [], mention_everyone: false, flags: 0, pinned: true })]);
    expect(latestWindow(s, cid).rows[0]).toMatchObject({ ...edited, pinned: true });
  });

  test('a latest page replaces a deleted head before updating last_message_id and unread', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { limit: 5 }, page(96, 100));
    const since = storeSeq(s), side = sidebarVersion(s);
    applyHistoryPage(s, cid, { limit: 5 }, [...page(96, 99), msg(101)], { since });
    expect(ids(latestWindow(s, cid))).toEqual(['96', '97', '98', '99', '101']);
    expect(channel(s, cid)?.last_message_id).toBe('101');
    expect(unread(s, cid)).toMatchObject({ unread: true, lastReadId: '100' });
    expect(sidebarVersion(s)).toBe(side + 1);
    event(s, 'MESSAGE_ACK', { message_id: '99' });
    expect(unread(s, cid).unread).toBe(true);
  });

  test('deleting the advertised head retains a newer cached head', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { limit: 3 }, page(100, 102));
    event(s, 'MESSAGE_DELETE', { id: '100' });
    expect(channel(s, cid)?.last_message_id).toBe('102');
    expect(unread(s, cid)).toMatchObject({ unread: true, lastReadId: '100' });
    expect(ids(latestWindow(s, cid))).toEqual(['101', '102']);
  });

  test('deleting an uncached head keeps its id and the known unread messages', () => {
    const s = createStore();
    applyDispatch(s, 'READY', { user: me, guilds: [{ id: '10', channels: [{ id: cid, last_message_id: '100' }] }],
      read_state: [{ id: cid, last_message_id: '90' }] }, 0);
    applyHistoryPage(s, cid, { around: '95', limit: 3 }, page(94, 96));
    event(s, 'MESSAGE_DELETE', { id: '100' });
    expect(channel(s, cid)?.last_message_id).toBe('100');
    expect(unread(s, cid)).toMatchObject({ unread: true, lastReadId: '90' });
    expect(ids(windowAround(s, cid, '95'))).toEqual(['94', '95', '96']);
  });

  test('server channel heads and cached confirmed ids only advance the head', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { limit: 3 }, page(100, 102));
    expect(channel(s, cid)?.last_message_id).toBe('102');
    readyAgain(s, '100');
    expect(channel(s, cid)?.last_message_id).toBe('102');
    event(s, 'CHANNEL_UPDATE', { id: cid, last_message_id: '101' });
    expect(channel(s, cid)?.last_message_id).toBe('102');
    for (const v of [1, 2]) applyDispatch(s, `PASSIVE_UPDATE_V${v}`, { guild_id: '10',
      [v === 1 ? 'channels' : 'updated_channels']: [{ id: cid, last_message_id: '101' }] }, 0);
    expect(channel(s, cid)?.last_message_id).toBe('102');
    send(s, '103');
    event(s, 'CHANNEL_UPDATE', { id: cid, last_message_id: '101' });
    expect(channel(s, cid)?.last_message_id).toBe('103');
    addPending(s, cid, { nonce: 'mine', content: 'hello', nowMs: 0 });
    expect(channel(s, cid)?.last_message_id).toBe('103');
  });

  test('a CREATE that supplies channel metadata keeps the newest cached confirmed id', () => {
    const s = createStore();
    applyHistoryPage(s, cid, { limit: 3 }, page(100, 102));
    send(s, '99');
    expect(channel(s, cid)?.last_message_id).toBe('102');
    expect(ids(latestWindow(s, cid))).toEqual(['100', '101', '102']);
  });

  for (const pending of [false, true]) test(`an empty unscoped latest page does not seal a ${pending ? 'pending-only' : 'missing'} tail`, () => {
    const s = createStore();
    if (pending) addPending(s, cid, { nonce: 'mine', content: 'hello', nowMs: 0 });
    applyHistoryPage(s, cid, { limit: 5 }, []);
    expect(latestWindow(s, cid)).toMatchObject({ reachesStart: false, fetchLatest: true });
    expect(keys(s)).toEqual(pending ? ['mine'] : []);
    applyHistoryPage(s, cid, { limit: 5 }, [], { since: storeSeq(s) });
    expect(latestWindow(s, cid)).toMatchObject({ reachesStart: true, fetchLatest: false, stale: false });
    expect(keys(s)).toEqual(pending ? ['mine'] : []);
  });

  test('an empty latest page without since retains its cached tail as stale and fetchable', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { limit: 5 }, page(96, 100));
    applyHistoryPage(s, cid, { limit: 5 }, []);
    expect(ids(latestWindow(s, cid))).toEqual(['96', '97', '98', '99', '100']);
    expect(latestWindow(s, cid)).toMatchObject({ stale: true, reachesStart: false, reachesNewest: true });
    applyHistoryPage(s, cid, { limit: 5 }, page(96, 100), { since: storeSeq(s) });
    expect(latestWindow(s, cid).stale).toBe(false);
  });

  test('CREATE does not connect a before page through an uncached previous last id', () => {
    const s = seeded();
    send(s, '101');
    applyHistoryPage(s, cid, { before: '100', limit: 5 }, page(97, 99));
    expect(ids(latestWindow(s, cid))).toEqual(['101']);
    expect(windowAround(s, cid, '98')).toMatchObject({ fetchAfter: '99', reachesNewest: false });
    applyHistoryPage(s, cid, { after: '99', limit: 5 }, page(100, 101));
    expect(ids(latestWindow(s, cid))).toEqual(['97', '98', '99', '100', '101']);
  });

  for (const reconnect of [true, false]) test(`pending confirmation keeps gaps after ${reconnect ? 'READY' : 'a disjoint latest page'}`, () => {
    const s = seeded();
    applyHistoryPage(s, cid, { limit: 5 }, page(96, 100));
    addPending(s, cid, { nonce: 'mine', content: 'hello', nowMs: 0 });
    if (reconnect) {
      readyAgain(s, '200');
      expect(keys(s)).toEqual(['mine']);
    }
    applyHistoryPage(s, cid, { limit: 3 }, page(198, 200));
    expect(keys(s)).toEqual(['198', '199', '200', 'mine']);
    send(s, '201', { author: me, nonce: 'mine' });
    expect(keys(s)).toEqual(['198', '199', '200', 'mine']);
    expect(ids(windowAround(s, cid, '99'))).toEqual(['96', '97', '98', '99', '100']);
    expect(windowAround(s, cid, '99').fetchAfter).toBe('100');
    expect(windowAround(s, cid, '150')).toMatchObject({ rows: [], fetchAround: '150' });
  });

  test('two exclusive cursors do not conceal their unfetched shared endpoint', () => {
    const s = seeded('103');
    applyHistoryPage(s, cid, { before: '100', limit: 5 }, page(97, 99));
    applyHistoryPage(s, cid, { after: '100', limit: 5 }, page(101, 103));
    expect(ids(latestWindow(s, cid))).toEqual(['101', '102', '103']);
    expect(windowAround(s, cid, '98')).toMatchObject({ fetchAfter: '99', reachesNewest: false });
    applyHistoryPage(s, cid, { around: '100', limit: 3 }, page(99, 101));
    expect(ids(latestWindow(s, cid))).toEqual(['97', '98', '99', '100', '101', '102', '103']);
  });

  test('a late CREATE below the live run does not stretch its lower bound', () => {
    const s = seeded();
    send(s, '101');
    send(s, '50');
    expect(ids(latestWindow(s, cid))).toEqual(['101']);
    expect(ids(windowAround(s, cid, '50'))).toEqual(['50']);
    expect(windowAround(s, cid, '75')).toMatchObject({ rows: [], fetchAround: '75' });
  });

  test('channel metadata advertising an unseen head demotes the live run before the next CREATE', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { limit: 5 }, page(96, 100));
    event(s, 'CHANNEL_UPDATE', { id: cid, last_message_id: '200' });
    send(s, '201');
    expect(ids(latestWindow(s, cid))).toEqual(['201']);
    expect(windowAround(s, cid, '150')).toMatchObject({ rows: [], fetchAround: '150' });
    expect(windowAround(s, cid, '99').fetchAfter).toBe('100');
  });

  test('late pages retain gateway edits, embeds and reactions but fill missing fields', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { limit: 1 }, [msg(100, { reactions: [] })]);
    const since = storeSeq(s);
    event(s, 'MESSAGE_UPDATE', { id: '100', content: 'edited', edited_timestamp: '2026-10-22T12:02:00Z' });
    event(s, 'MESSAGE_UPDATE', { id: '100', embeds: [{ title: 'new embed' }] });
    event(s, 'MESSAGE_REACTION_ADD', { message_id: '100', user_id: me.id, emoji: { name: '👍', id: null } });
    expect(storeSeq(s)).toBeGreaterThan(since);
    applyHistoryPage(s, cid, { limit: 1 }, [msg(100, { edited_timestamp: null, reactions: [], flags: 4 })], { since });
    expect(latestWindow(s, cid).rows[0]).toMatchObject({ content: 'edited', edited_timestamp: '2026-10-22T12:02:00Z',
      embeds: [{ title: 'new embed' }], reactions: [{ count: 1, me: true }], flags: 4 });
  });

  for (const type of ['MESSAGE_REACTION_REMOVE', 'MESSAGE_REACTION_REMOVE_EMOJI', 'MESSAGE_REACTION_REMOVE_ALL']) {
    test(`a late page keeps ${type}`, () => {
      const s = seeded();
      const emoji = { id: null, name: '👍' };
      const snapshot = msg(100, { reactions: [{ emoji, count: 1, me: true }] });
      applyHistoryPage(s, cid, { limit: 1 }, [snapshot]);
      const since = storeSeq(s);
      event(s, type, { message_id: '100', user_id: me.id, emoji });
      applyHistoryPage(s, cid, { limit: 1 }, [snapshot], { since });
      expect(latestWindow(s, cid).rows[0].reactions).toEqual([]);
    });
  }

  test('edit timestamps order content even without a request sequence', () => {
    const s = seeded();
    const edited = msg(100, { content: 'new', edited_timestamp: '2026-10-22T12:02:00.000Z' });
    applyHistoryPage(s, cid, { limit: 1 }, [edited]);
    applyHistoryPage(s, cid, { limit: 1 }, [msg(100, { edited_timestamp: '2026-10-22T12:01:00.000Z' })]);
    event(s, 'MESSAGE_UPDATE', { id: '100', content: 'older', edited_timestamp: null });
    expect(latestWindow(s, cid).rows[0]).toMatchObject({ content: 'new', edited_timestamp: edited.edited_timestamp });
    applyHistoryPage(s, cid, { limit: 1 }, [msg(100, { content: 'newest', edited_timestamp: '2026-10-22T12:03:00.000Z' })]);
    expect(latestWindow(s, cid).rows[0].content).toBe('newest');
  });

  test('edit timestamps retain their fractional precision', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { limit: 1 }, [msg(100, { content: 'new', edited_timestamp: '2026-10-22T12:02:00.000002+00:00' })]);
    applyHistoryPage(s, cid, { limit: 1 }, [msg(100, { content: 'old', edited_timestamp: '2026-10-22T12:02:00.000001Z' })]);
    expect(latestWindow(s, cid).rows[0].content).toBe('new');
  });

  test('a reaction dispatch protects the row even when its cached count already matches', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { limit: 1 }, [msg(100, { reactions: [] })]);
    const since = storeSeq(s);
    event(s, 'MESSAGE_REACTION_REMOVE_ALL', { message_id: '100' });
    applyHistoryPage(s, cid, { limit: 1 }, [msg(100, { reactions: [{ emoji: { id: null, name: '👍' }, count: 1, me: false }] })], { since });
    expect(latestWindow(s, cid).rows[0].reactions).toEqual([]);
  });

  test('fresh READY marks rows stale and a covering page removes missed deletions', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { limit: 5 }, page(96, 100));
    addPending(s, cid, { nonce: 'mine', content: 'hello', nowMs: 0 });
    readyAgain(s, '100');
    expect(latestWindow(s, cid).stale).toBe(true);
    applyHistoryPage(s, cid, { around: '98', limit: 5 }, page(96, 100).filter(row => row.id !== '99'));
    expect(latestWindow(s, cid)).toMatchObject({ stale: false });
    expect(ids(latestWindow(s, cid))).toEqual(['96', '97', '98', '100', null]);
    send(s, '99');
    expect(ids(latestWindow(s, cid))).not.toContain('99');
  });

  test('a page verifies only the rows it covers, and RESUMED keeps verified rows', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { limit: 5 }, page(96, 100));
    event(s, 'RESUMED', {});
    expect(latestWindow(s, cid).stale).toBe(false);
    readyAgain(s, '100');
    applyHistoryPage(s, cid, { around: '97', limit: 3 }, page(96, 98));
    expect(windowAround(s, cid, '97', 3).stale).toBe(false);
    expect(latestWindow(s, cid, 2).stale).toBe(true);
    applyHistoryPage(s, cid, { after: '98', limit: 3 }, page(99, 100));
    expect(latestWindow(s, cid).stale).toBe(false);
  });

  test('a page removes missing rows inside its cursor range but retains the excluded cursor', () => {
    for (const direction of ['before', 'after'] as const) {
      const s = seeded();
      applyHistoryPage(s, cid, { limit: 5 }, page(96, 100));
      const query = direction === 'before' ? { before: '100', limit: 3 } : { after: '96', limit: 3 };
      const rows = direction === 'before' ? [msg(98), msg(97), msg(96)] : [msg(100), msg(99), msg(98)];
      applyHistoryPage(s, cid, query, rows);
      expect(ids(latestWindow(s, cid))).toEqual(direction === 'before' ? ['96', '97', '98', '100'] : ['96', '98', '99', '100']);
    }
  });

  test('latest pages reconcile a deleted head, including a now-empty channel', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { limit: 5 }, page(96, 100));
    readyAgain(s, '100');
    applyHistoryPage(s, cid, { limit: 5 }, page(96, 99), { since: storeSeq(s) });
    expect(ids(latestWindow(s, cid))).toEqual(['96', '97', '98', '99']);
    expect(channel(s, cid)?.last_message_id).toBe('99');
    expect(latestWindow(s, cid).stale).toBe(false);
    applyHistoryPage(s, cid, { limit: 5 }, [], { since: storeSeq(s) });
    expect(latestWindow(s, cid)).toMatchObject({ rows: [], reachesStart: true, reachesNewest: true, stale: false });
    expect(channel(s, cid)?.last_message_id).toBe('99');
    send(s, '100');
    expect(latestWindow(s, cid).rows).toEqual([]);
  });

  test('without since, a latest page keeps rows newer than its own: they may have arrived in flight', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { limit: 5 }, page(96, 100));
    send(s, '101');
    applyHistoryPage(s, cid, { limit: 5 }, page(96, 100));
    expect(ids(latestWindow(s, cid))).toEqual(['96', '97', '98', '99', '100', '101']);
  });

  test('an empty latest snapshot retains messages created after its request', () => {
    const s = createStore();
    const since = storeSeq(s);
    send(s, '101');
    applyHistoryPage(s, cid, { limit: 5 }, [], { since });
    expect(ids(latestWindow(s, cid))).toEqual(['101']);
    expect(latestWindow(s, cid)).toMatchObject({ reachesStart: true, reachesNewest: true });
  });

  test('a request begun before READY cannot verify stale rows in the new session', () => {
    const s = seeded();
    applyHistoryPage(s, cid, { limit: 5 }, page(96, 100));
    const since = storeSeq(s);
    readyAgain(s, '100');
    applyHistoryPage(s, cid, { around: '98', limit: 5 }, page(96, 100), { since });
    expect(latestWindow(s, cid).stale).toBe(true);
    applyHistoryPage(s, cid, { around: '98', limit: 5 }, page(96, 100), { since: storeSeq(s) });
    expect(latestWindow(s, cid).stale).toBe(false);
  });

  test('deleting the newest rolls back unread state and DM order only over known history', () => {
    const s = seeded('90');
    event(s, 'CHANNEL_UPDATE', { id: cid, type: 1, last_message_id: '100' });
    event(s, 'CHANNEL_CREATE', { id: '2001', type: 1, last_message_id: '99' });
    applyHistoryPage(s, cid, { limit: 5 }, [msg(100), msg(99), msg(90)]);
    event(s, 'MESSAGE_DELETE', { id: '100' });
    event(s, 'MESSAGE_ACK', { message_id: '99' });
    expect(channel(s, cid)?.last_message_id).toBe('99');
    expect(unread(s, cid).unread).toBe(false);
    event(s, 'MESSAGE_DELETE_BULK', { ids: ['90', '99'] });
    expect(channel(s, cid)?.last_message_id).toBe('99');
    expect(dms(s).map(row => row.id)).toEqual([cid, '2001']);
    const gap = seeded();
    applyHistoryPage(gap, cid, { around: '50', limit: 3 }, page(49, 51));
    event(gap, 'MESSAGE_DELETE', { id: '100' });
    expect(channel(gap, cid)?.last_message_id).toBe('100');
  });

  test('a pending-only tail cannot answer a jump to an old message', () => {
    const s = seeded('500');
    addPending(s, cid, { nonce: 'mine', content: 'hello', nowMs: 0 });
    expect(windowAround(s, cid, '50')).toMatchObject({ rows: [], fetchAround: '50', fetchLatest: false });
    expect(latestWindow(s, cid)).toMatchObject({ fetchAround: null, fetchLatest: true });
  });

  for (const v of [1, 2]) test(`PASSIVE_UPDATE_V${v} advances channel metadata, preserves cursors and remembers members`, () => {
    const s = seeded();
    const version = channelVersion(s, cid), side = sidebarVersion(s);
    const type = `PASSIVE_UPDATE_V${v}`;
    const channelsKey = v === 1 ? 'channels' : 'updated_channels';
    const membersKey = v === 1 ? 'members' : 'updated_members';
    const payload = { guild_id: '10', [channelsKey]: [{ id: cid, last_message_id: '101', last_pin_timestamp: 'pinned' }],
      [membersKey]: [{ user: { id: '3', username: 'new member' } }] };
    applyDispatch(s, type, payload, 0);
    expect(channel(s, cid)).toMatchObject({ last_message_id: '101', last_pin_timestamp: 'pinned' });
    expect(unread(s, cid)).toEqual({ unread: true, mentionCount: 0, lastReadId: '100' });
    expect(sidebarVersion(s)).toBe(side + 1);
    expect(channelVersion(s, cid)).toBe(version + 1);
    expect(user(s, '3')?.username).toBe('new member');
    applyDispatch(s, type, { guild_id: '10', [channelsKey]: [{ id: cid, last_message_id: '99', last_pin_timestamp: null }] }, 0);
    expect(channel(s, cid)).toMatchObject({ last_message_id: '101', last_pin_timestamp: null });
    expect(unread(s, cid).lastReadId).toBe('100');
    expect(sidebarVersion(s)).toBe(side + 1);
  });

  test('batched reaction adds match singles, including an existing me reaction', () => {
    const single = seeded(), batch = seeded();
    const emoji = { id: null, name: '👍' }, custom = { id: '42', name: 'party' };
    for (const s of [single, batch]) applyHistoryPage(s, cid, { limit: 1 }, [msg(100, {
      reactions: [{ emoji, count: 1, me: true }],
    })]);
    const reactions = [{ emoji, users: ['2', '1', '3'] }, { emoji: custom, users: ['1', '2'] }];
    for (const r of reactions) for (const user_id of r.users) {
      event(single, 'MESSAGE_REACTION_ADD', { message_id: '100', emoji: r.emoji, user_id });
    }
    const version = channelVersion(batch, cid);
    event(batch, 'MESSAGE_REACTION_ADD_MANY', { message_id: '100', reactions });
    expect(latestWindow(batch, cid).rows[0].reactions).toEqual(latestWindow(single, cid).rows[0].reactions);
    expect(channelVersion(batch, cid)).toBe(version + 1);
    expect(latestWindow(batch, cid).rows[0].reactions).toEqual([
      { emoji, count: 3, me: true }, { emoji: custom, count: 2, me: true },
    ]);
  });
});
