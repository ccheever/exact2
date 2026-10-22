import { describe, expect, spyOn, test } from 'bun:test';
import { createHash } from 'node:crypto';
import { constants, createInflate } from 'node:zlib';
import { compareIds, createWorld, DISCORD_EPOCH, emptyMessage, NONCE_WINDOW_MS, pageMessages,
  recentNonceMessage, seededRandom, type Message } from './world.ts';
import { createZlibFramer, readyPayload, sizedReady, splitFrames, startFixture, syntheticGuild,
  type FixtureOptions } from './server.ts';

const hash = (value: unknown) => createHash('sha256').update(JSON.stringify(value)).digest('hex');

describe('seeded world', () => {
  test('same seed produces identical JSON, different seeds differ, generation stays under 300 ms', () => {
    const start = performance.now();
    const world = createWorld(1);
    const elapsed = performance.now() - start;
    console.log(`Excord fixture generation: ${elapsed.toFixed(1)} ms`);
    expect(elapsed).toBeLessThan(300);
    expect(hash(world)).toBe(hash(createWorld(1)));
    const other = createWorld(2);
    expect(hash(world)).not.toBe(hash(other));
    expect(world.messages[world.generalChannelId].map(message => message.content))
      .not.toEqual(other.messages[other.generalChannelId].map(message => message.content));
  });

  test('guilds, DMs, users and read state form a coherent user-account READY', () => {
    const world = createWorld();
    expect(world.guilds).toHaveLength(3);
    expect(world.token).toBe('fixture-token');
    const userIds = new Set(world.users.map(user => user.id));
    expect(userIds.size).toBeGreaterThanOrEqual(8);
    for (const guild of world.guilds) {
      expect(guild.icon).toMatch(/^[a-f\d]{32}$/);
      expect(guild.channels.some(channel => channel.type === 4)).toBe(true);
      expect(guild.channels.some(channel => channel.type === 5)).toBe(true);
      for (const channel of guild.channels) {
        expect(channel.guild_id).toBe(guild.id);
        if (channel.parent_id) expect(guild.channels.some(parent => parent.id === channel.parent_id && parent.type === 4)).toBe(true);
      }
      for (const member of guild.members) {
        expect(userIds.has(member.user.id)).toBe(true);
        expect(member.roles.every(id => guild.roles.some(role => role.id === id))).toBe(true);
      }
    }
    expect(world.private_channels.filter(channel => channel.type === 1).length).toBeGreaterThan(1);
    expect(world.private_channels.filter(channel => channel.type === 3)).toHaveLength(1);
    for (const channel of world.private_channels) {
      expect(channel.recipient_ids!.every(id => userIds.has(id))).toBe(true);
    }
    let read = 0, unread = 0, mentions = 0;
    for (const entry of world.read_state.entries) {
      const channel = world.channels.find(channel => channel.id === entry.id)!;
      if (entry.last_message_id === channel.last_message_id) read++; else unread++;
      if (entry.mention_count) mentions++;
    }
    expect(Math.min(read, unread, mentions)).toBeGreaterThan(0);
    const ready = readyPayload(world, 'session', 'ws://fixture/');
    expect(ready.guilds.find(guild => guild.id === world.lazyGuildId)).toEqual({ id: world.lazyGuildId, unavailable: true });
    expect(ready.guilds[0]).toHaveProperty('properties.name', world.guilds[0].name);
    expect(ready.private_channels[0]).toHaveProperty('recipient_ids');
    expect(new Set(ready.users.map(user => user.id)).size).toBe(ready.users.length);
    expect(ready.read_state.partial).toBe(false);
    const general = world.messages[world.generalChannelId];
    expect(ready.read_state.entries.find(entry => entry.id === world.generalChannelId)).toMatchObject({
      last_message_id: general.at(-151)!.id, mention_count: 3,
    });
    for (const channel of world.private_channels) {
      expect(world.messages[channel.id].every(message => message.author.id === world.me.id
        || channel.recipient_ids!.includes(message.author.id))).toBe(true);
    }
  });

  test('general has exactly 10,000 increasing snowflakes, author bursts, gaps and a year of day changes', () => {
    const world = createWorld();
    const history = world.messages[world.generalChannelId];
    expect(history).toHaveLength(10000);
    let grouped = 0, gaps = 0, run = 1, sameAuthorLongGap = 0;
    const runs = new Set<number>(), gapKinds = new Set<string>(), hours = new Map<string, number>();
    const authors = new Set<string>(), days = new Set<string>(), ids = new Set<string>();
    for (let i = 0; i < history.length; i++) {
      const message = history[i];
      authors.add(message.author.id); days.add(message.timestamp.slice(0, 10)); ids.add(message.id);
      const hour = message.timestamp.slice(0, 13);
      hours.set(hour, (hours.get(hour) ?? 0) + 1);
      expect(message.nonce).toBeUndefined();
      expect(Number(BigInt(message.id) >> 22n) + DISCORD_EPOCH).toBe(Date.parse(message.timestamp));
      if (!i) continue;
      expect(compareIds(history[i - 1].id, message.id)).toBeLessThan(0);
      const gap = Date.parse(message.timestamp) - Date.parse(history[i - 1].timestamp);
      expect(gap).toBeGreaterThan(0);
      const sameAuthor = history[i - 1].author.id === message.author.id;
      if (sameAuthor) run++; else { runs.add(run); run = 1; }
      if (gap < 7 * 60000 && sameAuthor) grouped++;
      if (gap > 7 * 60000) gaps++;
      if (gap > 7 * 60000 && sameAuthor) sameAuthorLongGap++;
      gapKinds.add(gap < 60000 ? 'seconds' : gap < 3600000 ? 'minutes' : gap < 86400000 ? 'hours' : 'days');
    }
    expect(ids.size).toBe(10000);
    expect(authors.size).toBeGreaterThanOrEqual(8);
    runs.add(run);
    expect([...runs].sort((a, b) => a - b)).toEqual(Array.from({ length: 12 }, (_, i) => i + 1));
    expect(gapKinds.size).toBe(4);
    expect(sameAuthorLongGap).toBeGreaterThan(100);
    expect(Math.max(...hours.values())).toBeGreaterThan(20);
    expect(grouped).toBeGreaterThan(6000);
    expect(gaps).toBeGreaterThan(1000);
    expect(days.size).toBeGreaterThan(200);
    expect(days.size).toBeLessThan(350);
    const spanDays = (Date.parse(history.at(-1)!.timestamp) - Date.parse(history[0].timestamp)) / 86400000;
    expect(spanDays).toBeGreaterThan(350);
    expect(spanDays).toBeLessThan(370);
    for (const [id, messages] of Object.entries(world.messages)) {
      if (id !== world.generalChannelId) expect(messages.length).toBeLessThanOrEqual(200);
      expect(world.channels.find(channel => channel.id === id)!.last_message_id).toBe(messages.at(-1)?.id ?? null);
    }
    expect(Object.values(world.messages).some(messages => messages.length === 0)).toBe(true);
  });

  test('Markdown, image sizes, embeds, reactions, edits and reply snapshots cover transcript shapes', () => {
    const world = createWorld();
    const history = world.messages[world.generalChannelId];
    const text = history.map(message => message.content).join('\n');
    for (const marker of ['**bold**', '*italic*', '__underlined__', '~~old plans~~', '||', '`const',
      '```typescript', '> ', '# ', '## ', '### ', '- ', '1. ', '](', '<@', '<#', '<@&', '<:workshop:', '<a:dance:', '<t:']) {
      expect(text).toContain(marker);
    }
    expect(new Set([...text.matchAll(/<t:\d+:([RtTdDfF])>/g)].map(match => match[1])).size).toBe(7);
    let images = 0, replies = 0, reactions = 0, edits = 0, deleted = 0, recent = 0, far = 0, total = 0;
    const sizes = new Set<string>(), attachmentCounts = new Set<number>(), fileTypes = new Set<string>();
    const systemTypes = new Set<number>();
    for (const messages of Object.values(world.messages)) {
      const earlier = new Map<string, { message: Message; index: number }>();
      for (const [index, message] of messages.entries()) {
        total++;
        expect(message.tts).toBe(false);
        expect(message.flags).toBe(0);
        if (message.edited_timestamp) {
          edits++;
          expect(Date.parse(message.edited_timestamp)).toBeGreaterThan(Date.parse(message.timestamp));
        }
        if (message.type === 6 || message.type === 7) {
          systemTypes.add(message.type);
          expect(message.content).toBe('');
          if (message.type === 6) expect(message.message_reference).toBeDefined();
        }
        reactions += message.reactions.length;
        for (const reaction of message.reactions) {
          expect(reaction.count).toBeGreaterThanOrEqual(1);
          expect(reaction.count).toBeLessThanOrEqual(30);
          expect(reaction.count_details.normal).toBe(reaction.count);
        }
        attachmentCounts.add(message.attachments.length);
        for (const file of message.attachments.filter(attachment => !attachment.content_type.startsWith('image/'))) {
          expect(file.width).toBeUndefined(); expect(file.height).toBeUndefined();
          expect(file.url).toBe(file.proxy_url);
          fileTypes.add(file.content_type);
        }
        for (const image of [...message.attachments.filter(attachment => attachment.content_type.startsWith('image/')),
          ...message.embeds.flatMap(embed => [embed.image, embed.thumbnail, embed.video].filter(Boolean))]) {
          images++;
          expect(image!.width).toBeGreaterThan(0);
          expect(image!.height).toBeGreaterThan(0);
          expect(image!.url).toBe(image!.proxy_url);
          sizes.add(`${image!.width}x${image!.height}`);
          expect(new URL(image!.url).pathname).toBe(`/assets/${image!.width}x${image!.height}.svg`);
        }
        if (message.message_reference) {
          expect([6, 19]).toContain(message.type);
          expect(message.message_reference.channel_id).toBe(message.channel_id);
          expect(compareIds(message.message_reference.message_id, message.id)).toBeLessThan(0);
          const referenced = earlier.get(message.message_reference.message_id);
          if (message.referenced_message === null) {
            deleted++;
            expect(message.type).toBe(19);
            expect(messages.some(value => value.id === message.message_reference!.message_id)).toBe(false);
          } else {
            expect(referenced).toBeDefined();
            expect(message.referenced_message!.id).toBe(referenced!.message.id);
            expect(message.referenced_message!.content).toBe(referenced!.message.content);
            expect(message.referenced_message).not.toBe(referenced!.message);
            expect(message.referenced_message!.referenced_message).toBeUndefined();
            if (message.type === 6) expect(referenced!.message.pinned).toBe(true);
            if (message.type === 19) {
              replies++;
              const distance = index - referenced!.index;
              if (distance <= 30) recent++;
              if (distance >= 1000) far++;
            }
          }
        }
        earlier.set(message.id, { message, index });
      }
    }
    expect(Math.min(images, replies, reactions, edits)).toBeGreaterThan(100);
    expect(edits / total).toBeGreaterThan(0.03); expect(edits / total).toBeLessThan(0.05);
    expect(deleted).toBe(1);
    expect(recent).toBeGreaterThan(100); expect(far).toBeGreaterThan(100);
    expect([...systemTypes].sort()).toEqual([6, 7]);
    expect([...attachmentCounts].sort()).toEqual([0, 1, 2, 3, 4]);
    expect([...fileTypes].sort()).toEqual(['application/pdf', 'application/zip']);
    expect(sizes.size).toBeGreaterThanOrEqual(10);
    for (const size of ['4032x3024', '750x2400', '32x24', '800x800']) expect(sizes.has(size)).toBe(true);
    for (const type of ['image', 'rich', 'gifv', 'video']) {
      expect(history.some(message => message.embeds.some(embed => embed.type === type))).toBe(true);
    }
    expect(history.some(message => message.reactions.length >= 3)).toBe(true);
    expect(history.some(message => message.reactions.some(reaction => reaction.me))).toBe(true);
    expect(history.some(message => message.reactions.some(reaction => reaction.emoji.id)
      && message.reactions.some(reaction => reaction.emoji.id === null))).toBe(true);
    expect(history.some(message => message.mentions.length)).toBe(true);
    expect(history.some(message => message.mention_roles.length)).toBe(true);
    expect(history.some(message => message.mention_everyone)).toBe(true);
    const distant = history.find(message => message.type === 19 && message.referenced_message
      && compareIds(message.referenced_message.id, history[1000].id) < 0 && compareIds(message.id, history[5000].id) > 0)!;
    expect(distant).toBeDefined();
    const page = pageMessages(history, { around: distant.message_reference!.message_id });
    expect(page.some(message => message.id === distant.message_reference!.message_id)).toBe(true);
  });

  test('content mixes short chat, sentences, paragraphs, multiline, emoji, links and combined Markdown', () => {
    for (const seed of [1, 2, 91]) {
      const world = createWorld(seed), history = world.messages[world.generalChannelId];
      const texts = history.map(message => message.content).filter(Boolean);
      const short = texts.filter(text => !text.includes('\n') && text.split(/\s+/).length <= 8 && /[a-z]/i.test(text));
      expect(short.length).toBeGreaterThan(texts.length / 2);
      expect(texts.some(text => !text.includes('\n') && text.split(/\s+/).length > 8 && text.length < 200)).toBe(true);
      expect(texts.some(text => !text.includes('\n') && text.length > 400)).toBe(true);
      expect(texts.some(text => text.split('\n').length >= 3)).toBe(true);
      expect(texts.some(text => /^(?:\p{Extended_Pictographic}|\p{Emoji_Modifier}|\uFE0F|\s)+$/u.test(text))).toBe(true);
      expect(texts.some(text => /^https:\/\/\S+$/.test(text))).toBe(true);
      expect(texts.some(text => text.includes('```') && text.includes('**') && text.includes('<@'))).toBe(true);
      expect(texts.slice(0, -13).some((text, index) => text !== texts[index + 13])).toBe(true);
    }
  });
});

describe('Discord history paging', () => {
  const world = createWorld();
  const messages = world.messages[world.generalChannelId];
  const ids = (messages: Message[]) => messages.map(message => message.id);

  test('latest is newest first, limits clamp to 1..100 and empty histories stay empty', () => {
    expect(ids(pageMessages(messages))).toEqual(ids(messages.slice(-50).reverse()));
    expect(pageMessages(messages, { limit: 0 })).toEqual([messages.at(-1)!]);
    expect(pageMessages(messages, { limit: -50 })).toEqual([messages.at(-1)!]);
    expect(pageMessages(messages, { limit: 1000 })).toHaveLength(100);
    expect(pageMessages(messages, { limit: NaN })).toHaveLength(50);
    expect(pageMessages([])).toEqual([]);
  });

  test('before and after are exclusive and adjacent to the cursor, including absent ids and edges', () => {
    expect(ids(pageMessages(messages, { before: messages[100].id, limit: 3 }))).toEqual(ids(messages.slice(97, 100).reverse()));
    expect(ids(pageMessages(messages, { after: messages[100].id, limit: 3 }))).toEqual(ids(messages.slice(101, 104).reverse()));
    expect(pageMessages(messages, { before: messages[0].id })).toEqual([]);
    expect(pageMessages(messages, { after: messages.at(-1)!.id })).toEqual([]);
    expect(pageMessages(messages, { before: messages[1].id })).toEqual([messages[0]]);
    expect(pageMessages(messages, { after: messages.at(-2)!.id })).toEqual([messages.at(-1)!]);
    const missing = (BigInt(messages[100].id) + 1n).toString();
    expect(pageMessages(messages, { before: missing, limit: 1 })).toEqual([messages[100]]);
    expect(pageMessages(messages, { after: missing, limit: 1 })).toEqual([messages[101]]);
  });

  test('around includes its anchor within the limit and fills either edge', () => {
    expect(ids(pageMessages(messages, { around: messages[100].id, limit: 5 }))).toEqual(ids(messages.slice(98, 103).reverse()));
    expect(ids(pageMessages(messages, { around: messages[100].id, limit: 4 }))).toEqual(ids(messages.slice(98, 102).reverse()));
    expect(pageMessages(messages, { around: messages[100].id, limit: 1000 })).toHaveLength(100);
    expect(pageMessages(messages, { around: messages[100].id, limit: 1 })).toEqual([messages[100]]);
    expect(ids(pageMessages(messages, { around: messages[0].id, limit: 5 }))).toEqual(ids(messages.slice(0, 5).reverse()));
    expect(ids(pageMessages(messages, { around: messages.at(-1)!.id, limit: 5 }))).toEqual(ids(messages.slice(-5).reverse()));
    for (const index of [0, 1, 2, 100, messages.length - 3, messages.length - 2, messages.length - 1]) {
      for (const limit of [1, 2, 3, 4, 5, 99, 100]) {
        const page = pageMessages(messages, { around: messages[index].id, limit });
        expect(page).toHaveLength(limit);
        expect(page).toContain(messages[index]);
        expect(ids(page)).toEqual(ids([...page].sort((a, b) => compareIds(b.id, a.id))));
      }
    }
    const short = messages.slice(0, 3);
    expect(pageMessages(short, { around: short[1].id, limit: 5 })).toEqual([...short].reverse());
    expect(pageMessages([], { around: messages[0].id, limit: 5 })).toEqual([]);
  });

  test('around absent cursors fills the window and refuses conflicting cursors', () => {
    const missing = (BigInt(messages[100].id) + 1n).toString();
    expect(pageMessages(messages, { around: missing, limit: 4 })).toEqual(messages.slice(99, 103).reverse());
    expect(pageMessages(messages, { around: '0', limit: 5 })).toEqual(messages.slice(0, 5).reverse());
    const afterLast = (BigInt(messages.at(-1)!.id) + 1n).toString();
    expect(pageMessages(messages, { around: afterLast, limit: 5 })).toEqual(messages.slice(-5).reverse());
    expect(() => pageMessages(messages, { before: '1', after: '2' })).toThrow('mutually exclusive');
    expect(() => pageMessages(messages, { before: '1', around: '2' })).toThrow('mutually exclusive');
  });
});

test('nonce matching requires the same author and an unexpired creation time', () => {
  const world = createWorld(), now = Date.UTC(2026, 0, 1);
  const message = { ...emptyMessage(world.private_channels[0], world.me, now, 'original'), nonce: '42' };
  const history = [message];
  expect(recentNonceMessage(history, world.me.id, 42, now)).toBe(message);
  expect(recentNonceMessage(history, world.me.id, '42', now + NONCE_WINDOW_MS - 1)).toBe(message);
  expect(recentNonceMessage(history, world.me.id, '42', now + NONCE_WINDOW_MS)).toBeUndefined();
  expect(recentNonceMessage(history, world.me.id, '42', now + 86400000)).toBeUndefined();
  expect(recentNonceMessage(history, world.users[1].id, '42', now)).toBeUndefined();
  expect(recentNonceMessage(history, world.me.id, '43', now)).toBeUndefined();
  expect(recentNonceMessage(history, world.me.id, '42', now - 1)).toBeUndefined();
  expect(recentNonceMessage(history, world.me.id, '42', now + 99, 100)).toBe(message);
  expect(recentNonceMessage(history, world.me.id, '42', now + 100, 100)).toBeUndefined();
});

function fixtureWithoutListener(options: FixtureOptions = {}) {
  let handle!: (request: Request, server: unknown) => Promise<Response>;
  let gateway: any;
  // Exercise the actual HTTP handler while replacing only the port binding.
  const serve = spyOn(Bun, 'serve').mockImplementation(((config: any) => {
    handle = config.fetch;
    gateway = config.websocket;
    return { port: 4790, stop: async () => {} };
  }) as typeof Bun.serve);
  let fixture: ReturnType<typeof startFixture>;
  try { fixture = startFixture(options); } finally { serve.mockRestore(); }
  const request = (path: string, init: RequestInit = {}) => {
    const headers = new Headers(init.headers);
    headers.set('Authorization', fixture.world.token);
    return handle(new Request(fixture.origin + path, { ...init, headers }), {});
  };
  const control = async (name: string, body: object = {}) => {
    const reply = await request(`/control/${name}`, { method: 'POST', body: JSON.stringify(body) });
    expect(reply.status).toBe(200);
    return reply.json();
  };
  const connect = async (capabilities: number) => {
    const frames: any[] = [];
    let socket: any;
    await handle(new Request(fixture.gatewayUrl.replace('ws:', 'http:'), { headers: { Upgrade: 'websocket' } }), {
      upgrade(_request: Request, { data }: any) {
        socket = { data, send(text: string) { frames.push(JSON.parse(text)); return text.length; },
          close(code: number) { gateway.close(socket, code); } };
        gateway.open(socket);
        return true;
      },
    });
    await socket.data.queue;
    const send = (frame: object) => gateway.message(socket, JSON.stringify(frame));
    await send({ op: 2, d: { token: fixture.world.token, capabilities } });
    return { frames, send, close: (code: number) => socket.close(code) };
  };
  return { fixture, request, control, connect };
}

describe('fixture HTTP handler without sockets', () => {
  test('clock advances nonce expiry, timestamps and rate-limit windows, then reset clears it', async () => {
    const clock = spyOn(Date, 'now').mockReturnValue(Date.UTC(2026, 0, 1));
    const { fixture, request, control } = fixtureWithoutListener({
      rateLimits: { default: { limit: 1, windowMs: 5000 }, messages: { limit: 100, windowMs: 5000 } },
    });
    try {
      const channelId = fixture.world.generalChannelId, path = `/api/v9/channels/${channelId}/messages`;
      const post = async (content: string, enforce_nonce = true, channelPath = path) => {
        const reply = await request(channelPath, { method: 'POST',
          body: JSON.stringify({ content, nonce: 'retry', enforce_nonce }) });
        expect(reply.status).toBe(200);
        return reply.json();
      };
      const first = await post('first');
      expect(first.timestamp).toBe(new Date(Date.now()).toISOString());
      const second = await post('unenforced', false);
      expect(second.id).not.toBe(first.id);
      expect(compareIds(first.id, second.id)).toBeLessThan(0);
      expect(second.timestamp).toBe(first.timestamp);
      expect((await post('retry')).id).toBe(second.id);
      const dmPath = `/api/v9/channels/${fixture.world.private_channels[0].id}/messages`;
      expect((await post('same author, another channel', true, dmPath)).id).toBe(second.id);

      const me = await request('/api/v9/users/@me');
      expect(me.status).toBe(200);
      expect((await request('/api/v9/users/@me')).status).toBe(429);
      await control('clock', { advance_ms: 4999 });
      expect((await request('/api/v9/users/@me')).status).toBe(429);
      await control('clock', { advance_ms: 1 });
      expect((await request('/api/v9/users/@me')).status).toBe(200);
      await control('clock', { advance_ms: NONCE_WINDOW_MS - 5001 });
      expect((await post('still a retry')).id).toBe(second.id);
      await control('clock', { advance_ms: 1 });
      const expired = await post('after expiry');
      expect(expired.id).not.toBe(second.id);
      expect(expired.content).toBe('after expiry');
      expect(Date.parse(expired.timestamp) - Date.parse(first.timestamp)).toBe(NONCE_WINDOW_MS);
      await control('clock', { advance_ms: 1000 });
      const edited = await control('edit', { channel_id: channelId, message_id: expired.id, content: 'edited' });
      expect(Date.parse(edited.edited_timestamp)).toBe(Date.now() + NONCE_WINDOW_MS + 1000);
      await control('burst', { channel_id: channelId, count: 1 });
      expect(fixture.world.messages[channelId].at(-1)!.timestamp).toBe(edited.edited_timestamp);
      const state = await (await request('/control/state')).json();
      expect(state).toMatchObject({ clock_offset_ms: NONCE_WINDOW_MS + 1000,
        now_ms: Date.now() + NONCE_WINDOW_MS + 1000, nonce_window_ms: NONCE_WINDOW_MS });
      expect((await request('/control/clock', { method: 'POST', body: '{"advance_ms":-1}' })).status).toBe(400);
      await control('reset');
      expect(await (await request('/control/state')).json()).toMatchObject({ clock_offset_ms: 0, now_ms: Date.now() });
      expect((await request('/api/v9/users/@me')).status).toBe(200);
    } finally { await fixture.close(); clock.mockRestore(); }
  });

  test('nonce window is configurable', async () => {
    const clock = spyOn(Date, 'now').mockReturnValue(Date.UTC(2026, 0, 1));
    const { fixture, request, control } = fixtureWithoutListener({ nonceWindowMs: 100 });
    try {
      const send = async () => (await request(`/api/v9/channels/${fixture.world.generalChannelId}/messages`, {
        method: 'POST', body: JSON.stringify({ content: 'send', nonce: 123, enforce_nonce: true }),
      })).json();
      const first = await send();
      await control('clock', { advance_ms: 99 });
      expect((await send()).id).toBe(first.id);
      await control('clock', { advance_ms: 1 });
      expect((await send()).id).not.toBe(first.id);
    } finally { await fixture.close(); clock.mockRestore(); }
  });

  test('assets decode at their declared SVG dimensions and non-image files have no dimensions', async () => {
    const { fixture, request } = fixtureWithoutListener();
    try {
      const assets = new Map<string, { width?: number; height?: number; size?: number; content_type?: string }>();
      for (const messages of Object.values(fixture.world.messages)) for (const message of messages) {
        for (const asset of [...message.attachments,
          ...message.embeds.flatMap(embed => [embed.image, embed.thumbnail, embed.video].filter(Boolean))]) {
          assets.set(asset!.url, asset!);
        }
      }
      for (const [url, asset] of assets) {
        const reply = await request(new URL(url).pathname);
        expect(reply.status).toBe(200);
        if (asset.width !== undefined) {
          expect(reply.headers.get('content-type')).toBe('image/svg+xml');
          const svg = await reply.text();
          const root = /^<svg\b[^>]*>/.exec(svg)![0];
          expect(Number(/\bwidth="(\d+)"/.exec(root)![1])).toBe(asset.width);
          expect(Number(/\bheight="(\d+)"/.exec(root)![1])).toBe(asset.height);
          if (asset.size !== undefined) expect(Buffer.byteLength(svg)).toBe(asset.size);
        } else {
          expect(asset.height).toBeUndefined();
          expect(reply.headers.get('content-type')).toBe(asset.content_type!);
          expect((await reply.arrayBuffer()).byteLength).toBe(asset.size!);
        }
      }
      expect((await request('/assets/0x20.svg')).status).toBe(404);
      expect((await request('/assets/999999999x20.svg')).status).toBe(404);
    } finally { await fixture.close(); }
  });

  test('preflights bypass buckets and leave forced route and global limits for REST requests', async () => {
    const { fixture, request, control } = fixtureWithoutListener({ rateLimits: { default: { limit: 1, windowMs: 60000 } } });
    try {
      for (const global of [false, true]) {
        await control('reset');
        await control('rate-limit', { global, route: global ? undefined : '/users/@me', count: 2, retry_after: 30 });
        for (let i = 0; i < 3; i++) {
          const reply = await request('/api/v9/users/@me', { method: 'OPTIONS', headers: {
            Origin: 'http://fixture-client.invalid', 'Access-Control-Request-Method': 'GET',
            'Access-Control-Request-Headers': 'Authorization, X-Super-Properties',
          } });
          expect(reply.status).toBe(204);
          expect(reply.headers.get('Access-Control-Allow-Origin')).toBe('http://fixture-client.invalid');
          expect(reply.headers.get('Access-Control-Allow-Headers')).toContain('X-Super-Properties');
          expect(reply.headers.has('X-RateLimit-Bucket')).toBe(false);
          expect(reply.headers.has('Retry-After')).toBe(false);
        }
        for (let i = 0; i < 2; i++) {
          const reply = await request('/api/v9/users/@me');
          expect(reply.status).toBe(429);
          expect(await reply.json()).toMatchObject({ retry_after: 30, global });
        }
        const admitted = await request('/api/v9/users/@me');
        expect(admitted.status).toBe(200);
        expect(admitted.headers.get('X-RateLimit-Remaining')).toBe('0');
        expect((await request('/api/v9/users/@me')).status).toBe(429);
        expect((await request('/api/v9/users/@me', { method: 'OPTIONS' })).status).toBe(204);
        const state = await (await request('/control/state')).json();
        expect(state.requests.filter((value: any) => value.method === 'OPTIONS')).toHaveLength(4);
        expect(state.logins_done).toBe(0);
      }
    } finally { await fixture.close(); }
  });

  test('bursts share history shapes, accept a seed and author, and remain fetchable through REST', async () => {
    const clock = spyOn(Date, 'now').mockReturnValue(Date.UTC(2026, 0, 1));
    const { fixture, request, control } = fixtureWithoutListener();
    try {
      const channelId = fixture.world.generalChannelId;
      const burst = async (seed: number) => {
        await control('reset');
        const before = fixture.world.messages[channelId].at(-1)!;
        const result = await control('burst', { channel_id: channelId, count: 600, seed });
        const messages = fixture.world.messages[channelId].slice(-600);
        expect(result.count).toBe(600);
        expect(result.ids).toEqual(messages.map(message => message.id));
        expect(compareIds(before.id, messages[0].id)).toBeLessThan(0);
        expect(new Set(messages.map(message => message.author.id)).size).toBeGreaterThan(8);
        expect(messages.some(message => message.content.length > 400)).toBe(true);
        expect(messages.some(message => message.attachments.length > 1)).toBe(true);
        expect(messages.some(message => message.embeds.length)).toBe(true);
        expect(messages.some(message => message.reactions.length > 1)).toBe(true);
        expect(messages.some(message => message.type === 19)).toBe(true);
        expect(messages.some(message => message.content.includes('Workshop update'))).toBe(false);
        const page = await (await request(`/api/v9/channels/${channelId}/messages?limit=100`)).json();
        expect(page).toEqual(messages.slice(-100).reverse());
        return hash(messages);
      };
      const first = await burst(19);
      expect(await burst(19)).toBe(first);
      expect(await burst(20)).not.toBe(first);
      const authorId = fixture.world.users[3].id;
      await control('burst', { channel_id: channelId, count: 30, author_id: authorId, seed: 0 });
      expect(fixture.world.messages[channelId].slice(-30).every(message => message.author.id === authorId)).toBe(true);
      const badSeed = await request('/control/burst', { method: 'POST',
        body: JSON.stringify({ channel_id: channelId, count: 1, seed: -1 }) });
      expect(badSeed.status).toBe(400);
    } finally { await fixture.close(); clock.mockRestore(); }
  });

  test('toggling a seeded reaction preserves counts greater than the named user population', async () => {
    const { fixture, control } = fixtureWithoutListener();
    try {
      const history = fixture.world.messages[fixture.world.generalChannelId];
      const message = history.find(message => message.reactions.some(reaction => reaction.count > 20 && !reaction.me))!;
      const reaction = message.reactions.find(reaction => reaction.count > 20 && !reaction.me)!;
      const count = reaction.count;
      const body = { channel_id: message.channel_id, message_id: message.id, emoji: reaction.emoji,
        user_id: fixture.world.me.id };
      await control('react', { ...body, add: true });
      expect(reaction.count).toBe(count + 1); expect(reaction.me).toBe(true);
      await control('react', { ...body, add: true });
      expect(reaction.count).toBe(count + 1);
      await control('react', { ...body, add: false });
      expect(reaction.count).toBe(count); expect(reaction.me).toBe(false);
      expect(reaction.count_details.normal).toBe(count);
    } finally { await fixture.close(); }
  });
});

describe('gateway handlers and framing without sockets', () => {
  test('capabilities select passive guild updates until a subscription, while DMs stay full', async () => {
    const { fixture, request, control, connect } = fixtureWithoutListener();
    try {
      const v1 = await connect(1 << 11), v2 = await connect((1 << 11) | (1 << 14));
      const plain = await connect(0), subscribed = await connect(1 << 14);
      const channelId = fixture.world.generalChannelId, guildId = fixture.world.guilds[0].id;
      await subscribed.send({ op: 14, d: { guild_id: guildId } });
      const clients = [v1, v2, plain, subscribed];
      for (const client of clients) client.frames.length = 0;
      const burst = await control('burst', { channel_id: channelId, count: 1 });
      const channels = [{ id: channelId, last_message_id: burst.ids[0], last_pin_timestamp: null }];
      expect(v1.frames.map(frame => [frame.t, frame.d])).toEqual([['PASSIVE_UPDATE_V1', {
        guild_id: guildId, channels, members: [], voice_states: [],
      }]]);
      expect(v2.frames.map(frame => [frame.t, frame.d])).toEqual([['PASSIVE_UPDATE_V2', {
        guild_id: guildId, updated_channels: channels, updated_members: [],
        removed_voice_states: [], updated_voice_states: [],
      }]]);
      for (const client of [plain, subscribed]) expect(client.frames.at(-1).t).toBe('MESSAGE_CREATE');
      const injected = { channel_id: channelId, message_id: burst.ids[0] };
      await control('edit', { ...injected, content: 'edited' });
      await control('react-many', { ...injected, emoji: '🌵', user_ids: [fixture.world.me.id, fixture.world.users[1].id] });
      await control('typing', { channel_id: channelId, user_id: fixture.world.users[1].id });
      await control('delete', injected);
      expect(v1.frames).toHaveLength(1);
      expect(v2.frames).toHaveLength(1);
      expect(subscribed.frames.map(frame => frame.t)).toEqual([
        'MESSAGE_CREATE', 'MESSAGE_UPDATE', 'MESSAGE_REACTION_ADD', 'MESSAGE_REACTION_ADD', 'TYPING_START', 'MESSAGE_DELETE',
      ]);
      await control('burst', { channel_id: fixture.world.private_channels[0].id, count: 1 });
      for (const client of clients) {
        expect(client.frames.at(-1).t).toBe('MESSAGE_CREATE');
        expect(client.frames.at(-1).d.guild_id).toBeUndefined();
      }
      await v2.send({ op: 37, d: { subscriptions: { [guildId]: { typing: true } } } });
      await control('burst', { channel_id: channelId, count: 1 });
      expect(v2.frames.at(-1).t).toBe('MESSAGE_CREATE');
      expect(v1.frames.at(-1).t).toBe('PASSIVE_UPDATE_V1');
      await control('subscribe', { guild_id: guildId });
      await control('burst', { channel_id: channelId, count: 1 });
      for (const client of clients) expect(client.frames.at(-1).t).toBe('MESSAGE_CREATE');
      const state = await (await request('/control/state')).json();
      expect(state.passive_updates).toBe(3);
      expect(state.session_details.map((session: any) => session.passive_updates)).toEqual([2, 1, 0, 0]);
      expect(state.session_details[0].capabilities).toBe(1 << 11);
      expect(state.session_details[1].subscriptions).toEqual([{ op: 37, guild_ids: [guildId] }]);
      expect(state.session_details[3].subscriptions).toEqual([{ op: 14, guild_ids: [guildId] }]);
      expect(state.session_details.every((session: any) => session.subscribed_guilds.includes(guildId))).toBe(true);
      expect(state.logins_done).toBe(4);
    } finally { await fixture.close(); }
  });

  test('subscribeAll applies to new sessions and heartbeat requests retain ACK behavior', async () => {
    const { fixture, request, control, connect } = fixtureWithoutListener({ subscribeAll: true });
    try {
      const first = await connect(1 << 14), second = await connect(1 << 11);
      const state = await (await request('/control/state')).json();
      for (const session of state.session_details) {
        expect(session.subscribed_guilds).toEqual(fixture.world.guilds.map(guild => guild.id));
      }
      await control('burst', { channel_id: fixture.world.generalChannelId, count: 1 });
      for (const client of [first, second]) expect(client.frames.at(-1).t).toBe('MESSAGE_CREATE');
      await control('heartbeat-request');
      for (const client of [first, second]) expect(client.frames.at(-1)).toEqual({ op: 1, d: null });
      await first.send({ op: 1, d: 4 });
      expect(first.frames.at(-1)).toEqual({ op: 11, d: null });
      await control('stop-acks', { on: true });
      const before = first.frames.length;
      await first.send({ op: 1, d: 4 });
      expect(first.frames).toHaveLength(before);
    } finally { await fixture.close(); }
  });

  test('reaction batches honor capabilities, the clock, message boundaries and removal order', async () => {
    const clock = spyOn(Date, 'now').mockReturnValue(Date.UTC(2026, 0, 1));
    const { fixture, request, control, connect } = fixtureWithoutListener();
    try {
      const batched = await connect(1 << 13), plain = await connect(0);
      const passive = await connect((1 << 13) | (1 << 14));
      for (const client of [batched, plain, passive]) client.frames.length = 0;
      const channelId = fixture.world.generalChannelId;
      const target = { channel_id: channelId, message_id: fixture.world.messages[channelId].at(-1)!.id };
      const userIds = fixture.world.users.slice(0, 3).map(user => user.id);
      await control('react-many', { ...target, emoji: '🌵', user_ids: userIds });
      expect(batched.frames.map(frame => [frame.t, frame.d])).toEqual([['MESSAGE_REACTION_ADD_MANY', {
        ...target, guild_id: fixture.world.guilds[0].id, reactions: [{ emoji: { id: null, name: '🌵' }, users: userIds }],
      }]]);
      expect(plain.frames.map(frame => frame.t)).toEqual(Array(3).fill('MESSAGE_REACTION_ADD'));
      expect(passive.frames).toEqual([]);
      const add = (emoji: string, user_id: string, destination = target) => control('react', {
        ...destination, emoji, user_id, add: true,
      });
      await add('🍋', userIds[0]);
      await control('clock', { advance_ms: 499 });
      expect(batched.frames).toHaveLength(1);
      await add('🍋', userIds[1]);
      await add('🍊', userIds[2]);
      await control('clock', { advance_ms: 1 });
      expect(batched.frames.at(-1).d.reactions).toEqual([
        { emoji: { id: null, name: '🍋' }, users: userIds.slice(0, 2) },
        { emoji: { id: null, name: '🍊' }, users: [userIds[2]] },
      ]);
      const dmId = fixture.world.private_channels[0].id;
      const dmTarget = { channel_id: dmId, message_id: fixture.world.messages[dmId].at(-1)!.id };
      await add('🌵', userIds[0], dmTarget);
      await add('🍉', userIds[0]);
      await control('clock', { advance_ms: 500 });
      expect(batched.frames.slice(-2).map(frame => frame.t)).toEqual(['MESSAGE_REACTION_ADD', 'MESSAGE_REACTION_ADD']);
      expect(batched.frames.slice(-2).map(frame => frame.d.channel_id)).toEqual([dmId, channelId]);
      expect(passive.frames).toHaveLength(1);
      expect(passive.frames[0].d.channel_id).toBe(dmId);
      await add('🍇', userIds[0]);
      await control('react', { ...target, emoji: '🍇', user_id: userIds[0], add: false });
      expect(batched.frames.slice(-2).map(frame => frame.t)).toEqual(['MESSAGE_REACTION_ADD', 'MESSAGE_REACTION_REMOVE']);
      const before = batched.frames.length;
      await control('clock', { advance_ms: 500 });
      expect(batched.frames).toHaveLength(before);
      const state = await (await request('/control/state')).json();
      expect(state.reaction_debounce_ms).toBe(500);
      expect(state.session_details.map((session: any) => session.pending_reaction_batches)).toEqual([0, 0, 0]);
      const reaction = fixture.world.messages[channelId].at(-1)!.reactions.find(reaction => reaction.emoji.name === '🌵')!;
      expect(reaction).toMatchObject({ count: 3, me: true, count_details: { normal: 3, burst: 0 } });
    } finally { await fixture.close(); clock.mockRestore(); }
  });

  test('reaction timer uses the configured deadline and reset cancels pending batches', async () => {
    const start = Date.UTC(2026, 0, 1);
    const clock = spyOn(Date, 'now').mockReturnValue(start);
    let nextTimer = 0;
    const timers = new Map<number, { run: () => Promise<unknown>; delay: number }>();
    const schedule = spyOn(globalThis, 'setTimeout').mockImplementation(((run: () => Promise<unknown>, delay: number) => {
      timers.set(++nextTimer, { run, delay });
      return nextTimer;
    }) as typeof setTimeout);
    const cancel = spyOn(globalThis, 'clearTimeout').mockImplementation(((id: number) => { timers.delete(id); }) as typeof clearTimeout);
    const { fixture, control, connect } = fixtureWithoutListener({ reactionDebounceMs: 75 });
    try {
      const client = await connect(1 << 13);
      client.frames.length = 0;
      const channelId = fixture.world.generalChannelId;
      const reaction = { channel_id: channelId, message_id: fixture.world.messages[channelId].at(-1)!.id,
        emoji: '🌵', user_id: fixture.world.me.id, add: true };
      await control('react', reaction);
      expect(client.frames).toEqual([]);
      expect(timers.size).toBe(1);
      const timer = [...timers.values()][0];
      expect(timer.delay).toBe(75);
      clock.mockReturnValue(start + 75);
      await timer.run();
      expect(client.frames.map(frame => frame.t)).toEqual(['MESSAGE_REACTION_ADD']);
      expect(timers.size).toBe(0);
      await control('react', { ...reaction, user_id: fixture.world.users[1].id });
      expect(timers.size).toBe(1);
      await control('reset');
      expect(timers.size).toBe(0);
      expect(client.frames).toHaveLength(1);
    } finally { await fixture.close(); schedule.mockRestore(); cancel.mockRestore(); clock.mockRestore(); }
  });

  test('READY size pads real guild/member content to the requested inflated byte count', () => {
    const world = createWorld();
    const frame = sizedReady(world, 'session', 'ws://fixture/', 1, 3 * 1024 * 1024);
    expect(Buffer.byteLength(JSON.stringify(frame))).toBe(3 * 1024 * 1024);
    expect(frame.d.guilds.length).toBeGreaterThan(100);
    expect(frame.d.guilds.at(-1).members.length).toBeGreaterThan(10);
    expect(frame.d.guilds.at(-1).channels.length).toBeGreaterThan(1);
  });

  test('the oversized guild case exceeds the requested compressed wire size', async () => {
    const requested = 1024 * 1024 + 4096;
    const framer = createZlibFramer();
    try {
      const world = createWorld();
      await framer.encode(JSON.stringify(readyPayload(world, 'session', 'ws://fixture/')));
      const guild = syntheticGuild(world, 10000, requested * 2);
      const frame = await framer.encode(JSON.stringify({ op: 0, t: 'GUILD_CREATE', s: 4, d: guild }));
      expect(frame.byteLength).toBeGreaterThan(requested);
      expect([...frame.subarray(-4)]).toEqual([0, 0, 255, 255]);
    } finally {
      await framer.close();
    }
  });

  test('one deflater preserves dictionary and ordering across sync flushes and split frames', async () => {
    const framer = createZlibFramer();
    const inflater = createInflate();
    let chunks: Buffer[] = [];
    inflater.on('data', (chunk: Buffer) => chunks.push(chunk));
    const random = seededRandom(91);
    const bytes = Buffer.allocUnsafe(2 * 1024 * 1024);
    for (let i = 0; i < bytes.length; i++) bytes[i] = Math.floor(random() * 256);
    const values = [
      { op: 10, d: { heartbeat_interval: 41250 } },
      { op: 0, t: 'READY', s: 1, d: { session_id: 'fixture', content: 'workshop '.repeat(200000) } },
      { op: 0, t: 'GUILD_CREATE', s: 2, d: { content: bytes.toString('base64') } },
      { op: 0, t: 'MESSAGE_CREATE', s: 3, d: { content: 'workshop '.repeat(100) } },
      { op: 11, d: null },
    ];
    try {
      // Concurrent calls must serialize through the same stream.
      const encoded = await Promise.all(values.map(value => framer.encode(JSON.stringify(value))));
      expect(encoded[2].length).toBeGreaterThan(1024 * 1024);
      for (let i = 0; i < encoded.length; i++) {
        const wire = encoded[i];
        expect([...wire.subarray(-4)]).toEqual([0, 0, 255, 255]);
        const parts = splitFrames(wire, 7);
        expect(Buffer.concat(parts)).toEqual(wire);
        expect(parts).toHaveLength(7);
        chunks = [];
        for (const part of parts) {
          await new Promise<void>((resolve, reject) => inflater.write(part, error => error ? reject(error) : resolve()));
        }
        await new Promise<void>(resolve => inflater.flush(constants.Z_SYNC_FLUSH, resolve));
        expect(JSON.parse(Buffer.concat(chunks).toString('utf8'))).toEqual(values[i]);
      }
    } finally {
      await framer.close();
      inflater.destroy();
    }
    await expect(framer.encode('{}')).rejects.toThrow('closed');
    expect(splitFrames(Uint8Array.of(1, 2), 10)).toHaveLength(2);
  });
});
