// threadFindStore.test.ts at T3 Code 7c3b3b3605ef (MIT, see LICENSE-T3) "findForMessageMatch", and the request a ⌘K
// message result leaves: taken once its thread is open, within 30 s, as find at the message.
import { describe, expect, it } from 'bun:test';
import { findForMessageMatch, findOpening, requestThreadFind, takeRequestedFind } from '../../src/shared/thread-find-requests';
import { threadFindState } from '../../src/timeline/thread-find-runtime';
import type { T3Client } from '../../src/client/client';

describe('findForMessageMatch', () => {
  it('opens find on the longest word as the snippet writes it, at the matched message', () => {
    expect(findForMessageMatch({ snippet: 'The cafeteria, then the Café', messageId: 'message-1' }, 'cafe')).toEqual({ query: 'Café', start: { entryId: 'message-1', occurrence: 0 } });
    expect(findForMessageMatch({ snippet: 'fix the sidebar lag' }, 'fix sidebar')).toEqual({ query: 'sidebar' });
    // The whole query, when the snippet holds it as typed.
    expect(findForMessageMatch({ snippet: 'then Vp  run dev again' }, 'vp run dev')).toEqual({ query: 'Vp  run dev' });
  });
});

describe('a message result\'s find request', () => {
  const client = (threadId: string, threadFind = true) => ({ generation: 1, environmentId: 'env', threadId, ready: true, config: { threadFind }, projection: { visibleTurnItems: [] } }) as unknown as T3Client;
  it('waits for its thread, then opens find there at the message, and the transcript yields for a moment', () => {
    const c = client('other');
    requestThreadFind(c, 'env:t1', { query: 'Café', start: { entryId: 'message-1', occurrence: 0 } }, 1_000);
    takeRequestedFind(c, 2_000);
    expect(findOpening(c, 2_000)).toBe(false); // another thread is open
    (c as unknown as { threadId: string }).threadId = 't1';
    expect(findOpening(c, 3_000)).toBe(true);
    takeRequestedFind(c, 3_000);
    const model = threadFindState(c).model;
    expect([model.open, model.query]).toEqual([true, 'Café']);
    expect(findOpening(c, 4_000)).toBe(true);
    expect(findOpening(c, 5_000)).toBe(false);
  });
  it('expires after 30 s, and a server without find keeps the remembered position', () => {
    const late = client('other');
    requestThreadFind(late, 'env:t1', { query: 'x' }, 1_000);
    takeRequestedFind(late, 31_001);
    (late as unknown as { threadId: string }).threadId = 't1';
    expect(findOpening(late, 31_002)).toBe(false);
    const old = client('t1', false);
    requestThreadFind(old, 'env:t1', { query: 'x' }, 1_000);
    expect(findOpening(old, 1_500)).toBe(false);
    takeRequestedFind(old, 1_500);
    expect(threadFindState(old).model.open).toBe(false);
  });
});
