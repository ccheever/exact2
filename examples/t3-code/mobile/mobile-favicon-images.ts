// Pinned365aa87982 ProjectFavicon.tsx/projectFaviconRequests.ts (MIT).
// @ref llp/1109.009-mobile-settings.decision.md#offline-cache-storage
import { mobileCacheReadRevision } from './mobile-client-cache';
import { mobileFaviconMissing, mobileFaviconResourceKey, mobileFaviconRevision,
  type MobileFaviconTarget } from './mobile-favicon-cache';

export interface MobileFaviconImageDemand {
  mountId: string;
  target: MobileFaviconTarget;
  url: string | null;
  /** Catalog identity plus the favicon clear revision, captured at admission. */
  scopeRevision: string;
}
export interface MobileFaviconImageItem {
  mountId: string;
  key: string;
  /** Opaque callback identity. Changes even when a removed URL returns unchanged. */
  requestKey: string;
  url: string;
  loaded: boolean;
  failed: boolean;
}
export interface MobileFaviconImagesView { revision: number; items: MobileFaviconImageItem[] }
interface Mount extends MobileFaviconImageItem {
  environmentId: string;
  scopeRevision: string;
  clearRevision: string;
  cacheKey: string;
  ownerKey: string;
}
interface Active { urls: Map<string, number>; currentUrl: string }
interface Loaded { environmentId: string; scopeRevision: string; clearRevision: string }
const MAX_LOADED = 256;

/** Plain image bookkeeping for projected active-route rows. This deliberately
 * does not claim native virtualized-cell mount/unmount parity. No native handle,
 * callback, timer or promise survives a call. The root reconciles the complete
 * admitted demand set before awaiting URL preparation and after its answer. */
export class MobileFaviconImages {
  private mounts = new Map<string, Mount>();
  private active = new Map<string, Active>();
  private loaded = new Map<string, Loaded>();
  private serial = 0;
  private revision = 0;

  private end(mount: Mount) {
    if (!mount.url) return;
    const active = this.active.get(mount.ownerKey);
    if (!active) return;
    const count = (active.urls.get(mount.url) ?? 1) - 1;
    if (count > 0) { active.urls.set(mount.url, count); return; }
    active.urls.delete(mount.url);
    if (!active.urls.size) this.active.delete(mount.ownerKey);
    else if (active.currentUrl === mount.url) active.currentUrl = [...active.urls.keys()].at(-1)!;
  }

  private begin(mount: Mount) {
    if (!mount.url) return;
    let active = this.active.get(mount.ownerKey);
    if (!active) {
      active = { urls: new Map(), currentUrl: mount.url };
      this.active.set(mount.ownerKey, active);
    }
    const count = active.urls.get(mount.url) ?? 0;
    active.urls.delete(mount.url);
    active.urls.set(mount.url, count + 1);
    active.currentUrl = mount.url;
  }

  /** Clear admission invalidates callbacks even before the root next runs. */
  private retireCleared() {
    let changed = false;
    for (const [id, mount] of this.mounts) {
      if (mount.clearRevision === mobileCacheReadRevision(mount.environmentId, 'project-favicon')) continue;
      this.end(mount); this.mounts.delete(id); changed = true;
    }
    for (const [key, loaded] of this.loaded) {
      if (loaded.clearRevision !== mobileCacheReadRevision(loaded.environmentId, 'project-favicon')) {
        this.loaded.delete(key); changed = true;
      }
    }
    if (changed) this.revision++;
  }

  reconcile(demands: readonly MobileFaviconImageDemand[]): MobileFaviconImagesView {
    this.retireCleared();
    // Duplicate mount identities denote one projected view; the final value wins.
    const desired = new Map(demands.map(demand => [demand.mountId, demand]));
    const next = new Map<string, Mount>();
    let changed = false;
    for (const [id, demand] of desired) {
      const key = mobileFaviconResourceKey(demand.target);
      const url = demand.url && !mobileFaviconMissing(demand.url) ? demand.url : '';
      const cacheKey = !url ? '' : url.startsWith('data:') ? key : mobileFaviconRevision(demand.target, url);
      const clearRevision = mobileCacheReadRevision(demand.target.environmentId, 'project-favicon');
      const ownerKey = JSON.stringify([cacheKey, demand.scopeRevision, clearRevision]);
      const previous = this.mounts.get(id);
      if (previous?.key === key && previous.url === url && previous.ownerKey === ownerKey) {
        next.set(id, previous); continue;
      }
      changed = true;
      // A token rotation preserves component-local status for the same source
      // cache key. A changed icon revision, catalog or clear starts a new view.
      const sameComponent = previous?.ownerKey === ownerKey;
      next.set(id, {
        mountId: id, key, url, cacheKey, ownerKey,
        environmentId: demand.target.environmentId, scopeRevision: demand.scopeRevision, clearRevision,
        requestKey: url ? JSON.stringify([cacheKey, ++this.serial]) : '',
        loaded: !!url && (sameComponent ? previous!.loaded : url.startsWith('data:') || this.loaded.has(ownerKey)),
        failed: !!url && !!sameComponent && previous!.failed,
      });
    }
    // Cleanup precedes admission. Stable requests do not re-begin on clock ticks
    // or list reorder, which would otherwise incorrectly supersede newer URLs.
    for (const [id, mount] of this.mounts) {
      if (next.get(id) !== mount) { this.end(mount); changed = true; }
    }
    for (const [id, mount] of next) if (this.mounts.get(id) !== mount) this.begin(mount);
    const previousOrder = [...this.mounts.keys()];
    if (!changed && [...next.keys()].some((id, index) => id !== previousOrder[index])) changed = true;
    this.mounts = next;
    // A catalog replacement discards old loaded memory for that environment.
    // Other environments and ordinary route departure keep the source LRU.
    const scopes = new Map(demands.map(demand => [demand.target.environmentId, demand.scopeRevision]));
    for (const [key, loaded] of this.loaded) {
      const scope = scopes.get(loaded.environmentId);
      if (scope !== undefined && scope !== loaded.scopeRevision) this.loaded.delete(key);
    }
    if (changed) this.revision++;
    return this.snapshot();
  }

  event(mountId: string, requestKey: string, url: string, event: 'load' | 'error') {
    this.retireCleared();
    const mount = this.mounts.get(mountId);
    if (!mount || !url || mount.requestKey !== requestKey || mount.url !== url
      || this.active.get(mount.ownerKey)?.currentUrl !== url) return { accepted: false, revision: this.revision };
    if (event === 'load') {
      this.loaded.delete(mount.ownerKey);
      this.loaded.set(mount.ownerKey, { environmentId: mount.environmentId,
        scopeRevision: mount.scopeRevision, clearRevision: mount.clearRevision });
      if (this.loaded.size > MAX_LOADED) this.loaded.delete(this.loaded.keys().next().value!);
    } else this.loaded.delete(mount.ownerKey);
    mount.loaded = event === 'load'; mount.failed = event === 'error';
    this.revision++;
    return { accepted: true, revision: this.revision };
  }

  snapshot(): MobileFaviconImagesView {
    this.retireCleared();
    return { revision: this.revision, items: [...this.mounts.values()].map(mount => ({
      mountId: mount.mountId, key: mount.key, requestKey: mount.requestKey,
      url: mount.url, loaded: mount.loaded, failed: mount.failed,
    })) };
  }
}

export const mobileFaviconImages = new MobileFaviconImages();
