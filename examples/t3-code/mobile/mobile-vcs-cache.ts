// Pinned365aa87982 state/vcs.ts and vcsRefInvalidation.ts (MIT).
// @ref llp/1109.009-mobile-settings.decision.md#offline-cache-storage
import { obj, type Obj } from './shared/domain';
import { letGo } from './shared/let-go';
import type { Native } from './shared/protocol';
import { mobileCacheClear, mobileCacheReadDecoded, mobileCacheReadRevision, mobileCacheTicket, mobileCacheWrite } from './mobile-client-cache';

export interface MobileVcsRefsInput extends Obj { cwd: string; limit?: number }
export interface MobileVcsRefsResult extends Obj {
  refs: Obj[]; isRepo: boolean; hasPrimaryRemote: boolean; nextCursor: number | null; totalCount: number;
}
export interface MobileVcsRefsRead {
  native: Native; environmentId: string; input: MobileVcsRefsInput;
  /** Captured environment/origin/generation/route/query ownership, checked after every await. */
  current: () => boolean;
  /** Omit only when disconnected; denial/errors from an actual request never use cache. */
  liveRead?: () => Promise<unknown>;
}
export type MobileVcsRefsDisplay = { source: 'cache' | 'live'; refs: MobileVcsRefsResult };
const natural = (value: unknown): value is number => Number.isSafeInteger(value) && Number(value) >= 0;
const nonempty = (value: unknown): value is string => typeof value === 'string' && value.trim().length > 0;
export function mobileVcsRefsCacheEligible(input: MobileVcsRefsInput): boolean {
  return input.limit === 100 && input.query === undefined && input.cursor === undefined
    && input.includeMatchingRemoteRefs === undefined && input.refKind === undefined;
}
/** Source VcsListRefsResult, including required occupation/default/remote fields.
 * Source TrimmedNonEmptyString decodes and encodes by trimming; nonblank raw
 * values are accepted and normalized. Sparse UI fixtures are not durable wire
 * results. Extra fields are discarded. */
export function decodeMobileVcsRefs(value: unknown): MobileVcsRefsResult | null {
  const data = obj(value);
  if (!Array.isArray(data.refs) || typeof data.isRepo !== 'boolean' || typeof data.hasPrimaryRemote !== 'boolean'
    || !natural(data.totalCount) || data.nextCursor !== null && !natural(data.nextCursor)) return null;
  const refs: Obj[] = [];
  for (const raw of data.refs) {
    const ref = obj(raw);
    if (!nonempty(ref.name) || typeof ref.current !== 'boolean' || typeof ref.isDefault !== 'boolean'
      || ref.worktreePath !== null && !nonempty(ref.worktreePath)
      || ref.isRemote !== undefined && typeof ref.isRemote !== 'boolean'
      || ref.remoteName !== undefined && !nonempty(ref.remoteName)) return null;
    refs.push({ name: ref.name.trim(), current: ref.current, isDefault: ref.isDefault,
      worktreePath: typeof ref.worktreePath === 'string' ? ref.worktreePath.trim() : null,
      ...(ref.isRemote === undefined ? {} : { isRemote: ref.isRemote }),
      ...(ref.remoteName === undefined ? {} : { remoteName: typeof ref.remoteName === 'string' ? ref.remoteName.trim() : ref.remoteName }) });
  }
  return { refs, isRepo: data.isRepo, hasPrimaryRemote: data.hasPrimaryRemote, nextCursor: data.nextCursor, totalCount: data.totalCount };
}
export function decodeMobileVcsRefsCache(payload: string, environmentId: string, cwd: string): MobileVcsRefsResult | null {
  try {
    const value = obj(JSON.parse(payload));
    return value.schemaVersion === 1 && value.environmentId === environmentId && value.cwd === cwd
      ? decodeMobileVcsRefs(value.refs) : null;
  } catch { return null; }
}
interface State { revision: number; readable: boolean; clearing: number }
/** Only plain invalidation state survives an answer. Native tickets serialize
 * writes against deletion; this owner never retains a native handle or promise. */
export class MobileVcsCache {
  private readonly states = new Map<string, State>();
  private state(environmentId: string): State {
    let state = this.states.get(environmentId);
    if (!state) { state = { revision: 0, readable: true, clearing: 0 }; this.states.set(environmentId, state); }
    return state;
  }
  /** Call after any settled ref/worktree mutation, including uncertain failure.
   * Source invalidation is environment-wide because repositories share refs. */
  async invalidate(native: Native, environmentId: string): Promise<boolean> {
    const state = this.state(environmentId), revision = ++state.revision;
    state.readable = false; state.clearing++;
    try {
      await mobileCacheClear(native, { environmentId, kind: 'vcs-refs' });
      if (state.revision === revision) state.readable = true;
      return true;
    } catch (error) { if (letGo(error)) throw error; return false; }
    finally { state.clearing--; }
  }
  async read(options: MobileVcsRefsRead): Promise<MobileVcsRefsDisplay | null> {
    const { native, environmentId, current, liveRead } = options;
    // Copy before awaiting: a query model may be edited while the request runs.
    const input = { ...options.input }, cwd = input.cwd, state = this.state(environmentId);
    const revision = state.revision;
    let clearRevision = mobileCacheReadRevision(environmentId);
    const owned = () => current() && state.revision === revision && mobileCacheReadRevision(environmentId) === clearRevision;
    if (!environmentId || !nonempty(cwd) || !owned()) return null;
    const eligible = mobileVcsRefsCacheEligible(input);
    // Source StoredVcsRefs.cwd is Schema.String and saveVcsRefs uses the original
    // input.cwd as its cache key, even though RPC input encoding trims that path.
    const key = { environmentId, kind: 'vcs-refs' as const, key: cwd };
    if (!liveRead) {
      if (!eligible || !state.readable || state.clearing) return null;
      try {
        const readable = () => owned() && state.readable && !state.clearing;
        const refs = await mobileCacheReadDecoded(native, key, payload => decodeMobileVcsRefsCache(payload, environmentId, cwd), readable);
        if (!readable()) return null;
        return refs ? { source: 'cache', refs } : null;
      } catch (error) { if (letGo(error)) throw error; return null; }
    }
    // Recovery happens before capturing the request's ticket, never after its
    // live result: clearing must not give a stale result a fresh write ticket.
    if (eligible && !state.readable && !state.clearing) {
      state.clearing++;
      try {
        const clearing = mobileCacheClear(native, { environmentId, kind: 'vcs-refs' });
        clearRevision = mobileCacheReadRevision(environmentId);
        await clearing;
        if (owned()) state.readable = true;
      } catch (error) { if (letGo(error)) throw error; }
      finally { state.clearing--; }
    }
    if (!owned()) return null;
    let ticket: string | null = null;
    if (eligible && state.readable && !state.clearing) {
      try { ticket = await mobileCacheTicket(native, key); }
      catch (error) { if (letGo(error)) throw error; }
    }
    if (!owned()) return null;
    const value = await liveRead(); // Never swallow a server denial as a cache miss.
    if (!owned()) return null;
    const refs = decodeMobileVcsRefs(value);
    if (!refs) return null;
    if (ticket && state.readable && !state.clearing) {
      const payload = JSON.stringify({ schemaVersion: 1, environmentId, cwd, refs });
      try { await mobileCacheWrite(native, key, ticket, payload); }
      catch (error) { if (letGo(error)) throw error; }
    }
    return owned() ? { source: 'live', refs } : null;
  }
}
export const mobileVcsCache = new MobileVcsCache();
