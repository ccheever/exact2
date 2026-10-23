import { expect, test } from 'bun:test';
import { applyDispatch, canView, channel, createStore, forgetAccount, guilds, type DiscordObject } from './store.ts';

const me = { id: '1', username: 'me' };
const GUILD = '10', MODS = '11', MUTED = '12';
const VIEW = String(1 << 10), ADMIN = String(1 << 3), SEND = String(1 << 11);

function store(roles: string[], overwrites: Record<string, DiscordObject[]>, extra: DiscordObject = {}) {
  const s = createStore();
  const ids = Object.keys(overwrites);
  applyDispatch(s, 'READY', {
    user: me,
    guilds: [{ id: GUILD, name: 'server', owner_id: '99', ...extra,
      roles: [{ id: GUILD, permissions: VIEW }, { id: MODS, permissions: SEND }, { id: MUTED, permissions: '0' }],
      channels: ids.map(id => ({ id, type: 0, permission_overwrites: overwrites[id] })) }],
    merged_members: [[{ user_id: me.id, roles }]],
  }, 0);
  return (id: string) => canView(s, channel(s, id)!);
}

test('@everyone sees a channel without overwrites, and its deny hides it', () => {
  const sees = store([], { '100': [], '101': [{ id: GUILD, type: 0, allow: '0', deny: VIEW }] });
  expect(sees('100')).toBe(true);
  expect(sees('101')).toBe(false);
});

test("a role's allow beats @everyone's deny, and the member's own deny beats both", () => {
  const sees = store([MODS], {
    '100': [{ id: GUILD, type: 0, allow: '0', deny: VIEW }, { id: MODS, type: 0, allow: VIEW, deny: '0' }],
    '101': [{ id: GUILD, type: 0, allow: '0', deny: VIEW }, { id: MODS, type: 0, allow: VIEW, deny: '0' },
      { id: me.id, type: 1, allow: '0', deny: VIEW }],
    // Role overwrites combine: one role's allow wins over another's deny.
    '102': [{ id: MUTED, type: 0, allow: '0', deny: VIEW }, { id: MODS, type: 0, allow: VIEW, deny: '0' }],
  });
  expect(sees('100')).toBe(true);
  expect(sees('101')).toBe(false);
  expect(sees('102')).toBe(true);
});

test("a role the account lacks changes nothing; another member's overwrite neither", () => {
  const sees = store([], { '100': [{ id: MODS, type: 0, allow: '0', deny: VIEW }, { id: '5', type: 1, allow: '0', deny: VIEW }] });
  expect(sees('100')).toBe(true);
});

test('an administrator and the owner see every channel', () => {
  const hidden = { '100': [{ id: GUILD, type: 0, allow: '0', deny: VIEW }] };
  const admin = createStore();
  applyDispatch(admin, 'READY', { user: me, guilds: [{ id: GUILD, roles: [{ id: GUILD, permissions: ADMIN }],
    channels: [{ id: '100', type: 0, permission_overwrites: hidden['100'] }] }], merged_members: [[{ user_id: me.id, roles: [] }]] }, 0);
  expect(canView(admin, channel(admin, '100')!)).toBe(true);
  expect(store([], hidden, { owner_id: me.id })('100')).toBe(true);
});

test('without the account\'s roles the channel is shown, and a member update then decides', () => {
  const s = createStore();
  applyDispatch(s, 'READY', { user: me, guilds: [{ id: GUILD, roles: [{ id: GUILD, permissions: '0' }, { id: MODS, permissions: VIEW }],
    channels: [{ id: '100', type: 0 }] }] }, 0);
  expect(canView(s, channel(s, '100')!)).toBe(true);
  applyDispatch(s, 'GUILD_MEMBER_UPDATE', { guild_id: GUILD, user: me, roles: [] }, 1);
  expect(canView(s, channel(s, '100')!)).toBe(false);
  applyDispatch(s, 'GUILD_MEMBER_UPDATE', { guild_id: GUILD, user: me, roles: [MODS] }, 2);
  expect(canView(s, channel(s, '100')!)).toBe(true);
  applyDispatch(s, 'GUILD_ROLE_UPDATE', { guild_id: GUILD, role: { id: MODS, permissions: '0' } }, 3);
  expect(canView(s, channel(s, '100')!)).toBe(false);
});

test('permissions above 2^53 are read whole', () => {
  const high = (1n << 40n | 1n << 10n).toString();
  const s = createStore();
  applyDispatch(s, 'READY', { user: me, guilds: [{ id: GUILD, roles: [{ id: GUILD, permissions: (1n << 60n | 1n << 10n).toString() }],
    channels: [{ id: '100', type: 0, permission_overwrites: [{ id: GUILD, type: 0, allow: high, deny: '0' }] }] }],
    merged_members: [[{ user_id: me.id, roles: [] }]] }, 0);
  expect(canView(s, channel(s, '100')!)).toBe(true);
});

test('forgetting the account empties the cache', () => {
  const s = createStore();
  applyDispatch(s, 'READY', { user: me, guilds: [{ id: GUILD, channels: [{ id: '100', type: 0 }] }] }, 0);
  forgetAccount(s);
  expect(s.meId).toBeNull();
  expect(guilds(s)).toEqual([]);
  expect(channel(s, '100')).toBeNull();
});

test('a deleted role grants nothing', () => {
  const s = createStore();
  applyDispatch(s, 'READY', { user: me, guilds: [{ id: GUILD, roles: [{ id: GUILD, permissions: '0' }, { id: MODS, permissions: '0' }],
    channels: [{ id: '100', type: 0, permission_overwrites: [{ id: MODS, type: 0, allow: VIEW, deny: '0' }] }] }],
    merged_members: [[{ user_id: me.id, roles: [MODS] }]] }, 0);
  expect(canView(s, channel(s, '100')!)).toBe(true);
  applyDispatch(s, 'GUILD_ROLE_DELETE', { guild_id: GUILD, role_id: MODS }, 1);
  expect(canView(s, channel(s, '100')!)).toBe(false);
});
