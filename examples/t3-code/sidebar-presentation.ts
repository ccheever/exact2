import { arr, obj, str, type Obj } from './domain';
import type { T3Client } from './client';
import type { Native } from './protocol';
import { RUNTIME_LOCALE, timestampFormatter } from './timestamp-format'; // desktop-shell-details: the host's locale

export interface SearchPart { id: string; text: string; hit: boolean }

/**
 * ThreadSearchMatchExcerpt: a result with a server message match (title
 * matches too) shows the speaker and the server's snippet, whitespace
 * collapsed, every case-folded occurrence of the query in semibold ink; the
 * row's single line truncates it as the reference's `truncate` does.
 */
export function searchMatch(_thread: Obj, query: string, server?: Obj): { matchLabel: string; matchUser: boolean; matchParts: SearchPart[] } {
  const none = { matchLabel: '', matchUser: false, matchParts: [] as SearchPart[] };
  const needle = query.trim();
  if (!needle || !server) return none;
  const fold = (value: string) => value.replace(/[A-Z]/g, character => character.toLowerCase());
  const text = str(server.snippet).replace(/\s+/g, ' ').trim();
  if (!text) return none;
  const folded = fold(text), parts: SearchPart[] = [];
  let cursor = 0;
  while (cursor < text.length) {
    const at = folded.indexOf(fold(needle), cursor);
    if (at < 0) { parts.push({ id: String(cursor), text: text.slice(cursor), hit: false }); break; }
    if (at > cursor) parts.push({ id: String(cursor), text: text.slice(cursor, at), hit: false });
    parts.push({ id: String(at), text: text.slice(at, at + needle.length), hit: true });
    cursor = at + needle.length;
  }
  const user = server.source === 'user';
  return { matchLabel: user ? 'You:' : 'Agent:', matchUser: user, matchParts: parts };
}

const searches = new WeakMap<T3Client, { query: string; matches: Obj[] }>();
const pendingSearches = new WeakMap<T3Client, string>();

/** useThreadSearch isPending: a two-character query whose server search has not answered. */
export function threadSearchPending(client: T3Client): boolean {
  const needle = client.query.trim();
  return needle.length >= 2 && pendingSearches.get(client) === needle;
}

/**
 * The sidebar's message search (orchestration.searchThreads: two to 200
 * characters, at most 50 matches). A response for a query the user has since
 * changed is dropped; a failed search leaves title matching in place.
 */
export async function runThreadSearch(client: T3Client, native: Native, query: string): Promise<void> {
  const needle = query.trim();
  if (needle.length < 2 || needle.length > 200) { searches.delete(client); pendingSearches.delete(client); return; }
  pendingSearches.set(client, needle);
  let matches: Obj[] = [];
  try { matches = arr(obj(await client.restAccess(native).request('orchestration.searchThreads', { query: needle, limit: 50 })).matches); }
  catch { matches = []; }
  if (client.query.trim() === needle) searches.set(client, { query: needle, matches });
  if (pendingSearches.get(client) === needle) pendingSearches.delete(client);
}

/**
 * The search field never waits on the server: the list shows "Searching
 * thread messages…" until the answer lands, then the module is asked to
 * repaint (sidebarNotify), as a streamed event would.
 */
export function startThreadSearch(client: T3Client, native: Native, query: string): void {
  void runThreadSearch(client, native, query).then(async () => {
    if (query.trim().length < 2) return;
    try { await native.later({ op: 'sidebarNotify' }); } catch { /* an older module repaints on the next event */ }
  });
}

/** The server's message matches for the current query, by thread. */
export function serverMatches(client: T3Client): Map<string, Obj> {
  const search = searches.get(client);
  const result = new Map<string, Obj>();
  if (!search || search.query !== client.query.trim()) return result;
  for (const match of search.matches) if (!result.has(str(match.threadId))) result.set(str(match.threadId), match);
  return result;
}

export interface SnoozePreset { id: string; label: string; wakeLabel: string; until: string }

/** formatShortTimestamp in the host's locale (timestamp-format.ts). */
function clockLabel(date: Date, format: string): string {
  return timestampFormatter(format).format(date);
}
function atHour(date: Date, hour: number, days = 0): Date {
  const result = new Date(date);
  result.setDate(result.getDate() + days);
  result.setHours(hour, 0, 0, 0);
  return result;
}

/**
 * client-runtime resolveSnoozePresets: an hour, three hours, this evening at
 * 18:00 while more than an hour remains, tomorrow at 9:00, and next Monday at
 * 9:00 unless that is tomorrow. Labels use the selected timestamp format.
 */
export function snoozePresets(now: number, format: string): SnoozePreset[] {
  if (!Number.isFinite(now) || now <= 0) return [];
  const base = new Date(now), hour = 3_600_000;
  const preset = (id: string, label: string, date: Date, wakeLabel = clockLabel(date, format)) =>
    ({ id, label, wakeLabel, until: date.toISOString() });
  const presets = [preset('hour', 'In 1 hour', new Date(now + hour)), preset('three-hours', 'In 3 hours', new Date(now + 3 * hour))];
  const evening = atHour(base, 18);
  if (evening.getTime() - now > hour) presets.push(preset('evening', 'This evening', evening));
  const tomorrow = atHour(base, 9, 1);
  presets.push(preset('tomorrow', 'Tomorrow', tomorrow));
  const nextWeek = atHour(base, 9, (1 - base.getDay() + 7) % 7 || 7);
  if (nextWeek.getTime() !== tomorrow.getTime()) {
    presets.push(preset('next-week', 'Next week', nextWeek,
      `${nextWeek.toLocaleDateString(RUNTIME_LOCALE, { weekday: 'short' })} ${clockLabel(nextWeek, format)}`));
  }
  return presets;
}
