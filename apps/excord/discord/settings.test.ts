import { expect, test } from 'bun:test';
import { settingsProto } from '../fixture/settings.ts';
import { guildFolders, guildOrder } from './settings.ts';
import { applyDispatch, createStore, guilds } from './store.ts';

const big = '1191168914227200001';

test('the folders give the order, a folder of several in place', () => {
  const proto = settingsProto([{ ids: ['30'] }, { ids: ['10', big], name: 'Work' }, { ids: ['20'] }]);
  expect(guildOrder(proto)).toEqual(['30', '10', big, '20']);
});

test('a folder carries its id, name and colour; a guild in none is a folder of one with neither', () => {
  const proto = settingsProto([{ ids: ['30'] }, { ids: ['10', big], id: 77, name: 'Work ⚙️', color: 0x5865f2 }]);
  expect(guildFolders(proto)).toEqual([{ id: '', name: '', color: null, guildIds: ['30'] },
    { id: '77', name: 'Work ⚙️', color: 0x5865f2, guildIds: ['10', big] }]);
  // A wrapper with nothing in it holds zero: folder 0, black.
  expect(guildFolders(settingsProto([{ ids: ['10', '20'], id: 0, color: 0 }]))).toEqual([{ id: '0', name: '', color: 0, guildIds: ['10', '20'] }]);
});

test('settings without folders say nothing about the order', () => {
  expect(guildOrder(btoa(String.fromCharCode(13 << 3 | 2, 2, 8, 2)))).toBeNull();
});

test('READY sorts the servers by the folders; one not in them comes first, newest first', () => {
  const s = createStore();
  applyDispatch(s, 'READY', { user: { id: '1' }, user_settings_proto: settingsProto([{ ids: ['20'] }, { ids: ['10'] }]),
    guilds: [{ id: '10', name: 'a' }, { id: '20', name: 'b' }, { id: '30', name: 'c', joined_at: '2026-01-01T00:00:00Z' },
      { id: '40', name: 'd', joined_at: '2026-02-01T00:00:00Z' }] }, 0);
  expect(guilds(s).map(g => g.id)).toEqual(['40', '30', '20', '10']);
});

test('a settings update moves the servers, and an unreadable one leaves them', () => {
  const s = createStore();
  applyDispatch(s, 'READY', { user: { id: '1' }, guilds: [{ id: '10' }, { id: '20' }],
    user_settings_proto: settingsProto([{ ids: ['10'] }, { ids: ['20'] }]) }, 0);
  const moved = applyDispatch(s, 'USER_SETTINGS_PROTO_UPDATE', { settings: { type: 1, proto: settingsProto([{ ids: ['20', '10'] }]) }, partial: false }, 0);
  expect(moved.sidebar).toBe(true);
  expect(guilds(s).map(g => g.id)).toEqual(['20', '10']);
  applyDispatch(s, 'USER_SETTINGS_PROTO_UPDATE', { settings: { type: 1, proto: 'not base64!' }, partial: false }, 0);
  expect(guilds(s).map(g => g.id)).toEqual(['20', '10']);
});
