// The sidebar's thread search (T3 Code 7c3b3b3605ef, MIT, see LICENSE-T3): Sidebar.tsx, Sidebar.logic.ts
// searchSidebarThreads, state/queries.ts useThreadSearch and chat/threadFindStore.ts findForMessageMatch. Title, branch
// and pull request matches rank as ⌘K ranks them, with picks off (a pick would move rows unexplained); after them come
// the threads listed only for their messages, every connected environment's matches ranked together, rows already shown
// keeping their places while slower environments answer. Only those rows show their message, and opening one opens
// find on it.
import type { T3Client } from '../client/client';
import { arr, obj, str, type Obj } from '../client/domain';
import type { Native } from '../client/protocol';
import { fleet, parseFleetThreadId, type EnvironmentFleet } from '../connections/settings-b-fleet';
import {
  findSearchTokenRanges, foldSearchQuery, messagePhrase, messageSearchTokens, nextShownOrder, rankMessageMatches, rankSearchItems,
  scoreThreadSearch, splitSearchText, threadSearchSubject, type ThreadSearchSubject,
} from '../palette/palette-search';
import { threadFindState } from '../timeline/thread-find-runtime';
import type { FindPosition } from '../timeline/thread-find';
import { envAccess } from './legacy-sidebar-fleet';
import type { SidebarSection } from './sidebar-model';
import { sidebarSession } from './sidebar-state';

/** THREAD_SEARCH_QUERY_MAX_LENGTH: a pasted wall of text searches by its start rather than failing the request. */
const QUERY_MAX_LENGTH = 200;
const searched = (query: string): string => query.trim().slice(0, QUERY_MAX_LENGTH).trim();
/** The server matches messages by a word of three or more letters (two Chinese, Japanese or Korean characters). */
const searchable = (query: string): boolean => messageSearchTokens(query).long.length > 0;

/** Message matches, each with its environment, and the query they were found for. */
interface Found { query: string; matches: Obj[] }
/** One query's search: each environment's answer by its place in the ask, null until it lands. */
interface Search { query: string; answers: (Obj[] | null)[] }
const searches = new WeakMap<T3Client, { current: Search | null; previous: Found | null }>();
function searchState(client: T3Client) {
  let state = searches.get(client);
  if (!state) { state = { current: null, previous: null }; searches.set(client, state); }
  return state;
}

/** useConnectedEnvironmentIds: the focused environment and every background one the fleet keeps synchronized. */
function searchedEnvironments(client: T3Client, native: Native, source: EnvironmentFleet) {
  const environments: { environmentId: string; request: (method: string, payload: Obj) => Promise<unknown> }[] = [];
  if (client.ready) environments.push({ environmentId: client.environmentId, request: (method, payload) => client.restAccess(native).request(method, payload) });
  for (const entry of source.entries.values()) {
    const access = envAccess(client, native, entry.environmentId, source);
    if (access) environments.push({ environmentId: access.environmentId, request: access.request });
  }
  return environments;
}

/**
 * useThreadSearch: the query's message matches from every connected environment (orchestration.searchThreads, each on
 * its own transport). An answer counts as it lands and `landed` repaints; an answer for a query since replaced is
 * dropped, and a failed search counts as no matches.
 */
export async function runThreadSearch(client: T3Client, native: Native, query: string, landed: () => Promise<void> | void = () => undefined, source: EnvironmentFleet = fleet): Promise<void> {
  const state = searchState(client), needle = searched(query);
  if (!searchable(needle)) { state.current = null; return; }
  const environments = searchedEnvironments(client, native, source);
  const search: Search = { query: needle, answers: environments.map(() => null) };
  state.current = search;
  await Promise.all(environments.map(async ({ environmentId, request }, index) => {
    let matches: Obj[] = [];
    try { matches = arr(obj(await request('orchestration.searchThreads', { query: needle })).matches); }
    catch { matches = []; }
    if (state.current !== search) return;
    search.answers[index] = matches.map(match => ({ ...match, environmentId }));
    await landed();
  }));
}

/** The search field never waits on the servers: the list repaints (sidebarNotify) as each answer lands. */
export function startThreadSearch(client: T3Client, native: Native, query: string): void {
  void runThreadSearch(client, native, query, async () => {
    try { await native.later({ op: 'sidebarNotify' }); } catch { /* an older module repaints on the next event */ }
  });
}

/**
 * What the list shows for the current query (useThreadSearch's `visible`): its own matches once any environment has
 * answered, else the previous query's while this one refines or widens it, so rows do not blink out and back on every
 * keystroke; an unrelated query's clear. `pending` while an environment has not answered.
 */
function visibleSearch(client: T3Client): { found: Found; pending: boolean } {
  const state = searchState(client), needle = searched(client.query);
  if (!searchable(needle)) return { found: { query: '', matches: [] }, pending: false };
  const current = state.current?.query === needle ? state.current : null;
  const matches = current ? current.answers.flatMap(answer => answer ?? []) : [];
  const answered = !!current && current.answers.every(answer => answer !== null);
  if (current && (answered || matches.length > 0)) {
    state.previous = { query: needle, matches };
    return { found: state.previous, pending: !answered };
  }
  const now = foldSearchQuery(needle), before = foldSearchQuery(state.previous?.query ?? '');
  const related = !!state.previous && !!now && !!before && (now.startsWith(before) || before.startsWith(now));
  return { found: related ? state.previous! : { query: '', matches: [] }, pending: true };
}
/** useThreadSearch isPending: "Searching thread messages…" while the list is empty. */
export const threadSearchPending = (client: T3Client): boolean => visibleSearch(client).pending;

export interface SearchRow { thread: Obj; section: SidebarSection; match?: Obj }
const SECTIONS: SidebarSection[] = ['pinned', 'active', 'working', 'snoozed', 'settled'];
/** A row's environment and its own thread id: a background environment's row carries `fleet:<environment>:<id>`. */
const rowKey = (client: T3Client, id: string): string => {
  const ref = parseFleetThreadId(id);
  return ref ? `${ref.environmentId}:${ref.threadId}` : `${client.environmentId}:${id}`;
};
/** threadSearchSubject(thread, null): no project title, so a project's name lists none of its threads. */
function subjectOf(client: T3Client, thread: Obj): ThreadSearchSubject {
  const ref = parseFleetThreadId(str(thread.id));
  return threadSearchSubject(ref ? { ...thread, id: ref.threadId } : thread, null, null, ref?.environmentId ?? client.environmentId);
}

/**
 * The search's rows over every shelf (searchSidebarThreads): title, branch and pull request matches ranked, then the
 * threads listed for their messages, in the order the servers ranked them and first showed them. Only those carry
 * their match. The painted rows and the query the matches were found for stay on the session for the keys and Enter.
 */
export function searchRows(client: T3Client, parts: Record<SidebarSection, Obj[]>, now: number): SearchRow[] {
  const session = sidebarSession(client), query = foldSearchQuery(client.query);
  if (!query) { session.searchShown = []; return []; }
  const threads: Obj[] = [], sections = new Map<Obj, SidebarSection>(), byKey = new Map<string, Obj>(), subjects = new Map<Obj, ThreadSearchSubject>();
  for (const section of SECTIONS) {
    for (const thread of parts[section]) {
      threads.push(thread); sections.set(thread, section); byKey.set(rowKey(client, str(thread.id)), thread); subjects.set(thread, subjectOf(client, thread));
    }
  }
  const titles = rankSearchItems(threads, (thread, options) => scoreThreadSearch(query, subjects.get(thread)!, now, { ...options, picks: false })).map(({ item }) => item);
  const { found } = visibleSearch(client);
  const keyOf = (match: Obj) => `${str(match.environmentId)}:${str(match.threadId)}`;
  const ranked = rankMessageMatches(found.matches, match => { const thread = byKey.get(keyOf(match)); return thread ? subjects.get(thread) : undefined; }, query, now, { picks: false });
  const matchByKey = new Map<string, Obj>();
  for (const match of ranked) if (!matchByKey.has(keyOf(match))) matchByKey.set(keyOf(match), match);
  session.messageOrder = nextShownOrder(session.messageOrder, found.query, [...matchByKey.keys()], Number.POSITIVE_INFINITY);
  session.searchFoundFor = found.query;
  const listed = new Set(titles);
  const rows: SearchRow[] = titles.map(thread => ({ thread, section: sections.get(thread)! }));
  for (const key of session.messageOrder.keys) {
    const thread = byKey.get(key);
    if (thread && !listed.has(thread)) rows.push({ thread, section: sections.get(thread)!, match: matchByKey.get(key) });
  }
  session.searchShown = rows.map(row => ({ id: str(row.thread.id), ...(row.match ? { match: row.match } : {}) }));
  return rows;
}

export interface SearchPart { id: string; text: string; hit: boolean }
/**
 * ThreadSearchMatchExcerpt: a row listed for its message shows the speaker and the server's snippet, whitespace
 * collapsed, with the words of the query it was found for in semibold ink; the row's single line truncates it.
 */
export function searchMatch(match: Obj | undefined, query: string): { matchLabel: string; matchUser: boolean; matchParts: SearchPart[] } {
  const text = str(match?.snippet).replace(/\s+/g, ' ').trim();
  if (!match || !text) return { matchLabel: '', matchUser: false, matchParts: [] };
  const parts = splitSearchText(text, findSearchTokenRanges(text, query)).map(part => ({ id: String(part.start), text: part.text, hit: part.highlighted }));
  const user = match.source === 'user';
  return { matchLabel: user ? 'You:' : 'Agent:', matchUser: user, matchParts: parts };
}

/**
 * findForMessageMatch: the find a message result opens, at the matched message: the whole query when the snippet holds
 * it as typed ("vp run dev"), else its longest word, as the most telling ("sidebar" in "fix sidebar"), written as the
 * snippet writes it ("Café" for "cafe", since find matches accents).
 */
export function findForMessageMatch(match: Obj, query: string): { query: string; start?: FindPosition } {
  const snippet = str(match.snippet), messageId = str(match.messageId);
  const start = messageId ? { start: { entryId: messageId, occurrence: 0 } } : {};
  // Apostrophes are straight in the query; straightening keeps offsets.
  const together = messagePhrase(query)?.exec(snippet.replace(/[‘’]/g, "'"));
  if (together) return { query: snippet.slice(together.index, together.index + together[0].length), ...start };
  const word = messageSearchTokens(query).long.reduce((longest, token) => (token.length > longest.length ? token : longest), '') || query;
  const ranges = findSearchTokenRanges(snippet, word);
  // A whole word first, as search ranked it: "Café", not "Cafe" in "Cafeteria".
  const shown = ranges.find(range => !/[\p{L}\p{N}]/u.test(snippet[range.end] ?? '')) ?? ranges[0];
  return { query: shown ? snippet.slice(shown.start, shown.end) : word, ...start };
}

// openThreadFindOnQuery: a picked message row's find waits for its thread, which another environment's row reaches
// through a reconnect. It opens once that thread is the open one; another thread shown first drops it.
const pendingFinds = new WeakMap<T3Client, { key: string; from: string; query: string; start?: FindPosition }>();
const shownThread = (client: T3Client): string => (client.threadId ? `${client.environmentId}:${client.threadId}` : '');
/** selectThreadSearchResult: the row the search opens, which finds its message when it was listed for one. */
export function pickSearchResult(client: T3Client, id: string): void {
  const session = sidebarSession(client), match = session.searchShown.find(row => row.id === id)?.match;
  pendingFinds.delete(client);
  if (match) pendingFinds.set(client, { key: rowKey(client, id), from: shownThread(client), ...findForMessageMatch(match, session.searchFoundFor) });
}
/** After a thread opens and after each refresh: a pending find opens on its thread once that thread shows. */
export function showPendingFind(client: T3Client): void {
  const pending = pendingFinds.get(client), shown = shownThread(client);
  if (!pending || !shown) return;
  if (shown !== pending.key) { if (shown !== pending.from) pendingFinds.delete(client); return; }
  pendingFinds.delete(client);
  if (client.config.threadFind !== true) return; // useThreadFindOpen: find shows only where the server can search
  const state = threadFindState(client);
  state.model.edit(pending.query, state.now);
  state.model.show(pending.start);
  state.model.schedule(0);
}
