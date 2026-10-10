// @ref llp/1109.009-mobile-settings.decision.md#offline-cache-storage
import type { T3Client } from './shared/client';
import { initialShell } from './shared/domain';
import type { Native } from './shared/protocol';
import { letGoAware } from './shared/let-go';
import { mobileCacheClear } from './mobile-client-cache';
import { mobileCacheDiscardDisplay, mobileCacheAdoptThreadPresentation } from './mobile-client-cache-sync';
import { MobileFaviconCache } from './mobile-favicon-cache';
import { withMobileFaviconIO } from './mobile-favicon-io';

/** One app-owned image memory store; invocation-bound IO never lives here. */
export const mobileFaviconCache = new MobileFaviconCache();

/** The source clears icon memory/persistence before the other disposable data.
 * Connected live content remains usable. Offline content from a formerly live
 * session is disposable too; clearing it must agree with a cold restored session.
 * Selected IDs, drafts, saved connections and all preferences remain untouched. */
export async function mobileClearClientCaches(client: T3Client, nativeInput: Native, environmentId?: string): Promise<void> {
  const native = letGoAware(nativeInput);
  const admittedEnvironment = client.environmentId, admittedGeneration = client.generation;
  const icons = await withMobileFaviconIO(native, io => mobileFaviconCache.clear(io, environmentId));
  if (!icons) throw new Error('Could not clear cached project icons.');
  await mobileCacheClear(native, environmentId === undefined ? {} : { environmentId });
  if (client.connection !== 'connected' && client.environmentId === admittedEnvironment && client.generation === admittedGeneration
    && (environmentId === undefined || client.environmentId === environmentId)) {
    mobileCacheDiscardDisplay(client);
    client.shell = initialShell(); client.shellLoaded = false; client.config = {}; client.thread = null;
    client.shellLive = false; client.configLive = false; client.threadLive = false;
    mobileCacheAdoptThreadPresentation(client, null);
    client.revision++;
  }
}
