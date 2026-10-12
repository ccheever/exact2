// The palette resource: which page the overlay shows for its mode, page and query.
import type { T3Client } from './client';
import { arr, str, type Obj } from './domain';
import type { Native } from './protocol';
import { closedView, commandView, threadJumpChords, type PaletteView } from './palette';
import { addProjectView, isBrowseQuery } from './palette-add';
import { contentSearchView, filePickerView } from './palette-files';
import { linkPullRequestView, resetLinkDialog } from './palette-linkpr';
import { linkedThreadItems, threadPickerView } from './pages-pr-links'; // pr-links-previews-and-routing

const searches = new WeakMap<T3Client, { key: string; matches: Obj[] }>();
/** useThreadSearch: orchestration.searchThreads for 2–200 characters, newest query only. */
async function threadMatches(client: T3Client, native: Native | null | undefined, query: string): Promise<{ matches: Map<string, Obj>; query: string }> {
  const needle = query.trim();
  const result = new Map<string, Obj>();
  if (!native?.available || !client.ready || needle.length < 2 || needle.length > 200 || needle.startsWith('>')) return { matches: result, query: needle };
  const key = `${client.generation}:${needle}`;
  let hit = searches.get(client);
  if (!hit || hit.key !== key) {
    let matches: Obj[] = [];
    try { matches = arr((await client.restAccess(native).request('orchestration.searchThreads', { query: needle, limit: 50 })).matches); }
    catch { matches = []; }
    hit = { key, matches };
    searches.set(client, hit);
  }
  for (const match of hit.matches) if (!result.has(str(match.threadId))) result.set(str(match.threadId), match);
  return { matches: result, query: needle };
}

export function isAddProjectPage(page: string): boolean {
  return page === 'add-project' || page.startsWith('add-project/') || page === 'new-project';
}

/** The list's full height (rows plus the list's 8pt end padding), for the scroll fades. */
export async function paletteView(client: T3Client, native: Native | null | undefined, args: unknown[]): Promise<PaletteView> {
  const view = await paletteViewOf(client, native, args);
  return { ...view, listHeight: view.rows.length ? Math.max(...view.rows.map(entry => entry.top + entry.height)) + 8 : 0 };
}
async function paletteViewOf(client: T3Client, native: Native | null | undefined, args: unknown[]): Promise<PaletteView> {
  const open = args[0] === true, mode = String(args[1] || 'command'), page = String(args[2] || ''), query = String(args[3] ?? '');
  const flags = String(args[4] || ''), highlighted = args[5] === true, now = Number(args[6]) || 0, scheme = args[7] === 'dark' ? 'dark' : 'light';
  if (page === 'link-pr' && open) return linkPullRequestView(client, query);
  resetLinkDialog(client);
  if (!open) return closedView;
  if (mode === 'files') return filePickerView(client, native, query);
  if (mode === 'content') return contentSearchView(client, native, query, flags);
  // The command palette's own field (CommandPalette.tsx handleKeyDown) takes the thread.jump.N chords; the file
  // picker, content search, Link pull request and the thread picker are other dialogs.
  const jumpKeys = threadJumpChords(client);
  if (isAddProjectPage(page) || (page === '' && isBrowseQuery(query))) return { ...await addProjectView(client, native, { page, query, highlighted }), jumpKeys };
  // pr-links-previews-and-routing: the panel's thread picker, and the linked threads' search (the root page, opened on the pull request's URL).
  if (page === 'pr-link-thread') return threadPickerView(client, query, false);
  const linkedThreads = linkedThreadItems(client, page, query), root = page.startsWith('pr-linked|') ? '' : page;
  const search = root === '' && !linkedThreads ? await threadMatches(client, native, query) : { matches: new Map<string, Obj>(), query: '' };
  return { ...commandView(client, { page: root, query, now, scheme, matches: search.matches, matchQuery: search.query, searching: false, linkedThreads }), page, jumpKeys };
}
