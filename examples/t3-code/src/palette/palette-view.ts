// The palette resource: which page the overlay shows for its mode, page and query.
import type { T3Client } from '../client/client';
import { arr, obj, str, type Obj } from '../client/domain';
import type { Native } from '../client/protocol';
import { closedView, commandView, rootFallback, threadJumpChords, type PaletteView } from './palette';
import { addProjectView, isBrowseQuery } from './palette-add';
import { forgetNewProjectEnvironment } from './palette-environments';
import { contentSearchView, filePickerView } from './palette-files';
import { linkPullRequestView, resetLinkDialog } from './palette-linkpr';
import { linkedThreadItems, threadPickerView } from '../pull-requests/pages-pr-links';
import { messageSearchTokens } from './palette-search';
import { fleet, fleetThreadId, type EnvironmentFleet } from '../connections/settings-b-fleet';
import { environmentRequest, environmentTarget } from '../pull-requests/pages-pr-environments';

/** THREAD_SEARCH_QUERY_MAX_LENGTH: a pasted wall of text searches by its start rather than failing the request. */
const THREAD_SEARCH_QUERY_MAX_LENGTH = 200;
/** How long a background environment's search may hold the palette, which draws once every environment answered. */
const BACKGROUND_SEARCH_SECONDS = 10;
const searches = new WeakMap<T3Client, { key: string; matches: Obj[] }>();
/**
 * useConnectedThreadSearch: orchestration.searchThreads on every connected environment (the focused one and each
 * background one the fleet keeps synchronized, on its own transport), newest query only, once the query holds a word
 * the server can match messages by (three or more letters, or two Chinese, Japanese or Korean characters). One match
 * per thread, keyed by its row's id (a background thread's `fleet:<environment>:<id>`), each environment's in its
 * server's order. A failed search counts as no matches.
 */
async function threadMatches(client: T3Client, native: Native | null | undefined, query: string, source: EnvironmentFleet = fleet): Promise<{ matches: Map<string, Obj>; query: string }> {
  const needle = query.trim().slice(0, THREAD_SEARCH_QUERY_MAX_LENGTH).trim();
  const result = new Map<string, Obj>();
  if (!native?.available || needle.startsWith('>') || messageSearchTokens(needle).long.length === 0) return { matches: result, query: needle };
  const environments = [...(client.ready ? [{ environmentId: client.environmentId, generation: client.generation }] : []),
    ...[...source.entries.values()].filter(entry => environmentTarget(client, entry.environmentId, source)?.key === entry.key)];
  const key = `${needle}\n${environments.map(environment => `${environment.environmentId}:${environment.generation}`).join(' ')}`;
  let hit = searches.get(client);
  if (!hit || hit.key !== key) {
    const answers = await Promise.all(environments.map(async ({ environmentId }) => {
      try {
        const reply = environmentId === client.environmentId ? await client.restAccess(native).request('orchestration.searchThreads', { query: needle })
          : await environmentRequest(client, native, environmentId, 'orchestration.searchThreads', { query: needle }, { timeoutSeconds: BACKGROUND_SEARCH_SECONDS }, source);
        return arr(obj(reply).matches).map(match => ({ ...match, environmentId }));
      } catch { return []; }
    }));
    hit = { key, matches: answers.flat() };
    searches.set(client, hit);
  }
  for (const match of hit.matches) {
    const environmentId = str(match.environmentId), threadId = str(match.threadId);
    const id = environmentId === client.environmentId ? threadId : fleetThreadId(environmentId, threadId);
    if (!result.has(id)) result.set(id, match);
  }
  return { matches: result, query: needle };
}
// The message rows the root page shows, by row id, and the query they were found for: picking one opens find at its
// message (pickThread), as a message item's run calls openThreadFindOnQuery.
const shownMessages = new WeakMap<T3Client, { query: string; matches: Map<string, Obj> }>();
/** The match of message row `id` when the root page shows it for `query` (the pick's); null for any other row. */
export function shownMessageMatch(client: T3Client, id: string, query: string): { match: Obj; query: string } | null {
  const shown = shownMessages.get(client), match = shown?.matches.get(id);
  return shown && match && shown.query === query.trim().slice(0, THREAD_SEARCH_QUERY_MAX_LENGTH).trim() ? { match, query: shown.query } : null;
}

/** Add project's pages, New project from the root, and an environment's Sources opened from it (`env:<id>`). */
export function isAddProjectPage(page: string): boolean {
  return page === 'add-project' || page.startsWith('add-project/') || page === 'new-project' || page.startsWith('env:');
}

/** The list's full height (rows plus the list's 8pt end padding), for the scroll fades. */
export async function paletteView(client: T3Client, native: Native | null | undefined, args: unknown[]): Promise<PaletteView> {
  const view = await paletteViewOf(client, native, args);
  return { ...view, listHeight: view.rows.length ? Math.max(...view.rows.map(entry => entry.top + entry.height)) + 8 : 0 };
}
async function paletteViewOf(client: T3Client, native: Native | null | undefined, args: unknown[]): Promise<PaletteView> {
  const open = args[0] === true, mode = String(args[1] || 'command'), page = String(args[2] || ''), query = String(args[3] ?? '');
  const flags = String(args[4] || ''), highlighted = args[5] === true, now = Number(args[6]) || 0, scheme = args[7] === 'dark' ? 'dark' : 'light';
  forgetNewProjectEnvironment(client, open ? page : '');
  if (page === 'link-pr' && open) return linkPullRequestView(client, query);
  resetLinkDialog(client);
  if (!open) return closedView;
  if (mode === 'files') return filePickerView(client, native, query);
  if (mode === 'content') return contentSearchView(client, native, query, flags);
  // The command palette's own field (CommandPalette.tsx handleKeyDown) takes the thread.jump.N chords; the file
  // picker, content search, Link pull request and the thread picker are other dialogs.
  const jumpKeys = threadJumpChords(client);
  // A list page (Environments, Sources) where nothing matches the query searches the root (filterCommandPaletteGroups allOptions).
  if (isAddProjectPage(page) || (page === '' && isBrowseQuery(query))) {
    const fallback = page !== '' && !isBrowseQuery(query) ? rootFallback(client, query, now, scheme) : undefined;
    return { ...await addProjectView(client, native, { page, query, highlighted, ...(fallback ? { fallback } : {}) }), jumpKeys };
  }
  // The panel's thread picker, and the linked threads' search (the root page, opened on the pull request's URL).
  if (page === 'pr-link-thread') return threadPickerView(client, query, false);
  const linkedThreads = linkedThreadItems(client, page, query), root = page.startsWith('pr-linked|') ? '' : page;
  const search = root === '' && !linkedThreads ? await threadMatches(client, native, query) : { matches: new Map<string, Obj>(), query: '' };
  const view = commandView(client, { page: root, query, now, scheme, matches: search.matches, matchQuery: search.query, searching: false, linkedThreads });
  const messages = view.rows.filter(entry => entry.key.startsWith('threads-messages:')).flatMap(entry => { const match = search.matches.get(entry.arg); return match ? [[entry.arg, match] as const] : []; });
  shownMessages.set(client, { query: search.query, matches: new Map(messages) });
  return { ...view, page, jumpKeys };
}
