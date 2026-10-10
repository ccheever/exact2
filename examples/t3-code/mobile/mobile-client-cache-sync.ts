// @ref llp/1109.009-mobile-settings.decision.md#offline-cache-storage
import type { T3Client } from './shared/client';
import { arr, initialShell, obj, str, type Obj, type Shell, type ThreadState } from './shared/domain';
import type { Native } from './shared/protocol';
import { applyStaged } from './shared/composer-controls';
import { decodeMobileCatalogPayload, encodeMobileCatalogPayload, mobileCacheCatalogCurrent, mobileCacheCatalogIdentity as savedIdentity, mobileCacheCatalogLive, mobileCacheCatalogPrepare } from './mobile-client-cache-catalog';
import { letGo } from './shared/let-go';
import { mobileCacheFlushDeletes, mobileCacheObserveThread, mobileCacheThreadDeleted } from './mobile-client-cache-lifecycle';
import { mobileCachePersistSnapshot, mobileCacheReadDecoded, mobileCacheReadRevision, mobileCacheTicket, mobileCacheWrite, type MobileCacheKey } from './mobile-client-cache';
import { encodeMobileShellCache, decodeMobileShellCache, encodeMobileThreadCache, decodeMobileThreadCache,
  encodeMobileConfigCache, decodeMobileConfigCache } from './mobile-client-cache-codec';

interface Display { environmentId: string; catalogIdentity: string; clearRevision: string; shell?: Shell; thread?: ThreadState; config?: Obj }
const displays = new WeakMap<T3Client, Display>();
const serials = new WeakMap<T3Client, number>();
/** App-owned display adaptation of shared updateSelectionFromThread. The shared
 * next-turn owner still supplies staged-choice precedence; no RPC is issued. */
export function mobileCacheAdoptThreadPresentation(client: T3Client, state: ThreadState | null): void {
  const thread = obj(state?.projection.thread), selection = obj(thread.modelSelection);
  client.providerId = str(selection.instanceId); client.modelId = str(selection.model);
  client.modelOptions = arr(selection.options);
  client.runtimeMode = str(thread.runtimeMode, 'approval-required');
  client.interactionMode = str(thread.interactionMode, 'default');
  client.projectId = str(thread.projectId, client.projectId);
  applyStaged(client);
}
/** Cached objects can be displayed while offline, but can never seed the shared
 * synchronization/resume path. Only objects this adapter installed are removed. */
export function mobileCacheDiscardDisplay(client: T3Client): void {
  const display = displays.get(client);
  if (!display) return;
  if (display.shell && client.shell === display.shell) { client.shell = initialShell(); client.shellLoaded = false; }
  if (display.thread && client.thread === display.thread) { client.thread = null; mobileCacheAdoptThreadPresentation(client, null); }
  if (display.config && client.config === display.config) client.config = {};
  displays.delete(client);
}
export function mobileCacheBeforeStatus(client: T3Client, value: Obj, generation: number): void {
  if (generation < client.generation) return;
  if (!['disconnected', 'connecting', 'connected', 'reconnecting', 'error'].includes(str(value.state))
    || typeof value.origin !== 'string' || typeof value.environmentId !== 'string' || typeof value.message !== 'string') return;
  const display = displays.get(client);
  if (display && (value.state === 'connected' || generation !== client.generation
    || str(value.environmentId) && value.environmentId !== display.environmentId
    || mobileCacheReadRevision(display.environmentId) !== display.clearRevision)) mobileCacheDiscardDisplay(client);
}

/** One awaited refresh boundary, after shared reducers have committed. This
 * first slice persists only the focused environment's active shell/config/thread.
 * Background fleet, VCS and favicon producers are separate follow-up consumers. */
export async function mobileCacheSync(client: T3Client, native: Native | null | undefined, saved: () => readonly Obj[]): Promise<void> {
  const serial = (serials.get(client) ?? 0) + 1; serials.set(client, serial);
  mobileCacheCatalogCurrent(client, saved(), client.environmentId);
  const prior = displays.get(client);
  if (prior && (!mobileCacheCatalogCurrent(client, saved(), prior.environmentId)
    || savedIdentity(saved(), prior.environmentId) !== prior.catalogIdentity
    || mobileCacheReadRevision(prior.environmentId) !== prior.clearRevision)) {
    mobileCacheDiscardDisplay(client); client.revision++;
  }
  mobileCacheObserveThread(client);
  await mobileCacheCatalogPrepare(client, native, saved);
  if (!native?.available) return;
  await mobileCacheFlushDeletes(client, native);
  const catalogIdentity = savedIdentity(saved(), client.environmentId);
  if (serials.get(client) !== serial || !client.environmentId || !catalogIdentity
    || !mobileCacheCatalogCurrent(client, saved(), client.environmentId)) return;
  const liveSnapshot = mobileCacheCatalogLive(client, saved(), client.environmentId, 'focused', client.generation,
    [client.shell, client.config, ...(client.thread ? [client.thread] : [])]);
  const environmentId = client.environmentId, origin = client.origin, generation = client.generation,
    threadId = client.threadId, threadEpoch = client.threadEpoch, revision = client.revision,
    clearRevision = mobileCacheReadRevision(environmentId);
  const current = () => serials.get(client) === serial && client.environmentId === environmentId && client.origin === origin
    && client.generation === generation && client.threadId === threadId && client.threadEpoch === threadEpoch
    && client.revision === revision && mobileCacheReadRevision(environmentId) === clearRevision
    && savedIdentity(saved(), environmentId) === catalogIdentity
    && mobileCacheCatalogCurrent(client, saved(), environmentId);
  const key = (kind: MobileCacheKey['kind'], cacheKey: string): MobileCacheKey => ({ environmentId, kind, key: cacheKey });
  const persist = async (cacheKey: MobileCacheKey, live: () => boolean, snapshot: () => object, encode: () => string) => {
    if (!current() || !live()) return;
    const ticket = await mobileCacheTicket(native, cacheKey);
    if (!current() || !live()) return;
    const captured = snapshot();
    await mobileCachePersistSnapshot(client, cacheKey, captured, ticket, catalogIdentity, generation, () => {
      const payload = encodeMobileCatalogPayload(catalogIdentity, encode()); // immutable before any later await
      return mobileCacheWrite(native, cacheKey, ticket, payload);
    }, () => current() && live() && snapshot() === captured);
  };
  try {
    if (client.connection === 'connected') {
      await persist(key('shell', 'snapshot'), () => client.shellLive && client.shellLoaded && liveSnapshot(client.shell), () => client.shell, () => encodeMobileShellCache(environmentId, client.shell));
      await persist(key('server-config', 'config'), () => client.configLive && liveSnapshot(client.config), () => client.config, () => encodeMobileConfigCache(environmentId, client.config));
      if (threadId) await persist(key('thread', threadId), () => client.threadLive && !!client.thread && liveSnapshot(client.thread) && obj(client.thread.projection.thread).id === threadId
        && !str(obj(client.thread.projection.thread).deletedAt) && !mobileCacheThreadDeleted(client, environmentId, threadId),
        () => client.thread!, () => encodeMobileThreadCache(environmentId, threadId, client.thread!));
      return;
    }
    // Connected permission/stream failures do not become cached RPC success.
    const display: Display = displays.get(client) ?? { environmentId, catalogIdentity, clearRevision };
    let shell: Shell | null = null, config: Obj | null = null, thread: ThreadState | null = null;
    if (!client.shellLive && !client.shellLoaded) {
      shell = await mobileCacheReadDecoded(native, key('shell', 'snapshot'), payload => decodeMobileShellCache(decodeMobileCatalogPayload(payload, catalogIdentity) ?? '', environmentId), current);
      if (!current() || client.connection === 'connected') return;
    }
    if (!client.configLive && !Object.keys(client.config).length) {
      config = await mobileCacheReadDecoded(native, key('server-config', 'config'), payload => decodeMobileConfigCache(decodeMobileCatalogPayload(payload, catalogIdentity) ?? '', environmentId), current);
      if (!current() || client.connection === 'connected') return;
    }
    if (threadId && !client.threadLive && !client.thread && !mobileCacheThreadDeleted(client, environmentId, threadId) && (shell ?? client.shell).threads.some(row => row.id === threadId)) {
      thread = await mobileCacheReadDecoded(native, key('thread', threadId), payload => decodeMobileThreadCache(decodeMobileCatalogPayload(payload, catalogIdentity) ?? '', environmentId, threadId), current);
      if (!current() || client.connection === 'connected') return;
    }
    if (!current() || client.connection === 'connected') return;
    let changed = false;
    // Commit the complete display adoption together; never leave an untagged
    // partial shell behind if a later await loses its owner.
    if (shell && !client.shellLive && !client.shellLoaded) { client.shell = display.shell = shell; client.shellLoaded = true; changed = true; }
    if (config && !client.configLive && !Object.keys(client.config).length) { client.config = display.config = config; changed = true; }
    if (thread && !client.threadLive && !client.thread) {
      client.thread = display.thread = thread; mobileCacheAdoptThreadPresentation(client, thread); changed = true;
    }
    if (changed) { displays.set(client, display); client.revision++; }

  } catch (error) {
    if (letGo(error)) throw error;
    // A disposable cache cannot turn an otherwise successful live read into an
    // app error. The storage screen reports store errors through its own read.
  }
}
