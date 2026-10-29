import { expect, test } from 'bun:test';
import * as S from './discord/store';
import { emojiChoices, resolveMentions, suggest, switchTargets } from './compose';

const cdn = 'http://cdn';
const me = { id: '1', username: 'charlie', global_name: 'Charlie' };
const ada = { id: '2', username: 'ada', global_name: 'Ada Lovelace' };
const adam = { id: '3', username: 'adam', global_name: 'Adam' };
const grace = { id: '4', username: 'grace', global_name: 'Grace Hopper', avatar: 'abc' };
const cid = '100', dm = '200';

function store(): S.Store {
  const s = S.createStore();
  S.applyDispatch(s, 'READY', { user: me, users: [ada, adam, grace], guilds: [{ id: '10', name: 'Club',
    channels: [{ id: cid, type: 0, name: 'general' }, { id: '101', type: 0, name: 'help' }, { id: '102', type: 2, name: 'voice' }],
    emojis: [{ id: '500', name: 'ferris', animated: false }, { id: '501', name: 'party_parrot', animated: true },
      { id: '502', name: 'gone', available: false }] }],
  private_channels: [{ id: dm, type: 1, recipients: [grace] }] }, 0);
  S.applyHistoryPage(s, cid, { limit: 50 }, [
    { id: '1001', channel_id: cid, author: adam, content: 'hi', timestamp: '2026-10-22T12:01:00.000Z', attachments: [], embeds: [] },
    { id: '1000', channel_id: cid, author: ada, content: 'hello', timestamp: '2026-10-22T12:00:00.000Z', attachments: [], embeds: [] },
  ]);
  return s;
}

test('@ suggests people, the most recent writer first, prefixes before substrings, never the reader', () => {
  const s = store();
  expect(suggest(s, cdn, cid, 'hey @ad').map(x => [x.label, x.detail, x.text]))
    .toEqual([['Adam', 'adam', 'hey @adam '], ['Ada Lovelace', 'ada', 'hey @ada ']]);
  expect(suggest(s, cdn, cid, '@hop').map(x => x.label)).toEqual(['Grace Hopper']);
  expect(suggest(s, cdn, cid, '@hop')[0].avatar).toBe(`${cdn}/avatars/4/abc.png?size=128`);
  expect(suggest(s, cdn, cid, '@char')).toEqual([]);
  // A DM suggests its recipients.
  expect(suggest(s, cdn, dm, '@').map(x => x.label)).toEqual(['Grace Hopper']);
  // Only the token at the end, after a space or the start.
  expect(suggest(s, cdn, cid, 'mail me@ad')).toEqual([]);
  expect(suggest(s, cdn, cid, '@ada said')).toEqual([]);
});

test('# suggests the text channels, : the server\'s emoji then Unicode ones', () => {
  const s = store();
  expect(suggest(s, cdn, cid, 'see #').map(x => x.text)).toEqual(['see #general ', 'see #help ']);
  expect(suggest(s, cdn, dm, '#')).toEqual([]);
  expect(suggest(s, cdn, cid, ':p')).toEqual([]);
  const party = suggest(s, cdn, cid, 'so :part');
  expect(party.map(x => [x.label, x.avatar, x.glyph, x.text])).toEqual([
    [':party_parrot:', `${cdn}/emojis/501.gif`, '', 'so :party_parrot: '],
    [':partying_face:', '', '🥳', 'so 🥳 '],
  ]);
  expect(suggest(s, cdn, cid, ':gone')).toEqual([]);
});

test('a send turns known names into mentions, outside code', () => {
  const s = store();
  expect(resolveMentions(s, cid, '@ada see #help :ferris: and :party_parrot:'))
    .toBe('<@2> see <#101> <:ferris:500> and <a:party_parrot:501>');
  expect(resolveMentions(s, cid, '@nobody #nowhere :unknown: me@ada.com')).toBe('@nobody #nowhere :unknown: me@ada.com');
  expect(resolveMentions(s, cid, '`@ada` and ```\n#help\n``` but @ada')).toBe('`@ada` and ```\n#help\n``` but <@2>');
  expect(resolveMentions(s, cid, 'already <@2> and <:ferris:500>')).toBe('already <@2> and <:ferris:500>');
  expect(resolveMentions(s, cid, 'thanks @ada, see #help.')).toBe('thanks <@2>, see <#101>.');
});

test('a server\'s emoji follow GUILD_EMOJIS_UPDATE, and a READY that nests them in properties', () => {
  const s = store();
  S.applyDispatch(s, 'GUILD_EMOJIS_UPDATE', { guild_id: '10', emojis: [{ id: '600', name: 'ferris_wave' }] }, 1);
  expect(suggest(s, cdn, cid, ':fer').map(x => x.label)).toEqual([':ferris_wave:']);
  const nested = S.createStore();
  S.applyDispatch(nested, 'READY', { user: me, guilds: [{ id: '10', properties: { name: 'Club', emojis: [{ id: '500', name: 'ferris' }] },
    channels: [{ id: cid, type: 0, name: 'general' }] }] }, 0);
  expect(resolveMentions(nested, cid, ':ferris:')).toBe('<:ferris:500>');
  expect(S.guilds(nested)[0]).not.toHaveProperty('emojis');
});

test('a mention resolves only to the one it names', () => {
  const s = store();
  S.applyDispatch(s, 'MESSAGE_CREATE', { id: '1002', channel_id: cid, author: { id: '5', username: 'ada.' }, content: 'x',
    timestamp: '2026-10-22T12:02:00.000Z', attachments: [], embeds: [] }, 2);
  // A handle that ends in a period is that person; a sentence's period after another is not part of it.
  expect(resolveMentions(s, cid, 'hi @ada.')).toBe('hi <@5>');
  expect(resolveMentions(s, cid, 'hi @adam.')).toBe('hi <@3>.');
  // Markup around a mention is no word.
  expect(resolveMentions(s, cid, '**@adam** *#help*')).toBe('**<@3>** *<#101>*');
  // Two text channels of one name, or a voice channel's name, stay text.
  S.applyDispatch(s, 'CHANNEL_CREATE', { id: '103', guild_id: '10', type: 0, name: 'help' }, 3);
  expect(resolveMentions(s, cid, '#help #voice')).toBe('#help #voice');
  // Code of any backtick run is sent as written; an unclosed run is text.
  expect(resolveMentions(s, cid, '``@adam` `` ``\n@adam\n`` `@adam')).toBe('``@adam` `` ``\n@adam\n`` `<@3>');
});

test('a prefix match is offered however many substring matches come before it', () => {
  const s = store();
  const many = Array.from({ length: 60 }, (_, i) => ({ id: String(900 + i), username: `x${i}zed` }));
  S.applyDispatch(s, 'READY', { user: me, users: [...many, { id: '999', username: 'zedd' }], guilds: [{ id: '10', name: 'Club',
    channels: [{ id: cid, type: 0, name: 'general' }] }] }, 0);
  const listed = suggest(s, cdn, cid, '@zed');
  expect(listed[0].detail).toBe('zedd');
  expect(listed).toHaveLength(8);
});

test('the reaction picker offers the server\'s emoji first, then Unicode ones, by name', () => {
  const s = store();
  expect(emojiChoices(s, cdn, '', 'x')).toEqual([]);
  const all = emojiChoices(s, cdn, cid, '');
  expect(all.slice(0, 2).map(c => [c.label, c.value])).toEqual([['ferris', '500'], ['party_parrot', '501']]);
  expect(all.find(c => c.label === 'gone')).toBeUndefined();
  expect(emojiChoices(s, cdn, cid, ':heart_').map(c => c.glyph)).toEqual(['😍', '❤️‍🔥']);
  expect(emojiChoices(s, cdn, cid, 'thumbsup')[0]).toMatchObject({ glyph: '👍', value: '👍' });
  expect(emojiChoices(s, cdn, dm, 'fer').map(c => c.label)).toEqual([]);
});

test('the quick switcher finds text channels in every server and the DMs, unread first; no search lists the unread', () => {
  const s = store();
  const name = (id: string) => ({ name: id === dm ? 'Grace Hopper' : 'group' }) as any;
  expect(switchTargets(s, cdn, 'he', name).map(t => [t.label, t.detail, t.server, t.glyph])).toEqual([['help', 'Club', '10', '#']]);
  expect(switchTargets(s, cdn, '#gen', name).map(t => t.id)).toEqual([cid]);
  expect(switchTargets(s, cdn, 'hop', name)).toMatchObject([{ id: dm, detail: 'Direct Message', server: '@me', avatar: `${cdn}/avatars/4/abc.png?size=128` }]);
  expect(switchTargets(s, cdn, 'voice', name)).toEqual([]);
  // #general has history and no read state, so it is unread; a mention puts #help before it.
  expect(switchTargets(s, cdn, '', name).map(t => t.label)).toEqual(['general']);
  S.applyDispatch(s, 'MESSAGE_CREATE', { id: '2000', channel_id: '101', author: ada, content: '<@1>', mentions: [me],
    timestamp: '2026-10-22T12:05:00.000Z', attachments: [], embeds: [] }, 5);
  expect(switchTargets(s, cdn, '', name).map(t => [t.label, t.mentions])).toEqual([['help', 1], ['general', 0]]);
});

test('code is what the parser reads as code: a fence closes at the next three backticks', () => {
  const s = store();
  expect(resolveMentions(s, cid, '```\n@adam\n```` then @adam')).toBe('```\n@adam\n```` then <@3>');
  expect(resolveMentions(s, cid, '\\`@adam\\` and `@adam`')).toBe('\\`<@3>\\` and `@adam`');
});
