// Pinned365aa87982 projectFaviconCache.ts/projectFavicon.ts (MIT).
// @ref llp/1109.009-mobile-settings.decision.md#offline-cache-storage
import { mobileCacheCompareMetadata, mobileCacheReadRevision, type MobileCacheKey, type MobileCacheRecord } from './mobile-client-cache';
import { letGo } from './shared/let-go';

export const MOBILE_FAVICON_MAX_DATA_URL = 32 * 1024;
export const MOBILE_FAVICON_MAX_BYTES = 1024 * 1024;
export const MOBILE_FAVICON_MAX_ENTRIES = 128;
export interface MobileFaviconTarget { environmentId: string; cwd: string; faviconPath?: string | null }
export interface MobileFaviconEntry extends MobileFaviconTarget { faviconPath: string | null; revision: string; dataUrl: string }
export type MobileFaviconMetadata = Omit<MobileCacheRecord, 'payload'>;
/** Invocation-owned IO. list returns at most limit metadata rows
 * across ALL environments, ordered by (updatedAt, environmentId, key), strictly
 * after the supplied keyset cursor. No OFFSET: hydration removes older rows.
 * read/remove/write reuse the cache's transactional and conditional operations.
 * load returns bounded inline data; standalone native helpers are prepared,
 * but their download bridge is not wired. */
export interface MobileFaviconIO {
  list(after: MobileFaviconMetadata | null, limit: number): Promise<MobileFaviconMetadata[]>;
  read(key: MobileCacheKey): Promise<MobileCacheRecord | null>;
  ticket(key: MobileCacheKey): Promise<string>;
  write(key: MobileCacheKey, ticket: string, payload: string): Promise<boolean>;
  remove(key: MobileCacheKey, expectedPayload?: string): Promise<unknown>;
  clear(environmentId?: string): Promise<unknown>;
  load(url: string, signal: AbortSignal): Promise<string>;
  /** Must change synchronously on admission of any matching clear, even failed
   * clears. Omit to use the production bridge's revision owner. */
  scopeRevision?: (environmentId: string) => string;
}
interface Saved { entry: MobileFaviconEntry; payload: string; scopeRevision: string }
export const mobileFaviconResourceKey = (target: MobileFaviconTarget) =>
  JSON.stringify([target.environmentId, target.cwd, target.faviconPath || null]);
function filename(url: string): string {
  const pathname = new URL(url, 'https://t3.invalid').pathname;
  return pathname.slice(pathname.lastIndexOf('/') + 1);
}
export function mobileFaviconRevision(target: MobileFaviconTarget, url: string): string {
  let revision = url;
  try { revision = filename(url); } catch { /* Source's malformed URL fallback. */ }
  return JSON.stringify([target.environmentId, target.cwd, revision]);
}
export function mobileFaviconMissing(url: string): boolean {
  try { return filename(url) === 'project-favicon-missing'; } catch { return false; }
}
export function mobileFaviconDataUrl(value: unknown): value is string {
  return typeof value === 'string' && value.length <= MOBILE_FAVICON_MAX_DATA_URL
    && /^data:image\/(?:png|jpeg|gif|webp|avif|svg\+xml|x-icon|vnd\.microsoft\.icon);base64,[A-Za-z0-9+/]+={0,2}$/.test(value);
}
/** Matches the pinned Entry schema: only environment ID is trimmed; empty cwd,
 * revision and faviconPath are valid. This checks the data URL alphabet/length,
 * not image bytes, dimensions, canonical Base64 or SVG contents. */
export function decodeMobileFaviconEntry(value: unknown): MobileFaviconEntry | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  const row = value as Record<string, unknown>;
  if (typeof row.environmentId !== 'string' || !row.environmentId.trim() || typeof row.cwd !== 'string'
    || row.faviconPath !== null && typeof row.faviconPath !== 'string'
    || typeof row.revision !== 'string' || !mobileFaviconDataUrl(row.dataUrl)) return null;
  return { environmentId: row.environmentId.trim(), cwd: row.cwd, faviconPath: row.faviconPath,
    revision: row.revision, dataUrl: row.dataUrl };
}
function decodeRecord(record: MobileCacheRecord): MobileFaviconEntry | null {
  if (record.kind !== 'project-favicon' || record.schemaVersion !== 1) return null;
  try {
    const entry = decodeMobileFaviconEntry(JSON.parse(record.payload));
    return entry && entry.environmentId === record.environmentId && mobileFaviconResourceKey(entry) === record.key ? entry : null;
  } catch { return null; }
}
const cacheKey = (target: MobileFaviconTarget): MobileCacheKey =>
  ({ environmentId: target.environmentId, kind: 'project-favicon', key: mobileFaviconResourceKey(target) });
const revisionOf = (io: Pick<MobileFaviconIO, 'scopeRevision'>, environmentId: string) =>
  io.scopeRevision ? io.scopeRevision(environmentId) : mobileCacheReadRevision(environmentId, 'project-favicon');

/** No native handle, callback, promise or unawaited persistence survives a call.
 * Callers explicitly await hydrate before resolve. A concurrent hydration does
 * not coalesce: it returns false and the next owned refresh can retry. Resolve
 * during hydration returns only a retained image/remote URL and admits no write.
 * LRU promotion is memory-only, just as the source; restart uses updatedAt order. */
export class MobileFaviconCache {
  private entries = new Map<string, Saved>();
  private serial = 0;
  private hydration = 0;
  private hydrated = false;
  private clearing = 0;
  private activeResolves = 0;
  private readonly failedClears = new Map<string, number>();
  private readonly keySerials = new Map<string, number>();
  private readonly environmentRevisions = new Map<string, number>();
  private generation = 0;

  peek(target: MobileFaviconTarget, io: Pick<MobileFaviconIO, 'scopeRevision'> = {}): string | null {
    const key = mobileFaviconResourceKey(target), saved = this.entries.get(key);
    if (saved && saved.scopeRevision !== revisionOf(io, target.environmentId)) {
      this.entries.delete(key); return null;
    }
    return saved?.entry.dataUrl ?? null;
  }
  private async remove(io: MobileFaviconIO, key: MobileCacheKey, payload: string, current: () => boolean) {
    if (!current()) return;
    try { await io.remove(key, payload); }
    catch (error) { if (letGo(error)) throw error; }
  }
  private async trim(io: MobileFaviconIO, entries: Map<string, Saved>, current: () => boolean) {
    let bytes = 0;
    for (const [key, saved] of entries) {
      if (saved.scopeRevision !== revisionOf(io, saved.entry.environmentId)) entries.delete(key);
      else bytes += saved.entry.dataUrl.length;
    }
    // Select all evictions synchronously, as the pinned owner does. Awaiting
    // between map mutations lets another resolve change byte totals underneath
    // this pass. Cleanup stays bounded and belongs to this caller's answer.
    const evicted: Saved[] = [];
    while (entries.size > MOBILE_FAVICON_MAX_ENTRIES || bytes > MOBILE_FAVICON_MAX_BYTES) {
      const oldest = entries.entries().next().value;
      if (!oldest) break;
      entries.delete(oldest[0]); bytes -= oldest[1].entry.dataUrl.length;
      evicted.push(oldest[1]);
    }
    for (const saved of evicted) {
      if (!current()) return;
      await this.remove(io, cacheKey(saved.entry), saved.payload,
        () => current() && saved.scopeRevision === revisionOf(io, saved.entry.environmentId));
    }
  }
  async hydrate(io: MobileFaviconIO, current: () => boolean): Promise<boolean> {
    if (!current() || this.clearing || this.hydration || this.activeResolves || this.failedClears.has('')) return false;
    if (this.hydrated) return true;
    const serial = ++this.serial;
    this.hydration = serial;
    const owned = () => current() && this.serial === serial && !this.clearing;
    const entries = new Map<string, Saved>();
    let after: MobileFaviconMetadata | null = null;
    try {
      for (;;) {
        const rows = await io.list(after, MOBILE_FAVICON_MAX_ENTRIES);
        if (!owned()) return false;
        if (rows.length > MOBILE_FAVICON_MAX_ENTRIES) return false;
        for (const metadata of rows) {
          if (metadata.kind !== 'project-favicon' || !Number.isSafeInteger(metadata.updatedAt) || metadata.updatedAt < 0
            || typeof metadata.environmentId !== 'string' || typeof metadata.key !== 'string'
            || after && mobileCacheCompareMetadata(metadata, after) <= 0) return false;
          after = { ...metadata };
          if (this.failedClears.has(metadata.environmentId)) continue;
          const scopeRevision = revisionOf(io, metadata.environmentId);
          const rowOwned = () => owned() && scopeRevision === revisionOf(io, metadata.environmentId);
          const record = await io.read(metadata);
          if (!owned()) return false;
          if (!record || !rowOwned()) continue;
          if (record.environmentId !== metadata.environmentId || record.key !== metadata.key || record.kind !== 'project-favicon') return false;
          // A concurrent replacement changes order; retry a fresh enumeration.
          if (record.updatedAt !== metadata.updatedAt) return false;
          const entry = decodeRecord(record);
          if (!entry) { await this.remove(io, metadata, record.payload, rowOwned); continue; }
          entries.set(record.key, { entry, payload: record.payload, scopeRevision });
          await this.trim(io, entries, owned);
          if (!owned()) return false;
        }
        if (rows.length < MOBILE_FAVICON_MAX_ENTRIES) break;
      }
      if (!owned()) return false;
      for (const [key, saved] of entries) {
        if (saved.scopeRevision !== revisionOf(io, saved.entry.environmentId)) entries.delete(key);
      }
      this.entries = entries; this.hydrated = true;
      return true;
    } catch (error) { if (letGo(error)) throw error; return false; }
    finally { if (this.hydration === serial) this.hydration = 0; }
  }
  async resolve(io: MobileFaviconIO, targetInput: MobileFaviconTarget, url: string | null,
    options: { current: () => boolean; signal: AbortSignal }): Promise<string | null> {
    const target = { ...targetInput }, { current, signal } = options;
    const peek = () => this.peek(target, io);
    if (!current() || signal.aborted || url === null) return peek();
    if (!this.hydrated || this.hydration || this.clearing || this.failedClears.has('')
      || this.failedClears.has(target.environmentId)) return peek() ?? url;
    this.activeResolves++;
    const key = cacheKey(target), scopeRevision = revisionOf(io, target.environmentId);
    const generation = this.generation, environmentRevision = this.environmentRevisions.get(target.environmentId) ?? 0;
    const serial = ++this.serial;
    this.keySerials.set(key.key, serial);
    const owned = () => current() && !signal.aborted && !this.clearing && generation === this.generation
      && environmentRevision === (this.environmentRevisions.get(target.environmentId) ?? 0)
      && scopeRevision === revisionOf(io, target.environmentId) && this.keySerials.get(key.key) === serial;
    try {
      if (mobileFaviconMissing(url)) {
        this.entries.delete(key.key);
        // Authoritative absence invalidates the native key ticket even when an
        // older write is still awaiting its reply and has no memory entry yet.
        // Eviction/corrupt cleanup remains conditional; absence is not eviction.
        try { await io.remove(key); }
        catch (error) { if (letGo(error)) throw error; }
        return null;
      }
      const revision = mobileFaviconRevision(target, url);
      peek(); // Drop a row invalidated through the external cache bridge.
      const cached = this.entries.get(key.key);
      if (cached) {
        this.entries.delete(key.key); this.entries.set(key.key, cached);
        if (cached.entry.revision === revision) return cached.entry.dataUrl;
      }
      let ticket: string | null = null;
      try { ticket = await io.ticket(key); }
      catch (error) { if (letGo(error)) throw error; }
      if (!owned()) return peek();
      const dataUrl = await io.load(url, signal);
      if (!owned()) return peek();
      if (!mobileFaviconDataUrl(dataUrl)) return peek() ?? url;
      const entry: MobileFaviconEntry = { ...target, faviconPath: target.faviconPath || null, revision, dataUrl };
      const saved = { entry, payload: JSON.stringify(entry), scopeRevision };
      let accepted = true;
      if (ticket) {
        try {
          accepted = await io.write(key, ticket, saved.payload);
        } catch (error) { if (letGo(error)) throw error; }
      }
      if (!owned()) { this.hydrated = false; return peek(); }
      if (!accepted) return peek() ?? url;
      this.entries.set(key.key, saved);
      await this.trim(io, this.entries, owned);
      return owned() ? peek() : null;
    } catch (error) {
      // A write may have reached disk before the caller was abandoned. Rehydrate
      // and trim on the next owned refresh; no stale answer schedules cleanup.
      if (letGo(error)) { this.hydrated = false; throw error; }
      return owned() ? peek() ?? url : peek();
    } finally {
      this.activeResolves--;
      if (this.keySerials.get(key.key) === serial) this.keySerials.delete(key.key);
    }
  }
  async clear(io: MobileFaviconIO, environmentId?: string): Promise<boolean> {
    this.serial++; this.hydrated = false; this.clearing++;
    if (environmentId === undefined) this.generation++;
    else this.environmentRevisions.set(environmentId, (this.environmentRevisions.get(environmentId) ?? 0) + 1);
    for (const [key, saved] of this.entries) {
      if (environmentId === undefined || saved.entry.environmentId === environmentId) this.entries.delete(key);
    }
    const scope = environmentId ?? '', clearSerial = this.serial;
    this.failedClears.set(scope, clearSerial);
    try {
      await io.clear(environmentId);
      for (const [failedScope, failedSerial] of this.failedClears) {
        if ((environmentId === undefined || failedScope === scope) && failedSerial <= clearSerial) this.failedClears.delete(failedScope);
      }
      return true;
    } catch (error) { if (letGo(error)) throw error; return false; }
    finally { this.clearing--; }
  }
}
