// A route to a thread the environment does not hold shows
// NoActiveThreadState (T3 Code 74541bab17 ChatView.tsx / NoActiveThreadState.tsx, threadRoutes.ts, MIT; see LICENSE-T3).
import { describe, expect, test } from 'bun:test';
import { landingKind, pagesHome } from '../../src/pages/pages-home';
import { opened } from '../support/composer-controls-fixture';

const base = { connected: true, ready: true, savedEnvironments: 1, connecting: false, projects: 1, projectId: 'p1', threadId: 't9' };

describe('no active thread', () => {
  test('landingKind names a missing thread only once connected and ready', () => {
    expect(landingKind({ ...base, missingThread: true })).toBe('no-thread');
    expect(landingKind({ ...base, missingThread: false })).toBe('');
    expect(landingKind({ ...base, ready: false, missingThread: true })).toBe('offline');
    expect(landingKind({ ...base, threadId: '', missingThread: true })).toBe('');
  });
  test('selecting a thread the shell does not hold routes to the empty state instead of refusing', async () => {
    const { client, native, command } = await opened();
    const result = await command('select-thread', 'gone');
    expect(result.message).toBe('');
    expect(client.threadId).toBe('gone');
    expect(client.thread).toBeNull();
    expect((await pagesHome(client, native, false)).landing).toBe('no-thread');
    await command('select-thread', 't2');
    await client.refresh(native, (await opened()).disk);
    expect((await pagesHome(client, native, false)).landing).toBe('');
  });
  // resolveThreadRouteRenderState: shell membership can lag a new thread; its own detail decides.
  test('a thread the shell does not list yet opens from its detail; an archived one is missing', async () => {
    const { client, native, command } = await opened();
    const fresh = structuredClone(native.details.t2!);
    ((fresh.projection as Record<string, unknown>).thread as Record<string, unknown>).id = 'fresh';
    native.details.fresh = fresh;
    await command('select-thread', 'fresh');
    expect(client.threadId).toBe('fresh');
    expect(client.absentThreadId).toBe('');
    expect(client.thread).not.toBeNull();
    expect((await pagesHome(client, native, false)).landing).toBe('');
    const archived = structuredClone(fresh);
    ((archived.projection as Record<string, unknown>).thread as Record<string, unknown>).id = 'old';
    ((archived.projection as Record<string, unknown>).thread as Record<string, unknown>).archivedAt = '2026-10-03T02:00:00.000Z';
    native.details.old = archived;
    await command('select-thread', 'old');
    expect(client).toMatchObject({ threadId: 'old', absentThreadId: 'old', thread: null });
    expect((await pagesHome(client, native, false)).landing).toBe('no-thread');
  });
});
