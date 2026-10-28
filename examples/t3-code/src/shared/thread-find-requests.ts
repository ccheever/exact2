// Adapted from T3 Code 7c3b3b3605ef (MIT); see LICENSE-T3: apps/web/src/components/chat/threadFindStore.ts
// (openThreadFindOnQuery, findForMessageMatch) and useThreadFind.ts's openOnQuery.
//
// A ⌘K message result asks for find on its query at its message. The request waits until that thread is open, for 30 s
// at most; then find opens there (through thread-find-runtime.ts's model) and the transcript shows the match instead of
// its remembered position (app-main.contract transcriptOpened reads `findOpening`).
import type { T3Client } from '../client/client';
import { findSearchTokenRanges, messagePhrase, messageSearchTokens } from '../palette/palette-search';
import { threadFindState } from '../timeline/thread-find-runtime';

/** PendingThreadFind: the query, and where find starts, such as the message a result came from. */
export type ThreadFindRequest = { query: string; start?: { entryId: string; occurrence: number } };
type Pending = ThreadFindRequest & { threadKey: string; at: number; takenAt: number };
const PENDING_MS = 30_000;
/** How long after find opened the transcript still yields to it: the thread's first layout can come a little later. */
const OPENING_MS = 2_000;
const requests = new WeakMap<T3Client, Pending>();

/**
 * findForMessageMatch: the whole query when the snippet holds it as typed ("vp run dev"), else its longest word, as the
 * most telling ("sidebar" in "fix sidebar"), written as the snippet writes it ("Café" for "cafe", since find matches
 * accents); at the matched message when the server named it.
 */
export function findForMessageMatch(match: { snippet: string; messageId?: string }, query: string): ThreadFindRequest {
  const start = match.messageId ? { start: { entryId: match.messageId, occurrence: 0 } } : {};
  // Apostrophes are straight in the query; straightening keeps offsets.
  const together = messagePhrase(query)?.exec(match.snippet.replace(/[‘’]/g, "'"));
  if (together) return { query: match.snippet.slice(together.index, together.index + together[0].length), ...start };
  const word = messageSearchTokens(query).long.reduce((longest, token) => (token.length > longest.length ? token : longest), '') || query;
  const ranges = findSearchTokenRanges(match.snippet, word);
  // A whole word first, as search ranked it: "Café", not "Cafe" in "Cafeteria".
  const shown = ranges.find(range => !/[\p{L}\p{N}]/u.test(match.snippet[range.end] ?? '')) ?? ranges[0];
  return { query: shown ? match.snippet.slice(shown.start, shown.end) : word, ...start };
}

/** openThreadFindOnQuery: find opens on `find` in the thread keyed `<environment>:<thread>` once it is open. */
export function requestThreadFind(client: T3Client, threadKey: string, find: ThreadFindRequest, at: number): void {
  requests.set(client, { ...find, threadKey, at, takenAt: 0 });
}

const openKey = (client: T3Client) => (client.environmentId && client.threadId ? `${client.environmentId}:${client.threadId}` : '');

/**
 * Each shell answer: the open thread's waiting request opens find, where the server can search the thread. The shell's
 * clock runs while a request waits or just opened (shell.ts `ticking`), so both windows end on time.
 */
export function takeRequestedFind(client: T3Client, now: number): void {
  const request = requests.get(client);
  if (!request) return;
  if (request.takenAt ? now - request.takenAt >= OPENING_MS : now - request.at > PENDING_MS) { requests.delete(client); return; }
  if (request.takenAt || request.threadKey !== openKey(client) || !client.ready) return;
  // A server that cannot search the thread drops the request, and the transcript keeps its remembered position.
  if (client.config.threadFind !== true) { requests.delete(client); return; }
  // openThreadFindOnQuery: find opens on the query and searches, at the matched message when the server named it (one
  // from before upstream #24343 names none), else as a typed query does.
  const model = threadFindState(client).model;
  model.show();
  model.edit(request.query, now);
  if (request.start) model.show(request.start);
  request.takenAt = now;
}

/** Whether the open thread's transcript yields its remembered position to find: a request waits for it, or just opened. */
export function findOpening(client: T3Client, now: number): boolean {
  const request = requests.get(client);
  if (!request || request.threadKey !== openKey(client) || client.config.threadFind !== true) return false;
  return request.takenAt ? now - request.takenAt < OPENING_MS : now - request.at <= PENDING_MS;
}
