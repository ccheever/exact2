// Pinned365aa87982 use-composer-command-menu.ts, state/queries.ts and runtime.ts.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import type { T3Client } from './shared/client';
import { arr, obj, str, type Obj } from './shared/domain';
import { ClientError, type Native } from './shared/protocol';
import { letGo } from './shared/let-go';
import { fleet } from './shared/settings-b-fleet';
import { mobileCacheCatalogIdentity } from './mobile-client-cache-catalog';
import { mobileSessionGrants } from './mobile-grants';
import { hasCompleteProviderWorkspaceSnapshot, type ServerProvider, type ComposerTrigger,
  type ComposerPullRequestEntry } from './composer-command-model';

export type ComposerQueryLane = 'path' | 'pullRequests' | 'discovery';
export interface ComposerQueryInput {
  owner: string; editorId: string; routeVisit: string; renderEpoch: string; mountId: string;
  documentRevision: number; prompt: string; active: boolean;
  environmentId: string; cwd: string; projectId: string; repository: string;
  provider: (ServerProvider & { instanceId: string }) | null; trigger: ComposerTrigger | null;
  /** Changes only when an observed session/catalog invalidation occurs, not each render. */
  permissionRevision: string; session?: Obj;
}
export interface ComposerQueryDue { key: string; dueAt: number; delayMs: number }
export interface ComposerQueryDemand {
  revision: number; admission: string; path: ComposerQueryDue; pullRequests: ComposerQueryDue; discovery: ComposerQueryDue;
}
export interface ComposerPathEntry { path: string; kind: 'file' | 'directory' }
export interface ComposerQuerySnapshot {
  revision: number; provider: ComposerQueryInput['provider']; pathEntries: ComposerPathEntry[];
  pullRequests: ComposerPullRequestEntry[]; pathPending: boolean; pullRequestsPending: boolean; pullRequestsError: string;
}
type Entry = { data: Obj | null; error: string; settled: number; idle: number | null; attempted: string; pending: number };
type Lane = { target: string; selected: string; dueAt: number; delayMs: number; key: string; mount: string };
type Discovery = { scope: string; provider: ComposerQueryInput['provider']; providerKey: string;
  complete: boolean; observedComplete: boolean; pending: number; retryAt: number; attemptedWake: string; wake: string; timer: boolean; key: string };
type State = { input: ComposerQueryInput; admission: string; scope: string; permission: string; revision: number; serial: number;
  path: Lane; pullRequests: Lane; cache: Map<string, Entry>; discovery: Discovery; allowed: boolean; config: string };
const owners = new WeakMap<T3Client, State>();
const emptyDue = (): ComposerQueryDue => ({ key: '', dueAt: 0, delayMs: 0 });
const lane = (): Lane => ({ target: '', selected: '', dueAt: 0, delayMs: 0, key: '', mount: '' });
const discovery = (): Discovery => ({ scope: '', provider: null, providerKey: '', complete: false, observedComplete: false,
  pending: 0, retryAt: 0, attemptedWake: '', wake: '', timer: false, key: '' });
const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value)) as T;
/** The command menu owns skills/commands, not the server's model catalog.
 * Keep complete consumed fields and absence semantics before any copy/signature.
 * Model selection, provider availability and send admission still use client.config. */
export function mobileComposerProvider(value: unknown): ComposerQueryInput['provider'] {
  if (value === null || value === undefined) return null;
  const pick = (input: unknown, keys: readonly string[]): Obj => {
    const row = obj(input);
    return Object.fromEntries(keys.filter(key => Object.hasOwn(row, key)).map(key => [key, row[key]]));
  };
  const skills = (input: unknown) => arr(input).map(row => pick(row,
    ['name', 'enabled', 'userInvocable', 'displayName', 'shortDescription', 'description', 'path', 'scope']));
  const commands = (input: unknown) => arr(input).map(row => pick(row, ['name', 'description']));
  const source = obj(value), projected = pick(source, ['instanceId', 'driver', 'showInteractionModeToggle', 'workspaceSnapshots']);
  projected.skills = skills(source.skills); projected.slashCommands = commands(source.slashCommands);
  if (Array.isArray(source.workspaceSnapshots)) projected.workspaceSnapshots = source.workspaceSnapshots.map(input => {
    const snapshot = obj(input), result = pick(snapshot, ['cwd', 'skills', 'slashCommands', 'slashCommandsPending']);
    if (Array.isArray(snapshot.skills)) result.skills = skills(snapshot.skills);
    if (Array.isArray(snapshot.slashCommands)) result.slashCommands = commands(snapshot.slashCommands);
    return result;
  });
  return clone(projected) as unknown as ComposerQueryInput['provider'];
}
const denied = () => new ClientError('The composer query was superseded.', 'superseded');
const errorText = (error: unknown) => error instanceof Error ? error.message : 'The query failed.';
function catalog(client: T3Client, input: ComposerQueryInput): string {
  return mobileCacheCatalogIdentity(fleet.saved, input.environmentId);
}
function connection(client: T3Client, input: ComposerQueryInput): string {
  return JSON.stringify([catalog(client, input), client.origin, client.environmentId, client.generation,
    client.connection, client.projectId, client.threadId, input.environmentId, input.cwd, input.projectId]);
}
function providerConfig(client: T3Client, input: ComposerQueryInput): string {
  return JSON.stringify(mobileComposerProvider(arr(client.config.providers).find(p => p.instanceId === input.provider?.instanceId)));
}
function available(client: T3Client, state: State): boolean {
  return state.input.active && !!catalog(client, state.input) && client.connection === 'connected'
    && client.environmentId === state.input.environmentId && connection(client, state.input) === state.scope
    && providerConfig(client, state.input) === state.config;
}
function cached(state: State, key: string): Entry {
  let entry = state.cache.get(key);
  if (!entry) { entry = { data: null, error: '', settled: 0, idle: null, attempted: '', pending: 0 }; state.cache.set(key, entry); }
  return entry;
}
function cacheKey(state: State, kind: string, payload: Obj): string { return JSON.stringify([state.scope, state.permission, kind, payload]); }
function select(state: State, target: Lane, value: string, now: number, delay: number, initial: boolean) {
  if (target.target === value) return;
  target.target = value; target.key = value ? String(++state.serial) : ''; target.delayMs = initial ? 0 : delay;
  target.dueAt = now + target.delayMs; target.mount = target.key;
}
function settleTarget(target: Lane, now: number) { if (now >= target.dueAt) target.selected = target.target; }
function prPayload(state: State): Obj {
  const query = state.input.trigger?.kind === 'pull-request' ? state.input.trigger.query : '';
  return { projectId: state.input.projectId, state: 'all', limit: 200, ...(!/^\d*$/.test(query) && query ? { query } : {}) };
}
function prKeys(state: State): { list: string; detail: string; number: number | null } {
  const list = cacheKey(state, 'list', prPayload(state)), query = state.input.trigger?.query ?? '';
  const number = /^\d+$/.test(query) && Number.isSafeInteger(Number(query)) && Number(query) > 0 ? Number(query) : null;
  const exact = arr(state.cache.get(list)?.data?.entries).some(p => p.number === number
    && str(p.repository).toLowerCase() === state.input.repository.toLowerCase());
  return { list, number, detail: number !== null && !exact ? cacheKey(state, 'detail', {
    projectId: state.input.projectId, repository: state.input.repository, number }) : '' };
}
function usedKeys(state: State): string[] {
  const keys: string[] = [];
  if (state.path.selected) keys.push(state.path.selected);
  if (state.pullRequests.selected && state.pullRequests.selected === state.pullRequests.target) {
    const pr = prKeys(state); keys.push(pr.list); if (pr.detail) keys.push(pr.detail);
  }
  return keys;
}
function idle(state: State, now: number) {
  const used = new Set(state.input.active ? usedKeys(state) : []);
  for (const [key, entry] of state.cache) {
    if (entry.idle !== null && now - entry.idle >= 300_000) { state.cache.delete(key); continue; }
    if (used.has(key)) entry.idle = null;
    else if (entry.idle === null) entry.idle = now;
    if (entry.idle !== null && now - entry.idle >= 300_000) state.cache.delete(key);
  }
}
function due(state: State, name: 'path' | 'pullRequests'): ComposerQueryDue {
  const item = state[name]; return item.key ? { key: item.key, dueAt: item.dueAt, delayMs: item.delayMs } : emptyDue();
}
/** Initial hook values are already debounced upstream. Later changes wait180/200ms.
 * Admission is synchronous; callers must run it on every observed owner/document/session change. */
export function mobileComposerQueryDemand(client: T3Client, input: ComposerQueryInput, now: number): ComposerQueryDemand {
  input = { ...input, provider: mobileComposerProvider(input.provider) };
  let state = owners.get(client);
  const scope = connection(client, input), permission = JSON.stringify([input.permissionRevision, input.session ?? null]);
  const identity = JSON.stringify([input.owner, input.editorId, input.routeVisit, input.renderEpoch, input.mountId]);
  const oldIdentity = state && JSON.stringify([state.input.owner, state.input.editorId, state.input.routeVisit, state.input.renderEpoch, state.input.mountId]);
  const initial = !state || identity !== oldIdentity || !state.input.active && input.active;
  const priorPrQuery = state?.input.trigger?.kind === 'pull-request' ? state.input.trigger.query : null;
  const priorPrDueAt = state?.pullRequests.dueAt ?? now;
  if (!state) {
    state = { input: clone(input), admission: '', scope, permission, revision: 0, serial: 0, path: lane(), pullRequests: lane(),
      cache: new Map(), discovery: discovery(), allowed: false, config: providerConfig(client, input) };
    owners.set(client, state);
  }
  const permissionChanged = permission !== state.permission;
  const reset = scope !== state.scope || permissionChanged;
  const admission = JSON.stringify([identity, scope, permission, input.documentRevision, input.prompt, input.trigger, input.provider, input.active]);
  if (state.admission !== admission) {
    state.admission = admission; state.revision++;
    for (const entry of state.cache.values()) if (entry.pending) { entry.pending = 0; entry.attempted = ''; }
    if (state.discovery.pending) { state.discovery.pending = 0; state.discovery.attemptedWake = ''; }
  }
  if (reset) { state.cache.clear(); state.path = lane(); state.pullRequests = lane(); state.allowed = false; state.discovery = discovery(); }
  if (initial) { state.path = lane(); state.pullRequests = lane(); state.discovery = discovery(); }
  state.input = clone(input); state.scope = scope; state.permission = permission; state.config = providerConfig(client, input);
  if (input.session !== undefined) state.allowed = mobileSessionGrants(input.session, 'filesystem:read');
  const live = available(client, state);
  if (!catalog(client, input) || client.connection !== 'connected' || client.environmentId !== input.environmentId) { state.cache.clear(); state.allowed = false; }
  const pathQuery = input.trigger?.kind === 'path' ? input.trigger.query.trim() : '';
  const path = live && input.cwd && pathQuery ? cacheKey(state, 'path', { cwd: input.cwd, query: pathQuery, limit: 20 }) : '';
  select(state, state.path, path, now, 200, initial || permissionChanged);
  const pr = live && input.trigger?.kind === 'pull-request' && input.projectId && input.repository
    ? JSON.stringify([scope, input.projectId, input.repository, input.trigger.query]) : '';
  select(state, state.pullRequests, pr, now, 180, initial);
  // PR debounce watches only query; changing project/environment must not restart it.
  if (reset && !initial && pr && priorPrQuery === input.trigger?.query) {
    state.pullRequests.dueAt = Math.max(now, priorPrDueAt);
    state.pullRequests.delayMs = priorPrDueAt > now ? 180 : 0;
  }
  settleTarget(state.path, now); settleTarget(state.pullRequests, now);
  const d = state.discovery, providerKey = JSON.stringify(input.provider);
  const discoveryScope = live && input.provider?.instanceId && input.cwd ? JSON.stringify([scope, input.provider.instanceId, input.cwd]) : '';
  const complete = hasCompleteProviderWorkspaceSnapshot(input.provider, input.cwd);
  if (d.scope !== discoveryScope || d.observedComplete && !complete) Object.assign(d, discovery(), { scope: discoveryScope });
  if (d.providerKey !== providerKey) { d.provider = clone(input.provider); d.providerKey = providerKey; d.complete = complete; }
  d.observedComplete = complete; if (complete) d.complete = true;
  d.wake = JSON.stringify([input.prompt, providerKey]);
  d.timer = !!d.provider?.workspaceSnapshots?.some(s => s.cwd === input.cwd && s.slashCommandsPending === true);
  const mayRetry = !d.retryAt || d.timer || now >= d.retryAt && d.wake !== d.attemptedWake;
  if (d.scope && !d.complete && !d.pending && mayRetry) d.key = JSON.stringify([d.scope, d.retryAt]); else d.key = '';
  idle(state, now);
  return { revision: state.revision, admission, path: due(state, 'path'), pullRequests: due(state, 'pullRequests'),
    discovery: d.key ? { key: d.key, dueAt: d.retryAt || now, delayMs: d.retryAt > now ? 10_000 : 0 } : emptyDue() };
}
function current(client: T3Client, state: State, admission: string): boolean {
  return owners.get(client) === state && state.admission === admission && available(client, state);
}
function fresh(entry: Entry, now: number, stale: number) { return (entry.data !== null || !!entry.error) && now - entry.settled < stale; }
function pending(state: State, key: string, mount: string, now: number, stale: number): boolean {
  const entry = state.cache.get(key); return !entry || !!entry.pending || entry.attempted !== mount && !fresh(entry, now, stale);
}
function pullRequests(state: State): ComposerPullRequestEntry[] {
  const keys = prKeys(state), detail = keys.detail ? state.cache.get(keys.detail)?.data : null;
  const query = state.input.trigger?.query ?? '', numeric = /^\d*$/.test(query), words = query.toLowerCase().split(/\s+/).filter(Boolean);
  const repo = numeric ? state.input.repository.trim().toLowerCase() : state.input.repository.toLowerCase();
  const rows = [...(detail ? [detail] : []), ...arr(state.cache.get(keys.list)?.data?.entries)].filter(row =>
    row.projectId === state.input.projectId && (numeric ? str(row.repository).trim() : str(row.repository)).toLowerCase() === repo
    && (numeric ? String(row.number).includes(query) : words.every(word => `${str(row.title)} ${str(row.headBranch)} ${str(row.baseBranch)}`.toLowerCase().includes(word))));
  const unique = new Map<number, Obj>(); for (const row of rows) if (!unique.has(Number(row.number))) unique.set(Number(row.number), row);
  return [...unique.values()].sort((a, b) => (numeric ? Number(String(b.number) === query) - Number(String(a.number) === query) : 0)
    || str(b.updatedAt).localeCompare(str(a.updatedAt))).slice(0, 20).map(row => clone(row) as unknown as ComposerPullRequestEntry);
}
export function mobileComposerQuerySnapshot(client: T3Client, admission: string, now: number): ComposerQuerySnapshot {
  const state = owners.get(client), empty: ComposerQuerySnapshot = { revision: state?.revision ?? 0, provider: null,
    pathEntries: [], pullRequests: [], pathPending: false, pullRequestsPending: false, pullRequestsError: '' };
  if (!state || !current(client, state, admission)) return empty;
  const pr = prKeys(state), prReady = !!state.pullRequests.target && state.pullRequests.selected === state.pullRequests.target;
  const list = state.cache.get(pr.list), detail = state.cache.get(pr.detail);
  const unavailable = state.input.trigger?.kind === 'pull-request' && (!state.input.projectId || !state.input.repository);
  return { revision: state.revision, provider: clone(state.discovery.provider ?? state.input.provider),
    pathEntries: state.allowed ? arr(state.cache.get(state.path.selected)?.data?.entries).filter(e => e.kind === 'file' || e.kind === 'directory')
      .map(e => ({ path: str(e.path), kind: e.kind as 'file' | 'directory' })) : [],
    pullRequests: prReady ? pullRequests(state) : [],
    pathPending: state.path.target !== state.path.selected || !!state.path.target && pending(state, state.path.target, state.path.mount, now, 15_000),
    pullRequestsPending: !!state.pullRequests.target && (!prReady || pending(state, pr.list, state.pullRequests.mount, now, 30_000)
      || !!pr.detail && pending(state, pr.detail, state.pullRequests.mount, now, 60_000)),
    pullRequestsError: unavailable ? 'Pull requests are unavailable for this project.' : prReady
      ? list?.error || str(arr(list?.data?.errors)[0]?.message) || detail?.error || '' : '' };
}
/** Native and clock belong only to this invocation. Cancellation never starts cleanup or another request. */
export async function mobileComposerQueryPrepare(client: T3Client, admission: string, laneName: ComposerQueryLane,
  key: string, nativeInput: Native, clock: () => number = Date.now): Promise<{ revision: number }> {
  const state = owners.get(client); if (!state || !current(client, state, admission) || !nativeInput.available) return { revision: state?.revision ?? 0 };
  const item = laneName === 'discovery' ? state.discovery : state[laneName];
  const dueAt = laneName === 'discovery' ? state.discovery.retryAt : state[laneName].dueAt;
  if (!key || item.key !== key || clock() < dueAt) return { revision: state.revision };
  if (laneName !== 'discovery') { settleTarget(state[laneName], clock()); idle(state, clock()); }
  let abandoned: unknown;
  const check = () => { if (abandoned !== undefined) throw abandoned; if (!current(client, state, admission)) throw denied(); };
  const native: Native = { available: nativeInput.available, watch(topic) { check(); nativeInput.watch(topic); }, async later(input) {
    check(); try { const result = await nativeInput.later(input); check(); return result; }
    catch (error) { if (letGo(error)) { abandoned = error; throw new ClientError('Composer query was let go.', 'superseded'); } throw error; }
  } };
  const request = async (method: string, payload: Obj) => { check(); const value = await client.request(native, method, payload); check(); return value; };
  try {
    if (laneName === 'discovery') {
      const d = state.discovery; if (d.pending || d.complete) return { revision: state.revision };
      const ticket = ++state.serial; d.pending = ticket; d.attemptedWake = d.wake; d.key = ''; state.revision++;
      try {
        const result = await request('server.refreshProviders', { instanceId: state.input.provider!.instanceId, cwd: state.input.cwd }); check();
        if (d.pending !== ticket) throw denied();
        const provider = arr(result.providers).find(p => p.instanceId === state.input.provider?.instanceId);
        if (provider) d.provider = mobileComposerProvider(provider);
        d.complete = hasCompleteProviderWorkspaceSnapshot(d.provider, state.input.cwd);
      } catch (error) { if (letGo(error)) throw error; }
      finally { if (d.pending === ticket) { d.pending = 0; d.retryAt = d.complete || abandoned !== undefined ? 0 : clock() + 10_000; d.attemptedWake = d.wake; state.revision++; } }
    } else if (laneName === 'path') {
      const target = state.path, entry = cached(state, target.selected);
      if (entry.pending || entry.attempted === target.mount) return { revision: state.revision };
      if (fresh(entry, clock(), 15_000) && state.allowed) { entry.attempted = target.mount; return { revision: state.revision }; }
      const ticket = ++state.serial; entry.pending = ticket; entry.attempted = target.mount; state.revision++;
      let sessionAuthorized = false;
      try {
        const session = state.input.session ?? await client.http(native, '/api/auth/session'); check();
        if (!mobileSessionGrants(session, 'filesystem:read')) {
          state.allowed = false; for (const [cacheId] of state.cache) if (cacheId !== target.selected && (JSON.parse(cacheId) as unknown[])[2] === 'path') state.cache.delete(cacheId);
          entry.data = null; entry.error = 'This connection cannot search host files.'; return { revision: ++state.revision };
        }
        state.allowed = true; sessionAuthorized = true;
        const payload = obj((JSON.parse(target.selected) as unknown[])[3]);
        const data = await request('projects.searchEntries', payload); check(); if (entry.pending !== ticket) throw denied();
        entry.data = clone(data); entry.error = ''; entry.settled = clock();
      } catch (error) {
        if (letGo(error)) { entry.attempted = ''; throw error; }
        if (!sessionAuthorized) { state.allowed = false; for (const [cacheId, row] of state.cache) if ((JSON.parse(cacheId) as unknown[])[2] === 'path') row.data = null; }
        entry.error = errorText(error); entry.settled = clock();
      }
      finally { if (entry.pending === ticket) { entry.pending = 0; state.revision++; } }
    } else {
      const keys = prKeys(state), mount = state.pullRequests.mount;
      const read = async (cacheId: string, method: string, payload: Obj, stale: number) => {
        const entry = cached(state, cacheId); if (entry.pending || entry.attempted === mount) return;
        if (fresh(entry, clock(), stale)) { entry.attempted = mount; return; }
        const ticket = ++state.serial; entry.pending = ticket; entry.attempted = mount; state.revision++;
        try { const data = await request(method, payload); check(); if (entry.pending !== ticket) throw denied(); entry.data = clone(data); entry.error = ''; entry.settled = clock(); }
        catch (error) { if (letGo(error)) { entry.attempted = ''; throw error; } entry.error = errorText(error); entry.settled = clock(); }
        finally { if (entry.pending === ticket) { entry.pending = 0; state.revision++; } }
      };
      const calls = [read(keys.list, 'pullRequests.list', prPayload(state), 30_000)];
      if (keys.detail) calls.push(read(keys.detail, 'pullRequests.detail', { projectId: state.input.projectId, repository: state.input.repository, number: keys.number }, 60_000));
      const results = await Promise.allSettled(calls); for (const result of results) if (result.status === 'rejected') throw result.reason;
    }
  } catch (error) { if (abandoned !== undefined) throw abandoned; throw error; }
  return { revision: state.revision };
}
