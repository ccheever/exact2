import { expect, test } from 'bun:test';
import { applyDispatch, createStore } from './store.ts';

test('a member list follows SYNC, INSERT, UPDATE, DELETE and INVALIDATE, and starts over for a new list', () => {
  const s = createStore();
  applyDispatch(s, 'READY', { user: { id: '1' }, guilds: [{ id: '10', channels: [] }] }, 0);
  const member = (id: string) => ({ member: { user: { id, username: `u${id}` }, roles: [], presence: { status: 'online' } } });
  const ids = () => s.memberLists.get('10')!.items.map(i => !i ? '-' : 'group' in i ? i.group.id : i.member.user.id);
  applyDispatch(s, 'GUILD_MEMBER_LIST_UPDATE', { guild_id: '10', id: 'a', ops: [{ op: 'SYNC', range: [0, 99],
    items: [{ group: { id: 'online', count: 2 } }, member('2'), member('3')] }] }, 1);
  expect(ids()).toEqual(['online', '2', '3']);
  expect(s.users.get('2')).toMatchObject({ username: 'u2' });
  applyDispatch(s, 'GUILD_MEMBER_LIST_UPDATE', { guild_id: '10', id: 'a', ops: [{ op: 'INSERT', index: 1, item: member('4') },
    { op: 'DELETE', index: 3 }, { op: 'UPDATE', index: 2, item: member('5') }] }, 2);
  expect(ids()).toEqual(['online', '4', '5']);
  applyDispatch(s, 'GUILD_MEMBER_LIST_UPDATE', { guild_id: '10', id: 'a', ops: [{ op: 'INVALIDATE', range: [1, 1] }] }, 3);
  expect(ids()).toEqual(['online', '-', '5']);
  const revision = s.memberLists.get('10')!.revision;
  applyDispatch(s, 'GUILD_MEMBER_LIST_UPDATE', { guild_id: '10', id: 'b', ops: [{ op: 'SYNC', range: [0, 0], items: [member('6')] }] }, 4);
  expect(ids()).toEqual(['6']);
  expect(s.memberLists.get('10')!.revision).toBeGreaterThan(revision);
});

test('a SYNC replaces its whole range, and a role change or another guild never repeats a revision', () => {
  const s = createStore();
  applyDispatch(s, 'READY', { user: { id: '1' }, guilds: [{ id: '10', channels: [], roles: [] }, { id: '11', channels: [] }] }, 0);
  const member = (id: string) => ({ member: { user: { id }, roles: [] } });
  const sync = (guild: string, list: string, items: unknown[], seq: number) =>
    applyDispatch(s, 'GUILD_MEMBER_LIST_UPDATE', { guild_id: guild, id: list, ops: [{ op: 'SYNC', range: [0, 99], items }] }, seq);
  const ids = () => s.memberLists.get('10')!.items.map(i => i && 'member' in i ? i.member.user.id : '-');
  sync('10', 'a', [member('2'), member('3'), member('4')], 1);
  sync('10', 'a', [member('2')], 2);
  expect(ids()).toEqual(['2', '-', '-']);
  const first = s.memberLists.get('10')!.revision;
  sync('11', 'a', [member('5')], 3);
  expect(s.memberLists.get('11')!.revision).not.toBe(first);
  applyDispatch(s, 'GUILD_ROLE_UPDATE', { guild_id: '10', role: { id: '7', name: 'Admin', color: 0xff0000 } }, 4);
  expect(s.memberLists.get('10')!.revision).toBeGreaterThan(s.memberLists.get('11')!.revision);
  sync('10', 'a', Array.from({ length: 100 }, (_, i) => member(`${100 + i}`)), 5);
  applyDispatch(s, 'GUILD_MEMBER_LIST_UPDATE', { guild_id: '10', id: 'a', ops: [{ op: 'INVALIDATE', range: [50, 99] },
    { op: 'INSERT', index: 99, item: member('4') }] }, 6);
  expect(ids().indexOf('4')).toBe(99);
  sync('10', 'a', [], 7);
  expect(ids().filter(id => id !== '-')).toEqual([]);
});

test('a list\'s rows revision moves with every op but UPDATE, with a new list and with a role', () => {
  const s = createStore();
  applyDispatch(s, 'READY', { user: { id: '1' }, guilds: [{ id: '10', channels: [] }] }, 0);
  const member = (id: string) => ({ member: { user: { id }, roles: [] } });
  const rows = (id: string, op: object) => {
    applyDispatch(s, 'GUILD_MEMBER_LIST_UPDATE', { guild_id: '10', id, ops: [op] }, 1);
    return s.memberLists.get('10')!.rows;
  };
  const synced = rows('a', { op: 'SYNC', range: [0, 99], items: [member('2'), member('3')] });
  expect(synced).toBeGreaterThan(0);
  expect(rows('a', { op: 'UPDATE', index: 0, item: member('2') })).toBe(synced);
  const invalidated = rows('a', { op: 'INVALIDATE', range: [0, 99] });
  expect(invalidated).toBeGreaterThan(synced);
  const resynced = rows('a', { op: 'SYNC', range: [0, 99], items: [member('2')] });
  expect(resynced).toBeGreaterThan(invalidated);
  const inserted = rows('a', { op: 'INSERT', index: 0, item: member('4') });
  expect(inserted).toBeGreaterThan(resynced);
  const deleted = rows('a', { op: 'DELETE', index: 0 });
  expect(deleted).toBeGreaterThan(inserted);
  const fresh = rows('b', { op: 'UPDATE', index: 0, item: member('5') });
  expect(fresh).toBeGreaterThan(deleted);
  applyDispatch(s, 'GUILD_ROLE_UPDATE', { guild_id: '10', role: { id: '20', name: 'Owners', color: 0xff0000 } }, 2);
  expect(s.memberLists.get('10')!.rows).toBeGreaterThan(fresh);
});
