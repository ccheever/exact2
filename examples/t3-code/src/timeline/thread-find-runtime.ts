import type { T3Client } from '../client/client';
import { arr, obj, str, num, type Obj } from '../client/domain';
import type { Native } from '../client/protocol';
import { letGo } from '../client/let-go';
import { pushToast } from '../shell/toast';
import { ThreadFind, findCount, type FindResult, type FindMatch, type FindPosition } from './thread-find';
import { markdownSkills } from './message-chips';
import { findSearchTokenRanges, messagePhrase, messageSearchTokens } from '../palette/palette-search';

type State = { model: ThreadFind; key: string; serial: number; now: number; loading: string; retired: string[]; viewport: string };
const states = new WeakMap<T3Client, State>();
let streamSerial = 0;
const owner = (client: T3Client) => `${client.generation}|${client.environmentId}|${client.threadId}`;
const entryId = (row: Obj) => str(obj(row.item).messageId) || str(obj(row.item).id) || str(row.sourceItemId);
export function threadFindState(client: T3Client): State {
  let state = states.get(client);
  if (!state || state.model.owner !== owner(client)) {
    state = { model: new ThreadFind(owner(client)), key: '', serial: 0, now: 0, loading: '', viewport: '', retired: [...(state?.retired ?? []), ...(state?.key ? [state.key] : [])] };
    states.set(client, state);
    const pending = pendingFinds.get(client), shown = threadOf(client);
    if (pending && pending.from !== shown) pendingFinds.delete(client);
    if (pending?.thread === shown) state.model.openOn(pending.find.query, pending.find.start);
  }
  return state;
}

/** PendingThreadFind: the query find opens on, and where it starts, such as the message a search result came from. */
export type PendingFind = { query: string; start?: FindPosition };
// A find opened for a thread that is not shown yet (`from` is the one shown then): that thread's find takes it once it
// shows. Find belongs to the thread it opened in (retainThreadFindForRoute), so a third thread showing first drops it.
const pendingFinds = new WeakMap<T3Client, { thread: string; from: string; find: PendingFind }>();
const threadOf = (client: T3Client, threadId = client.threadId) => `${client.environmentId}|${threadId}`;

/** openThreadFindOnQuery: find opens on the query in that thread, now or once it shows, where the server can search it. */
export function openThreadFindOnQuery(client: T3Client, threadId: string, find: PendingFind): void {
  if (!threadId || client.config.threadFind !== true || !find.query.trim()) return;
  if (threadId !== client.threadId) { pendingFinds.set(client, { thread: threadOf(client, threadId), from: threadOf(client), find }); return; }
  pendingFinds.delete(client);
  threadFindState(client).model.openOn(find.query, find.start);
}

/**
 * findForMessageMatch: the find a message search result opens, at the matched message. The whole query when the
 * snippet holds it as typed ("vp run dev"), else its longest word, as the most telling ("sidebar" in "fix sidebar"),
 * written as the snippet writes it ("Café" for "cafe", since find matches accents).
 */
export function findForMessageMatch(match: { snippet: string; messageId?: string }, query: string): PendingFind {
  const start = match.messageId ? { start: { entryId: match.messageId, occurrence: 0 } } : {};
  // Apostrophes are straight in the query; straightening keeps offsets.
  const together = messagePhrase(query)?.exec(match.snippet.replace(/[\u2018\u2019]/g, "'"));
  if (together) return { query: match.snippet.slice(together.index, together.index + together[0].length), ...start };
  const word = messageSearchTokens(query).long.reduce((longest, token) => token.length > longest.length ? token : longest, '') || query;
  const ranges = findSearchTokenRanges(match.snippet, word);
  // A whole word first, as search ranked it: "Café", not "Cafe" in "Cafeteria".
  const shown = ranges.find(range => !/[\p{L}\p{N}]/u.test(match.snippet[range.end] ?? '')) ?? ranges[0];
  return { query: shown ? match.snippet.slice(shown.start, shown.end) : word, ...start };
}
export const threadFindExpanded = (client: T3Client): boolean => { const m = threadFindState(client).model; return m.open && !!m.query.trim(); };
export function threadFindAction(client: T3Client, op: string, value: string): string {
  const state = threadFindState(client), model = state.model;
  if (op === 'open') {
    if (!client.threadId || client.config.threadFind !== true) {
      pushToast(client, { kind: 'warning', title: 'Thread search is unavailable on this server.', description: 'Update the server to enable it.' }); return '';
    }
    let start;
    try {
      const key = JSON.parse(state.viewport) as unknown;
      if (Array.isArray(key) && typeof key[1] === 'string') {
        const row = arr(client.projection.visibleTurnItems).find(row => row.sourceThreadId === key[0] && row.sourceItemId === key[1]);
        start = { entryId: row ? entryId(row) : key[1], occurrence: 0 };
      }
    } catch { /* A header or an empty viewport has no entry. */ }
    model.show(start);
  } else if (op === 'close') { model.close(); }
  else if (op === 'query') model.edit(value, state.now);
  else if (op === 'next' || op === 'previous') model.move(op === 'next' ? 1 : -1);
  else if (op === 'retry') model.schedule(0);
  return '';
}
function resultOf(value: Obj): FindResult | null {
  if (!Number.isSafeInteger(value.totalMatches) || num(value.totalMatches) < 0 || !Number.isSafeInteger(value.activeIndex)) return null;
  const candidate = obj(value.match);
  const match: FindMatch | null = str(candidate.entryId) && Number.isSafeInteger(candidate.occurrence) && num(candidate.occurrence) >= 0 ? { entryId: str(candidate.entryId), runId: str(candidate.runId) || null, occurrence: num(candidate.occurrence) } : null;
  return { complete: value.complete !== false, snapshotSequence: num(value.snapshotSequence), totalMatches: num(value.totalMatches), activeIndex: num(value.activeIndex), match,
    navigation: arr(value.navigation).slice(0, 17).filter(entry => str(entry.entryId) && num(entry.count) > 0).map(entry => ({ entryId: str(entry.entryId), runId: str(entry.runId) || null, startIndex: num(entry.startIndex), count: num(entry.count) })) };
}
/** The transport passes progressive updates before ordinary long-lived stream handling. */
export function threadFindEvent(client: T3Client, entry: Obj): boolean {
  if (!str(entry.key).startsWith('thread-find:')) return false;
  const state = threadFindState(client), model = state.model;
  if (entry.key !== state.key || num(entry.generation, -1) !== client.generation) return true;
  const value = obj(entry.value);
  if (value._transportError) model.fail(state.serial, str(obj(value._transportError).message, 'Search failed'));
  else if (value._streamEnded) { if (model.pending) model.fail(state.serial, 'Search ended before completing.'); }
  else { const result = resultOf(value); if (result) model.accept(state.serial, result); }
  return true;
}
export function threadFindView(client: T3Client) {
  const state = threadFindState(client), model = state.model, match = model.result?.match;
  const row = match ? arr(client.projection.visibleTurnItems).find(row => entryId(row) === match.entryId || str(row.sourceItemId) === match.entryId) : undefined;
  return { open: model.open, query: model.query, focus: model.focus, count: model.error ? 'Search failed' : model.pending && !model.result ? 'Searching…' : findCount(model.result),
    error: model.error, pending: model.pending, complete: model.result?.complete !== false, empty: model.result?.totalMatches === 0, navigation: model.navigation,
    entry: row ? JSON.stringify([row.sourceThreadId, row.sourceItemId]) : '', occurrence: match?.occurrence ?? -1, highlight: model.result?.match ? model.query.trim() : '', attention: !!state.key && !model.open || state.retired.length > 0 || model.due > 0 };
}
/** Debouncing uses the Contract clock, and every answer remains scoped to its original connection and thread. */
export async function threadFindTick(client: T3Client, native: Native, now: number, skills: { name: string; displayName?: string }[] = [], viewport = '') {
  const state = threadFindState(client), model = state.model; state.now = now; state.viewport = viewport;
  const current = () => threadFindState(client) === state;
  if (state.key && (!model.open || model.due > 0 && !model.pending)) { state.retired.push(state.key); state.key = ''; }
  while (state.retired.length) {
    const key = state.retired[0]!;
    try { await client.restAccess(native).call({ op: 'unsubscribe', key }); }
    catch (error) { if (letGo(error)) throw error; }
    state.retired.shift();
    if (!current()) {
      const next = threadFindState(client);
      next.retired = [...new Set([...next.retired, ...state.retired])];
      state.retired.length = 0;
      return threadFindView(client);
    }
  }
  if (!current()) return threadFindView(client);
  model.changed(`${client.thread?.sequence ?? 0}`, now);
  const names = markdownSkills(skills);
  const request = model.request(client.threadId, names, now);
  if (request) {
    state.serial = request.serial;
    try {
      if (client.config.threadFindProgressive === true) {
        state.key = `thread-find:${++streamSerial}`;
        await client.restAccess(native).call({ op: 'subscribe', key: state.key, method: 'orchestration.searchThreadStream', payload: { ...request.payload } });
      } else {
        const result = resultOf(await client.rpc(native, 'orchestration.searchThread', { ...request.payload }));
        if (current() && result) model.accept(request.serial, result);
        else if (current()) model.fail(request.serial, 'The server returned an invalid search result.');
      }
    } catch (error) { if (letGo(error)) { if (current()) { model.pending = false; model.schedule(0); } throw error; } if (current()) model.fail(request.serial, error instanceof Error ? error.message : 'Search failed'); }
  }
  const match = model.result?.match;
  if (current() && match && !threadFindView(client).entry && client.thread?.hasMore && state.loading !== match.entryId) {
    state.loading = match.entryId;
    try { await client.history(native, match.entryId); }
    catch (error) { if (letGo(error)) { state.loading = ''; throw error; } model.fail(model.serial, error instanceof Error ? error.message : 'Could not load the matching message.'); }
    finally { if (current()) state.loading = ''; }
  }
  return threadFindView(client);
}
