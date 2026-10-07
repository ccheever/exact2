// composer-fidelity G15: a route to a thread the environment does not hold shows
// NoActiveThreadState (T3 Code 1e2ecbd975 ChatView.tsx / NoActiveThreadState.tsx, MIT; see LICENSE-T3).
import { describe, expect, test } from 'bun:test';
import { landingKind, pagesHome } from './pages-home';
import { opened } from './composer-controls-fixture';

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
});
