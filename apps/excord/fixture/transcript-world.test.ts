import { expect, test } from 'bun:test';
import { createHash } from 'node:crypto';
import { statSync } from 'node:fs';
import { channelSpecs, emojiNames, generate, members, WORLD_NOW } from './transcript-content.ts';
import { readyPayload } from './server.ts';
import { createTranscriptWorld } from './transcript-world.ts';
import { compareIds, replySnapshot, snowflakeFromMs } from './world.ts';

const origin = 'http://127.0.0.1:4790';
const hash = (value: unknown) => createHash('sha256').update(JSON.stringify(value)).digest('hex');

test('transcript world is deterministic, generates under one second and has one complete guild', () => {
  const started = performance.now();
  const world = createTranscriptWorld(origin);
  const elapsed = performance.now() - started;
  console.log(`Transcript world generation: ${elapsed.toFixed(1)} ms`);
  expect(elapsed).toBeLessThan(1000);
  expect(hash(world)).toBe(hash(createTranscriptWorld(origin)));
  expect(world.me).toMatchObject({ username: 'charlie', global_name: 'Charlie' });
  expect(world.users).toEqual(members.map((member, i) => ({ id: snowflakeFromMs(Date.UTC(2024, 0, 1), i),
    username: member.username, global_name: member.global_name, discriminator: '0',
    avatar: i % 4 === 3 ? null : expect.stringMatching(/^a[0-9a-f]+$/), bot: member.bot })));
  expect(world.guilds).toHaveLength(1);
  expect(world.guilds[0].name).toBe('Exact Club');
  expect(world.guilds[0].roles.map(role => role.name)).toEqual(['@everyone', 'Maintainers', 'Cyclists']);
  expect(world.guilds[0].roles[0].id).toBe(world.guilds[0].id);
  expect(world.guilds[0].emojis).toEqual(emojiNames.map((name, i) => ({ id: String(i + 500), name,
    animated: false, available: true })));
  expect(world.lazyGuildId).toBe('');
  const ready = readyPayload(world, 'test', 'ws://127.0.0.1/');
  expect(ready.guilds).toHaveLength(1);
  expect(ready.guilds[0]).toHaveProperty('properties.name', 'Exact Club');
  expect(ready.guilds[0].unavailable).toBeUndefined();
});

test('all channel histories preserve the generator, unread positions, replies, media and reactions', () => {
  const world = createTranscriptWorld(origin);
  const userIds = new Map(members.map((member, i) => [member.id, world.users[i].id]));
  const channelIds = new Map(channelSpecs.map((spec, i) => [spec.id, world.guilds[0].channels[i].id]));
  const roleIds = new Map([['201', world.guilds[0].roles[1].id], ['202', world.guilds[0].roles[2].id]]);
  const rewrite = (content: string) => content.replace(/<@(me|\d+)>/g, (_, id) => `<@${userIds.get(id)}>`)
    .replace(/<#([a-z]+)>/g, (_, id) => `<#${channelIds.get(id)}>`)
    .replace(/<@&(\d+)>/g, (_, id) => `<@&${roleIds.get(id)}>`);
  let attachments = 0, unknownSizes = 0, embeds = 0, replies = 0, deleted = 0;
  for (const [c, spec] of channelSpecs.entries()) {
    const source = generate(spec.id), channel = world.guilds[0].channels[c], history = world.messages[channel.id];
    expect(channel).toMatchObject({ name: spec.name, topic: spec.topic, position: c, parent_id: null,
      type: spec.id === 'announcements' ? 5 : 0, guild_id: world.guilds[0].id });
    expect(history).toHaveLength(spec.count);
    expect(Date.parse(history.at(-1)!.timestamp) - Date.parse(history[0].timestamp)).toBe(spec.days * 86400000);
    const entry = world.read_state.entries.find(entry => entry.id === channel.id)!;
    expect(entry.last_message_id).toBe(history[spec.count - spec.unread - 1]?.id ?? null);
    const unread = history.slice(history.findIndex(message => message.id === entry.last_message_id) + 1);
    expect(unread).toHaveLength(spec.unread);
    expect(entry.mention_count).toBe(unread.filter(message => message.mention_everyone
      || message.mentions.some(user => user.id === world.me.id)).length);
    if (spec.id === 'general') {
      expect(entry.mention_count).toBe(1);
      expect(unread.filter(message => message.mentions.some(user => user.id === world.me.id))).toHaveLength(1);
    }
    const byId = new Map(history.map(message => [message.id, message]));
    const sourceIndex = new Map(source.messages.map((message, i) => [message.id, i]));
    for (const [i, message] of history.entries()) {
      const original = source.messages[i];
      expect(message.id).toBe(snowflakeFromMs(original.timestamp, i + 1));
      if (i) expect(compareIds(history[i - 1].id, message.id)).toBeLessThan(0);
      expect(message).toMatchObject({ author: world.users[members.findIndex(user => user.id === original.author.id)],
        content: original.forwarded ? '' : rewrite(original.content), timestamp: new Date(original.timestamp).toISOString(),
        edited_timestamp: original.edited_timestamp === null ? null : new Date(original.edited_timestamp).toISOString(),
        type: original.type, pinned: false, tts: false, flags: 0 });
      expect(message.mentions).toEqual(world.users.filter(user => message.content.includes(`<@${user.id}>`)));
      expect(message.mention_roles).toEqual(world.guilds[0].roles.filter(role =>
        message.content.includes(`<@&${role.id}>`)).map(role => role.id));
      expect(message.mention_everyone).toBe(/@(?:everyone|here)\b/.test(message.content));
      expect(message.reactions).toEqual(original.reactions.map(reaction => ({ ...reaction,
        count_details: { burst: 0, normal: reaction.count }, burst_count: 0, me_burst: false, burst_colors: [] })));
      if (original.message_reference) {
        replies++;
        const reference = message.message_reference!;
        expect(reference).toMatchObject({ channel_id: channel.id, guild_id: world.guilds[0].id });
        if (source.deleted.includes(original.message_reference.message_id)) {
          deleted++;
          expect(byId.has(reference.message_id)).toBe(false);
          expect(message.referenced_message).toBeNull();
        } else {
          const target = history[sourceIndex.get(original.message_reference.message_id)!];
          expect(reference.message_id).toBe(target.id);
          expect(message.referenced_message).toEqual(replySnapshot(target));
          expect(message.referenced_message).not.toHaveProperty('referenced_message');
          expect(message.referenced_message).not.toBe(target);
        }
      } else if (original.forwarded) expect(message.message_reference).toMatchObject({ type: 1, channel_id: channel.id });
      else expect(message.message_reference).toBeUndefined();
      expect(message.attachments).toHaveLength(original.attachments.length);
      for (const [a, attachment] of message.attachments.entries()) {
        attachments++;
        const source = original.attachments[a], url = `${origin}/${source.url.replace(/^assets\//, '')}`;
        expect(attachment).toMatchObject({ filename: `${source.spoiler ? 'SPOILER_' : ''}${source.filename}`,
          url, proxy_url: url, content_type: 'image/png', size: statSync(`${import.meta.dir}/../${source.url}`).size });
        expect(attachment.id).toMatch(/^\d+$/);
        expect(attachment.width).toBe(source.width); expect(attachment.height).toBe(source.height);
        if (source.width === undefined) {
          unknownSizes++;
          expect(attachment).not.toHaveProperty('width'); expect(attachment).not.toHaveProperty('height');
        }
      }
      expect(message.embeds).toHaveLength(original.embeds.length);
      for (const [e, embed] of message.embeds.entries()) {
        embeds++;
        const { image, thumbnail, ...rest } = original.embeds[e];
        expect(embed).toMatchObject(rest);
        for (const [key, source] of Object.entries({ image, thumbnail })) if (source) {
          const url = `${origin}/${source.url.replace(/^assets\//, '')}`;
          expect(embed[key as 'image' | 'thumbnail']).toEqual({ ...source, url, proxy_url: url });
        }
      }
    }
  }
  expect(Math.min(attachments, unknownSizes, embeds, replies, deleted)).toBeGreaterThan(0);
});

test('DMs contain only their participants, increasing ids and the intended read positions', () => {
  const world = createTranscriptWorld(origin);
  expect(world.private_channels.map(channel => channel.type)).toEqual([1, 1, 3]);
  expect(world.private_channels.map(channel => channel.recipient_ids)).toEqual([
    [world.users[1].id], [world.users[2].id], world.users.slice(1, 4).map(user => user.id),
  ]);
  expect(world.private_channels[2].name).toBe('Weekend rides');
  for (const [c, channel] of world.private_channels.entries()) {
    const history = world.messages[channel.id];
    expect(history.length).toBeGreaterThanOrEqual(20); expect(history.length).toBeLessThanOrEqual(40);
    const read = world.read_state.entries.find(entry => entry.id === channel.id)!;
    expect(read.last_message_id).toBe(history.at(c === 0 ? -4 : -1)!.id);
    for (const [i, message] of history.entries()) {
      expect([world.me.id, ...channel.recipient_ids!]).toContain(message.author.id);
      expect(message).not.toHaveProperty('guild_id');
      expect(Date.parse(message.timestamp)).toBeLessThan(WORLD_NOW);
      expect(message.content.length).toBeLessThan(100);
      if (i) expect(compareIds(history[i - 1].id, message.id)).toBeLessThan(0);
    }
  }
});
