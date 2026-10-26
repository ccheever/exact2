import { expect, test } from 'bun:test';
import * as S from './discord/store';
import { snowflakeFromMs } from './discord/ids';
import { avatarUrl, createRenderer, windowOf, type Directories, type Stored, type Unread } from './render';
import type { Attachment, Message, Reaction, User } from './fixture/world';

const NOW = Date.UTC(2026, 9, 22, 12), DIRECTORY = Date.UTC(2024, 0, 1);
const gid = snowflakeFromMs(DIRECTORY + 1), cid = snowflakeFromMs(DIRECTORY + 2);
const help = snowflakeFromMs(DIRECTORY + 3), role = snowflakeFromMs(DIRECTORY + 4);
const ferris = snowflakeFromMs(DIRECTORY + 5), dance = snowflakeFromMs(DIRECTORY + 6);
const missing = snowflakeFromMs(DIRECTORY + 7), cdn = 'http://127.0.0.1:4790';
const me: User = { id: snowflakeFromMs(DIRECTORY), username: 'charlie', global_name: 'Charlie Cheever',
  discriminator: '0', avatar: null, bot: false };
const ada: User = { ...me, id: snowflakeFromMs(DIRECTORY, 1), username: 'ada', global_name: 'Ada' };
const bot: User = { ...me, id: snowflakeFromMs(DIRECTORY, 2), username: 'buildbot', global_name: '', bot: true };

function message(offset = 0, patch: Partial<Message> = {}): Message {
  return { id: snowflakeFromMs(NOW + offset), channel_id: cid, guild_id: gid, author: ada, content: 'hello',
    timestamp: new Date(NOW + offset).toISOString(), edited_timestamp: null, attachments: [], embeds: [],
    reactions: [], mentions: [], mention_roles: [], mention_everyone: false, pinned: false, tts: false,
    type: 0, flags: 0, ...patch };
}
const reference = (id: string) => ({ message_id: id, channel_id: cid, guild_id: gid });
function fixture(messages: Message[], query: S.HistoryQuery = { limit: 50 }) {
  const store = S.createStore();
  S.applyDispatch(store, 'READY', { user: me, users: [me, ada, bot], guilds: [{ id: gid, name: 'Exact Club',
    roles: [{ id: role, name: 'Maintainers', color: 0xe67e22, position: 1, permissions: '0',
      hoist: true, managed: false, mentionable: true }], channels: [
      { id: cid, name: 'general', type: 0, last_message_id: messages.at(-1)?.id ?? null },
      { id: help, name: 'help', type: 0, last_message_id: null },
    ] }], private_channels: [] }, NOW);
  S.applyHistoryPage(store, cid, query, [...messages].reverse());
  const dirs: Directories = {
    me: () => store.meId ?? '',
    user: id => S.user(store, id),
    channelName: id => S.channel(store, id)?.name ?? 'unknown-channel',
    channelGuild: id => S.channel(store, id)?.guild_id ?? '',
    guildName: id => store.guildMap.get(id)?.name ?? '',
    canOpen: id => S.channel(store, id) !== null,
    roleName: id => {
      for (const guild of S.guilds(store)) for (const role of guild.roles ?? []) if (role.id === id) return role.name;
      return 'unknown-role';
    },
    message: (channel, id) => (S.message(store, channel, id) as Stored | null) ?? undefined,
    deleted: (channel, id) => S.deleted(store, channel, id),
    emoji: (id, animated) => `${cdn}/emojis/${id}.${animated ? 'gif' : 'png'}`,
    sticker: (id, extension) => `${cdn}/stickers/${id}.${extension}`,
    avatar: u => avatarUrl(cdn, u),
    nameColor: (channel, roles) => {
      const r = (S.guilds(store)[0]?.roles ?? []).find((x: { id: string }) => roles.includes(x.id));
      return r?.color ? `#${r.color.toString(16).padStart(6, '0')}` : '';
    },
    roles: () => S.guilds(store)[0]?.roles,
  };
  const render = createRenderer(dirs);
  const row = (id: string) => S.message(store, cid, id)!;
  const view = (w = S.latestWindow(store, cid, 200), anchor = '', since = '',
    unread: Unread = { count: 0, more: false, at: 0 }) => {
    const firstKey = w.rows[0] ? (w.rows[0].id ?? w.rows[0].key) : '';
    const previous = firstKey ? S.windowBefore(store, cid, firstKey, 1).rows[0] : undefined;
    return windowOf({ rows: w.rows, previous, hasBefore: w.moreBefore, hasAfter: w.moreAfter },
      cid, anchor, since, unread, render);
  };
  return { store, render, row, view };
}

test('store rows supply the transcript identity, author, clock and declared fields', () => {
  const m = message(0, { author: me }), f = fixture([m, message(1000, { author: bot, member: { roles: [role] } })]);
  const [first, second] = f.view().rows;
  expect(first).toMatchObject({ key: m.id, id: m.id, authorId: me.id, author: 'Charlie Cheever',
    avatar: `${cdn}/embed/avatars/0.png`, color: '', bot: false, mine: true, at: NOW, prevAt: 0, state: 'sent',
    grouped: false, edited: false, mentionsMe: false, jumbo: false, system: '', plain: 'hello', source: 'hello',
    isNew: false, media: [], embeds: [], reactions: [], reply: [] });
  expect(second).toMatchObject({ author: 'buildbot', color: '#e67e22', bot: true, mine: false,
    at: NOW + 1000, prevAt: NOW, grouped: false });
  expect(Object.keys(first).sort()).toEqual(['at', 'author', 'authorId', 'avatar', 'blocks', 'bot', 'color', 'edited', 'embeds', 'files', 'forwarded', 'grouped', 'id', 'isNew', 'jumbo', 'key', 'media', 'mentionsMe', 'mine', 'note', 'plain', 'poll', 'prevAt', 'reactions', 'reply', 'source', 'state', 'stickers', 'system', 'systemIcon']);
  expect(f.render(f.row(m.id))).toBe(first);
  // A role's new colour redraws its holders' rows.
  S.applyDispatch(f.store, 'GUILD_ROLE_UPDATE', { guild_id: gid, role: { id: role, name: 'Maintainers', color: 0x112233,
    position: 1, permissions: '0', hoist: true, managed: false, mentionable: true } }, NOW);
  expect(f.view().rows[1].color).toBe('#112233');
  // A new avatar seen on a later message redraws the earlier rows; the same author again keeps them.
  const drawn = f.view().rows[1];
  S.applyDispatch(f.store, 'MESSAGE_CREATE', message(2000, { author: { ...bot } }), NOW);
  expect(f.view().rows[1]).toBe(drawn);
  S.applyDispatch(f.store, 'MESSAGE_CREATE', message(3000, { author: { ...bot, avatar: 'fresh' } }), NOW);
  expect(f.view().rows[1].avatar).toBe(`${cdn}/avatars/${bot.id}/fresh.png?size=128`);
});

test('Markdown becomes headings, subtext, lists, quotes, code and marked runs', () => {
  const content = ['# Title', '## Second', '### Third', '-# A quiet note',
    '**bold** *italic* __underline__ ~~strike~~ ||secret **bold secret**|| and ||other||', '',
    '`inline` [**label**](https://example.com) https://example.com/plain', 'line one\nline two', '',
    '> quoted **text**', '', '- first\n  - nested\n\n  later paragraph\n- second', '',
    '3. third\n4. fourth', '', '```rust\nlet n = 2;\n```', '', '>>> long quote\nsecond quoted line'].join('\n');
  const row = fixture([message(0, { content, edited_timestamp: new Date(NOW + 1).toISOString() })]).view().rows[0];
  const runs = row.blocks.flatMap(b => b.runs);
  expect(row.blocks.filter(b => b.kind === 'heading').map(b => [b.level, b.runs[0].text]))
    .toEqual([[1, 'Title'], [2, 'Second'], [3, 'Third']]);
  expect(row.blocks.find(b => b.kind === 'subtext')?.runs.map(r => r.text)).toEqual(['A quiet note']);
  const code = row.blocks.find(b => b.kind === 'code')!;
  expect(code).toMatchObject({ lang: 'rust', text: 'let n = 2;' });
  expect(code.runs.map(r => [r.kind, r.text])).toEqual([['hl-keyword', 'let'], ['hl-plain', ' n = '], ['hl-number', '2'], ['hl-plain', ';']]);
  expect(row.blocks.filter(b => b.kind === 'item').map(b => [b.marker, b.level]))
    .toEqual([['•', 0], ['•', 1], ['', 0], ['•', 0], ['3.', 0], ['4.', 0]]);
  expect(row.blocks.filter(b => b.quote).map(b => b.runs.map(r => r.text).join('')))
    .toEqual(['quoted text', 'long quote\nsecond quoted line (edited)']);
  for (const mark of ['bold', 'italic', 'underline', 'strike'] as const) {
    expect(runs.find(r => r.text === mark)).toMatchObject({ [mark]: true });
  }
  expect(runs.filter(r => r.spoiler).map(r => [r.text, r.spoiler, r.bold]))
    .toEqual([['secret ', 's0', false], ['bold secret', 's0', true], ['other', 's1', false]]);
  expect(runs.find(r => r.text === 'label')).toMatchObject({ kind: 'link', bold: true, href: 'https://example.com' });
  expect(runs.find(r => r.text === 'https://example.com/plain')).toMatchObject({ kind: 'link', href: 'https://example.com/plain' });
  expect(runs.find(r => r.kind === 'code')?.text).toBe('inline');
  expect(runs.at(-1)).toMatchObject({ kind: 'edited', text: ' (edited)' });
  expect(row).toMatchObject({ edited: true, source: content });
  expect(row.plain).toContain('line one\nline two');
  expect(row.plain).not.toContain('(edited)');
  expect(new Set(runs.map(r => r.id)).size).toBe(runs.length);
});

test('user, channel and role mentions resolve through READY directories', () => {
  const content = `<@${me.id}> <@!${ada.id}> <#${help}> <@&${role}> @everyone @here <@${missing}> <#${missing}> <@&${missing}>`;
  const row = fixture([message(0, { content, mentions: [me, ada], mention_roles: [role], mention_everyone: true })]).view().rows[0];
  expect(row.blocks[0].runs.filter(r => r.kind !== 'text').map(r => [r.kind, r.text, r.self])).toEqual([
    ['user', '@Charlie Cheever', true], ['user', '@Ada', false], ['channel', '#help', false],
    ['role', '@Maintainers', false], ['everyone', '@everyone', false], ['everyone', '@here', false],
    ['user', `@${missing}`, false], ['channel', '#unknown-channel', false], ['role', '@unknown-role', false],
  ]);
  expect(row.plain).toBe(`@Charlie Cheever @Ada #help @Maintainers @everyone @here @${missing} #unknown-channel @unknown-role`);
  expect(row.mentionsMe).toBe(true);
  // The text an edit starts from names the people the directories know, outside code, as the composer
  // sends them back; a channel or emoji stays as written.
  expect(row.source).toBe(`@charlie @ada <#${help}> <@&${role}> @everyone @here <@${missing}> <#${missing}> <@&${missing}>`);
  const coded = fixture([message(0, { content: `\`<@${ada.id}>\` <:ferris:${ferris}>` })]).view().rows[0];
  expect(coded.source).toBe(`\`<@${ada.id}>\` <:ferris:${ferris}>`);
});

test('a mention or a link to Discord names what pressing it opens', () => {
  const content = `<@${ada.id}> <#${help}> <#${missing}> https://discord.com/channels/${gid}/${cid}/${missing} https://www.Discord.com/channels/@me/${cid}?x=1 https://discord.com/channels/${gid}/${missing} https://example.com/x`;
  const runs = fixture([message(0, { content })]).view().rows[0].blocks[0].runs.filter(r => r.ref !== '' || r.kind === 'link' || r.kind === 'channel');
  // The channel's own guild, whatever the link says; a channel the store does not hold opens nothing here.
  expect(runs.map(r => [r.kind, r.ref])).toEqual([['user', `u:${ada.id}`], ['channel', `c:${gid}:${help}`], ['channel', ''],
    ['link', `m:${gid}:${cid}:${missing}`], ['link', `m:${gid}:${cid}:`], ['link', ''], ['link', '']]);
});

test('mention flags count wire mentions and leave code and own wire mentions unhighlighted', () => {
  const patches: Partial<Message>[] = [{ content: `<@${me.id}>` }, { mention_everyone: true }, { mentions: [me] },
    { content: `\`<@${me.id}>\`` }, { author: me, mentions: [me], mention_everyone: true }, { mentions: [ada] }];
  const rows = fixture(patches.map((p, i) => message(i, p))).view().rows;
  expect(rows.map(r => r.mentionsMe)).toEqual([true, true, true, false, false, false]);
  expect(rows[3].blocks[0].runs[0]).toMatchObject({ kind: 'code', text: `<@${me.id}>`, self: false });
});

test('Discord timestamp styles become time runs with millisecond instants', () => {
  const content = ['t', 'T', 'd', 'D', 'R', 'f', 'F'].map(s => `<t:1792692000:${s}>`).join(' ');
  const row = fixture([message(0, { content })]).view().rows[0];
  const times = row.blocks.flatMap(b => b.runs).filter(r => r.kind === 'time');
  expect(times.map(r => [r.at, r.format])).toEqual([
    [1792692000000, 'time-short'], [1792692000000, 'time-medium'], [1792692000000, 'date-short'],
    [1792692000000, 'date-long'], [1792692000000, 'relative'], [1792692000000, 'date-long'],
    [1792692000000, 'time-short'], [1792692000000, 'date-full'], [1792692000000, 'time-short'],
  ]);
  expect(times[0].text).toBe('6:00 PM UTC');
  expect(times[2].text).toBe('October 22, 2026');
});

test('jumbo custom emoji and reactions use the directory with the animation flag', () => {
  const reaction = (id: string | null, name: string, count: number, animated = false): Reaction => ({
    emoji: { id, name, animated }, count, me: count === 2, count_details: { normal: count, burst: 0 },
    burst_count: 0, me_burst: false, burst_colors: [],
  });
  const rows = fixture([message(0, { content: `🎉 <:ferris:${ferris}> <a:dance:${dance}>`, reactions: [
    reaction(null, '👍', 2), reaction(ferris, 'ferris', 3), reaction(dance, 'dance', 4, true),
  ] }), message(1, { content: '🎉' }), message(2, { content: `hello <:ferris:${ferris}>` })]).view().rows;
  expect(rows.map(r => r.jumbo)).toEqual([true, true, false]);
  // One paragraph, its custom emoji between its Unicode ones in the order written.
  expect(rows[0].blocks.map(b => b.kind)).toEqual(['paragraph']);
  expect(rows[0].blocks[0].runs.filter(r => r.text.trim()).map(r => [r.kind, r.text.trim(), r.href]))
    .toEqual([['text', '🎉', ''], ['emoji', ':ferris:', `${cdn}/emojis/${ferris}.png`], ['emoji', ':dance:', `${cdn}/emojis/${dance}.gif`]]);
  expect(rows[1].blocks[0].runs[0].text).toBe('🎉');
  expect(rows[2].blocks[0].runs.find(r => r.kind === 'emoji')?.href).toBe(`${cdn}/emojis/${ferris}.png`);
  expect(rows[0].reactions).toEqual([
    { id: '👍', emoji: '👍', src: '', name: '👍', count: 2, me: true },
    { id: ferris, emoji: '', src: `${cdn}/emojis/${ferris}.png`, name: 'ferris', count: 3, me: false },
    { id: dance, emoji: '', src: `${cdn}/emojis/${dance}.gif`, name: 'dance', count: 4, me: false },
  ]);
});

test('image reservations use wire dimensions, MIME types and SPOILER_ filenames; large ones ask the proxy for 3x their box', () => {
  const attachment = (n: number, filename: string, patch: Partial<Attachment> = {}): Attachment => ({
    id: snowflakeFromMs(NOW, n), filename, url: `${cdn}/media/${n}`, proxy_url: `${cdn}/proxy/${n}`,
    content_type: 'image/png', size: 1024, ...patch,
  });
  const attachments = [attachment(1, 'SPOILER_large.png', { width: 1200, height: 600 }),
    attachment(2, 'small.png', { width: 64, height: 64 }), attachment(3, 'unknown.png'),
    attachment(4, 'image', { width: 100, height: 200 }),
    attachment(5, 'fallback.PNG', { content_type: 'application/octet-stream', width: 640, height: 1280 }),
    attachment(6, 'notes.txt', { content_type: 'text/plain' }), attachment(7, 'SPOILER_unknown.png')];
  const row = fixture([message(0, { attachments })]).view().rows[0];
  expect(row.media).toEqual([
    { id: attachments[0].id, full: `${cdn}/media/1`, src: `${cdn}/proxy/1?width=960&height=480`, alt: 'SPOILER_large.png', spoiler: true,
      width: 1200, height: 600, fitWidth: 320, fitHeight: 160 },
    { id: attachments[1].id, full: `${cdn}/media/2`, src: `${cdn}/media/2`, alt: 'small.png', spoiler: false,
      width: 64, height: 64, fitWidth: 64, fitHeight: 64 },
    { id: attachments[2].id, full: `${cdn}/media/3`, src: `${cdn}/media/3`, alt: 'unknown.png', spoiler: false,
      width: 0, height: 0, fitWidth: 0, fitHeight: 0 },
    { id: attachments[3].id, full: `${cdn}/media/4`, src: `${cdn}/media/4`, alt: 'image', spoiler: false,
      width: 100, height: 200, fitWidth: 100, fitHeight: 200 },
    { id: attachments[4].id, full: `${cdn}/media/5`, src: `${cdn}/proxy/5?width=480&height=960`, alt: 'fallback.PNG', spoiler: false,
      width: 640, height: 1280, fitWidth: 160, fitHeight: 320 },
    { id: attachments[6].id, full: `${cdn}/media/7`, src: `${cdn}/media/7`, alt: 'SPOILER_unknown.png', spoiler: true,
      width: 0, height: 0, fitWidth: 0, fitHeight: 0 },
  ]);
});

test('rich embeds retain metadata and Markdown; image, gifv and video embeds use one picture', () => {
  const image = { url: `${cdn}/image`, proxy_url: `${cdn}/proxy`, width: 800, height: 1600 };
  const thumbnail = { ...image, url: `${cdn}/thumb`, width: 32, height: 32 };
  const row = fixture([message(0, { embeds: [{ type: 'rich', color: 0x0123ab, title: 'Title',
    url: 'https://example.com', provider: { name: 'Provider' }, author: { name: 'Writer' },
    description: '**description**', image, thumbnail }, { type: 'image', image, thumbnail },
    { type: 'gifv', thumbnail }, { type: 'video', thumbnail }] })]).view().rows[0];
  expect(row.embeds[0]).toMatchObject({ id: 'e0', color: '#0123ab', title: 'Title', url: 'https://example.com',
    provider: 'Provider', author: 'Writer' });
  expect(row.embeds[0].description[0].runs[0]).toMatchObject({ kind: 'text', text: 'description', bold: true });
  expect(row.embeds[0].image).toEqual([{ id: 'e0.image', full: image.url, src: `${image.proxy_url}?width=480&height=960`, width: 800, height: 1600,
    fitWidth: 160, fitHeight: 320, alt: 'Title', spoiler: false }]);
  expect(row.embeds[0].thumbnail[0]).toMatchObject({ src: thumbnail.url, fitWidth: 32, fitHeight: 32 });
  expect(row.embeds.slice(1).map(e => [e.image[0].src, e.thumbnail, e.color]))
    .toEqual([[`${image.proxy_url}?width=480&height=960`, [], ''], [thumbnail.url, [], ''], [thumbnail.url, [], '']]);
});

test('join rows use the author directory and expose their system text as plain text', () => {
  const row = fixture([message(0, { type: 7, content: '' })]).view().rows[0];
  expect(row).toMatchObject({ author: 'Ada', system: 'Ada joined the server.', plain: 'Ada joined the server.',
    source: '', grouped: false, blocks: [] });
});

test('grouping stops at seven minutes, a different author, replies and system rows', () => {
  const first = message(0), near = message(7 * 60000 - 1), boundary = message(7 * 60000);
  const reply = message(1000, { type: 19, message_reference: reference(first.id), referenced_message: first });
  const join = message(2000, { type: 7 }), other = message(3000, { author: me });
  const f = fixture([first, reply, join, other, near, boundary]), previous = f.row(first.id);
  expect(f.render(f.row(near.id), previous)).toMatchObject({ grouped: true, prevAt: NOW });
  expect(f.render(f.row(boundary.id), previous)).toMatchObject({ grouped: false, prevAt: NOW });
  for (const m of [reply, join, other]) expect(f.render(f.row(m.id), previous).grouped).toBe(false);
  for (const m of [reply, join]) expect(f.render(f.row(near.id), f.row(m.id)).grouped).toBe(false);
  expect(f.render(previous, f.row(near.id)).grouped).toBe(false);
  expect(f.render(previous)).toMatchObject({ grouped: false, prevAt: 0 });
});

test('pending sends keep their nonce key when MESSAGE_CREATE supplies the server id', () => {
  const f = fixture([message(0, { author: me })]), nonce = snowflakeFromMs(NOW + 1000);
  S.addPending(f.store, cid, { nonce, content: 'on my way', nowMs: NOW + 1000 });
  const pending = f.view().rows.at(-1)!;
  expect(pending).toMatchObject({ key: nonce, id: nonce, state: 'pending', at: NOW + 1000,
    prevAt: NOW, grouped: true, plain: 'on my way', mine: true });
  const confirmed = message(2000, { author: me, nonce, content: 'on my way' });
  S.applyDispatch(f.store, 'MESSAGE_CREATE', confirmed, NOW + 2000);
  expect(f.view().rows.map(r => [r.key, r.id, r.state])).toEqual([
    [message().id, message().id, 'sent'], [nonce, confirmed.id, 'sent'],
  ]);
  expect(f.view().rows[1]).toMatchObject({ key: nonce, id: confirmed.id, at: NOW + 2000, plain: 'on my way' });
});

test('a failed send breaks the next row group and invalidates its cached rendering', () => {
  const f = fixture([message(0, { author: me })]), nonce = snowflakeFromMs(NOW + 1000);
  S.addPending(f.store, cid, { nonce, content: 'first', nowMs: NOW + 1000 });
  const next = message(2000, { author: me, content: 'second' });
  S.applyDispatch(f.store, 'MESSAGE_CREATE', next, NOW + 2000);
  const before = f.view().rows;
  expect(before.map(r => r.grouped)).toEqual([false, true, true]);
  S.failPending(f.store, cid, nonce, 'Offline');
  const after = f.view().rows;
  expect(after[1]).toMatchObject({ key: nonce, state: 'failed', plain: 'first' });
  expect(after[2]).toMatchObject({ id: next.id, grouped: false, prevAt: NOW + 1000 });
  expect(after[2]).not.toBe(before[2]);
});

test('reply previews prefer held originals, follow edits and report deletions despite old snapshots', () => {
  const original = message(0, { content: '**original**\nline' });
  const reply = message(1000, { type: 19, content: 'answer', message_reference: reference(original.id),
    referenced_message: { ...original, content: 'stale snapshot' } });
  const f = fixture([original, reply]);
  expect(f.view().rows[1].reply).toEqual([{ id: original.id, author: 'Ada', avatar: `${cdn}/embed/avatars/0.png`, color: '',
    preview: 'original line', state: 'present' }]);
  S.applyDispatch(f.store, 'MESSAGE_UPDATE', { id: original.id, channel_id: cid, content: '**changed**\nline',
    edited_timestamp: new Date(NOW + 2000).toISOString() }, NOW + 2000);
  expect(f.view().rows[0]).toMatchObject({ edited: true, plain: 'changed\nline' });
  expect(f.view().rows[1].reply[0]).toMatchObject({ preview: 'changed line', state: 'present' });
  S.applyDispatch(f.store, 'MESSAGE_DELETE', { id: original.id, channel_id: cid }, NOW + 3000);
  expect(f.view().rows[0].reply).toEqual([{ id: original.id, author: '', avatar: '', color: '', preview: '', state: 'deleted' }]);
});

test('unloaded replies distinguish a wire snapshot, an unknown original and a null deleted original', () => {
  const original = message(0, { content: `**preview**\n${'x'.repeat(110)}` });
  const replies = [message(1000, { type: 19, message_reference: reference(original.id), referenced_message: original }),
    message(2000, { type: 19, message_reference: reference(missing) }),
    message(3000, { type: 19, message_reference: reference(missing), referenced_message: null })];
  const f = fixture(replies);
  expect(S.message(f.store, cid, original.id)).toBeNull();
  expect(f.view().rows.map(r => r.reply[0])).toEqual([
    { id: original.id, author: 'Ada', avatar: `${cdn}/embed/avatars/0.png`, color: '', preview: `preview ${'x'.repeat(92)}`, state: 'present' },
    { id: missing, author: '', avatar: '', color: '', preview: '', state: 'unknown' },
    { id: missing, author: '', avatar: '', color: '', preview: '', state: 'deleted' },
  ]);
});

test('cached rows refresh reactions and late embeds without changing their keys', () => {
  const m = message(), f = fixture([m]), before = f.view().rows[0];
  S.applyDispatch(f.store, 'MESSAGE_REACTION_ADD', { channel_id: cid, message_id: m.id,
    user_id: me.id, emoji: { id: ferris, name: 'ferris' } }, NOW + 1000);
  const reacted = f.view().rows[0];
  expect(reacted.reactions).toEqual([{ id: ferris, emoji: '', src: `${cdn}/emojis/${ferris}.png`,
    name: 'ferris', count: 1, me: true }]);
  expect(reacted.key).toBe(before.key);
  expect(reacted).not.toBe(before);
  S.applyDispatch(f.store, 'MESSAGE_UPDATE', { channel_id: cid, id: m.id, embeds: [{ type: 'image',
    image: { url: `${cdn}/late.png`, proxy_url: `${cdn}/proxy.png`, width: 640, height: 320 } }] }, NOW + 2000);
  const embedded = f.view().rows[0];
  expect(embedded.embeds[0].image).toEqual([{ id: 'e0.image', src: `${cdn}/late.png`, full: `${cdn}/late.png`, width: 640, height: 320,
    fitWidth: 320, fitHeight: 160, alt: '', spoiler: false }]);
  expect(embedded).toMatchObject({ key: m.id, edited: false, plain: 'hello', reactions: reacted.reactions });
  S.applyDispatch(f.store, 'MESSAGE_REACTION_REMOVE', { channel_id: cid, message_id: m.id,
    user_id: me.id, emoji: { id: ferris, name: 'ferris' } }, NOW + 3000);
  expect(f.view().rows[0].reactions).toEqual([]);
});

test('pending replies show their original and start a group before confirmation', () => {
  const original = message(0, { author: me, content: 'question' }), f = fixture([original]);
  const nonce = snowflakeFromMs(NOW + 1000);
  S.addPending(f.store, cid, { nonce, content: 'answer', nowMs: NOW + 1000, replyTo: { messageId: original.id } });
  expect(f.view().rows[1]).toMatchObject({ key: nonce, state: 'pending', grouped: false,
    reply: [{ id: original.id, author: 'Charlie Cheever', avatar: `${cdn}/embed/avatars/0.png`, color: '', preview: 'question', state: 'present' }] });
});

test('cached replies notice deletion of an original that was never loaded', () => {
  const reply = message(0, { type: 19, message_reference: reference(missing) }), f = fixture([reply]);
  expect(f.view().rows[0].reply[0].state).toBe('unknown');
  S.applyDispatch(f.store, 'MESSAGE_DELETE', { id: missing, channel_id: cid }, NOW + 1000);
  expect(f.view().rows[0].reply).toEqual([{ id: missing, author: '', avatar: '', color: '', preview: '', state: 'deleted' }]);
});

test('a window withholds an unheld predecessor and keeps shared rows stable when history arrives', () => {
  const all = Array.from({ length: 12 }, (_, i) => message(i * 1000)), f = fixture(all.slice(6), { limit: 6 });
  const before = f.view();
  expect(before.rows.map(r => r.id)).toEqual(all.slice(7).map(m => m.id));
  expect(before.rows[0]).toMatchObject({ grouped: true, prevAt: NOW + 6000 });
  expect(before).toMatchObject({ hasBefore: true, hasAfter: false, firstId: all[7].id, lastId: all[11].id, pin: all[7].id });
  S.applyHistoryPage(f.store, cid, { before: all[6].id, limit: 50 }, all.slice(0, 6).reverse());
  const after = f.view(S.latestWindow(f.store, cid, 6));
  expect(after.rows.map(r => r.id)).toEqual(all.slice(6).map(m => m.id));
  expect(after.rows[0]).toMatchObject({ grouped: true, prevAt: NOW + 5000 });
  expect(after.rows.slice(1)).toEqual(before.rows);
  expect(f.view().rows[0]).toMatchObject({ id: all[0].id, grouped: false, prevAt: 0 });
});

test('the New divider follows the read position only when its predecessor is held', () => {
  const all = Array.from({ length: 12 }, (_, i) => message(i * 1000)), f = fixture(all.slice(6), { limit: 6 });
  const since = all[5].id;
  const unread = { count: 6, more: true, at: NOW + 6000 };
  const unheld = f.view(S.latestWindow(f.store, cid), '', since, unread);
  expect(unheld.rows.filter(r => r.isNew)).toEqual([]);
  expect(unheld).toMatchObject({ unread: 6, unreadMore: true, unreadAt: NOW + 6000 });
  S.applyHistoryPage(f.store, cid, { before: all[6].id, limit: 50 }, all.slice(0, 6).reverse());
  const around = f.view(S.windowAround(f.store, cid, since, 6), since, since, { ...unread, more: false });
  expect(around.rows.filter(r => r.isNew).map(r => r.id)).toEqual([all[6].id]);
  expect(around).toMatchObject({ channel: cid, anchor: since, pin: since, firstId: all[2].id,
    lastId: all[7].id, hasBefore: true, hasAfter: true, unread: 6, unreadMore: false, unreadAt: NOW + 6000 });
  expect(f.view().rows.filter(r => r.isNew)).toEqual([]);
  S.applyDispatch(f.store, 'MESSAGE_DELETE', { id: all[6].id, channel_id: cid }, NOW + 12000);
  expect(f.view(undefined, '', since).rows.filter(r => r.isNew).map(r => r.id)).toEqual([all[7].id]);
});

test('empty windows return empty boundaries and no rendered rows', () => {
  const f = fixture([]);
  expect(windowOf({ rows: [], hasBefore: false, hasAfter: false }, cid, '', '',
    { count: 0, more: false, at: 0 }, f.render)).toEqual({ channel: cid, anchor: '', rows: [],
    hasBefore: false, hasAfter: false, firstId: '', lastId: '', pin: '', unread: 0, unreadMore: false, unreadAt: 0, need: '', cursor: '' });
});

test('a sticker is a picture of its own, and a Lottie one its name', () => {
  const still = message(0, { content: '', sticker_items: [{ id: '601', name: 'Wave', format_type: 1 }] } as Partial<Message>);
  const gif = message(1000, { content: 'look', sticker_items: [{ id: '603', name: 'Spin', format_type: 4 }] } as Partial<Message>);
  const lottie = message(2000, { content: '', sticker_items: [{ id: '602', name: 'Party *Parrot*', format_type: 3 }] } as Partial<Message>);
  const [a, b, c] = fixture([still, gif, lottie]).view().rows;
  expect(a.stickers).toEqual([{ id: '601', src: `${cdn}/stickers/601.png`, name: 'Wave' }]);
  expect(a.media).toEqual([]);
  expect(b.stickers[0].src).toBe(`${cdn}/stickers/603.gif`);
  expect(b.plain).toBe('look\nSpin');
  expect(c.stickers).toEqual([]);
  expect(c.plain).toBe('Party *Parrot*');
  expect(a.plain).toBe('Wave');
});

test('a Lottie sticker\'s name is words, never markup, and a reply to a sticker previews its name', () => {
  const lottie = message(0, { content: '', sticker_items: [{ id: '604', name: '@everyone [wave](https://example.com)', format_type: 3 }] } as Partial<Message>);
  const still = message(1000, { content: '', sticker_items: [{ id: '601', name: 'Wave', format_type: 1 }] } as Partial<Message>);
  const reply = message(2000, { content: 'nice', type: 19, message_reference: reference(still.id) } as Partial<Message>);
  const [l, , r] = fixture([lottie, still, reply]).view().rows;
  expect(l.mentionsMe).toBe(false);
  expect(l.blocks.flatMap(b => b.runs).map(x => [x.kind, x.text, x.italic, x.href])).toEqual([['text', '@everyone [wave](https://example.com)', true, '']]);
  expect(r.reply[0].preview).toBe('Wave');
});

test('a Lottie name follows an edited message\'s marker, is never jumbo, and plain names every sticker', () => {
  const both = message(0, { content: '👋', edited_timestamp: new Date(NOW).toISOString(),
    sticker_items: [{ id: '601', name: 'Wave', format_type: 1 }, { id: '602', name: 'Party', format_type: 3 }] } as Partial<Message>);
  const [row] = fixture([both]).view().rows;
  expect(row.jumbo).toBe(false);
  expect(row.blocks.map(b => b.runs.map(r => r.kind))).toEqual([['text', 'edited'], ['text']]);
  expect(row.plain).toBe('👋\nWave, Party');
});

test('a forward shows the message it forwards, under its label, and is never edited', () => {
  const forward = message(0, { content: '', message_reference: { ...reference(snowflakeFromMs(NOW - 5000)), type: 1 },
    message_snapshots: [{ message: { content: 'look **here**', attachments: [], embeds: [], type: 0,
      timestamp: new Date(NOW - 5000).toISOString(), edited_timestamp: null } }] } as Partial<Message>);
  const reply = message(1000, { content: 'nice', type: 19, message_reference: reference(forward.id) } as Partial<Message>);
  const [f, r] = fixture([forward, reply]).view().rows;
  expect(f.forwarded).toBe(true);
  expect(f.plain).toBe('look here');
  expect(f.reply).toEqual([]);
  expect(r.forwarded).toBe(false);
  expect(r.reply[0].preview).toBe('look here');
  expect(f.source).toBe('');
});

test('a user a forward mentions is named from the forwarded message', () => {
  const friend: User = { id: snowflakeFromMs(DIRECTORY, 9), username: 'friend', global_name: 'Outside Friend' };
  const forward = message(0, { content: '', message_reference: { ...reference(snowflakeFromMs(NOW - 5000)), type: 1 },
    message_snapshots: [{ message: { content: `hi <@${friend.id}>`, attachments: [], embeds: [], mentions: [friend], type: 0,
      timestamp: new Date(NOW - 5000).toISOString(), edited_timestamp: null } }] } as Partial<Message>);
  const [f] = fixture([forward]).view().rows;
  expect(f.plain).toBe('hi @Outside Friend');
});

test('a forward\'s own words show above it, and only they mention the reader', () => {
  const forward = message(0, { content: 'see this', message_reference: { ...reference(snowflakeFromMs(NOW - 5000)), type: 1 },
    message_snapshots: [{ message: { content: `ping <@${me.id}>`, attachments: [], embeds: [], mentions: [me], type: 0,
      timestamp: new Date(NOW - 5000).toISOString(), edited_timestamp: null } }] } as Partial<Message>);
  const [f] = fixture([forward]).view().rows;
  expect(f.note.flatMap(b => b.runs).map(r => r.text).join('')).toBe('see this');
  expect(f.plain).toBe('see this\nping @Charlie Cheever');
  expect(f.mentionsMe).toBe(false);
  const spoilers = (bs: typeof f.blocks) => bs.flatMap(b => b.runs).map(r => r.spoiler).filter(Boolean);
  const both = message(1000, { content: '||a||', message_reference: { ...reference(snowflakeFromMs(NOW - 5000)), type: 1 },
    message_snapshots: [{ message: { content: '||b||', attachments: [], embeds: [], type: 0,
      timestamp: new Date(NOW - 5000).toISOString(), edited_timestamp: null } }] } as Partial<Message>);
  const reply = message(2000, { content: 'ok', type: 19, message_reference: reference(both.id) } as Partial<Message>);
  const [, s, r] = fixture([forward, both, reply]).view().rows;
  expect(spoilers(s.note).some(id => spoilers(s.blocks).includes(id))).toBe(false);
  expect(r.reply[0].preview).toBe('a b');
});

test('a reply to a forward the cache does not hold names the users the forward mentions', () => {
  const friend: User = { id: snowflakeFromMs(DIRECTORY, 9), username: 'friend', global_name: 'Outside Friend' };
  const quoted = { id: snowflakeFromMs(NOW - 9000), channel_id: cid, author: bot, content: '', type: 0,
    message_snapshots: [{ message: { content: `hi <@${friend.id}>`, mentions: [friend], attachments: [], embeds: [] } }] };
  const reply = message(0, { content: 'yes', type: 19, message_reference: reference(quoted.id), referenced_message: quoted } as Partial<Message>);
  const [r] = fixture([reply]).view().rows;
  expect(r.reply[0].preview).toBe('hi @Outside Friend');
});

test('a poll shows its answers with their share of the votes, and its closing names it', () => {
  const poll = { question: { text: 'Lunch?' }, allow_multiselect: false, expiry: new Date(NOW + 3600000).toISOString(),
    answers: [{ answer_id: 1, poll_media: { text: 'Tacos', emoji: { id: null, name: '🌮' } } },
      { answer_id: 2, poll_media: { text: 'Ramen', emoji: { id: '501', name: 'ramen' } } }, { answer_id: 3, poll_media: { text: 'Salad' } }],
    results: { is_finalized: true, answer_counts: [{ id: 1, count: 2, me_voted: true }, { id: 2, count: 1, me_voted: false }] } };
  const asked = message(0, { content: '', poll } as Partial<Message>);
  const closed = message(1000, { content: '', type: 46, author: bot, message_reference: reference(asked.id),
    embeds: [{ type: 'poll_result', fields: [{ name: 'poll_question_text', value: 'Lunch?' }] }] } as Partial<Message>);
  const [p, c] = fixture([asked, closed]).view().rows;
  expect(p.plain).toBe('Lunch?');
  expect(p.poll).toEqual([{ question: 'Lunch?', multi: false, known: true, expiry: NOW + 3600000, closed: true, voted: true, votes: 3, answers: [
    { id: '1', text: 'Tacos', emoji: '🌮', src: '', count: 2, percent: 67, me: true, winner: true },
    { id: '2', text: 'Ramen', emoji: '', src: `${cdn}/emojis/501.png`, count: 1, percent: 33, me: false, winner: false },
    { id: '3', text: 'Salad', emoji: '', src: '', count: 0, percent: 0, me: false, winner: false }] }]);
  expect(c).toMatchObject({ system: "buildbot's poll Lunch? has closed.", systemIcon: 'chart', embeds: [] });
  const reply = message(2000, { content: 'ok', type: 19, message_reference: reference(asked.id) } as Partial<Message>);
  expect(fixture([asked, reply]).view().rows[1].reply[0].preview).toBe('Lunch?');
});

test('an embed shows its fields, author picture and footer with its time', () => {
  const m = message(0, { embeds: [{ type: 'rich', title: 'Deploy', author: { name: 'CI', icon_url: 'https://x/a.png', url: 'https://ci' },
    fields: [{ name: 'Branch', value: '`main`', inline: true }, { name: 'Notes', value: '||soon||' }],
    footer: { text: 'Buildkite', proxy_icon_url: 'https://x/f.png' }, timestamp: new Date(NOW).toISOString() }] } as Partial<Message>);
  const [e] = fixture([m]).view().rows[0].embeds;
  expect(e).toMatchObject({ author: 'CI', authorIcon: 'https://x/a.png', authorUrl: 'https://ci', footer: 'Buildkite',
    footerIcon: 'https://x/f.png', at: NOW });
  expect(e.fields.map(f => [f.id, f.name, f.inline, f.value.flatMap(b => b.runs).map(r => r.text).join('')]))
    .toEqual([['e0.f0', 'Branch', true, 'main'], ['e0.f1', 'Notes', false, 'soon']]);
  expect(e.fields[1].value[0].runs[0].spoiler).toBe('s0');
  // Discord reads `#` in a field's value as text, not a heading.
  const h = message(1000, { embeds: [{ type: 'rich', fields: [{ name: 'Ticket', value: '# 12345' }] }] } as Partial<Message>);
  const [field] = fixture([h]).view().rows[0].embeds[0].fields;
  expect([field.value[0].kind, field.value[0].runs.map(r => r.text).join('')]).toEqual(['paragraph', '# 12345']);
});

test('an app\'s answer to a command names who used it, and system messages read as Discord words them', () => {
  const used = message(0, { type: 20, author: bot, content: 'done', interaction_metadata: { id: '9', type: 2, user: ada },
    interaction: { id: '9', type: 2, name: 'deploy', user: ada } } as Partial<Message>);
  const menu = message(1000, { type: 23, author: bot, content: 'ok', interaction_metadata: { id: '8', type: 2, user: ada } } as Partial<Message>);
  const added = message(2000, { type: 1, content: '', author: me, mentions: [ada] } as Partial<Message>);
  const left = message(3000, { type: 2, content: '', author: ada, mentions: [ada] } as Partial<Message>);
  const thread = message(4000, { type: 18, content: 'Ride plans', author: me } as Partial<Message>);
  const boosted = message(5000, { type: 8, content: '2', author: me } as Partial<Message>);
  const [u, c, a, l, t, b] = fixture([used, menu, added, left, thread, boosted]).view().rows;
  expect(u.reply).toEqual([{ id: 'used:9', author: 'Ada', avatar: expect.any(String), color: '', preview: '/deploy', state: 'command' }]);
  expect(c.reply[0].preview).toBe('a command');
  // This channel is in a server, so a member comes and goes from a thread.
  expect([a, l, t, b].map(r => [r.system, r.systemIcon])).toEqual([
    ['Charlie Cheever added Ada to the thread.', 'follow'], ['Ada left the thread.', 'minus-circle'],
    ['Charlie Cheever started a thread: Ride plans.', 'messages'], ['Charlie Cheever just boosted the server 2 times!', 'sparkles']]);
  const at = (offset: number) => new Date(NOW + offset).toISOString();
  const missed = message(6000, { type: 3, content: '', call: { participants: [ada.id], ended_timestamp: at(6000 + 12 * 60000) } } as Partial<Message>);
  const ongoing = message(7000, { type: 3, content: '', call: { participants: [ada.id] } } as Partial<Message>);
  const level = message(8000, { type: 10, content: '', author: me } as Partial<Message>);
  const renamed = message(9000, { type: 4, content: '' } as Partial<Message>);
  expect(fixture([missed, ongoing, level, renamed]).view().rows.map(r => r.system)).toEqual([
    'You missed a call from Ada that lasted 12 minutes.', 'Ada started a call.',
    "Charlie Cheever just boosted the server! Exact Club has achieved Level 2!", 'Ada removed the custom group name.']);
});

test('a command\'s answer redraws when the person who used it changes', () => {
  const used = message(0, { type: 20, author: bot, content: 'done', interaction: { id: '9', type: 2, name: 'deploy', user: ada },
    interaction_metadata: { id: '9', type: 2, user: ada } } as Partial<Message>);
  const f = fixture([used]);
  expect(f.view().rows[0].reply[0].author).toBe('Ada');
  S.applyDispatch(f.store, 'MESSAGE_CREATE', message(1000, { author: { ...ada, global_name: 'Ada L.' } }), NOW);
  expect(f.view().rows[0].reply[0].author).toBe('Ada L.');
  // So does a line that names a member, or the server.
  const added = message(2000, { type: 1, content: '', author: me, mentions: [ada] } as Partial<Message>);
  const level = message(3000, { type: 9, content: '', author: me } as Partial<Message>);
  const g = fixture([added, level]);
  expect(g.view().rows[0].system).toBe('Charlie Cheever added Ada to the thread.');
  S.applyDispatch(g.store, 'MESSAGE_CREATE', message(4000, { author: { ...ada, global_name: 'Ada L.' } }), NOW);
  S.applyDispatch(g.store, 'GUILD_UPDATE', { id: gid, name: 'Exact Club 2' }, NOW);
  expect(g.view().rows.slice(0, 2).map(r => r.system)).toEqual(['Charlie Cheever added Ada L. to the thread.',
    'Charlie Cheever just boosted the server! Exact Club 2 has achieved Level 1!']);
  // A command's answer names its user from the message, newer than the directory's.
  const fresh = message(5000, { type: 20, author: bot, content: 'ok', interaction_metadata: { id: '7', type: 2,
    user: { ...ada, global_name: 'Ada New' } } } as Partial<Message>);
  expect(fixture([fresh]).view().rows[0].reply[0].author).toBe('Ada New');
});

test('a call\'s length reads as Discord words it', () => {
  const call = (ms: number, offset: number) => message(offset, { type: 3, content: '', author: me,
    call: { participants: [me.id], ended_timestamp: new Date(NOW + offset + ms).toISOString() } } as Partial<Message>);
  const rows = fixture([call(20000, 0), call(60000, 1000), call(12 * 60000, 2000), call(60 * 60000, 3000),
    call(3 * 3600000, 4000), call(30 * 3600000, 5000), call(3 * 86400000, 6000), call(89.75 * 60000, 7000),
    call(35.75 * 3600000, 8000), call(30 * 86400000, 9000)]).view().rows;
  expect(rows.map(r => r.system.replace('Charlie Cheever started a call that lasted ', ''))).toEqual(
    ['a few seconds.', 'a minute.', '12 minutes.', 'an hour.', '3 hours.', 'a day.', '3 days.', 'an hour.', 'a day.', 'a month.']);
});

test('a reply preview cut at its length never splits an emoji', () => {
  // The 100th character is an emoji: a cut in UTF-16 units left half of it, a lone surrogate, which a
  // host's JSON reader refuses, and with it the whole window.
  const original = message(0, { content: `${'a'.repeat(99)}😀 and more` });
  const reply = message(1000, { content: 'ok', type: 19, message_reference: reference(original.id) } as Partial<Message>);
  const preview = fixture([original, reply]).view().rows[1].reply[0].preview;
  expect(preview).toBe(`${'a'.repeat(99)}😀`);
  expect(/[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/.test(preview)).toBe(false);
});
