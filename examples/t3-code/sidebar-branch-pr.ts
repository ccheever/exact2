// A sidebar row's pull request when the thread links none (T3 Code
// 1e2ecbd975, MIT; see LICENSE-T3): ThreadStatusIndicators.tsx useLinkedThreadPullRequest. With no current link in the
// thread's `pullRequests`, the row reads `(!supportsLinks ? linkedPullRequest : null) ?? branchPullRequest` (the pull
// request the server found for the thread's branch) through `linkedPullRequestDetailAtom`, the server's
// `pullRequests.summary` (client-runtime pullRequests.ts createLinkedPullRequestSummaryAtomFamily: stale after 60 s,
// refreshed every 60 s), and both sidebars draw it as prStatusIndicator does: the state's glyph and tone, the number,
// "PR #113 - Open: Document the release steps". The "Studio branch" row (feature/release-notes) carries
// `branchPullRequest` #113 and `pullRequests: []`, so the clone, which read `pullRequests` alone, drew nothing.
// Data sources have no clock: the summaries are asked inside each refresh (sidebar-commands.ts sidebarRefreshed, as the
// favicons are), each again once it is a minute old.
import type { T3Client } from './client';
import { num, obj, str, type Obj } from './domain';
import type { Native } from './protocol';
import { letGo } from './let-go';
import { parseChangeRequestUrl } from './palette-linkpr';
import { visiblePullRequests } from './shell-pr';
import { lifecycle, lifecycleIcon, statusTooltip } from './r5-panels-pr';
import { fleet, fleetThreads, parseFleetThreadId, type EnvironmentFleet } from './settings-b-fleet';

/** The pull request a row shows from its summary (pullRequestDetailToVcsStatus with the summary's provider). */
export type BranchPr = { number: number; url: string; title: string; state: string; isDraft: boolean; provider: string };
type Entry = { at: number; pr: BranchPr | null; reading: boolean };

const STALE_MS = 60_000;
const ASKS_PER_REFRESH = 32;
const summaries = new WeakMap<T3Client, Map<string, Entry>>();
const cache = (client: T3Client) => { let map = summaries.get(client); if (!map) { map = new Map(); summaries.set(client, map); } return map; };

/** The environment a row's thread lives on ('' for the focused one) and its capabilities. */
function home(client: T3Client, thread: Obj, source: EnvironmentFleet): { environmentId: string; config: Obj } {
  const parsed = parseFleetThreadId(str(thread.id));
  if (!parsed || parsed.environmentId === client.environmentId) return { environmentId: '', config: client.config };
  const entry = [...source.entries.values()].find(candidate => candidate.environmentId === parsed.environmentId);
  return { environmentId: parsed.environmentId, config: entry?.config ?? {} };
}

/** useLinkedThreadPullRequest's `reference`: null while the thread has a current link (its snapshot draws the row). */
export function fallbackReference(thread: Obj, config: Obj): Obj | null {
  if (visiblePullRequests(thread.pullRequests).length) return null;
  const supportsLinks = obj(obj(obj(config).environment).capabilities).threadPullRequests === true;
  const pick = (value: unknown) => (value && typeof value === 'object' ? obj(value) : null);
  const fallback = (!supportsLinks ? pick(thread.linkedPullRequest) : null) ?? pick(thread.branchPullRequest);
  if (!fallback || num(fallback.number) <= 0 || !str(fallback.repository) || !str(fallback.projectId)) return null;
  const host = parseChangeRequestUrl(str(fallback.url))?.host;
  return { projectId: str(fallback.projectId), ...(host ? { host } : {}), repository: str(fallback.repository), number: num(fallback.number) };
}

const keyOf = (client: T3Client, environmentId: string, reference: Obj) => JSON.stringify([environmentId || client.environmentId, reference]);

/** The summary a row's fallback reference has (null until it arrives, or when the server has none). */
export function branchPullRequest(client: T3Client, thread: Obj, source: EnvironmentFleet = fleet): BranchPr | null {
  const { environmentId, config } = home(client, thread, source);
  const reference = fallbackReference(thread, config);
  return reference ? cache(client).get(keyOf(client, environmentId, reference))?.pr ?? null : null;
}

function asPr(value: Obj): BranchPr | null {
  return num(value.number) > 0 ? { number: num(value.number), url: str(value.url), title: str(value.title), state: str(value.state, 'open'), isDraft: value.isDraft === true, provider: str(value.provider, 'github') } : null;
}

/**
 * Inside each refresh: ask the server (the thread's own environment, through the pull request router) for each live
 * thread's fallback summary that is missing or a minute old, the newest threads first. A failure is remembered as none
 * until it is a minute old; an answer let go mid-read settles nothing. Each read is its own call (`readSummary`): a
 * closure over a loop's `const` saw another iteration's value in the app's runtime (the live drive's
 * "Cannot set property 'reading' of undefined").
 */
export async function syncBranchPullRequests(client: T3Client, native: Native, now: number, source: EnvironmentFleet = fleet): Promise<void> {
  if (!native?.available || client.connection !== 'connected') return;
  const map = cache(client), due = new Map<string, { environmentId: string; reference: Obj }>();
  const threads = [...client.shell.threads, ...fleetThreads(source)].filter(thread => !thread.archivedAt)
    .sort((a, b) => str(b.updatedAt).localeCompare(str(a.updatedAt)));
  for (const thread of threads) {
    if (due.size >= ASKS_PER_REFRESH) break;
    const { environmentId, config } = home(client, thread, source);
    const reference = fallbackReference(thread, config);
    if (!reference) continue;
    const key = keyOf(client, environmentId, reference), cached = map.get(key);
    if (cached && (cached.reading || now - cached.at < STALE_MS)) continue;
    due.set(key, { environmentId, reference });
  }
  await Promise.all([...due].map(([key, { environmentId, reference }]) => readSummary(client, native, map, key, environmentId, reference, now)));
}
async function readSummary(client: T3Client, native: Native, map: Map<string, Entry>, key: string, environmentId: string, reference: Obj, now: number): Promise<void> {
  const cached = map.get(key), entry: Entry = { at: now, pr: cached?.pr ?? null, reading: true };
  map.set(key, entry);
  try { entry.pr = asPr(obj(await client.rpc(native, 'pullRequests.summary', { ...(environmentId ? { environmentId } : {}), ...reference }))); }
  catch (error) {
    if (letGo(error)) { if (map.get(key) === entry) { if (cached) map.set(key, cached); else map.delete(key); } throw error; }
    entry.pr = null;
  }
  entry.reading = false;
  if (map.size > 128) map.delete(map.keys().next().value!);
}

/** sidebarPrBadge's fields for the fallback: the state's glyph and tone, the number. */
export function branchPrBadge(pr: BranchPr): { badge: string; stacked: boolean; badgeIcon: string; badgeState: string } {
  const state = lifecycle(pr.state, pr.isDraft);
  return { badge: String(pr.number), stacked: false, badgeIcon: lifecycleIcon(state), badgeState: state };
}
/** prStatusIndicator(pr, provider).tooltip: "PR #113 - Open: Document the release steps". */
export const branchPrTooltip = (pr: BranchPr) => statusTooltip(pr);
