// ThreadRelationshipsControl at T3 Code 2359643cff (MIT, see LICENSE-T3): side chats are a section of their own, Lineage's
// heading offers Disconnect agent session, a failed Merge back or Disconnect is a toast the next try closes, and
// Stop's spinner is motion-safe.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import type { Files, Native } from '../../src/client/protocol';
import { lineageView } from '../../src/details/shell-lineage';
import { canDetachSession, detachSession, mergeBack } from '../../src/details/lineage-actions';
import { THREAD_DETAILS_ITEMS } from '../../src/details/thread-details-items';
import { toasts } from '../../src/shell/toast';
import { contractComponent } from '../support/contract-action-fixture';

const at = (minutes: number) => new Date(Date.UTC(2026, 10, 1, 10, minutes)).toISOString();
const shell = (id: string, extra: Obj = {}): Obj => ({ id, title: `T ${id}`, status: 'idle', createdAt: at(0), lineage: { parentThreadId: null, relationshipToParent: null }, ...extra });
const sideChat = (id: string, status = 'idle') => shell(id, { status, createdAt: at(2), lineage: { parentThreadId: 'me', relationshipToParent: 'fork', sideChat: true } });
const live = { providerSessions: [{ id: 's1', status: 'running' }], providerThreads: [], runs: [] };

describe('Side chats and Lineage', () => {
  test('side chats leave Lineage for their own group, with the side chat glyph and hint', () => {
    const fork = shell('fork', { createdAt: at(1), lineage: { parentThreadId: 'me', relationshipToParent: 'fork' } });
    const view = lineageView([shell('me'), fork, sideChat('side', 'running')], { thread: { id: 'me', lineage: {} }, runs: [], subagents: [] }, 'me', [], 0);
    expect(view.lineage.map(row => [row.id, row.group, row.icon, row.hint, row.status])).toEqual([
      ['side', 'side', 'messages-square', 'Open side chat in this chat', 'Running'], ['fork', 'related', 'git-fork', 'Open fork in this chat', 'Idle']]);
    expect([view.showLineage, view.canDetach]).toEqual([true, false]);
  });
  test('a thread with only side chats shows no Lineage, unless it offers Disconnect', () => {
    const only = [shell('me'), sideChat('side')];
    expect(lineageView(only, { thread: { id: 'me', lineage: {} }, runs: [], subagents: [] }, 'me', [], 0).showLineage).toBe(false);
    expect(lineageView(only, { thread: { id: 'me', lineage: {} }, subagents: [], ...live }, 'me', [], 0)).toMatchObject({ showLineage: true, canDetach: true });
  });
  test('Side chats is an item of the card, on by default after Lineage', () => {
    const ids = THREAD_DETAILS_ITEMS.map(item => item.id);
    expect(ids.slice(ids.indexOf('lineage'), ids.indexOf('lineage') + 2)).toEqual(['lineage', 'sideChats']);
    expect(THREAD_DETAILS_ITEMS.find(item => item.id === 'sideChats')).toEqual({ id: 'sideChats', label: 'Side chats', defaultVisible: true });
  });
  test('the card draws the Side chats section with its running count, and a side chat row opens its tab', () => {
    const body = contractComponent('shell-details.contract', 'DetailsBody');
    const details = { items: { sideChats: true, lineage: true }, lineage: [{ group: 'side', status: 'Running' }, { group: 'side', status: 'Idle' }], showLineage: false };
    const derived = body.derives({}, { details, density: 'full' });
    expect([derived.showSideChats, derived.showLineage]).toEqual([true, false]);
    const hidden = body.derives({}, { details: { ...details, items: { sideChats: false, lineage: true } }, density: 'full' });
    expect(hidden.showSideChats).toBe(false);
    const row = contractComponent('shell-details.contract', 'LineageRow');
    const pressed = (group: string) => row.attribute('testId=`details-lineage-${row.id}`', 'press', {}, { row: { group, id: 'x', canStop: false }, hovered: '', focused: '', act: (...args: unknown[]) => args });
    expect([pressed('side'), pressed('related')]).toEqual([['shelllocal:surface-side-chat-open', 'x', ''], ['select-thread', 'x', '']]);
  });
});

describe('Disconnect agent session (canDetachThreadProviderSession)', () => {
  test('the active run\'s session, else the attached provider thread\'s, else the newest live one', () => {
    expect(canDetachSession(null)).toBe(false);
    expect(canDetachSession({ thread: { id: 'me' }, ...live })).toBe(true);
    expect(canDetachSession({ thread: { id: 'me' }, runs: [], providerThreads: [], providerSessions: [{ id: 's1', status: 'stopped' }] })).toBe(false);
    expect(canDetachSession({ thread: { id: 'me', activeProviderThreadId: 'pt' }, runs: [], providerThreads: [{ id: 'pt', providerSessionId: 's0' }],
      providerSessions: [{ id: 's0', status: 'error' }, { id: 's1', status: 'running' }] })).toBe(false);
  });
});

describe('a failed Merge back or Disconnect is a toast the next try closes', () => {
  const native = { available: true, watch() {}, later: async () => ({}) } as unknown as Native;
  function client(fail: boolean) {
    const sent: Obj[] = [], opened: string[] = [];
    const access = {
      ids: async () => ['c1'],
      dispatch: async (_storage: Files, payload: Obj) => { sent.push(payload); if (fail) { value.error = 'refused'; throw new Error('refused'); } return {}; },
      request: async (method: string, payload: Obj) => {
        if (method === 'orchestration.getThreadProjection') return { providerSessions: [{ id: 's1' }, { id: 's2' }] };
        sent.push(payload); if (fail) throw new Error('refused'); return {};
      },
    };
    const value = { threadId: 'fork', error: '', projection: { thread: { id: 'fork' }, ...live }, shell: { threads: [shell('fork')] }, restAccess: () => access,
      openSelected: async (_native: Native, id: string) => { opened.push(id); } };
    return { client: value as unknown as T3Client & { error: string }, sent, opened };
  }
  test('Merge back: the toast, no banner and no navigation; a retry closes the old toast first', async () => {
    const { client: c, sent, opened } = client(true);
    await mergeBack(c, native, {} as Files, 'fork', 'parent|run1');
    expect(sent[0]).toMatchObject({ type: 'thread.merge_back', sourceThreadId: 'fork', targetThreadId: 'parent', sourcePoint: { type: 'run', runId: 'run1' } });
    expect([toasts(c).map(toast => [toast.kind, toast.title, toast.description]), c.error, opened]).toEqual([[['error', 'Could not merge back', '']], '', []]);
    const first = toasts(c)[0]!.id;
    await mergeBack(c, native, {} as Files, 'fork', 'parent|run1');
    expect(toasts(c).map(toast => toast.id)).not.toContain(first);
    expect(toasts(c)).toHaveLength(1);
    const ok = client(false);
    await mergeBack(ok.client, native, {} as Files, 'fork', 'parent|run1');
    expect([ok.opened, toasts(ok.client)]).toEqual([['parent'], []]);
  });
  test('Disconnect detaches each provider session; a failure toasts', async () => {
    const ok = client(false);
    await detachSession(ok.client, native, 'fork');
    expect(ok.sent).toEqual([
      { type: 'provider-session.detach', commandId: 'c1:detach:s1', threadId: 'fork', providerSessionId: 's1', reason: 'client-requested' },
      { type: 'provider-session.detach', commandId: 'c1:detach:s2', threadId: 'fork', providerSessionId: 's2', reason: 'client-requested' }]);
    const failed = client(true);
    await detachSession(failed.client, native, 'fork');
    expect(toasts(failed.client).map(toast => toast.title)).toEqual(['Could not disconnect agent session']);
  });
  test('the heading offers the menu only while a session can be detached', () => {
    const heading = contractComponent('shell-details.contract', 'LineageHeading');
    expect(heading.drawn('testId="details-detach-session"', heading.state(), { details: { canDetach: true } })).toBe(true);
    expect(heading.drawn('testId="details-detach-session"', heading.state(), { details: { canDetach: false } })).toBe(false);
  });
});

describe('Stop\'s spinner is motion-safe (Spinner)', () => {
  test('it stays still under Reduce motion', () => {
    const row = contractComponent('shell-details.contract', 'LineageRow');
    const spin = (still: boolean) => row.attribute('animation=(still', 'animation', {}, { still, row: { id: 'x', canStop: true }, hovered: '', focused: '' });
    expect([spin(false), spin(true)]).toEqual(['lineageSpin 1s linear infinite', 'none']);
  });
});
