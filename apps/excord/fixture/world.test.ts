import { describe, expect, spyOn, test } from 'bun:test';
import { createHash } from 'node:crypto';
import { constants, createInflate } from 'node:zlib';
import { compareIds, createWorld, DISCORD_EPOCH, pageMessages, seededRandom, type Message } from './world.ts';
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

  test('around centers its anchor, clips both edges and refuses conflicting cursors', () => {
    expect(ids(pageMessages(messages, { around: messages[100].id, limit: 5 }))).toEqual(ids(messages.slice(98, 103).reverse()));
    expect(ids(pageMessages(messages, { around: messages[100].id, limit: 4 }))).toEqual(ids(messages.slice(98, 103).reverse()));
    expect(pageMessages(messages, { around: messages[100].id, limit: 1000 })).toHaveLength(101);
    expect(pageMessages(messages, { around: messages[100].id, limit: 1 })).toEqual([messages[100]]);
    expect(ids(pageMessages(messages, { around: messages[0].id, limit: 5 }))).toEqual(ids(messages.slice(0, 3).reverse()));
    expect(ids(pageMessages(messages, { around: messages.at(-1)!.id, limit: 5 }))).toEqual(ids(messages.slice(-3).reverse()));
    expect(() => pageMessages(messages, { before: '1', after: '2' })).toThrow('mutually exclusive');
  });
});

function fixtureWithoutListener(options: FixtureOptions = {}) {
  let handle!: (request: Request, server: unknown) => Promise<Response>;
  // Exercise the actual HTTP handler while replacing only the port binding.
  const serve = spyOn(Bun, 'serve').mockImplementation(((config: any) => {
    handle = config.fetch;
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
  return { fixture, request, control };
}

describe('fixture HTTP handler without sockets', () => {
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
    } finally { await fixture.close(); }
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

describe('gateway framing without sockets', () => {
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
