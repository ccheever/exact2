// The fixture's seeded Discord world (llp/0001 §4.4): three guilds, DMs, and #general's 10,000
// real-shaped messages. The same seed gives the same world, byte for byte.
export interface User {
  id: string;
  username: string;
  global_name: string;
  discriminator: string;
  avatar: string | null;
  bot: boolean;
}

export interface Role {
  id: string;
  name: string;
  color: number;
  position: number;
  permissions: string;
  hoist: boolean;
  managed: boolean;
  mentionable: boolean;
}

export interface Member {
  user: User;
  roles: string[];
  nick: string | null;
  joined_at: string;
  deaf: boolean;
  mute: boolean;
  flags: number;
}

export interface Channel {
  id: string;
  type: number;
  name?: string;
  guild_id?: string;
  position: number;
  parent_id: string | null;
  topic: string | null;
  last_message_id: string | null;
  recipient_ids?: string[];
  owner_id?: string;
  icon?: string | null;
  nsfw?: boolean;
  permission_overwrites: unknown[];
}

export interface Guild {
  id: string;
  name: string;
  icon: string | null;
  owner_id: string;
  description: string;
  features: string[];
  member_count: number;
  roles: Role[];
  channels: Channel[];
  members: Member[];
  unavailable: boolean;
  emojis?: { id: string; name: string; animated: false; available: true }[];
}

export interface Image {
  url: string;
  proxy_url: string;
  width: number;
  height: number;
}

export interface Attachment {
  id: string;
  url: string;
  proxy_url: string;
  width?: number;
  height?: number;
  filename: string;
  content_type: string;
  size: number;
}

export interface Embed {
  type: string;
  url?: string;
  title?: string;
  description?: string;
  color?: number;
  provider?: { name: string; url?: string };
  author?: { name: string };
  image?: Image;
  thumbnail?: Image;
  video?: Image;
}

export interface Emoji { id: string | null; name: string; animated?: boolean }
export interface Reaction {
  emoji: Emoji;
  count: number;
  me: boolean;
  count_details: { burst: number; normal: number };
  burst_count: number;
  me_burst: boolean;
  burst_colors: string[];
}

export interface Message {
  id: string;
  channel_id: string;
  guild_id?: string;
  author: User;
  content: string;
  timestamp: string;
  edited_timestamp: string | null;
  attachments: Attachment[];
  embeds: Embed[];
  reactions: Reaction[];
  mentions: User[];
  mention_roles: string[];
  mention_everyone: boolean;
  pinned: boolean;
  tts: false;
  type: number;
  flags: number;
  nonce?: string | number;
  message_reference?: { message_id: string; channel_id: string; guild_id?: string };
  referenced_message?: Message | null;
  member?: { roles: string[] };
}

export interface World {
  seed: number;
  token: string;
  me: User;
  users: User[];
  guilds: Guild[];
  channels: Channel[];
  private_channels: Channel[];
  messages: Record<string, Message[]>;
  read_state: { entries: ReadState[]; partial: false; version: number };
  generalChannelId: string;
  lazyGuildId: string;
  nextTimestamp: number;
}

export interface ReadState {
  id: string;
  last_message_id: string | null;
  mention_count: number;
  last_pin_timestamp: null;
  flags: number;
}

export const DISCORD_EPOCH = 1420070400000;
const START = Date.UTC(2025, 0, 1, 9);

export function snowflakeFromMs(ms: number, sequence = 0): string {
  return ((BigInt(Math.trunc(ms) - DISCORD_EPOCH) << 22n) + BigInt(sequence)).toString();
}

export function compareIds(a: string, b: string): number {
  return a.length - b.length || (a < b ? -1 : a > b ? 1 : 0);
}

export function seededRandom(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) | 0;
    let value = Math.imul(state ^ (state >>> 15), state | 1);
    value ^= value + Math.imul(value ^ (value >>> 7), value | 61);
    return ((value ^ (value >>> 14)) >>> 0) / 4294967296;
  };
}

function hash(random: () => number): string {
  return Array.from({ length: 4 }, () => Math.floor(random() * 4294967296)
    .toString(16).padStart(8, '0')).join('');
}

function member(user: User, roles: string[]): Member {
  return { user, roles, nick: null, joined_at: '2024-01-01T00:00:00.000Z', deaf: false, mute: false, flags: 0 };
}

export function emptyMessage(channel: Channel, author: User, ms: number, content: string): Message {
  return {
    id: snowflakeFromMs(ms), channel_id: channel.id,
    ...(channel.guild_id ? { guild_id: channel.guild_id } : {}), author, content,
    timestamp: new Date(ms).toISOString(), edited_timestamp: null, attachments: [], embeds: [],
    reactions: [], mentions: [], mention_roles: [], mention_everyone: false,
    pinned: false, tts: false, type: 0, flags: 0,
  };
}

// An embedded reply is a snapshot, not a recursive copy of the reply chain.
export function replySnapshot(message: Message): Message {
  const { referenced_message: _reference, ...snapshot } = message;
  return structuredClone(snapshot);
}

const shortLines = [
  'morning!', 'yes please', 'looks good to me', 'one sec', 'thank you!', 'on my way',
  'wait, really?', 'same here', 'coffee first', 'that worked!', 'back in five', 'oh nice',
  'can you send a photo?', 'I found it', 'sounds like a plan', 'almost done',
  'let me check', 'which version?', 'see you tomorrow', 'love this', 'not yet',
  'anyone around?', 'the blue one', 'good catch', 'saved me a trip', 'haha exactly',
];
const sentences = [
  'I left the new sketches on the table by the window if anyone wants to take a look.',
  'Can we move the meeting to after lunch so I have time to finish the prototype?',
  'The first version worked on my laptop, but the smaller screen still needs some attention.',
  'I think the simplest option is to try it together and write down where we get stuck.',
  'The light was lovely on the walk home, so I stopped to take a few pictures.',
  'Has anyone seen the notebook with the green cover? I might have left it upstairs.',
  'We can bring a spare charger and an extra chair if more people are coming tonight.',
  'Small update from yesterday: the replacement part arrived and everything fits this time.',
];
const paragraphs = [
  'I tried the new layout this afternoon with the window much narrower than usual. The main idea still works, '
    + 'but the labels start to wrap before the picture has enough room, and the last button disappears below the fold. '
    + 'I took a few screenshots so we can compare them at the workshop. My suggestion is to keep the first row simple '
    + 'and move the extra details into the next section. There is no rush to settle this tonight; I would like to see '
    + 'what happens with a longer title before we change the spacing again.',
  'A little report from the trail: the lower path is open again, though there are still a couple of muddy patches '
    + 'near the bridge. We took the long way around the lake and stopped for lunch where the trees open up. '
    + 'It was quieter than last time and we had plenty of room to sit together. If we go next weekend, I would '
    + 'start an hour earlier, bring another bottle of water, and leave time for the lookout on the way back. '
    + 'I have the route saved and can share the photos once I sort through them.',
  'Thanks to everyone who stayed to help clean up. We found a home for all the spare materials and put the tools '
    + 'back in their boxes, so the next group should have a much easier start. There are still three unfinished '
    + 'projects on the shelf by the door; please leave a name on yours when you come in. I wrote down the supplies '
    + 'we used most often, and we can decide together which ones to order. The room felt really welcoming today, '
    + 'especially when people started showing each other what they were making.',
];
const imageSizes = [[640, 400], [320, 480], [800, 800], [32, 24], [4032, 3024],
  [750, 2400], [1920, 1080], [1200, 300], [96, 96], [480, 640]] as const;

export function imageArtwork(width: number, height: number): string {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 640 400" preserveAspectRatio="none"><rect width="640" height="400" fill="#254b62"/><circle cx="470" cy="110" r="55" fill="#ffda8a"/><path d="M0 330L180 140 340 320 490 200 640 340V400H0Z" fill="#80ada0"/><text x="32" y="365" fill="white" font-size="28">Workshop ${width} x ${height}</text></svg>`;
}

function blankPdf(): Uint8Array {
  const objects = ['<< /Type /Catalog /Pages 2 0 R >>', '<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
    '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>'];
  let text = '%PDF-1.4\n';
  const offsets = [0];
  for (const object of objects) {
    offsets.push(text.length);
    text += `${offsets.length - 1} 0 obj\n${object}\nendobj\n`;
  }
  const xref = text.length;
  text += 'xref\n0 4\n0000000000 65535 f \n';
  for (const offset of offsets.slice(1)) text += `${String(offset).padStart(10, '0')} 00000 n \n`;
  text += `trailer\n<< /Size 4 /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  return new TextEncoder().encode(text);
}

export const fileAssets: Record<string, { content_type: string; body: Uint8Array }> = {
  'notes.pdf': { content_type: 'application/pdf', body: blankPdf() },
  'sketches.zip': { content_type: 'application/zip', body: Uint8Array.from([80, 75, 5, 6, ...Array(18).fill(0)]) },
};

function pick<T>(random: () => number, values: readonly T[]): T {
  return values[Math.floor(random() * values.length)];
}

function messageGap(random: () => number): number {
  const kind = random();
  if (kind < 0.8) return (5 + random() * 85) * 1000;
  if (kind < 0.9) return (1 + random() * 29) * 60000;
  if (kind < 0.995) return (1 + random() * 9) * 3600000;
  return (1 + random() * 3) * 86400000;
}

export function createMessageGenerator(options: {
  channel: Channel; users: User[]; me: User; roleId?: string; assetOrigin: string; random: () => number;
}) {
  const { channel, users, me, roleId, assetOrigin, random } = options;
  const participants = channel.recipient_ids
    ? [me, ...users.filter(user => channel.recipient_ids!.includes(user.id))] : users;
  const emojiId = snowflakeFromMs(START - 50000000);
  const animatedId = snowflakeFromMs(START - 50000000, 1);
  const emojis: Emoji[] = [{ id: null, name: '👍' }, { id: null, name: '❤️' }, { id: null, name: '🎉' },
    { id: null, name: '👀' }, { id: emojiId, name: 'workshop' }, { id: animatedId, name: 'dance', animated: true }];
  let author: User | undefined, remaining = 0;
  const image = (): Image => {
    const [width, height] = pick(random, imageSizes);
    const url = `${assetOrigin}/assets/${width}x${height}.svg`;
    return { url, proxy_url: url, width, height };
  };
  const markdown = (ms: number): string => pick(random, [
    'The **bold** idea has *italic* notes, __underlined__ dates and ~~old plans~~.',
    'The surprise is ||a new workshop||. Keep it quiet!',
    'Try `const answer = 42` in the console.',
    '```typescript\nconst greet = (name: string) => `Hello, ${name}!`;\nconsole.log(greet("team"));\n```',
    '```python\nfor sketch in notebook:\n    print(sketch.title)\n```',
    '> Make something small, then make it useful.\nThat is our plan for today.',
    '# This week\n## Workshop notes\n### What comes next',
    '- Sketch the idea\n- Build a prototype\n1. Share it\n2. Listen to feedback',
    '[The project notebook](https://example.invalid/notebook) has the latest sketches.',
    `Thanks <@${pick(random, participants).id}>! Let us keep the sketches here.`,
    channel.guild_id ? `Let us move this to <#${channel.id}>.` : 'Send me the **finished** sketch?',
    roleId ? `<@&${roleId}> is anyone free this afternoon?` : 'Is anyone free this afternoon?',
    `<:workshop:${emojiId}> <a:dance:${animatedId}>`,
    `Meet here <t:${Math.floor(ms / 1000)}:${pick(random, ['R', 't', 'T', 'd', 'D', 'f', 'F'])}>.`,
    '@everyone the studio is open this afternoon.',
  ]);
  const content = (ms: number): string => {
    const kind = random();
    let text: string;
    if (kind < 0.55) text = pick(random, shortLines);
    else if (kind < 0.70) text = pick(random, sentences);
    else if (kind < 0.78) text = pick(random, paragraphs);
    else if (kind < 0.84) text = `${pick(random, sentences)}\n\n${pick(random, sentences)}\nSee you there!`;
    else if (kind < 0.89) return pick(random, ['😂', '🙌 🙌', '☕', '🎉✨', '❤️', '👀', '👍🏽']);
    else if (kind < 0.93) return pick(random, ['https://example.invalid/notebook',
      'https://example.invalid/photos/lake', 'https://example.invalid/projects/bridge?view=sketch']);
    else return Array.from({ length: 2 + Math.floor(random() * 3) }, () => markdown(ms)).join('\n\n');
    if (random() < (kind < 0.55 ? 0.06 : 0.3)) text += `\n\n${markdown(ms)}`;
    return text;
  };
  const reference = (message: Message, earlier: Message) => {
    message.message_reference = { message_id: earlier.id, channel_id: channel.id,
      ...(channel.guild_id ? { guild_id: channel.guild_id } : {}) };
    message.referenced_message = replySnapshot(earlier);
  };
  return (history: readonly Message[], ms: number, authorId?: string): Message => {
    if (authorId !== undefined) {
      author = users.find(user => user.id === authorId);
      if (!author) throw new Error('Unknown author');
    } else if (!remaining) {
      author = pick(random, participants.filter(user => user.id !== author?.id));
      remaining = 1 + Math.floor(random() * 12);
    }
    remaining = Math.max(0, remaining - 1);
    const message = emptyMessage(channel, author!, ms, content(ms));
    const system = random();
    if (channel.guild_id && system < 0.004) {
      message.content = '';
      message.type = system < 0.002 || !history.length ? 7 : 6;
      if (message.type === 6) {
        const earlier = pick(random, history);
        earlier.pinned = true;
        reference(message, earlier);
      }
      return message;
    }
    if (random() < 0.04) message.edited_timestamp = new Date(ms + 1000 + Math.floor(random() * 300000)).toISOString();
    message.pinned = random() < 0.004;
    message.mentions = users.filter(user => message.content.includes(`<@${user.id}>`));
    message.mention_roles = roleId && message.content.includes(`<@&${roleId}>`) ? [roleId] : [];
    message.mention_everyone = message.content.includes('@everyone');
    if (random() < 0.14) {
      const count = random() < 0.65 ? 1 : 2 + Math.floor(random() * 3);
      for (let i = 0; i < count; i++) {
        const id = snowflakeFromMs(ms, i + 1);
        if (random() < 0.8) {
          const picture = image();
          message.attachments.push({ ...picture, id, filename: `sketch-${i + 1}.svg`,
            content_type: 'image/svg+xml', size: imageArtwork(picture.width, picture.height).length });
        } else {
          const pdf = random() < 0.5, filename = pdf ? 'notes.pdf' : 'sketches.zip';
          const url = `${assetOrigin}/assets/${filename}`;
          message.attachments.push({ id, url, proxy_url: url, filename,
            content_type: fileAssets[filename].content_type, size: fileAssets[filename].body.byteLength });
        }
      }
    }
    if (random() < 0.07) message.embeds.push({ type: 'image', image: image() });
    if (random() < 0.06) message.embeds.push({ type: 'rich', url: 'https://example.invalid/notebook',
      title: 'Workshop notebook', description: pick(random, sentences),
      provider: { name: 'Notebook', url: 'https://example.invalid' }, thumbnail: image() });
    if (random() < 0.02) {
      // Local stills exercise the declared video box without a remote media dependency.
      message.embeds.push({ type: random() < 0.5 ? 'gifv' : 'video',
        url: 'https://example.invalid/clips/workshop', video: image(), thumbnail: image() });
    }
    if (random() < 0.18) {
      const available = [...emojis], count = 1 + Math.floor(random() * 4);
      for (let i = 0; i < count; i++) {
        const emoji = available.splice(Math.floor(random() * available.length), 1)[0];
        const count = 1 + Math.floor(random() * 30);
        message.reactions.push({ emoji, count, me: random() < 0.3, count_details: { burst: 0, normal: count },
          burst_count: 0, me_burst: false, burst_colors: [] });
      }
    }
    if (history.length && random() < 0.09) {
      const distance = history.length > 200 && random() < 0.35
        ? 100 + Math.floor(random() * (history.length - 100 + 1))
        : 1 + Math.floor(random() * Math.min(30, history.length));
      message.type = 19;
      reference(message, history[history.length - distance]);
    }
    return message;
  };
}

export function createWorld(seed = 1, assetOrigin = 'http://127.0.0.1:4790'): World {
  const random = seededRandom(seed);
  const names = ['Alex', 'Maya', 'Theo', 'Jun', 'Amara', 'Leo', 'Nora', 'Inez', 'Sam', 'Ari', 'Robin', 'Sasha'];
  const users = names.map((name, i): User => ({
    id: snowflakeFromMs(START - 86400000, i), username: name.toLowerCase(), global_name: name,
    discriminator: '0', avatar: i % 3 ? hash(random) : null, bot: false,
  }));
  const me = users[0];
  const guilds = ['Harbor Workshop', 'Night School', 'Trail Friends'].map((name, g): Guild => {
    const id = snowflakeFromMs(START - 80000000, g * 100);
    const role: Role = {
      id, name: '@everyone', color: 0, position: 0, permissions: '104324673',
      hoist: false, managed: false, mentionable: false,
    };
    const roles = [role, { ...role, id: snowflakeFromMs(START - 80000000, g * 100 + 1),
      name: 'Regulars', color: 0x7289da, position: 1, hoist: true, mentionable: true }];
    const channels = [
      [4, 'Community', null], [0, 'general', 0], [5, 'announcements', 0],
      [4, 'Projects', null], [0, 'workbench', 3], [0, 'quiet-room', 3],
    ].map(([type, channelName, parent], c): Channel => ({
      id: snowflakeFromMs(START - 70000000, g * 100 + c), guild_id: id,
      type: type as number, name: channelName as string, position: c,
      parent_id: parent === null ? null : snowflakeFromMs(START - 70000000, g * 100 + (parent as number)),
      topic: type === 4 ? null : `${name}: share what you are making.`, last_message_id: null,
      permission_overwrites: [],
    }));
    return { id, name, icon: hash(random), owner_id: users[g + 1].id, description: `A place for ${name.toLowerCase()}.`,
      features: ['COMMUNITY'], member_count: users.length, roles, channels,
      members: users.map(user => member(user, [roles[1].id])), unavailable: false };
  });
  const private_channels: Channel[] = [1, 2, 3, 4].map((u, i) => ({
    id: snowflakeFromMs(START - 60000000, i), type: i === 3 ? 3 : 1,
    ...(i === 3 ? { name: 'Weekend plans', owner_id: me.id, icon: null } : {}),
    recipient_ids: i === 3 ? users.slice(4, 8).map(user => user.id) : [users[u].id],
    position: i, parent_id: null, topic: null, last_message_id: null, permission_overwrites: [],
  }));
  const channels = [...guilds.flatMap(guild => guild.channels), ...private_channels];
  const generalChannelId = guilds[0].channels[1].id;
  const messages: World['messages'] = {};
  let nextTimestamp = START;
  for (let c = 0; c < channels.length; c++) {
    const channel = channels[c];
    if (channel.type === 4) continue;
    const count = channel.id === generalChannelId ? 10000
      : channel.name === 'quiet-room' ? 0 : 20 + Math.floor(random() * 181);
    const history: Message[] = [];
    messages[channel.id] = history;
    const generate = createMessageGenerator({ channel, users, me, assetOrigin, random,
      roleId: guilds.find(guild => guild.id === channel.guild_id)?.roles[1].id });
    const offsets = [0];
    for (let i = 1; i < count; i++) offsets.push(offsets[i - 1] + messageGap(random));
    // Keep a year's history across seeds without making the gaps uniform.
    const span = (channel.id === generalChannelId ? 364 : 7) * 86400000;
    const start = START + 364 * 86400000 - span + c;
    const scale = span / (offsets.at(-1) || 1);
    const deletedReply = channel.id === generalChannelId ? count - 1 - Math.floor(random() * 100) : -1;
    for (let i = 0; i < count; i++) {
      const ms = start + Math.round(offsets[i] * scale);
      const created = generate(history, ms);
      if (i === deletedReply) {
        created.type = 19;
        created.content = 'Does anyone still have the original sketch?';
        created.mentions = []; created.mention_roles = []; created.mention_everyone = false;
        created.message_reference = { channel_id: channel.id, guild_id: channel.guild_id,
          message_id: snowflakeFromMs(START, 1048575) };
        created.referenced_message = null;
      }
      history.push(created);
      nextTimestamp = Math.max(nextTimestamp, ms + 1000);
    }
    channel.last_message_id = history.at(-1)?.id ?? null;
  }
  const entries = channels.filter(channel => channel.type !== 4).map((channel): ReadState => {
    const history = messages[channel.id];
    const unread = channel.id === generalChannelId ? 150
      : random() < 0.4 ? 0 : 1 + Math.floor(random() * history.length);
    return { id: channel.id, last_message_id: history.at(-unread - 1)?.id ?? null,
      mention_count: channel.id === generalChannelId ? 3 : unread && history.length ? Math.floor(random() * 5) : 0,
      last_pin_timestamp: null, flags: 0 };
  });
  return { seed, token: 'fixture-token', me, users, guilds, channels, private_channels, messages,
    read_state: { entries, partial: false, version: 1 }, generalChannelId,
    lazyGuildId: guilds[2].id, nextTimestamp };
}

export interface PageQuery { before?: string; after?: string; around?: string; limit?: number }

// Discord documents "the past few minutes"; five minutes is the fixture's default.
export const NONCE_WINDOW_MS = 5 * 60 * 1000;

export function recentNonceMessage(messages: readonly Message[], authorId: string, nonce: string | number,
  nowMs: number, windowMs = NONCE_WINDOW_MS): Message | undefined {
  return messages.findLast(message => {
    const age = nowMs - Date.parse(message.timestamp);
    return message.author.id === authorId && message.nonce !== undefined
      && String(message.nonce) === String(nonce) && age >= 0 && age < windowMs;
  });
}

function lowerBound(messages: readonly Message[], id: string): number {
  let lo = 0, hi = messages.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (compareIds(messages[mid].id, id) < 0) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

export function pageMessages(messages: readonly Message[], query: PageQuery = {}): Message[] {
  if ([query.before, query.after, query.around].filter(value => value !== undefined).length > 1) {
    throw new Error('before, after and around are mutually exclusive');
  }
  const limit = Math.max(1, Math.min(100, Math.trunc(Number.isFinite(query.limit) ? query.limit! : 50)));
  let start: number, end: number;
  if (query.after !== undefined) {
    start = lowerBound(messages, query.after);
    if (messages[start]?.id === query.after) start++;
    end = Math.min(messages.length, start + limit);
  } else if (query.around !== undefined) {
    const center = lowerBound(messages, query.around);
    // An even limit gives the older side one extra message; that split is unverified.
    start = Math.max(0, center - Math.floor(limit / 2));
    end = Math.min(messages.length, start + limit);
    start = Math.max(0, end - limit);
  } else {
    end = query.before === undefined ? messages.length : lowerBound(messages, query.before);
    start = Math.max(0, end - limit);
  }
  return messages.slice(start, end).reverse();
}
