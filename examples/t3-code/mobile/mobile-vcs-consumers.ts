// @ref llp/1109.009-mobile-settings.decision.md#offline-cache-storage
// Pinned365aa87982 state/vcs.ts: cached presentation and real settled requests.
import type { T3Client } from './shared/client';
import { str, type Obj } from './shared/domain';
import { letGo, letGoAware } from './shared/let-go';
import { ClientError, type Native } from './shared/protocol';
import { fleet } from './shared/settings-b-fleet';
import { cardPages } from './shared/r4-git-branch';
import { stripPages } from './shared/composer-controls-branch';
import { mobileCacheCatalogCurrent, mobileCacheCatalogIdentity, mobileCacheCatalogPrepare } from './mobile-client-cache-catalog';
import { mobileCacheDisplayRevision } from './mobile-client-cache';
import { mobileVcsCache, mobileVcsRefsCacheEligible, type MobileVcsRefsInput } from './mobile-vcs-cache';

export interface MobileVcsScope {
  owner: object; environmentId: string; saved: () => readonly Obj[]; current: () => boolean;
}
export const MOBILE_VCS_INVALIDATING_METHODS = new Set(['vcs.pull', 'vcs.refreshStatus', 'vcs.createWorktree',
  'vcs.removeWorktree', 'vcs.createRef', 'vcs.switchRef', 'vcs.init']);
const superseded = () => new ClientError('The branch request was superseded.', 'superseded');

/** Public shared request methods dispatch dynamically to the mobile subtype.
 * This captures identity only; Native and callbacks live in the current answer. */
export function mobileVcsFocusedScope(client: T3Client, saved: () => readonly Obj[] = () => fleet.saved): MobileVcsScope {
  const environmentId = client.environmentId, origin = client.origin, generation = client.generation;
  return { owner: client, environmentId, saved,
    current: () => client.environmentId === environmentId && client.origin === origin && client.generation === generation };
}
function capture(scope: MobileVcsScope) {
  const identity = mobileCacheCatalogIdentity(scope.saved(), scope.environmentId);
  return { identity, current: () => scope.current() && identity !== ''
    && mobileCacheCatalogIdentity(scope.saved(), scope.environmentId) === identity
    && mobileCacheCatalogCurrent(scope.owner, scope.saved(), scope.environmentId) };
}

/** Cache is a separate presentation read, never a substitute protocol response. */
export async function mobileVcsCachedRead(scope: MobileVcsScope, nativeInput: Native, input: MobileVcsRefsInput) {
  const native = letGoAware(nativeInput), captured = capture(scope);
  if (!captured.identity || !scope.current() || !mobileVcsRefsCacheEligible(input)) return null;
  await mobileCacheCatalogPrepare(scope.owner, native, scope.saved);
  if (!captured.current()) return null;
  if (!await mobileVcsCache.recover(native, scope.environmentId) || !captured.current()) return null;
  return mobileVcsCache.read({ native, environmentId: scope.environmentId, input,
    catalogIdentity: captured.identity, current: captured.current });
}

/** Observe one actual request. The caller's transport remains its only sender;
 * no cached value can become an RPC reply or confer permissions. */
export async function mobileVcsRequest(scope: MobileVcsScope, nativeInput: Native, method: string,
  input: Obj, send: () => Promise<Obj>): Promise<Obj> {
  const invalidating = MOBILE_VCS_INVALIDATING_METHODS.has(method);
  if (method !== 'vcs.listRefs' && !invalidating) return send();
  const native = letGoAware(nativeInput), captured = capture(scope);
  // No saved identity means no disk authority. Actual transport still decides
  // whether this request is admitted, as it did before cache integration.
  if (!captured.identity) return send();
  if (invalidating) {
    let result: Obj | undefined, failure: unknown, failed = false;
    try { result = await send(); }
    catch (error) { failure = error; failed = true; }
    // Persisted refs may now be stale even if the command returned an error.
    // Do not call native after Exact lets go; recovery precedes the next read.
    if (letGo(failure)) { mobileVcsCache.unreadable(scope.environmentId); throw failure; }
    try { await mobileVcsCache.invalidate(native, scope.environmentId); }
    catch (error) {
      mobileVcsCache.unreadable(scope.environmentId);
      if (letGo(error)) throw error;
    }
    if (failed) throw failure;
    return result!;
  }
  if (!mobileVcsRefsCacheEligible(input as MobileVcsRefsInput) || !str(input.cwd).trim()) return send();
  await mobileCacheCatalogPrepare(scope.owner, native, scope.saved);
  if (!scope.current() || mobileCacheCatalogIdentity(scope.saved(), scope.environmentId) !== captured.identity) throw superseded();
  if (!captured.current()) return send(); // Failed disposable cleanup must not block a live read.
  let result: Obj | undefined, sent = false, liveRevision = '';
  const display = await mobileVcsCache.read({ native, environmentId: scope.environmentId,
    input: input as MobileVcsRefsInput, catalogIdentity: captured.identity, current: captured.current,
    liveRead: async () => { sent = true; liveRevision = mobileVcsCache.revision(scope.environmentId); result = await send(); return result; } });
  if (!sent || !captured.current() || liveRevision !== mobileVcsCache.revision(scope.environmentId)) throw superseded();
  // Preserve every original field, including server additions the durable codec
  // intentionally drops. A malformed cache payload never fabricates a reply.
  void display;
  return result!;
}

/** Invalidate shared lexical readers' existing pager objects through their public
 * accessors. Never seed them with cached rows or fabricate repository status. */
export function mobileVcsRetireFocusedPages(client: T3Client, environmentId: string): void {
  if (client.environmentId !== environmentId) return;
  const page = cardPages(client); if (page) page.stale = true;
  for (const strip of stripPages(client)) { strip.refsQuery = null; }
}

export function mobileVcsDisplayOwner(scope: MobileVcsScope, input: MobileVcsRefsInput): string {
  const captured = capture(scope);
  return captured.current() ? JSON.stringify([captured.identity, input, mobileCacheDisplayRevision(scope.environmentId, 'vcs-refs')]) : '';
}
