import { statSync } from 'node:fs';
import { channelSpecs, emojiNames, generate, members, photos, WORLD_NOW } from './transcript-content.ts';
import type { Embed as TranscriptEmbed } from './transcript-content.ts';
import {
  emptyMessage, replySnapshot, snowflakeFromMs,
  type Channel, type Embed, type Guild, type Image, type Message, type ReadState, type Role, type World,
} from './world.ts';

const DIRECTORY_EPOCH = Date.UTC(2024, 0, 1);
const mediaSizes = new Map(photos.map(photo => [photo.name,
  statSync(`${import.meta.dir}/../assets/media/${photo.name}.png`).size]));
const mediaUrl = (origin: string, url: string) => `${origin}/${url.replace(/^assets\//, '')}`;

export function transcriptEmbed(source: TranscriptEmbed, assetOrigin: string): Embed {
  const { image, thumbnail, ...rest } = source;
  const convert = (image: NonNullable<TranscriptEmbed['image']>): Image => ({
    url: mediaUrl(assetOrigin, image.url), proxy_url: mediaUrl(assetOrigin, image.url),
    width: image.width!, height: image.height!,
  });
  return { ...rest, type: 'rich', ...(image ? { image: convert(image) } : {}),
    ...(thumbnail ? { thumbnail: convert(thumbnail) } : {}) };
}

// A selection of fixture.ts's private `chat` lines for the smaller DM histories.
const dmChat = [
  'Morning! Coffee is on.', 'That fixed it, thank you.', 'I can take a look after lunch.',
  'Nice, that feels much better.', 'One more run on a real phone.', 'Can reproduce on my machine.',
  'The window stays put now 🎉', 'It was a stale cache entry all along.', 'Lunch at the noodle place?',
  'Back from the bike shop.', 'Windy on the bridge today.', 'The new tires are so much quieter.',
  'Anyone riding the coast on Saturday?', 'I made way too much focaccia.', 'The garden finally has tomatoes.',
  'Rain check on the hike?', 'That trail is worth the climb.', 'Tea break, back in ten.',
  'Tomorrow works for me.', 'Thanks for explaining the ownership bit.',
];

export function createTranscriptWorld(assetOrigin: string): World {
  const users = members.map((user, i) => ({ id: snowflakeFromMs(DIRECTORY_EPOCH, i),
    username: user.username, global_name: user.global_name!, discriminator: '0',
    // Most members set a picture; every fourth keeps Discord's default.
    avatar: i % 4 === 3 ? null : `a${(i * 2654435761 >>> 0).toString(16)}`, bot: user.bot }));
  const me = users[0];
  const userIds = new Map(members.map((user, i) => [user.id, users[i].id]));
  const guildId = snowflakeFromMs(DIRECTORY_EPOCH + 1);
  const role: Role = { id: guildId, name: '@everyone', color: 0, position: 0, permissions: '104324673',
    hoist: false, managed: false, mentionable: false };
  const roles = [role, ...[['Maintainers', 0xe67e22], ['Cyclists', 0x1abc9c]].map(([name, color], i) => ({ ...role,
    id: snowflakeFromMs(DIRECTORY_EPOCH + 1, i + 1), name: name as string, color: color as number, position: i + 1,
    hoist: true, mentionable: true }))];
  // Who holds which role: the higher role's colour names its holders.
  const memberRoles = (i: number) => [...(i % 3 === 1 ? [roles[1].id] : []), ...(i % 2 === 0 && i > 0 ? [roles[2].id] : [])];
  const roleIds = new Map([['201', roles[1].id], ['202', roles[2].id]]);
  const guildChannels: Channel[] = channelSpecs.map((spec, i) => ({
    id: snowflakeFromMs(DIRECTORY_EPOCH + 2, i), guild_id: guildId, name: spec.name,
    type: spec.id === 'announcements' ? 5 : 0, position: i, parent_id: null, topic: spec.topic, ...(spec.nsfw ? { nsfw: true } : {}),
    // #random is a private channel: @everyone may not see it, Cyclists may.
    last_message_id: null, permission_overwrites: spec.id !== 'random' ? [] : [
      { id: guildId, type: 0, allow: '0', deny: '1024' }, { id: roles[2].id, type: 0, allow: '1024', deny: '0' }],
  }));
  const channelIds = new Map(channelSpecs.map((spec, i) => [spec.id, guildChannels[i].id]));
  const content = (text: string) => text.replace(/<@(!?)(me|\d+)>/g,
    (match, prefix, id) => userIds.has(id) ? `<@${prefix}${userIds.get(id)}>` : match)
    .replace(/<#([a-z]+)>/g, (match, id) => channelIds.has(id) ? `<#${channelIds.get(id)}>` : match)
    .replace(/<@&(\d+)>/g, (match, id) => roleIds.has(id) ? `<@&${roleIds.get(id)}>` : match);
  const messages: World['messages'] = {};
  const entries: ReadState[] = [];
  const read = (channel: Channel, lastRead: string | null) => {
    const history = messages[channel.id];
    channel.last_message_id = history.at(-1)!.id;
    const unread = history.slice(history.findIndex(message => message.id === lastRead) + 1);
    entries.push({ id: channel.id, last_message_id: lastRead, mention_count: unread.filter(message =>
      message.mention_everyone || message.mentions.some(user => user.id === me.id)).length,
    last_pin_timestamp: null, flags: 0 });
  };
  for (const [c, spec] of channelSpecs.entries()) {
    const source = generate(spec.id), channel = guildChannels[c];
    const ids = new Map(source.messages.map((message, i) => [message.id, snowflakeFromMs(message.timestamp, i + 1)]));
    const history = source.messages.map((message, i): Message => {
      const author = users[members.findIndex(user => user.id === message.author.id)];
      const converted = emptyMessage(channel, author, message.timestamp, content(message.content));
      converted.member = { roles: memberRoles(users.indexOf(author)) };
      converted.id = ids.get(message.id)!;
      converted.type = message.type;
      converted.edited_timestamp = message.edited_timestamp === null ? null : new Date(message.edited_timestamp).toISOString();
      converted.mentions = users.filter(user => new RegExp(`<@!?${user.id}>`).test(converted.content));
      converted.mention_roles = roles.filter(role => converted.content.includes(`<@&${role.id}>`)).map(role => role.id);
      converted.mention_everyone = /@(?:everyone|here)\b/.test(converted.content);
      converted.attachments = message.attachments.map((attachment, a) => {
        const url = mediaUrl(assetOrigin, attachment.url);
        const { width, height } = attachment;
        return { id: snowflakeFromMs(message.timestamp, 100000 + i * 10 + a),
          filename: `${attachment.spoiler ? 'SPOILER_' : ''}${attachment.filename}`,
          size: mediaSizes.get(attachment.filename.replace(/\.png$/, ''))!, url, proxy_url: url,
          content_type: 'image/png', ...(width === undefined ? {} : { width }), ...(height === undefined ? {} : { height }) };
      });
      converted.embeds = message.embeds.map(embed => transcriptEmbed(embed, assetOrigin));
      if (message.sticker_items) (converted as Record<string, unknown>).sticker_items = message.sticker_items;
      if (message.poll) (converted as Record<string, unknown>).poll = structuredClone(message.poll);
      if (message.command) (converted as Record<string, unknown>).interaction_metadata = { id: snowflakeFromMs(message.timestamp - 1000),
        type: 2, user: users[members.findIndex(user => user.id === message.command!.by)],
        authorizing_integration_owners: { 0: guildId } };
      if (message.command) (converted as Record<string, unknown>).interaction = { id: snowflakeFromMs(message.timestamp - 1000),
        type: 2, name: message.command.name, user: users[members.findIndex(user => user.id === message.command!.by)] };
      // A forward is Discord's: no words of its own, the forwarded message in `message_snapshots`.
      if (message.forwarded) Object.assign(converted, { content: '', attachments: [], embeds: [],
        message_reference: { type: 1, message_id: snowflakeFromMs(message.timestamp - 86400000), channel_id: channel.id, guild_id: guildId },
        message_snapshots: [{ message: { content: message.forwarded, attachments: [], embeds: [], type: 0,
          timestamp: new Date(message.timestamp - 86400000).toISOString(), edited_timestamp: null, flags: 0, mentions: [], mention_roles: [] } }] });
      converted.reactions = message.reactions.map(reaction => ({ ...reaction, emoji: { ...reaction.emoji },
        count_details: { burst: 0, normal: reaction.count }, burst_count: 0, me_burst: false, burst_colors: [] }));
      if (message.message_reference) converted.message_reference = {
        message_id: ids.get(message.message_reference.message_id) ?? snowflakeFromMs(message.timestamp - 1),
        channel_id: channel.id, guild_id: guildId,
      };
      return converted;
    });
    const byId = new Map(history.map(message => [message.id, message]));
    // A reply names the message it answers; a forward's reference (type 1) does not.
    for (const message of history) if (message.message_reference && (message.message_reference as { type?: number }).type !== 1) {
      const target = byId.get(message.message_reference.message_id);
      message.referenced_message = target ? replySnapshot(target) : null;
    }
    messages[channel.id] = history;
    read(channel, ids.get(source.lastRead) ?? null);
  }
  const private_channels: Channel[] = [[users[1]], [users[2]], users.slice(1, 4)].map((recipients, i) => ({
    id: snowflakeFromMs(DIRECTORY_EPOCH + 3, i), type: i === 2 ? 3 : 1,
    ...(i === 2 ? { name: 'Weekend rides', owner_id: me.id, icon: null } : {}),
    recipient_ids: recipients.map(user => user.id), position: i, parent_id: null, topic: null,
    last_message_id: null, permission_overwrites: [],
  }));
  for (const [c, channel] of private_channels.entries()) {
    const participants = [me, ...users.filter(user => channel.recipient_ids!.includes(user.id))];
    const history = Array.from({ length: 30 }, (_, i) => {
      const message = emptyMessage(channel, participants[i % participants.length],
        WORLD_NOW - (31 - i) * 60000 - c * 1000, dmChat[(i + c * 7) % dmChat.length]);
      message.id = snowflakeFromMs(Date.parse(message.timestamp), i + 1);
      return message;
    });
    messages[channel.id] = history;
    read(channel, history.at(c === 0 ? -4 : -1)!.id);
  }
  const guild: Guild = { id: guildId, name: 'Exact Club', icon: null, owner_id: me.id,
    description: 'Building Exact, and everything around it.', features: ['COMMUNITY'], member_count: users.length,
    roles, channels: guildChannels, members: users.map((user, i) => ({ user, roles: memberRoles(i), nick: null,
      joined_at: new Date(DIRECTORY_EPOCH).toISOString(), deaf: false, mute: false, flags: 0 })), unavailable: false,
    emojis: emojiNames.map((name, i) => ({ id: String(500 + i), name, animated: false, available: true })) };
  return { seed: 1, token: 'fixture-token', me, users, guilds: [guild], channels: [...guildChannels, ...private_channels],
    private_channels, messages, read_state: { entries, partial: false, version: 1 },
    generalChannelId: guildChannels[0].id, lazyGuildId: '', nextTimestamp: WORLD_NOW };
}
