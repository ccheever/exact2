// Pull requests across several servers (task pr-links-previews-and-routing): which server lists a
// repository, where a pull request can be acted on, the environments' listings folded into one, and
// the routing of a pull request read or write through another connected server that shares the
// same GitHub account. Sources: T3 Code (MIT, see LICENSE-T3) at 1e2ecbd975 —
// apps/web/src/components/pullRequest/{pullRequestProjectAssignment.logic,pullRequestList.logic}.ts
// (assignProjectsToEnvironments, resolvePickableEnvironments, mergePullRequestLists,
// pullRequestEntryViewer, pullRequestEnvironmentSetKey, resolveProjectScope, findScopedProject,
// resolveQueryEnvironmentIds, resolveSelectedEnvironmentId), apps/web/src/routes/_chat.pull-requests.tsx
// (PAGE_SIZE, MAX_PAGE_SIZE), packages/client-runtime/src/connection/githubRoutingPermissions.ts
// (gitHubRoutingPermissionFor) and packages/client-runtime/src/state/pullRequestRouting.ts
// (createPullRequestRouter).
//
// Port changes. The reference router is an Effect over the connection registry; here it is a class
// over a `PullRequestRouterHost` the caller builds from the focused connection and the background
// fleet: `environments()` is the registry's entries (read again at every decision, as the reference
// reads its SubscriptionRef), `request` is `registry.run(id, request(tag, input))`. Effect.cached is a
// lazily started, memoized promise. Timeouts (30 s reads, 2 s identity probes, 1 s invalidations) are
// the transport's own per-request deadline (`timeoutSeconds`, T3Transport `timeout`): a data module
// has no timer (X19). Interruption is Exact letting the answer go (`letGo`), which every fallback
// rethrows, as the reference re-raises `Cause.hasInterrupts`. The SSH-profile re-check of
// `routingAllowed` is folded into `permission` by the host: an SSH environment whose saved profile no
// longer produces the stored connection key reads "off" (routingPermissionFor over the key the
// current catalog computes), the same answer the reference's profile-store comparison gives.
// `dispatch` is the same routing for a write sent detached (the Viewed ticks, whose reply comes back
// in a later answer): its invalidations are `afterWrite`, run by the answer that sees the reply.
import { arr, num, obj, str, type Obj } from './domain';
import { ClientError } from './protocol';
import { letGo } from './let-go';

// ── One server per repository (pullRequestProjectAssignment.logic.ts) ───────

/** The little of a project this needs: who holds it, and which repository it is a copy of. */
export interface AssignableProject {
  readonly id: string;
  readonly environmentId: string;
  readonly repositoryIdentity?: { readonly canonicalKey?: string | undefined } | null | undefined;
}

/** The remote's normalized URL is what says "same repository" across machines. */
function repositoryKey(project: AssignableProject): string | undefined {
  const key = project.repositoryIdentity?.canonicalKey;
  return typeof key === 'string' && key ? key.toLowerCase() : undefined;
}

/**
 * Two servers can hold the same repository, and both would list the same pull requests, so one
 * copy is picked by its repository key: the preferred server where it has it, else the first. A
 * project with no identity is never de-duplicated.
 */
export function assignProjectsToEnvironments(projects: ReadonlyArray<AssignableProject>, environmentIds: ReadonlyArray<string>, preferredEnvironmentId?: string | null): Map<string, string[]> {
  const rank = new Map(environmentIds.map((id, index) => [id, index] as const));
  const owner = new Map<string, string>();
  for (const project of projects) {
    const key = repositoryKey(project);
    if (!key) continue;
    const environmentRank = rank.get(project.environmentId);
    if (environmentRank === undefined) continue;
    const current = owner.get(key);
    if (current === undefined) { owner.set(key, project.environmentId); continue; }
    if (current === preferredEnvironmentId) continue;
    if (project.environmentId === preferredEnvironmentId || environmentRank < (rank.get(current) ?? Number.MAX_SAFE_INTEGER)) owner.set(key, project.environmentId);
  }
  const assignment = new Map<string, string[]>();
  for (const project of projects) {
    if (!rank.has(project.environmentId)) continue;
    const key = repositoryKey(project);
    if (key && owner.get(key) !== project.environmentId) continue;
    const listed = assignment.get(project.environmentId);
    if (listed === undefined) assignment.set(project.environmentId, [project.id]);
    else listed.push(project.id);
  }
  return assignment;
}

/** A copy of the repository the reader could act on, named by the server holding it. */
export interface PickableEnvironment {
  readonly environmentId: string;
  readonly projectId: string;
  readonly workspaceRoot: string;
  readonly label: string;
  readonly machine?: string;
}

/**
 * Which servers a pull request could be acted on, given the one it is listed under: the panel's own
 * server first, then one entry per other server holding a copy. Empty where there is no choice to
 * offer (one server, no repository identity, projects not read yet).
 */
export function resolvePickableEnvironments(current: { environmentId: string; projectId: string }, projects: ReadonlyArray<AssignableProject & { readonly workspaceRoot: string }>,
  environments: ReadonlyArray<{ environmentId: string; label: string; machine?: string }>): ReadonlyArray<PickableEnvironment> {
  const own = projects.find(project => project.environmentId === current.environmentId && project.id === current.projectId);
  const key = own === undefined ? undefined : repositoryKey(own);
  const ownEnvironment = environments.find(environment => environment.environmentId === current.environmentId);
  if (own === undefined || !key || ownEnvironment === undefined) return [];
  const others = environments.flatMap(environment => {
    if (environment.environmentId === current.environmentId) return [];
    const copy = projects.find(project => project.environmentId === environment.environmentId && repositoryKey(project) === key);
    return copy === undefined ? [] : [{ environmentId: environment.environmentId, projectId: copy.id, workspaceRoot: copy.workspaceRoot, label: environment.label,
      ...(environment.machine === undefined ? {} : { machine: environment.machine }) }];
  });
  if (others.length === 0) return [];
  return [{ environmentId: current.environmentId, projectId: own.id, workspaceRoot: own.workspaceRoot, label: ownEnvironment.label,
    ...(ownEnvironment.machine === undefined ? {} : { machine: ownEnvironment.machine }) }, ...others];
}

// ── Several environments' listings (pullRequestList.logic.ts) ──────────────

/** One whole page from the host and no more (GitHub serves a hundred, the probe asks one beyond). */
export const PR_PAGE_SIZE = 99;
/** The largest page the listing accepts. */
export const PR_MAX_PAGE_SIZE = 500;

/**
 * The folded answer. `viewers` is keyed `"<environmentId> <host>"`, so one host's two accounts stay
 * two accounts; `nextCursors` per environment, each per repository as that environment gave it;
 * `truncatedEnvironments` the environments with rows still on their hosts.
 */
export type MergedPullRequestList = { viewers: Record<string, string>; providers: Obj[]; entries: Obj[]; errors: Obj[]; truncated: boolean; nextCursors: Record<string, Record<string, string>>; truncatedEnvironments: string[] };

const stringRecord = (value: unknown): Record<string, string> => Object.fromEntries(Object.entries(obj(value)).filter((entry): entry is [string, string] => typeof entry[1] === 'string'));

/**
 * The environments' answers folded into one. A host reached from more than one environment is one
 * switcher row, readable if any could read it and searched on the host only if every one did.
 */
export function mergePullRequestLists(answers: ReadonlyArray<readonly [string, Obj]>): MergedPullRequestList | null {
  if (answers.length === 0) return null;
  const viewers: Record<string, string> = {};
  const truncatedEnvironments: string[] = [];
  const providers = new Map<string, Obj>();
  const entries: Obj[] = [];
  const errors: Obj[] = [];
  const nextCursors: Record<string, Record<string, string>> = {};
  let truncated = false;
  for (const [environmentId, answer] of answers) {
    for (const [host, login] of Object.entries(stringRecord(answer.viewers))) viewers[`${environmentId} ${host}`] = login;
    for (const provider of arr(answer.providers)) {
      const host = str(provider.host), held = providers.get(host);
      providers.set(host, held === undefined ? provider : {
        ...(held.configured === true ? held : provider),
        projectCount: num(held.projectCount) + num(provider.projectCount),
        searchesOnHost: held.searchesOnHost === true && provider.searchesOnHost === true,
        configured: held.configured === true || provider.configured === true,
      });
    }
    entries.push(...arr(answer.entries).map(entry => ({ ...entry, environmentId })));
    errors.push(...arr(answer.errors).map(error => ({ ...error, environmentId })));
    truncated ||= answer.truncated === true;
    if (answer.truncated === true) truncatedEnvironments.push(environmentId);
    const cursors = stringRecord(answer.nextCursors);
    if (Object.keys(cursors).length > 0) nextCursors[environmentId] = cursors;
  }
  return { viewers, providers: [...providers.values()], entries: entries.sort((left, right) => str(right.updatedAt).localeCompare(str(left.updatedAt))), errors, truncated, nextCursors, truncatedEnvironments };
}

/** A row's key: two environments' copies of one pull request are two rows (pullRequestEntryKey). */
export function pullRequestEntryKey(entry: Obj): string {
  return `${entry.environmentId === undefined ? '' : `${str(entry.environmentId)}:`}${str(entry.host)}:${str(entry.repository)}#${num(entry.number)}`;
}
/** The viewer key of a row: its environment and host (pullRequestViewerKey). */
export function pullRequestViewerKey(entry: Obj): string { return `${str(entry.environmentId)} ${str(entry.host)}`; }
const normalizeLogin = (value: unknown) => { const login = typeof value === 'string' ? value.trim().toLowerCase() : ''; return login || null; };
/** The signed-in login for the host a row came from: the environment's own answer, else a plain host key (one server, a snapshot). */
export function pullRequestEntryViewer(entry: Obj, viewers: Record<string, string>): string | null {
  return normalizeLogin(viewers[pullRequestViewerKey(entry)] ?? viewers[str(entry.host)]);
}
/** isAuthoredByViewer: authorship is per host and per server, never per provider kind. */
export function isAuthoredByViewer(entry: Obj, viewers: Record<string, string>): boolean {
  const viewer = pullRequestEntryViewer(entry, viewers);
  return viewer !== null && normalizeLogin(obj(entry.author).login) === viewer;
}

/** Keyed by the whole set of environments the list was read from, whichever order they connected in. */
export function pullRequestEnvironmentSetKey(environmentIds: ReadonlyArray<string>): string {
  return [...environmentIds].sort((left, right) => left.localeCompare(right)).join(',');
}

/** The project scope to actually ask for: an id the environment does not have is dropped, once its projects are known. */
export function resolveProjectScope(projectId: string | undefined, projects: ReadonlyArray<{ id: string }>, projectsKnown: boolean): string | undefined {
  if (projectId === undefined || !projectsKnown) return projectId;
  return projects.some(project => project.id === projectId) ? projectId : undefined;
}

/** The project an id names, on the server that owns it; a bare id only where one server has it. */
export function findScopedProject<P extends { id: string; environmentId: string }>(projects: ReadonlyArray<P>, environmentId: string | null | undefined, projectId: string | undefined): P | undefined {
  if (projectId === undefined) return undefined;
  const matches = projects.filter(project => project.id === projectId);
  if (environmentId === null || environmentId === undefined) return matches.length === 1 ? matches[0] : undefined;
  return matches.find(project => project.environmentId === environmentId);
}

/** Which environments a listing should ask, once the project scope is known. */
export function resolveQueryEnvironmentIds(environmentIds: ReadonlyArray<string>, projects: ReadonlyArray<{ id: string; environmentId: string }>, scopedProject: { environmentId: string } | undefined,
  scopedProjectId: string | undefined, projectsKnown: boolean): ReadonlyArray<string> {
  if (scopedProject !== undefined) return environmentIds.filter(environmentId => environmentId === scopedProject.environmentId);
  if (scopedProjectId === undefined || !projectsKnown) return environmentIds;
  const holders = new Set(projects.filter(project => project.id === scopedProjectId).map(project => project.environmentId));
  return environmentIds.filter(environmentId => holders.has(environmentId));
}

/** The server a saved selection names: a known one is kept even while it connects; only a name outside the catalog falls back. */
export function resolveSelectedEnvironmentId(named: string | undefined, known: ReadonlySet<string>, fallback: string | null): string | null {
  if (named === undefined) return fallback;
  return known.has(named) ? named : fallback;
}

// ── GitHub sharing (githubRoutingPermissions.ts) ───────────────────────────

export type RoutingPermission = 'off' | 'read' | 'read-write';
/** gitHubRoutingPermissionFor: trust belongs to the saved endpoints; a key nothing stored names, or none at all, is "off". */
export function routingPermissionFor(key: string | null, stored: Record<string, string>): RoutingPermission {
  if (key === null) return 'off';
  const permission = stored[key];
  return permission === 'read' || permission === 'read-write' ? permission : 'off';
}

// ── Routing (pullRequestRouting.ts) ────────────────────────────────────────

export const PR_ROUTED_READS: ReadonlySet<string> = new Set([
  'pullRequests.summary', 'pullRequests.stack', 'pullRequests.detail', 'pullRequests.preview', 'pullRequests.checks', 'pullRequests.activity',
  'pullRequests.threadComments', 'pullRequests.diffFileContents', 'pullRequests.filesViewed', 'pullRequests.reviewerCandidates', 'pullRequests.labelCandidates',
]);
export const PR_ROUTED_WRITES: ReadonlySet<string> = new Set([
  'pullRequests.setFilesViewed', 'pullRequests.runAction', 'pullRequests.update', 'pullRequests.comment', 'pullRequests.updateComment', 'pullRequests.submitReview',
  'pullRequests.replyToThread', 'pullRequests.setReaction', 'pullRequests.setThreadResolution', 'pullRequests.requestReviewers', 'pullRequests.setLabels',
]);
const INVALIDATE = 'pullRequests.invalidate';
const SET_FILES_VIEWED = 'pullRequests.setFilesViewed';

const trimmedNonEmpty = (value: unknown) => typeof value === 'string' && value.length > 0 && value.trim() === value;
/** Schema.is(PullRequestRef): a project, a repository and a positive number, the optional fields well formed. */
export function isPullRequestRef(payload: unknown): boolean {
  if (payload === null || typeof payload !== 'object' || Array.isArray(payload)) return false;
  const ref = payload as Obj;
  return trimmedNonEmpty(ref.projectId) && trimmedNonEmpty(ref.repository) && typeof ref.number === 'number' && Number.isInteger(ref.number) && ref.number >= 1
    && (ref.host === undefined || trimmedNonEmpty(ref.host)) && (ref.expectedAccountId === undefined || trimmedNonEmpty(ref.expectedAccountId))
    && (ref.allowStale === undefined || typeof ref.allowStale === 'boolean');
}
/** Schema.is(PullRequestInvalidateInput). */
function isInvalidation(payload: unknown): boolean {
  if (payload === null || typeof payload !== 'object' || Array.isArray(payload)) return false;
  const input = payload as Obj;
  return (input.reference === undefined || isPullRequestRef(input.reference)) && (input.filesViewedOnly === undefined || typeof input.filesViewedOnly === 'boolean');
}

/** A remembered routed reference covers a filter naming the same project, repository and number (and host, where the filter names one). */
export function matchesReference(reference: Obj, filter: Obj): boolean {
  return reference.projectId === filter.projectId && str(reference.repository).toLowerCase() === str(filter.repository).toLowerCase() && reference.number === filter.number
    && (filter.host === undefined || str(reference.host).toLowerCase() === str(filter.host).toLowerCase());
}

const UNAVAILABLE_KINDS = new Set(['EnvironmentRpcUnavailableError', 'EnvironmentNotRegisteredError', 'Disconnected']);
/** A guard rejection is returned before the operation starts. Other write failures are ambiguous. */
export function rejectedBeforeDispatch(error: unknown): boolean {
  if (error instanceof ClientError) {
    if (UNAVAILABLE_KINDS.has(error.kind)) return true;
    return error.kind === 'PullRequestOperationError' && error.message.includes('routeIdentity');
  }
  if (typeof error !== 'object' || error === null || !('_tag' in error)) return false;
  const tagged = error as { _tag?: unknown; operation?: unknown };
  return tagged._tag === 'EnvironmentRpcUnavailableError' || tagged._tag === 'EnvironmentNotRegisteredError'
    || (tagged._tag === 'PullRequestOperationError' && tagged.operation === 'routeIdentity');
}

/** One environment as the router sees it. `permission` already says "off" for an SSH environment whose profile no longer matches its stored key. */
export type RoutedEnvironment = { id: string; enabled: boolean; connected: boolean; local: boolean; permission: RoutingPermission; pullRequestChecks?: boolean };

/** Writes route only when both servers are "read-write"; reads when neither is "off". Both must be switched on. */
export function routingAllowed(origin: RoutedEnvironment | undefined, destination: RoutedEnvironment | undefined, write: boolean): boolean {
  if (!origin?.enabled || !destination?.enabled) return false;
  return write ? origin.permission === 'read-write' && destination.permission === 'read-write' : origin.permission !== 'off' && destination.permission !== 'off';
}

export interface PullRequestRouterHost {
  readonly originId: string;
  /** Read again at every decision, so a permission revoked during a probe is seen. */
  environments(): ReadonlyArray<RoutedEnvironment>;
  request(environmentId: string, method: string, payload: Obj, options: { write: boolean; timeoutSeconds?: number }): Promise<Obj>;
}

/** Where a reference was read through: its origin, the reference with the verified host, and the servers that answered. */
export type RoutedRead = { origin: string; reference: Obj; targets: Set<string> };
/**
 * The write leg of `dispatch`: puts the write on one server and answers once it has left, its reply coming back later
 * (a detached write, composer-replies.ts). It throws, before anything leaves, when that server cannot take it now
 * (EnvironmentRpcUnavailableError), which passes the write to the next candidate as a refusal before dispatch does.
 */
export type DetachedSend = (environmentId: string, payload: Obj) => Promise<Obj>;
/** The routed-reads memory, shared by every router over the same connections (the reference keys it by registry). */
export type RoutedReadMemory = Map<string, RoutedRead>;
const MEMORY_LIMIT = 256;
const READ_TIMEOUT = 30, PROBE_TIMEOUT = 2, INVALIDATE_TIMEOUT = 1;
type Identity = { accountId: string; host: string; provider: string; projectTitle?: string; workspaceRoot?: string };

/** Effect.cached: started on first use, answered once. */
function memo(run: () => Promise<Obj>): () => Promise<Obj> {
  let started: Promise<Obj> | null = null;
  return () => (started ??= run());
}
/** A failed probe is an old server or an unknown account: the existing path stays (never an interruption). */
async function orNull<T>(promise: Promise<T>): Promise<T | null> {
  try { return await promise; } catch (error) { if (letGo(error)) throw error; return null; }
}
function identityOf(value: Obj | null): Identity | null {
  if (value === null) return null;
  const accountId = str(value.accountId), host = str(value.host);
  if (!accountId || !host) return null;
  return { accountId, host, provider: str(value.provider), ...(typeof value.projectTitle === 'string' ? { projectTitle: value.projectTitle } : {}),
    ...(typeof value.workspaceRoot === 'string' ? { workspaceRoot: value.workspaceRoot } : {}) };
}

/** Credentials stay on their environments. Only a verified host and account cross the wire. */
export class PullRequestRouter {
  constructor(private readonly used: RoutedReadMemory = new Map()) {}

  /** Forget the routed reads (tests; a connection set that changed wholesale). */
  reset(): void { this.used.clear(); }

  /** readOrWrite: a read with an alternate goes strict, local origin first; summary and detail fall back to the plain read. */
  async request(host: PullRequestRouterHost, method: string, payload: Obj, write: boolean): Promise<Obj> {
    // The method decides where it may go, as the reference's tag does; the caller's flag (a write's
    // uncertainty on a lost reply) travels with every request made for it.
    if (!PR_ROUTED_READS.has(method) || !isPullRequestRef(payload)) return this.routed(host, method, payload, write);
    const origin = host.originId;
    const environments = host.environments();
    if (environments.length < 2) return host.request(origin, method, payload, { write });
    const originEntry = environments.find(entry => entry.id === origin);
    const allowed = environments.some(entry => entry.id !== origin && routingAllowed(originEntry, entry, false));
    if (!allowed) return host.request(origin, method, payload, { write });
    const strictInput = { ...payload, allowStale: false };
    const source = memo(() => host.request(origin, method, strictInput, { write, timeoutSeconds: READ_TIMEOUT }));
    const routed = () => this.routed(host, method, strictInput, write, source);
    try {
      if (originEntry?.local === true) {
        try { return await source(); } catch (error) { if (letGo(error)) throw error; return await routed(); }
      }
      return await routed();
    } catch (error) {
      if (letGo(error)) throw error;
      if (payload.allowStale !== false && (method === 'pullRequests.summary' || method === 'pullRequests.detail')) return host.request(origin, method, payload, { write });
      throw error;
    }
  }

  /**
   * A routed write whose reply comes back later (the Viewed ticks' flush, pages-pr-code.ts: no answer waits on it). It is
   * routed as `request` routes a write (the identity probes, the reference's candidate order, a server that refuses before
   * the write leaves passing it to the next), but `send` puts it on the chosen server and returns at once. The readers'
   * invalidations (the reference's `finish`) follow the host's acceptance: the caller runs `afterWrite` in the answer that
   * sees the reply, because a native call needs an answer to run in and the reply arrives outside this one.
   */
  dispatch(host: PullRequestRouterHost, method: string, payload: Obj, send: DetachedSend): Promise<Obj> {
    return this.routed(host, method, payload, true, undefined, send);
  }

  /** The reference's `finish` after a write the host accepted: every server read through for the pull request is told (`filesViewedOnly` for the Viewed marks). */
  async afterWrite(host: PullRequestRouterHost, method: string, ref: Obj): Promise<void> {
    if (!PR_ROUTED_WRITES.has(method) || this.used.size === 0) return;
    const origin = host.originId;
    const targets = new Map<string, Obj[]>([[origin, [ref]]]);
    for (const entry of this.used.values()) {
      if (entry.origin !== origin || !matchesReference(entry.reference, ref)) continue;
      for (const target of [...entry.targets, origin]) {
        const refs = targets.get(target) ?? [];
        if (!refs.some(existing => existing.host === entry.reference.host)) refs.push(entry.reference);
        targets.set(target, refs);
      }
    }
    const viewedOnly = method === SET_FILES_VIEWED;
    await this.fanOut([...targets].map(([target, refs]) => [target, [...refs.map(reference => ({ reference, ...(viewedOnly ? { filesViewedOnly: true } : {}) })), ...(viewedOnly ? [] : [{}])]] as const), host);
  }

  private allowed(host: PullRequestRouterHost, destination: string, write: boolean): boolean {
    const environments = host.environments();
    return routingAllowed(environments.find(entry => entry.id === host.originId), environments.find(entry => entry.id === destination), write);
  }

  /** invalidateTarget: GitHub already accepted the write; a stalled reader must not delay its confirmation. */
  private async invalidateTarget(host: PullRequestRouterHost, target: string, inputs: ReadonlyArray<Obj>): Promise<void> {
    if (target !== host.originId && !this.allowed(host, target, false)) return;
    for (let index = 0; index < inputs.length; index += 3) {
      await Promise.all(inputs.slice(index, index + 3).map(async input => {
        try { await host.request(target, INVALIDATE, input, { write: false, timeoutSeconds: INVALIDATE_TIMEOUT }); }
        catch (error) { if (letGo(error)) throw error; }
      }));
    }
  }

  private remember(origin: string, refKey: string, reference: Obj, target: string): void {
    const entry = this.used.get(refKey) ?? { origin, reference, targets: new Set<string>() };
    entry.targets.add(target);
    if (this.used.size >= MEMORY_LIMIT && !this.used.has(refKey)) {
      const oldest = this.used.keys().next().value;
      if (oldest !== undefined) this.used.delete(oldest);
    }
    this.used.set(refKey, entry);
  }

  /**
   * routedRequest: the invalidate fan-out, then a read or write through the first allowed server with the same GitHub
   * account. `send` (dispatch) replaces the request's own leg on each server and leaves the fan-out to `afterWrite`.
   */
  private async routed(host: PullRequestRouterHost, method: string, input: Obj, write: boolean, sourceRead?: () => Promise<Obj>, send?: DetachedSend): Promise<Obj> {
    const origin = host.originId;
    const reads = PR_ROUTED_READS.has(method), writes = PR_ROUTED_WRITES.has(method), uncertain = write || writes;
    const leg = (id: string, payload: Obj, options: { write: boolean; timeoutSeconds?: number }) => send ? send(id, payload) : host.request(id, method, payload, options);
    const source = sourceRead ?? memo(() => leg(origin, input, { write: uncertain }));
    if (method === INVALIDATE && isInvalidation(input)) {
      const result = await host.request(origin, method, input, { write });
      const targets = new Map<string, Obj>();
      const filter = input.reference === undefined ? undefined : obj(input.reference);
      for (const entry of this.used.values()) {
        if (entry.origin !== origin) continue;
        if (filter !== undefined && !matchesReference(entry.reference, filter)) continue;
        for (const target of entry.targets) targets.set(target, entry.reference);
        if (filter !== undefined) targets.set(origin, entry.reference);
      }
      await this.fanOut([...targets].map(([target, reference]) => [target, [{ ...input, ...(filter === undefined ? {} : { reference }) }]] as const), host);
      return result;
    }
    if ((!reads && !writes) || !isPullRequestRef(input)) return leg(origin, input, { write });
    const ref = input;
    const refKey = JSON.stringify([origin, str(ref.projectId), typeof ref.host === 'string' ? ref.host.toLowerCase() : null, str(ref.repository).toLowerCase(), String(num(ref.number))]);
    const finish = async (operation: () => Promise<Obj>): Promise<Obj> => {
      const result = await operation();
      if (!send) await this.afterWrite(host, method, ref);
      return result;
    };
    const environments = host.environments();
    const originEntry = environments.find(entry => entry.id === origin);
    const alternatives: { id: string; local: boolean }[] = [];
    for (const entry of environments) {
      if (entry.id === origin || !routingAllowed(originEntry, entry, writes) || !entry.connected) continue;
      if (method === 'pullRequests.checks' && entry.pullRequestChecks !== true) continue;
      alternatives.push({ id: entry.id, local: entry.local });
    }
    if (alternatives.length === 0) return finish(source);
    const identity = identityOf(await orNull(host.request(origin, 'pullRequests.routing', ref, { write: false, timeoutSeconds: PROBE_TIMEOUT })));
    // Old servers and unknown accounts retain the existing path.
    if (identity === null || identity.provider !== 'github') return finish(source);
    const routedInput = { ...input, host: identity.host, expectedAccountId: identity.accountId };
    const guardedSource = sourceRead ?? memo(() => leg(origin, routedInput, { write: uncertain }));
    const sourceLocal = originEntry?.local === true;
    const candidates = [
      ...(sourceLocal && writes ? [origin] : []),
      ...alternatives.filter(entry => entry.local).map(entry => entry.id),
      ...(!sourceLocal && writes ? [origin] : []),
      ...alternatives.filter(entry => !entry.local).map(entry => entry.id),
      ...(reads ? [origin] : []),
    ];
    const run = async (id: string): Promise<Obj> => {
      let result: Obj;
      if (id === origin) result = await guardedSource();
      else {
        if (!host.environments().some(entry => entry.id === id)) throw new ClientError('The environment was removed.', 'EnvironmentRpcUnavailableError');
        result = await leg(id, routedInput, { write: uncertain, ...(reads ? { timeoutSeconds: READ_TIMEOUT } : {}) });
      }
      this.remember(origin, refKey, { ...ref, host: identity.host }, id);
      return result;
    };
    const visit = async (index: number): Promise<Obj> => {
      const id = candidates[index];
      if (id === undefined) return guardedSource();
      if (id !== origin) {
        if (!this.allowed(host, id, writes)) return visit(index + 1);
        // An older server would discard expectedAccountId. Verify it implements the guard first.
        const alternate = identityOf(await orNull(host.request(id, 'pullRequests.routingIdentity', { host: identity.host }, { write: false, timeoutSeconds: PROBE_TIMEOUT })));
        if (alternate === null || alternate.provider !== 'github' || alternate.host.toLowerCase() !== identity.host.toLowerCase() || alternate.accountId !== identity.accountId) return visit(index + 1);
        if (!this.allowed(host, id, writes)) return visit(index + 1);
      }
      let result: Obj;
      try { result = await run(id); }
      catch (error) {
        if (letGo(error)) throw error;
        if ((reads || rejectedBeforeDispatch(error)) && index + 1 < candidates.length) return visit(index + 1);
        throw error;
      }
      return typeof result === 'object' && result !== null && 'projectId' in result
        ? { ...result, projectId: ref.projectId, ...(method === 'pullRequests.detail' ? { projectTitle: identity.projectTitle, workspaceRoot: identity.workspaceRoot } : {}) }
        : result;
    };
    return finish(() => visit(0));
  }

  /** Effect.forEach(targets, …, { concurrency: 4 }). */
  private async fanOut(targets: ReadonlyArray<readonly [string, ReadonlyArray<Obj>]>, host: PullRequestRouterHost): Promise<void> {
    for (let index = 0; index < targets.length; index += 4) {
      await Promise.all(targets.slice(index, index + 4).map(([target, inputs]) => this.invalidateTarget(host, target, inputs)));
    }
  }
}

/** The app's one router: its routed-reads memory outlives any single answer. */
export const pullRequestRouter = new PullRequestRouter();
