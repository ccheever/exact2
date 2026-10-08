// The Pull Requests page (lane "pages"): pullRequests.list / listStats /
// detail / activity, grouped, sorted and presented as the reference page does.
// Sources: T3 Code (MIT, see LICENSE-T3) apps/web/src/routes/_chat.pull-requests.tsx
// and components/pullRequest/{pullRequestList.logic,pullRequestListPreferences,
// PullRequestListFilters,PullRequestListRow,pullRequestPresentation,
// pullRequestIcons,PullRequestDetailPanel,PullRequestListEmptyState}.tsx.
import { diffSchemeOf } from './r4-timeline-chips';
import { arr, num, obj, str, type Obj } from './domain';
import { ClientError, type Native } from './protocol';
import { pagesPrefs } from './pages-prefs';
import { projectIdentity } from './presentation';
import type { T3Client } from './client';
import { letGo } from './let-go';
import { holdPullRequestRefreshes, pullRequestRefreshEpoch } from './pages-pr-refresh';
import { listRelist, overrideListEntries, settleListOverrides } from './pages-pr-actions'; // pr-header-actions-and-stacks: the panel's onActed

export const SORTS = [
  { value: 'ready', label: 'Merge readiness' }, { value: 'blocked', label: 'Blocked on me' }, { value: 'updated', label: 'Recently updated' },
  { value: 'newest', label: 'Newest shown' }, { value: 'oldest', label: 'Oldest shown' }, { value: 'largest', label: 'Largest shown' }, { value: 'smallest', label: 'Smallest shown' },
];
export const STATES = [{ value: 'all', label: 'All' }, { value: 'open', label: 'Open' }, { value: 'closed', label: 'Closed' }, { value: 'merged', label: 'Merged' }];
export const INVOLVEMENTS = [{ value: 'all', label: 'All' }, { value: 'reviewing', label: 'Reviewing' }, { value: 'authored', label: 'Authored' }];
export const DRAFTS = [{ value: 'all', label: 'All' }, { value: 'only', label: 'Drafts only' }, { value: 'hide', label: 'Hide drafts' }];
export const REVIEWS = [{ value: 'all', label: 'All' }, { value: 'approved', label: 'Approved' }, { value: 'changes-requested', label: 'Changes requested' },
  { value: 'review-required', label: 'Review required' }, { value: 'none', label: 'No reviews' }];
export const CHECKS = [{ value: 'all', label: 'All' }, { value: 'passing', label: 'Passing' }, { value: 'failing', label: 'Failing' }];
export const STATE_ICONS: Record<string, string> = { all: 'layers', open: 'git-pull-request-arrow', closed: 'git-pull-request-closed', merged: 'git-merge' };
export const INVOLVEMENT_ICONS: Record<string, string> = { all: 'layers', reviewing: 'eye', authored: 'pen-line' };
export const DRAFT_ICONS: Record<string, string> = { all: 'layers', only: 'git-pull-request-draft', hide: 'eye-off' };
export const REVIEW_ICONS: Record<string, string> = { all: 'layers', approved: 'circle-check', 'changes-requested': 'circle-x', 'review-required': 'circle-dashed', none: 'circle-slash' };
export const CHECKS_ICONS: Record<string, string> = { all: 'layers', passing: 'circle-check', failing: 'circle-x' };
const GROUP_LABELS: Record<string, string> = { authored: 'Authored', reviewRequested: 'Review requested', others: 'Others' };

// ── Preferences (pullRequestListPreferences.ts) ─────────────────────────────

export type PrPrefs = { involvement: string; state: string; host: string; projectId: string; q: string; draft: string; review: string; checks: string; author: string; labels: string[]; sort: string };
export const defaultPrPrefs = (): PrPrefs => ({ involvement: 'all', state: 'open', host: '', projectId: '', q: '', draft: '', review: '', checks: '', author: '', labels: [], sort: 'ready' });
const bounded = (value: unknown) => str(value).trim().slice(0, 200);
export function decodePrPrefs(saved: unknown): PrPrefs {
  const value = obj(saved), next = defaultPrPrefs();
  if (INVOLVEMENTS.some(option => option.value === value.involvement)) next.involvement = str(value.involvement);
  if (STATES.some(option => option.value === value.state)) next.state = str(value.state);
  if (SORTS.some(option => option.value === value.sort)) next.sort = str(value.sort);
  if (value.draft === 'only' || value.draft === 'hide') next.draft = value.draft;
  if (REVIEWS.some(option => option.value === value.review && option.value !== 'all')) next.review = str(value.review);
  if (value.checks === 'passing' || value.checks === 'failing') next.checks = value.checks;
  next.host = bounded(value.host); next.projectId = bounded(value.projectId); next.q = bounded(value.q); next.author = bounded(value.author);
  next.labels = (Array.isArray(value.labels) ? value.labels : []).map(bounded).filter(Boolean).slice(0, 10);
  return next;
}
/** pullRequestListPreferences: the list controls, kept in the client's preference record. */
export function prPrefs(owner: { local: object }): PrPrefs {
  return decodePrPrefs(pagesPrefs(owner).pullRequests);
}
function savePrPrefs(owner: { local: object }, prefs: PrPrefs): void {
  pagesPrefs(owner).pullRequests = { ...prefs, labels: [...prefs.labels] } as unknown as Obj;
}

// ── Query parsing (parsePullRequestQuery) ───────────────────────────────────

const REVIEW_VALUES: Record<string, string> = { approved: 'approved', changes_requested: 'changes-requested', 'changes-requested': 'changes-requested', required: 'review-required', 'review-required': 'review-required', none: 'none' };
const CHECKS_VALUES: Record<string, string> = { success: 'passing', passing: 'passing', failure: 'failing', failing: 'failing' };
const unquote = (raw: string) => raw.split('"').join('').trim();
const splitList = (raw: string) => /^\s*"[^"]*"\s*$/.test(raw) ? [unquote(raw)].filter(Boolean) : raw.split(',').map(unquote).filter(Boolean);
const boundedNames = (names: string[]) => names.slice(0, 10).map(name => name.slice(0, 200).trim()).filter(Boolean);
export type PrFilters = { labels?: string[][]; excludedLabels?: string[]; author?: string; draft?: string; review?: string; checks?: string };
export function parsePrQuery(raw: string): { text: string; filters: PrFilters } {
  const text: string[] = [], labels: string[][] = [], excluded: string[] = [];
  const filters: PrFilters = {};
  for (const [token] of raw.matchAll(/(?:[^\s"]|"[^"]*")+/g)) {
    const qualifier = /^(-?)([A-Za-z][A-Za-z0-9_-]*):(.*)$/.exec(token);
    const value = qualifier ? unquote(qualifier[3] ?? '') : '';
    const negated = qualifier?.[1] === '-';
    const key = value.length === 0 ? '' : (qualifier?.[2] ?? '').toLowerCase();
    if (key === 'label') {
      const names = boundedNames(splitList(qualifier?.[3] ?? ''));
      if (names.length) { if (negated) excluded.push(...names); else labels.push(names); continue; }
    } else if (key === 'author' && !negated) { filters.author = value.slice(0, 200).trim(); continue; }
    else if (key === 'draft' && !negated && ['true', 'false'].includes(value.toLowerCase())) { filters.draft = value.toLowerCase() === 'true' ? 'only' : 'hide'; continue; }
    else if (key === 'review' && !negated && REVIEW_VALUES[value.toLowerCase()]) { filters.review = REVIEW_VALUES[value.toLowerCase()]; continue; }
    else if ((key === 'status' || key === 'checks') && !negated && CHECKS_VALUES[value.toLowerCase()]) { filters.checks = CHECKS_VALUES[value.toLowerCase()]; continue; }
    else if (key && !['label', 'author', 'draft', 'review', 'status', 'checks'].includes(key) && !value.startsWith('/')) {
      const names = boundedNames(splitList(qualifier?.[3] ?? '').map(name => name.includes(':') ? name : `${qualifier?.[2] ?? ''}:${name}`));
      if (names.length) { if (negated) excluded.push(...names); else labels.push(names); continue; }
    }
    text.push(token);
  }
  if (labels.length) filters.labels = labels.slice(0, 10);
  if (excluded.length) filters.excludedLabels = excluded.slice(0, 10);
  return { text: text.join(' '), filters };
}

// ── Grouping and ranking (pullRequestList.logic.ts) ─────────────────────────

const lower = (value: unknown) => str(value).trim().toLowerCase();
const viewerFor = (entry: Obj, viewers: Obj) => lower(viewers[` ${str(entry.host)}`] ?? viewers[str(entry.host)]);
const authoredByViewer = (entry: Obj, viewers: Obj) => { const viewer = viewerFor(entry, viewers); return !!viewer && lower(obj(entry.author).login) === viewer; };
export function groupEntries(entries: Obj[], viewers: Obj): { key: string; label: string; entries: Obj[] }[] {
  const buckets: Record<string, Obj[]> = { authored: [], reviewRequested: [], others: [] };
  for (const entry of entries) {
    if (authoredByViewer(entry, viewers)) buckets.authored!.push(entry);
    else if (entry.viewerReviewRequested === true) buckets.reviewRequested!.push(entry);
    else buckets.others!.push(entry);
  }
  return ['authored', 'reviewRequested', 'others'].filter(key => buckets[key]!.length).map(key => ({ key, label: GROUP_LABELS[key]!, entries: buckets[key]! }));
}
export function filterByInvolvement(entries: Obj[], viewers: Obj, involvement: string): Obj[] {
  if (involvement === 'reviewing') return entries.filter(entry => entry.viewerReviewRequested === true);
  if (involvement === 'authored') return entries.filter(entry => authoredByViewer(entry, viewers));
  return entries;
}
const size = (entry: Obj) => num(entry.additions) + num(entry.deletions);
const measured = (entry: Obj) => size(entry) > 0;
const updated = (entry: Obj) => str(entry.updatedAt);
const at = (value: unknown) => { const parsed = Date.parse(str(value)); return Number.isFinite(parsed) ? parsed : null; };
export function rankByMergeReadiness(entries: Obj[]): Obj[] {
  const tier = (entry: Obj) => entry.mergeability === 'conflicting' ? 4 : entry.state !== 'open' ? 3 : entry.isDraft === true ? 2
    : entry.checksState === 'passing' && entry.reviewDecision === 'approved' ? 0 : entry.checksState === 'passing' ? 1 : 2;
  return [...entries].sort((left, right) => (tier(left) - tier(right)) || (Number(measured(right)) - Number(measured(left))) || (size(left) - size(right)) || updated(right).localeCompare(updated(left)));
}
function rankByTier(entries: Obj[], tier: (entry: Obj) => number): Obj[] {
  return [...entries].sort((left, right) => {
    const byTier = tier(left) - tier(right);
    if (byTier) return byTier;
    const a = at(left.updatedAt), b = at(right.updatedAt);
    const known = Number(a === null) - Number(b === null);
    return known || (a === null || b === null ? 0 : b - a);
  });
}
const blockedOnAuthor = (entries: Obj[]) => rankByTier(entries, entry => entry.state !== 'open' ? 6 : entry.mergeability === 'conflicting' ? 0
  : entry.reviewDecision === 'changes-requested' ? 1 : entry.checksState === 'failing' ? 2 : entry.isDraft === true ? 3
    : entry.checksState === 'passing' && entry.reviewDecision === 'approved' ? 5 : 4);
const blockedOnReviewer = (entries: Obj[]) => rankByTier(entries, entry => entry.state === 'open' ? 0 : 1);
export function scoreMatch(entry: Obj, query: string): number {
  const needle = query.trim().toLowerCase();
  if (!needle) return 0;
  const number = needle.replace(/^#/u, '');
  if (/^\d+$/u.test(number)) return String(num(entry.number)) === number ? 100 : 0;
  const title = lower(entry.title), terms = needle.split(/\s+/u).filter(Boolean);
  if (title === needle) return 90;
  if (title.includes(needle)) return 80;
  if (terms.length > 1 && terms.every(term => title.includes(term))) return 70;
  if (lower(entry.headBranch).includes(needle)) return 60;
  if (lower(obj(entry.author).login).includes(needle)) return 50;
  if (lower(entry.repository).includes(needle)) return 40;
  if (terms.some(term => title.includes(term))) return 30;
  return 10;
}
export function sortGroups(groups: { key: string; label: string; entries: Obj[] }[], sort: string, searchText: string, involvement: string) {
  const each = (rank: (entries: Obj[]) => Obj[]) => groups.map(group => ({ ...group, entries: rank(group.entries) }));
  if (searchText.trim()) return each(entries => [...entries].sort((left, right) => (scoreMatch(right, searchText) - scoreMatch(left, searchText)) || updated(right).localeCompare(updated(left))));
  if (sort === 'ready') return each(rankByMergeReadiness);
  if (sort === 'blocked') return groups.map(group => {
    const role = group.key === 'others' ? involvement : group.key === 'authored' ? 'authored' : 'reviewing';
    return role === 'all' ? group : { ...group, entries: role === 'authored' ? blockedOnAuthor(group.entries) : blockedOnReviewer(group.entries) };
  });
  if (sort === 'updated') return groups;
  const stamp = (entry: Obj) => at(entry.updatedAt) ?? at(entry.createdAt) ?? 0;
  return each(entries => [...entries].sort((left, right) => {
    if (sort === 'newest' || sort === 'oldest') {
      const a = at(left.createdAt), b = at(right.createdAt);
      const known = Number(b !== null) - Number(a !== null), dated = (a ?? 0) - (b ?? 0);
      return known || (sort === 'newest' ? -dated : dated) || stamp(right) - stamp(left);
    }
    const known = Number(measured(right)) - Number(measured(left)), sized = size(left) - size(right);
    return known || (sort === 'largest' ? -sized : sized) || stamp(right) - stamp(left);
  }));
}
/** matchesPullRequestFilters: what the hosts may not have narrowed themselves. */
export function matchesFilters(entry: Obj, filters: PrFilters, viewer: string): boolean {
  const labels = arr(entry.labels).map(label => lower(label.name));
  const holds = (label: string) => labels.includes(label.trim().toLowerCase());
  const author = filters.author?.toLowerCase() === 'me' && viewer ? viewer : filters.author;
  return (!filters.draft || (entry.isDraft === true) === (filters.draft === 'only'))
    && (!filters.review || (filters.review === 'none' ? !entry.reviewDecision : entry.reviewDecision === filters.review))
    && (!filters.labels || filters.labels.every(group => group.some(holds)))
    && (!filters.excludedLabels || !filters.excludedLabels.some(holds))
    && (!author || lower(obj(entry.author).login) === author.toLowerCase());
}

// ── Presentation (pullRequestPresentation.tsx, timestampFormat.ts) ──────────

export function relativeLabel(iso: unknown, now: number): string {
  const time = at(iso);
  if (time === null) return '';
  const diff = now - time;
  if (diff < 60_000) return 'just now';
  const minutes = Math.floor(diff / 60_000);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  return `${Math.floor(hours / 24)}d ago`;
}
export const stateKey = (entry: Obj) => entry.state === 'open' && entry.isDraft === true ? 'draft' : str(entry.state, 'open');
const STATE_LABELS: Record<string, string> = { open: 'Open', draft: 'Draft', closed: 'Closed', merged: 'Merged' };
export const conflictLabel = (entry: Obj) => entry.state === 'open' && entry.isDraft !== true && entry.mergeability === 'conflicting'
  ? (str(entry.baseBranch) ? `Conflicts with ${str(entry.baseBranch)}` : 'Has conflicts') : '';
const CHECKS_LABELS: Record<string, string> = { passing: 'All checks have passed', failing: 'Some checks were not successful', pending: "Some checks haven't completed yet" };
const REVIEW_LABELS: Record<string, string> = { approved: 'Approved', 'changes-requested': 'Changes requested', 'review-required': 'Awaiting review' };
const count = (value: number) => String(Math.round(value)).replace(/\B(?=(\d{3})+(?!\d))/g, ',');
export const entryKey = (entry: Obj) => `${str(entry.host)}:${str(entry.repository)}#${num(entry.number)}`;
const labelColor = (color: unknown) => { const hex = str(color).trim().replace(/^#/, ''); return /^[0-9a-fA-F]{6}$/.test(hex) ? `#${hex}` : ''; };
const actor = (value: unknown) => { const person = obj(value), login = str(person.login, 'ghost'); return { login, avatar: str(person.avatarUrl), initial: login.slice(0, 1).toUpperCase(), name: str(person.name) }; };

/** The detail's address for a row, as the list and the panel pass it around. */
export const rowRef = (entry: Obj) => JSON.stringify({ projectId: str(entry.projectId), host: str(entry.host), repository: str(entry.repository), number: num(entry.number) });
const hex2 = (value: number) => Math.round(Math.max(0, Math.min(255, value))).toString(16).padStart(2, '0');
/** color-mix(in srgb, label p%, base): the badge's text tone. */
function mix(label: string, base: string, share: number): string {
  const channel = (color: string, index: number) => parseInt(color.slice(1 + index * 2, 3 + index * 2), 16);
  return `#${[0, 1, 2].map(index => hex2(channel(label, index) * share + channel(base, index) * (1 - share))).join('')}`;
}
/** PullRequestLabelChip: a wash of the label's color with its name in a mix of it and the foreground. */
export function labelChip(name: string, color: string) {
  if (!color) return { key: name.toLowerCase(), name, background: 'light-dark(#f4f4f5, #ffffff0f)', ink: 'light-dark(#27272a, #f5f5f5)' };
  return { key: name.toLowerCase(), name, background: `light-dark(${color}14, ${color}1f)`, ink: `light-dark(${mix(color, '#262626', 0.3)}, ${mix(color, '#f5f5f5', 0.45)})` };
}

export function presentRow(entry: Obj, now: number, selected: string) {
  const author = actor(entry.author);
  const ref = rowRef(entry);
  return {
    key: entryKey(entry), ref, number: num(entry.number), title: str(entry.title), state: stateKey(entry), stateLabel: STATE_LABELS[stateKey(entry)] ?? 'Open',
    conflict: conflictLabel(entry), checks: str(entry.checksState), checksLabel: entry.checksState ? `Checks: ${CHECKS_LABELS[str(entry.checksState)] ?? ''}` : '',
    review: str(entry.reviewDecision), reviewLabel: REVIEW_LABELS[str(entry.reviewDecision)] ?? '', additions: measured(entry) ? `+${count(num(entry.additions))}` : '',
    deletions: measured(entry) ? `-${count(num(entry.deletions))}` : '', author: author.login, avatar: author.avatar, initial: author.initial,
    repository: str(entry.repository), age: relativeLabel(entry.updatedAt, now), selected: ref === selected,
    labels: arr(entry.labels).slice(0, 3).map(label => labelChip(str(label.name), labelColor(label.color))),
    firstLabel: arr(entry.labels).slice(0, 1).map(label => labelChip(str(label.name), labelColor(label.color))),
    moreLabels: Math.max(0, arr(entry.labels).length - 1), projectId: str(entry.projectId), host: str(entry.host),
    // context-menu-gaps: the number's right-click (pageslocal:pr-link-menu) names the host it was read from.
    linkMenu: `${str(entry.provider)} ${str(entry.url)}`,
  };
}
export type PrRow = ReturnType<typeof presentRow>;

// ── The list resource ───────────────────────────────────────────────────────

type ListCache = { key: string; refresh: number; epoch: number; relist: number; result: Obj | null; error: string; stats: Map<string, Obj> };
const lists = new WeakMap<object, ListCache>();
/** The Refresh press whose invalidate was sent, per client. */
const invalidated = new WeakMap<object, number>();
export type PrInput = { open: boolean; refresh: number; now: number; selected: string; query: string; typed: boolean };

export function listPayload(prefs: PrPrefs, query: string): Obj {
  const parsed = parsePrQuery(query);
  const filters: Obj = { ...parsed.filters };
  if (prefs.draft && !filters.draft) filters.draft = prefs.draft;
  if (prefs.review && !filters.review) filters.review = prefs.review;
  if (prefs.checks && !filters.checks) filters.checks = prefs.checks;
  if (prefs.author && !filters.author) filters.author = prefs.author;
  if (prefs.labels.length) filters.labels = [...(parsed.filters.labels ?? []), ...prefs.labels.map(label => [label])].slice(0, 10);
  const payload: Obj = { state: prefs.state, involvement: prefs.involvement, limit: 99 };
  if (Object.keys(filters).length) payload.filters = filters as Obj;
  if (prefs.projectId) payload.projectId = prefs.projectId;
  if (prefs.host) payload.host = prefs.host;
  if (parsed.text.trim()) payload.query = parsed.text.trim().slice(0, 200);
  return payload;
}

export async function pullRequestsPage(client: T3Client, native: Native | null | undefined, input: PrInput) {
  const prefs = prPrefs(client);
  const query = input.typed ? input.query : prefs.q;
  const view = emptyList(prefs, query);
  view.diffScheme = diffSchemeOf(client);
  view.available = obj(obj(client.config.environment).capabilities).pullRequests === true;
  view.hasProjects = client.shell.projects.length > 0;
  view.projects = client.shell.projects.map(project => {
    const identity = projectIdentity(str(project.title));
    return { id: str(project.id), name: str(project.title), mark: identity.projectMark, ink: identity.projectInk, surface: identity.projectSurface, selected: prefs.projectId === project.id, unavailable: '' };
  });
  if (!input.open && native?.available && client.ready) await holdPullRequestRefreshes(client, native, 'list', false);
  if (!input.open || !native?.available || !client.ready) { view.loading = input.open && client.ready; return view; }
  if (!view.available) { view.empty = 'Pull requests unavailable'; view.emptyDetail = 'Update your T3 Code servers to browse pull requests.'; return view; }
  if (!view.hasProjects) { view.empty = 'No projects in this workspace'; view.emptyDetail = 'Add a project, and the pull requests from its repository appear here.'; view.emptyAction = 'add-project'; return view; }
  const payload = listPayload(prefs, query);
  // pr-conversation-and-refresh: the server's announcements read the list again (its refreshTrigger), the rows staying meanwhile.
  await holdPullRequestRefreshes(client, native, 'list', true);
  const key = JSON.stringify([client.environmentId, payload, input.refresh]);
  let cached = lists.get(client);
  if (!cached || cached.key !== key || cached.epoch !== pullRequestRefreshEpoch(client) || cached.relist !== listRelist(client)) {
    const previous = cached, previousStats = cached?.stats ?? new Map<string, Obj>();
    cached = { key, refresh: input.refresh, epoch: pullRequestRefreshEpoch(client), relist: listRelist(client), result: null, error: '', stats: previousStats };
    try {
      // One invalidate per Refresh press: each one announces a change, which asks this read again while
      // the first is still out, and that read would invalidate (and announce) again (pr-writing-and-metadata
      // live drive: 141 invalidates, ~50 detail and list reads in 3 s after one press).
      if (input.refresh > 0 && previous?.refresh === input.refresh - 1 && invalidated.get(client) !== input.refresh) {
        invalidated.set(client, input.refresh);
        await client.rpc(native, 'pullRequests.invalidate', {}).catch(() => ({}));
      }
      cached.result = await client.rpc(native, 'pullRequests.list', payload);
      // The host's word outranks the reader's once it has said it: an override goes when an answer agrees with it.
      settleListOverrides(client, arr(cached.result.entries), input.now);
      const unmeasured = arr(cached.result.entries).filter(entry => !measured(entry) && !cached!.stats.has(entryKey(entry)));
      if (unmeasured.length) {
        try {
          const stats = await client.rpc(native, 'pullRequests.listStats', { refs: unmeasured.slice(0, 500).map(entry => ({ projectId: entry.projectId, host: entry.host, repository: entry.repository, number: entry.number })) });
          for (const stat of arr(stats.stats)) cached.stats.set(`${arr(cached.result.entries).find(entry => entry.repository === stat.repository && entry.number === stat.number)?.host ?? ''}:${str(stat.repository)}#${num(stat.number)}`, stat);
        } catch { /* rows draw without counts */ }
      }
    } catch (error) { if (letGo(error)) throw error; cached.error = error instanceof Error ? error.message : 'Pull requests could not be read.'; }
    cached.epoch = pullRequestRefreshEpoch(client); // an announcement that landed while the read was out is answered by it
    lists.set(client, cached);
  }
  // The reader's pending answers (a closed pull request leaves an open list on the click), before the filters.
  const answered = cached.result ? { ...cached.result, entries: overrideListEntries(client, arr(cached.result.entries), prefs.state) } : null;
  return presentList(view, answered, cached.error, cached.stats, prefs, query, input.now, input.selected);
}

/** The list's row for a selection, as the detail ghost seeds itself from it (PullRequestDetailGhost `seed`). */
export function listEntryFor(client: object, selection: { projectId: string; host: string; repository: string; number: number }): Obj | null {
  return arr(lists.get(client)?.result?.entries).find(entry => entry.projectId === selection.projectId && num(entry.number) === selection.number
    && str(entry.repository).toLowerCase() === selection.repository.toLowerCase() && (!selection.host || str(entry.host).toLowerCase() === selection.host.toLowerCase())) ?? null;
}

export function emptyList(prefs: PrPrefs, query: string) {
  const labels = prefs.labels;
  const filterCount = [prefs.state !== 'open', prefs.involvement !== 'all', prefs.host, prefs.projectId, prefs.draft, prefs.review, prefs.checks, prefs.author, ...labels].filter(Boolean).length;
  return {
    available: true, loading: false, hasProjects: true, error: '', empty: '', emptyDetail: '', emptyAction: '', query, diffScheme: 'red-green',
    sort: prefs.sort, sortLabel: SORTS.find(option => option.value === prefs.sort)?.label ?? 'Merge readiness',
    state: prefs.state, involvement: prefs.involvement, draft: prefs.draft || 'all', review: prefs.review || 'all', checks: prefs.checks || 'all',
    author: prefs.author, labels: labels.map(label => ({ key: label, name: label })), host: prefs.host, hostLabel: prefs.host ? 'GitHub' : 'All',
    filterCount,
    stateLabel: STATES.find(option => option.value === prefs.state)?.label ?? 'Open', involvementLabel: INVOLVEMENTS.find(option => option.value === prefs.involvement)?.label ?? 'All',
    draftLabel: DRAFTS.find(option => option.value === (prefs.draft || 'all'))?.label ?? 'All', reviewLabel: REVIEWS.find(option => option.value === (prefs.review || 'all'))?.label ?? 'All',
    checksLabel: CHECKS.find(option => option.value === (prefs.checks || 'all'))?.label ?? 'All', projectLabel: 'All projects', projectId: prefs.projectId,
    hosts: [] as { value: string; label: string; selected: boolean; unavailable: string }[],
    projects: [] as { id: string; name: string; mark: string; ink: string; surface: string; selected: boolean; unavailable: string }[],
    authors: [] as { key: string; login: string; avatar: string; initial: string; detail: string; selected: boolean }[],
    labelFacets: [] as { key: string; name: string; color: string; count: number; selected: boolean }[],
    groups: [] as { key: string; label: string; count: number; rows: PrRow[] }[], truncated: false, errors: [] as { key: string; text: string }[], total: 0,
    authorLabel: prefs.author || 'Anyone', labelsLabel: labels.length ? `${labels.length} selected` : 'Any',
    stateIcon: STATE_ICONS[prefs.state] ?? 'layers', involvementIcon: INVOLVEMENT_ICONS[prefs.involvement] ?? 'layers',
    draftIcon: DRAFT_ICONS[prefs.draft || 'all'] ?? 'layers', reviewIcon: REVIEW_ICONS[prefs.review || 'all'] ?? 'layers', checksIcon: CHECKS_ICONS[prefs.checks || 'all'] ?? 'layers',
    filterBadge: filterCount > 0 ? String(filterCount) : '', filtersWidth: 85.8 + (filterCount > 0 ? 18 + 7 * String(filterCount).length : 0), providerIconOnly: !!prefs.host,
  };
}
export type PrListView = ReturnType<typeof emptyList>;

export function presentList(view: PrListView, result: Obj | null, error: string, stats: Map<string, Obj>, prefs: PrPrefs, query: string, now: number, selected: string): PrListView {
  if (!result) {
    view.loading = !error; view.error = error;
    if (error) { view.empty = 'Could not load pull requests'; view.emptyDetail = error; view.emptyAction = 'refresh'; }
    return view;
  }
  const viewers = obj(result.viewers);
  const parsed = parsePrQuery(query);
  const narrowed = arr(result.entries)
    .map(entry => { const stat = stats.get(entryKey(entry)); return stat && !measured(entry) ? { ...entry, additions: stat.additions, deletions: stat.deletions } : entry; })
    .filter(entry => (prefs.state === 'all' || entry.state === prefs.state) && (!prefs.projectId || entry.projectId === prefs.projectId) && (!prefs.host || entry.host === prefs.host))
    .filter(entry => matchesFilters(entry, { ...parsed.filters, ...(prefs.draft ? { draft: prefs.draft } : {}), ...(prefs.review ? { review: prefs.review } : {}), ...(prefs.author ? { author: prefs.author } : {}),
      ...(prefs.labels.length ? { labels: [...(parsed.filters.labels ?? []), ...prefs.labels.map(label => [label])] } : {}) }, viewerFor(entry, viewers)));
  const involved = filterByInvolvement(narrowed, viewers, prefs.involvement);
  const groups = sortGroups(groupEntries(involved, viewers), prefs.sort, parsed.text, prefs.involvement);
  view.groups = groups.map(group => ({ key: group.key, label: group.label, count: group.entries.length, rows: group.entries.map(entry => presentRow(entry, now, selected)) }));
  view.total = involved.length;
  view.truncated = result.truncated === true;
  view.errors = arr(result.errors).map(entry => ({ key: str(entry.projectId), text: `${str(entry.projectTitle)}: ${str(entry.message)}` }));
  const unavailable = new Map(arr(result.errors).map(entry => [str(entry.projectId), str(entry.message)]));
  view.projects = view.projects.map(project => ({ ...project, unavailable: unavailable.get(project.id) ?? '' }));
  view.projectLabel = view.projects.find(project => project.selected)?.name ?? 'All projects';
  const providers = arr(result.providers);
  view.hosts = [{ value: '', label: 'All', selected: !prefs.host, unavailable: '' },
    ...providers.map(provider => ({ value: str(provider.host), label: providers.filter(other => other.kind === provider.kind).length > 1 ? str(provider.host) : providerName(str(provider.kind)),
      selected: prefs.host === provider.host, unavailable: provider.configured === false ? str(provider.detail, 'Not signed in') : '' }))];
  view.hostLabel = view.hosts.find(host => host.selected)?.label ?? 'All';
  // collectPullRequestListFacets: authors by merges then count; labels by count.
  const authors = new Map<string, { actor: Obj; count: number; merged: number }>(), labels = new Map<string, { name: string; color: string; count: number }>();
  for (const entry of arr(result.entries)) {
    const inState = prefs.state === 'all' || entry.state === prefs.state, login = lower(obj(entry.author).login);
    if (login) { const held = authors.get(login); authors.set(login, { actor: held?.actor ?? obj(entry.author), count: (held?.count ?? 0) + Number(inState), merged: (held?.merged ?? 0) + Number(entry.state === 'merged') }); }
    if (!inState) continue;
    for (const label of arr(entry.labels)) { const name = lower(label.name); if (!name) continue; const held = labels.get(name); labels.set(name, { name: held?.name ?? str(label.name), color: held?.color ?? labelColor(label.color), count: (held?.count ?? 0) + 1 }); }
  }
  view.authors = [...authors.values()].filter(author => author.count > 0).sort((a, b) => b.merged - a.merged || b.count - a.count || str(a.actor.login).localeCompare(str(b.actor.login))).slice(0, 10)
    .map(author => { const person = actor(author.actor); return { key: person.login.toLowerCase(), login: person.login, avatar: person.avatar, initial: person.initial, detail: `${author.merged} merges loaded`, selected: lower(prefs.author) === person.login.toLowerCase() }; });
  const chosen = new Set(prefs.labels.map(label => label.toLowerCase()));
  view.labelFacets = [...prefs.labels.filter(label => !labels.has(label.toLowerCase())).map(name => ({ name, color: '', count: 0 })), ...[...labels.values()].sort((a, b) => b.count - a.count || a.name.localeCompare(b.name))]
    .map(label => ({ key: label.name.toLowerCase(), name: label.name, color: label.color, count: label.count, selected: chosen.has(label.name.toLowerCase()) }));
  if (view.total === 0) {
    const filtered = prefs.state !== 'open' || prefs.involvement !== 'all' || !!prefs.projectId || !!prefs.host;
    const shown = query.length > 48 ? `${query.slice(0, 48)}…` : query;
    if (query.trim()) { view.empty = `Nothing matches “${shown}”`; view.emptyDetail = 'The hosts were searched for it. Try fewer words, or search by number, author or branch.'; view.emptyAction = 'clear'; }
    else { view.empty = filtered ? 'Nothing under these filters' : 'No pull requests'; view.emptyDetail = filtered ? 'Widen the state, involvement or project filter to see more.' : 'Pull requests from every project in this workspace appear here.'; view.emptyAction = 'refresh'; }
  }
  return view;
}
function providerName(kind: string): string {
  return ({ github: 'GitHub', gitlab: 'GitLab', bitbucket: 'Bitbucket', 'azure-devops': 'Azure DevOps', forgejo: 'Forgejo' } as Record<string, string>)[kind] ?? kind;
}

// ── Commands ────────────────────────────────────────────────────────────────

/** pageslocal:pr-* — the list controls, remembered as the reference remembers them. */
export function prLocal(client: { local: object }, op: string, value: string): string {
  const prefs = prPrefs(client);
  if (op === 'sort' && SORTS.some(option => option.value === value)) prefs.sort = value;
  else if (op === 'state' && STATES.some(option => option.value === value)) prefs.state = value;
  else if (op === 'involvement' && INVOLVEMENTS.some(option => option.value === value)) prefs.involvement = value;
  else if (op === 'draft' && DRAFTS.some(option => option.value === value)) prefs.draft = value === 'all' ? '' : value;
  else if (op === 'review' && REVIEWS.some(option => option.value === value)) prefs.review = value === 'all' ? '' : value;
  else if (op === 'checks' && CHECKS.some(option => option.value === value)) prefs.checks = value === 'all' ? '' : value;
  else if (op === 'author') prefs.author = prefs.author.toLowerCase() === value.toLowerCase() ? '' : bounded(value);
  else if (op === 'host') prefs.host = bounded(value);
  else if (op === 'project') prefs.projectId = bounded(value);
  else if (op === 'label') prefs.labels = prefs.labels.some(label => label.toLowerCase() === value.toLowerCase()) ? prefs.labels.filter(label => label.toLowerCase() !== value.toLowerCase()) : [...prefs.labels, bounded(value)].slice(0, 10);
  else if (op === 'query') prefs.q = bounded(value);
  else throw new ClientError(`Unknown pull request control: ${op}`);
  savePrPrefs(client, prefs);
  return '';
}
