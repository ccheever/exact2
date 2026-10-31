// The Browser tab's history as threads write and read it (T3 Code 7c3b3b3605ef, MIT, see LICENSE-T3:
// apps/web/src/browserHistoryStore.ts resolveThreadProjectKey, recordVisitForThread, recordPreviewTitles,
// startPreviewTitleRecording, removeUrlForThread, useThreadHistoryProjectKey).
//
// History follows the sidebar's project rows: a thread's visits land under its project's logical key
// (project-groups.ts `logicalKey`, the grouping the sidebar lists), so every thread of a project shares one Recently
// used. A visit registers its thread's project first; until the preferences load (the grouping is unknown before
// then) or the project is known, the last registration stands and the visit waits in the store's queue. A visit fills
// the title of an untitled entry from the thread's settled tabs; every thread's tabs title their pages as they load or
// retitle, and a tab that settles again on the same page and title (a resize, a list reconcile) leaves the newer title
// another thread gave it.
import type { T3Client } from '../client/client';
import { str, type Obj } from '../client/domain';
import { fleet, type EnvironmentFleet } from '../connections/settings-b-fleet';
import { logicalKey } from '../sidebar/project-groups';
import { parseScopedThreadKey, type ScopedThreadRef } from '../terminal/terminal-ui-state';
import { browserHistory, type BrowserHistoryEntry } from './browser-history';
import { browserHost, effectiveNav } from './browser-surface';
import { environmentHostname } from './browser-targets';
import type { PreviewNavStatus } from './browser-state';

/** The thread's project, as its environment lists it: a server thread's from its shell, a draft's from the draft it is. */
function threadProject(client: T3Client, ref: ScopedThreadRef, source: EnvironmentFleet): { environmentId: string; project: Obj } | null {
  const shell = ref.environmentId === client.environmentId ? client.shell : [...source.entries.values()].find(entry => entry.environmentId === ref.environmentId)?.shell;
  if (!shell) return null;
  let projectId = str(shell.threads.find(thread => thread.id === ref.threadId)?.projectId);
  if (!projectId && ref.environmentId === client.environmentId) {
    // A draft's id is kept with its draft key, `<environment>:new:<project>` (draft-thread-id.ts).
    const draftKey = Object.entries(client.local.composerControls?.draftThreads ?? {}).find(([, id]) => id === ref.threadId)?.[0] ?? '';
    const prefix = `${ref.environmentId}:new:`;
    if (draftKey.startsWith(prefix)) projectId = draftKey.slice(prefix.length);
  }
  const project = projectId ? shell.projects.find(entry => entry.id === projectId) : undefined;
  return project ? { environmentId: ref.environmentId, project } : null;
}

/** useThreadHistoryProjectKey: the history key of the thread's current project, else the one it was registered under. */
export function threadHistoryProjectKey(client: T3Client, ref: ScopedThreadRef, source: EnvironmentFleet = fleet): string | null {
  const found = client.preferencesLoaded ? threadProject(client, ref, source) : null;
  return found ? logicalKey(client, found.environmentId, found.project) : browserHistory(client.local).registeredProject(ref) ?? null;
}

/** resolveThreadProjectKey: registers the thread under its current project, landing its queued visits and their titles. */
function resolveThreadProjectKey(client: T3Client, ref: ScopedThreadRef, source: EnvironmentFleet): string | undefined {
  const history = browserHistory(client.local), registered = history.registeredProject(ref);
  const found = client.preferencesLoaded ? threadProject(client, ref, source) : null;
  if (!found) return registered;
  const projectKey = logicalKey(client, found.environmentId, found.project);
  const landed = history.registerThreadProject(ref, projectKey);
  if (landed.length) titleVisits(client, ref, landed);
  return projectKey;
}

/** recordVisitForThread: the visit, then the title of its page if a tab of the thread already settled on it. */
export function recordVisit(client: T3Client, ref: ScopedThreadRef, url: string, at: number, source: EnvironmentFleet = fleet): void {
  const projectKey = resolveThreadProjectKey(client, ref, source);
  browserHistory(client.local).recordVisitForThread(ref, url, at, environmentHostname(client));
  if (projectKey) titleVisits(client, ref, [url]);
}

export function removeVisit(client: T3Client, ref: ScopedThreadRef, url: string, source: EnvironmentFleet = fleet): void {
  const projectKey = resolveThreadProjectKey(client, ref, source);
  if (projectKey) browserHistory(client.local).removeUrl(projectKey, url);
}

/** useThreadRecentHistory. */
export function recentHistory(client: T3Client, ref: ScopedThreadRef, limit: number, source: EnvironmentFleet = fleet): ReadonlyArray<BrowserHistoryEntry> {
  return browserHistory(client.local).projectHistory(threadHistoryProjectKey(client, ref, source), limit);
}

type Settled = { tabId: string; nav: PreviewNavStatus; updatedAt: string };
/** The thread's tabs as they show now, oldest settle first: tabs can share a URL, so the newest settle wins. */
function settledTabs(client: T3Client, ref: ScopedThreadRef): Settled[] {
  return Object.values(browserHost(client).store.read(ref).sessions)
    .map(session => ({ tabId: session.tabId, nav: effectiveNav(client, ref, session.tabId).nav, updatedAt: str(session.updatedAt) }))
    .sort((a, b) => a.updatedAt.localeCompare(b.updatedAt));
}

/** recordPreviewTitles with `urls`: a visit only fills untitled entries, so replaying an old settle cannot clobber a newer title. */
function titleVisits(client: T3Client, ref: ScopedThreadRef, urls: string[]): void {
  const history = browserHistory(client.local), projectKey = history.registeredProject(ref);
  const entries = history.projectHistory(projectKey, Number.MAX_SAFE_INTEGER);
  if (!projectKey || !entries.length) return;
  const host = environmentHostname(client);
  const titled = new Set(entries.flatMap(entry => entry.title ? [history.titleKey(entry.url, host)] : []));
  const wanted = new Set(urls.map(url => history.titleKey(url, host)).filter(key => key !== null && !titled.has(key)));
  for (const { nav } of settledTabs(client, ref)) {
    if (nav._tag !== 'Success' || !nav.title || !wanted.has(history.titleKey(nav.url, host))) continue;
    history.setTitleForUrl(projectKey, nav.url, nav.title, host);
  }
}

/** What each followed thread's tabs showed at the last pass, by thread key and tab id; a thread stays followed. */
const followed = new WeakMap<T3Client, Map<string, Map<string, PreviewNavStatus>>>();
/**
 * startPreviewTitleRecording, run on every shell answer: the threads the first pass sees are its baseline, so a launch
 * replays no settle; a thread that gets its first tab later starts from none. A tab that has loaded or retitled since the
 * last pass titles its page in the thread's project history.
 */
export function recordPreviewTitles(client: T3Client): void {
  let threads = followed.get(client);
  const started = !!threads;
  if (!threads) followed.set(client, threads = new Map());
  const history = browserHistory(client.local), host = environmentHostname(client);
  for (const [threadKey] of browserHost(client).store.active()) {
    const ref = parseScopedThreadKey(threadKey);
    if (!ref) continue;
    const tabs = settledTabs(client, ref), previous = threads.get(threadKey) ?? (started ? new Map<string, PreviewNavStatus>() : undefined);
    threads.set(threadKey, new Map(tabs.map(tab => [tab.tabId, tab.nav])));
    const projectKey = history.registeredProject(ref);
    if (!previous || !projectKey || !history.projectHistory(projectKey, 1).length) continue;
    for (const { tabId, nav } of tabs) {
      const before = previous.get(tabId);
      const settledAgain = before?._tag === 'Success' && nav._tag === 'Success' && before.url === nav.url && before.title === nav.title;
      if (nav._tag !== 'Success' || !nav.title || settledAgain) continue;
      history.setTitleForUrl(projectKey, nav.url, nav.title, host);
    }
  }
}
