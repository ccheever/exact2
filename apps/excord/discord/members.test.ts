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
  expect(s.memberLists.get('10')!.revision).toBe(1);
  expect(revision).toBe(3);
});
