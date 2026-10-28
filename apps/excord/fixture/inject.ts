import { attachment, linkEmbed } from './transcript-content.ts';
import { transcriptEmbed } from './transcript-world.ts';
import { compareIds, type Message, type World } from './world.ts';

export interface InjectedDispatch { session_id: string; s: number; t: string; d: any }
export interface Injection { channel_id: string; what: string; near: string; at_ms: number }
interface Controls {
  origin: string;
  typingUsers: Map<string, Set<string>>;
  failNext: Set<string>;
  isCurrent(): boolean;
  create(channelId: string, content: string, authorId: string, at: number): Message;
  typing(channelId: string, userId: string): Promise<void>;
  remove(channelId: string, ids: string[], bulk: boolean): Promise<void>;
  react(channelId: string, messageId: string): Promise<void>;
  broadcast(event: string, data: unknown): Promise<unknown>;
  badRequest(message: string): never;
  holdGuilds(ms: number): void;
}

export async function inject(world: World, request: Injection, controls: Controls): Promise<void> {
  const { channel_id: channelId, what, near, at_ms: at } = request;
  const { typingUsers, failNext, badRequest } = controls;
  const history = world.messages[channelId];
  if (what === 'fail-next') { failNext.add(channelId); return; }
  // The next launch's guilds come 3 s after its READY (the launch-restore tests).
  if (what === 'hold-guilds') { controls.holdGuilds(3000); return; }
  if (what === 'typing') {
    const user = world.users[typingUsers.get(channelId)?.has(world.users[1].id) ? 2 : 1];
    await controls.typing(channelId, user.id);
    return;
  }
  if (what === 'message' || what === 'burst') {
    const texts = ['The new build is ready to try.', 'The anchor held on my phone.',
      'Nice! Checking a longer thread next.', 'I added a photo to the reproduction.',
      'Works after rotating, too.', 'Thanks for checking it!'];
    for (let i = 0; i < (what === 'burst' ? 6 : 1) && controls.isCurrent(); i++) {
      const typingId = [...typingUsers.get(channelId) ?? []].find(id => id !== world.me.id);
      const author = typingId ?? world.users[1 + i % 3].id;
      const created = controls.create(channelId, texts[i], author, at + i * 500);
      await controls.broadcast('MESSAGE_CREATE', created);
    }
    return;
  }
  // A message to translate (LLP 1133.027 §10, drive 7).
  if (what === 'japanese') {
    const created = controls.create(channelId, '明日の会議は十時からです。', world.users[1].id, at);
    await controls.broadcast('MESSAGE_CREATE', created);
    return;
  }
  // A message with a link to each of two channels and to a direct message: what an address change by an
  // in-app link opens (the warm-navigation tests); and a link to a message #general never had, which leaves
  // the reader at that channel's end (the restore tests).
  if (what === 'links') {
    const named = (name: string) => world.channels.find(c => c.name === name)!.id;
    const dm = world.private_channels[0].id;
    const content = `Moved to <#${named('help')}>, also <#${named('random')}>, back in <#${named('general')}>, and https://discord.com/channels/@me/${dm}`
      + `, or [a message never sent](https://discord.com/channels/@me/${named('general')}/1)`;
    const created = controls.create(channelId, content, world.users[1].id, at);
    await controls.broadcast('MESSAGE_CREATE', created);
    return;
  }
  if (what === 'delete-unread') {
    const lastRead = world.read_state.entries.find(entry => entry.id === channelId)!.last_message_id;
    const first = history.find(message => lastRead === null || compareIds(message.id, lastRead) > 0);
    if (!first) badRequest('Nothing is unread.');
    await controls.remove(channelId, [first.id], false);
    return;
  }
  if (!['edit-above', 'delete-above', 'react-above', 'embed-above'].includes(what)) {
    badRequest(`Unknown fixture event: ${what}`);
  }
  const found = near ? history.findIndex(message => message.id === near) : -1;
  const message = history[Math.max(0, found >= 0 ? found - 3 : history.length - 11)];
  if (!message) badRequest('No message to change.');
  if (what === 'delete-above') { await controls.remove(channelId, [message.id], false); return; }
  if (what === 'react-above') { await controls.react(channelId, message.id); return; }
  if (what === 'edit-above') {
    message.content += '\nA longer update from the device:\nThe image finished decoding.'
      + '\nThe keyboard opened and closed.\nThe visible row kept its offset.';
    message.edited_timestamp = new Date(at).toISOString();
  } else {
    const embed = linkEmbed(2), photo = attachment(1);
    embed.image = { url: photo.url, width: photo.width, height: photo.height };
    message.embeds.push(transcriptEmbed(embed, controls.origin));
  }
  await controls.broadcast('MESSAGE_UPDATE', message);
}
