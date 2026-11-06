// Hiding tagged threads, ported from T3 Code 5d3d78ab93 (MIT; see LICENSE-T3):
// packages/client-runtime/src/state/threadTagFilter.ts (visibilityParentKey, visibleLinkedThreads,
// hiddenThreadKeys), state/server.ts (hiddenThreadTagsAtom) and state/threadShell.ts (hiddenThreadKeysAtom,
// anyEnvironmentHidingAtom). With an environment's `hideTaggedThreads` on, its threads carrying one of its
// `hiddenThreadTags` leave every list that shows threads to the user; subagents, side chats and delegated tasks
// follow their parent. The shell keeps them, so code that asks whether a thread exists never mistakes a hidden one
// for a missing one. A key is `<environmentId>\n<threadId>`.
import type { T3Client } from '../client/client';
import { isListedThreadLineage, obj, str, type Obj } from '../client/domain';
import { fleet, parseFleetThreadId, type EnvironmentFleet } from '../connections/settings-b-fleet';
import { hiddenThreadTags, threadTags } from '../client/server-fields';

export type TagFilteredEnvironment = {
  environmentId: string;
  threads: readonly Obj[];
  /** Tags this environment hides, or null while its filter is off. */
  hiddenTags: readonly string[] | null;
};

export const NO_HIDDEN_THREADS: ReadonlySet<string> = new Set();
export const threadKey = (environmentId: string, threadId: string): string => `${environmentId}\n${threadId}`;

/** hiddenThreadTagsAtom over a server config's settings; null while its filter is off. */
export const hiddenTagsOf = (config: Obj | undefined): string[] | null => hiddenThreadTags(obj(config?.settings));

/** The thread a subagent, side chat or delegated task takes its visibility from, if any. */
export function visibilityParentKey(environmentId: string, thread: Obj): string | null {
  const lineage = obj(thread.lineage), parentId = lineage.parentThreadId == null ? null : str(lineage.parentThreadId);
  if (!isListedThreadLineage(lineage) && parentId !== null) return threadKey(environmentId, parentId);
  const from = thread.delegatedFrom == null ? null : obj(thread.delegatedFrom);
  return from ? threadKey(str(from.environmentId), str(from.threadId)) : null;
}

/**
 * Pull request linked-thread rows the tag filter leaves visible. Loaded threads use the parent-aware hidden set.
 * Rows the shell leaves out carry no lineage, so while any environment hides tags they stay out.
 */
export function visibleLinkedThreads<Row extends { id?: unknown }>(rows: Row[], environmentId: string, loadedThreadIds: ReadonlySet<string>,
  hiddenKeys: ReadonlySet<string>, hidingAnywhere: boolean): Row[] {
  if (!hidingAnywhere && hiddenKeys.size === 0) return rows;
  return rows.filter(row => loadedThreadIds.has(str(row.id)) ? !hiddenKeys.has(threadKey(environmentId, str(row.id))) : !hidingAnywhere);
}

const hasHiddenTag = (thread: Obj, hiddenTags: readonly string[] | null) => hiddenTags?.some(tag => threadTags(thread).includes(tag)) === true;

/**
 * Keys of threads the tag filter hides. Children (subagents, side chats, delegated tasks) follow their parent; while
 * any environment hides tags, a child whose parent is not loaded stays hidden. For a partial list such as an archive
 * page, `alreadyHidden` and `loadedElsewhere` give the hidden and loaded keys of live threads, and `hidingAnywhere`
 * the global flag.
 */
export function hiddenThreadKeys(environments: readonly TagFilteredEnvironment[], alreadyHidden: ReadonlySet<string> = NO_HIDDEN_THREADS,
  loadedElsewhere: ReadonlySet<string> = NO_HIDDEN_THREADS, hidingAnywhere = environments.some(environment => environment.hiddenTags !== null)): ReadonlySet<string> {
  if (alreadyHidden.size === 0 && !hidingAnywhere) return NO_HIDDEN_THREADS;
  const byKey = new Map<string, { thread: Obj; environment: TagFilteredEnvironment }>();
  for (const environment of environments) {
    for (const thread of environment.threads) byKey.set(threadKey(environment.environmentId, str(thread.id)), { thread, environment });
  }
  const hiddenByKey = new Map<string, boolean>();
  const isHidden = (key: string): boolean => {
    if (alreadyHidden.has(key)) return true;
    const known = hiddenByKey.get(key);
    if (known !== undefined) return known;
    const entry = byKey.get(key);
    if (entry === undefined) return false;
    const { thread, environment } = entry;
    const parentKey = visibilityParentKey(environment.environmentId, thread);
    const followsParent = parentKey !== null && (byKey.has(parentKey) || alreadyHidden.has(parentKey));
    if (parentKey !== null && !followsParent && loadedElsewhere.has(parentKey)) { hiddenByKey.set(key, false); return false; }
    if (parentKey !== null && !followsParent && hidingAnywhere) { hiddenByKey.set(key, true); return true; }
    if (!followsParent) {
      const hidden = hasHiddenTag(thread, environment.hiddenTags);
      hiddenByKey.set(key, hidden);
      return hidden;
    }
    // Seed before following the parent so a malformed cycle ends visible.
    hiddenByKey.set(key, false);
    const hidden = isHidden(parentKey!);
    hiddenByKey.set(key, hidden);
    return hidden;
  };
  const hidden = new Set<string>();
  for (const key of byKey.keys()) if (isHidden(key)) hidden.add(key);
  return hidden.size === 0 ? NO_HIDDEN_THREADS : hidden;
}

/** The focused environment and each background one, with their shells and the tags they hide. */
export function tagFilteredEnvironments(client: Pick<T3Client, 'environmentId' | 'config' | 'shell'>, source: EnvironmentFleet = fleet): TagFilteredEnvironment[] {
  const environments: TagFilteredEnvironment[] = client.environmentId ? [{ environmentId: client.environmentId, threads: client.shell.threads, hiddenTags: hiddenTagsOf(client.config) }] : [];
  for (const entry of source.entries.values()) {
    if (entry.environmentId && entry.environmentId !== client.environmentId) environments.push({ environmentId: entry.environmentId, threads: entry.shell?.threads ?? [], hiddenTags: hiddenTagsOf(entry.config) });
  }
  return environments;
}

let cache: { client: unknown; revision: number; fleet: number; hiding: boolean; keys: ReadonlySet<string> } | null = null;
/** hiddenThreadKeysAtom and anyEnvironmentHidingAtom, kept while neither the client nor the fleet changed. */
export function threadTagFilter(client: Pick<T3Client, 'environmentId' | 'config' | 'shell' | 'revision'>, source: EnvironmentFleet = fleet): { hiding: boolean; keys: ReadonlySet<string> } {
  if (source === fleet && cache && cache.client === client && cache.revision === client.revision && cache.fleet === fleet.revision) return cache;
  const environments = tagFilteredEnvironments(client, source);
  const hiding = environments.some(environment => environment.hiddenTags !== null);
  // Read the shells only while some environment hides tags, so a streaming tick costs nothing otherwise.
  const keys = hiding ? hiddenThreadKeys(environments) : NO_HIDDEN_THREADS;
  if (source === fleet) cache = { client, revision: client.revision, fleet: fleet.revision, hiding, keys };
  return { hiding, keys };
}

/** The key of a sidebar row's thread: its own id on the focused environment, `fleet:<environment>:<id>` on another. */
export function rowThreadKey(client: Pick<T3Client, 'environmentId'>, rowId: string): string {
  const ref = parseFleetThreadId(rowId);
  return ref ? threadKey(ref.environmentId, ref.threadId) : threadKey(client.environmentId, rowId);
}
/** Whether the tag filter hides the thread a row or a focused-environment id names. */
export function isTagHidden(client: Pick<T3Client, 'environmentId' | 'config' | 'shell' | 'revision'>, rowId: string): boolean {
  const { keys } = threadTagFilter(client);
  return keys.size > 0 && keys.has(rowThreadKey(client, rowId));
}

/** An environment's shell threads (the focused one's or a background one's), for the loaded-thread checks below. */
function environmentThreads(client: Pick<T3Client, 'environmentId' | 'shell'>, environmentId: string, source: EnvironmentFleet): readonly Obj[] {
  if (!environmentId || environmentId === client.environmentId) return client.shell.threads;
  return [...source.entries.values()].find(entry => entry.environmentId === environmentId)?.shell.threads ?? [];
}
/** useVisibleThreadShells for one environment's threads: none the tag filter hides. */
export function visibleEnvironmentThreads<Row extends Obj>(client: Pick<T3Client, 'environmentId' | 'config' | 'shell' | 'revision'>, environmentId: string, threads: Row[],
  source: EnvironmentFleet = fleet): Row[] {
  const { keys } = threadTagFilter(client, source), environment = environmentId || client.environmentId;
  return keys.size ? threads.filter(thread => !keys.has(threadKey(environment, str(thread.id)))) : threads;
}
/** visibleLinkedThreads over an environment's pull request relations, judged against its loaded shell. */
export function visibleRelationThreads<Row extends Obj>(client: Pick<T3Client, 'environmentId' | 'config' | 'shell' | 'revision'>, environmentId: string, rows: Row[],
  source: EnvironmentFleet = fleet): Row[] {
  const { keys, hiding } = threadTagFilter(client, source), environment = environmentId || client.environmentId;
  if (!hiding && keys.size === 0) return rows;
  return visibleLinkedThreads(rows, environment, new Set(environmentThreads(client, environment, source).map(thread => str(thread.id))), keys, hiding);
}

/**
 * createArchivedThreadSnapshotsAtomFamily's filter: the archived threads to leave out, judged in one pass over every
 * environment's page against the live threads, so archived tasks follow parents archived elsewhere or still live.
 * Any connected machine may hide an unloaded parent, so the global flag decides, not the archive's scope.
 */
export function hiddenArchivedKeys(client: Pick<T3Client, 'environmentId' | 'config' | 'shell' | 'revision'>, pages: readonly { environmentId: string; threads: readonly Obj[] }[],
  source: EnvironmentFleet = fleet): ReadonlySet<string> {
  const live = tagFilteredEnvironments(client, source), { keys, hiding } = threadTagFilter(client, source);
  const tags = new Map(live.map(environment => [environment.environmentId, environment.hiddenTags]));
  const loaded = hiding ? new Set(live.flatMap(environment => environment.threads.map(thread => threadKey(environment.environmentId, str(thread.id))))) : undefined;
  return hiddenThreadKeys(pages.map(page => ({ ...page, hiddenTags: tags.get(page.environmentId) ?? null })), keys, loaded, hiding);
}

/**
 * ThreadRouteView: the open thread leaves its route for the index when the tag filter hides it. While any environment
 * hides tags, a thread the shell does not list (its cached detail may predate a tag or an archive) stays open only as
 * the child of a loaded, visible parent, as lineage never changes.
 */
export function routeHiddenByTags(client: Pick<T3Client, 'environmentId' | 'config' | 'shell' | 'revision' | 'threadId' | 'thread'>, source: EnvironmentFleet = fleet): boolean {
  const open = client.threadId;
  if (!open) return false;
  const { keys, hiding } = threadTagFilter(client, source);
  if (keys.has(threadKey(client.environmentId, open))) return true;
  if (!hiding || !client.thread || client.shell.threads.some(thread => thread.id === open)) return false;
  const parent = visibilityParentKey(client.environmentId, obj(client.thread.projection.thread));
  const loaded = parent !== null && tagFilteredEnvironments(client, source).some(environment => environment.threads.some(thread => threadKey(environment.environmentId, str(thread.id)) === parent));
  return !(loaded && !keys.has(parent!));
}

/** ThreadRelationshipsPanel's reading of the filter: which of the focused environment's threads it hides, and whether any environment hides. */
export function lineageTags(client: Pick<T3Client, 'environmentId' | 'config' | 'shell' | 'revision'>): { hidden: (threadId: string) => boolean; hiding: boolean } {
  const { keys, hiding } = threadTagFilter(client);
  return { hidden: threadId => keys.has(threadKey(client.environmentId, threadId)), hiding };
}
/** The focused environment's shell without the threads the tag filter hides (useVisibleThreadShells), for project order by activity. */
export function visibleShell<S extends { threads: Obj[] }>(client: Pick<T3Client, 'environmentId' | 'config' | 'revision'> & { shell: S }): S {
  const threads = visibleEnvironmentThreads(client as Pick<T3Client, 'environmentId' | 'config' | 'shell' | 'revision'>, client.environmentId, client.shell.threads);
  return threads === client.shell.threads ? client.shell : { ...client.shell, threads };
}
