// @ref llp/1109.009-mobile-settings.decision.md#offline-cache-storage
import { obj, str } from './shared/domain';
import { bridgeReply, ClientError, type Native } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';

export const MOBILE_CACHE_KINDS = ['shell', 'thread', 'server-config', 'vcs-refs', 'project-favicon'] as const;
export type MobileCacheKind = typeof MOBILE_CACHE_KINDS[number];
export interface MobileCacheKey { environmentId: string; kind: MobileCacheKind; key: string }
export interface MobileCacheRecord extends MobileCacheKey { schemaVersion: number; payload: string; updatedAt: number }
export type MobileCacheMetadata = Omit<MobileCacheRecord, 'payload'>;
export interface MobileCacheSummaryRow { environmentId: string; kind: MobileCacheKind; recordCount: number; payloadBytes: number }
export interface MobileCacheSummary { rows: MobileCacheSummaryRow[]; recordCount: number; payloadBytes: number }
export type MobileCacheScope = { environmentId?: string; kind?: MobileCacheKind; key?: string };
const natural = (value: unknown): value is number => Number.isSafeInteger(value) && Number(value) >= 0;
const kind = (value: unknown): value is MobileCacheKind => MOBILE_CACHE_KINDS.includes(value as MobileCacheKind);
let clearRevision = 0;
let displayClearRevision = 0;
let kindClearRevision = 0;
const kindClears = new Map<MobileCacheKind, number>();
const environmentClears = new Map<string, number>();
const displayEnvironmentClears = new Map<string, number>();
// An omitted kind observes a combined display, so every kind-wide clear must
// invalidate it. Explicit kinds retain isolation from unrelated kind clears.
export const mobileCacheReadRevision = (environmentId: string, cacheKind?: MobileCacheKind) =>
  `${clearRevision}:${environmentClears.get(environmentId) ?? 0}:${cacheKind ? kindClears.get(cacheKind) ?? 0 : kindClearRevision}`;
/** Explicit clears evict rendered offline data. A settled VCS invalidation can
 * instead retain already-rendered read-only rows while its next live read runs.
 * It still invalidates every pending disk read, write and native ticket. */
export const mobileCacheDisplayRevision = (environmentId: string, cacheKind?: MobileCacheKind) =>
  `${displayClearRevision}:${displayEnvironmentClears.get(environmentId) ?? 0}:${cacheKind ? kindClears.get(cacheKind) ?? 0 : kindClearRevision}`;
const invalid = () => new ClientError('The local cache returned an invalid reply.', 'Cache');

/** SQLite BINARY compares UTF-8, which orders valid Unicode by scalar value.
 * JavaScript's string comparison orders UTF-16 code units instead. */
function compareText(a: string, b: string): number {
  let i = 0, j = 0;
  while (i < a.length && j < b.length) {
    const left = a.codePointAt(i)!, right = b.codePointAt(j)!;
    if (left !== right) return left - right;
    i += left > 0xffff ? 2 : 1; j += right > 0xffff ? 2 : 1;
  }
  return (i < a.length ? 1 : 0) - (j < b.length ? 1 : 0);
}
export const mobileCacheCompareMetadata = (a: MobileCacheMetadata, b: MobileCacheMetadata): number =>
  a.updatedAt - b.updatedAt || compareText(a.environmentId, b.environmentId) || compareText(a.key, b.key);

/** No native handle or pending promise is retained beyond its calling answer. */
async function request(nativeInput: Native, fields: object) {
  const reply = await bridgeReply(letGoAware(nativeInput), { op: 'mobileClientCache', ...fields });
  if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind);
  return obj(reply.value);
}
export async function mobileCacheTicket(native: Native, key: MobileCacheKey): Promise<string> {
  const value = await request(native, { action: 'ticket', ...key });
  if (!str(value.ticket)) throw invalid();
  return str(value.ticket);
}
export async function mobileCacheRead(native: Native, key: MobileCacheKey): Promise<MobileCacheRecord | null> {
  const value = await request(native, { action: 'read', ...key });
  if (value.record === null) return null;
  const row = obj(value.record);
  if (row.environmentId !== key.environmentId || row.kind !== key.kind || row.key !== key.key
    || row.schemaVersion !== 1 || typeof row.payload !== 'string' || !natural(row.updatedAt)) throw invalid();
  return { ...key, schemaVersion: 1, payload: row.payload, updatedAt: row.updatedAt };
}
/** Global, bounded keyset enumeration. Deleting earlier rows never skips a page.
 * Payload validation remains in read, so unsupported records can be discarded. */
export async function mobileCacheList(native: Native, cacheKind: MobileCacheKind,
  after: MobileCacheMetadata | null = null, limit = 128): Promise<MobileCacheMetadata[]> {
  if (!kind(cacheKind) || !natural(limit) || limit < 1 || limit > 128
    || after && after.kind !== cacheKind) throw invalid();
  const value = await request(native, { action: 'list', kind: cacheKind, after, limit });
  if (!Array.isArray(value.rows) || value.rows.length > limit) throw invalid();
  let previous = after;
  return value.rows.map(raw => {
    const row = obj(raw);
    if (!str(row.environmentId) || !str(row.key) || row.kind !== cacheKind
      || !natural(row.schemaVersion) || !natural(row.updatedAt)) throw invalid();
    const metadata = { environmentId: str(row.environmentId), kind: cacheKind, key: str(row.key),
      schemaVersion: row.schemaVersion, updatedAt: row.updatedAt };
    if (previous && mobileCacheCompareMetadata(metadata, previous) <= 0) throw invalid();
    previous = metadata;
    return metadata;
  });
}
/** Obtain ticket before capturing the immutable payload. Clear rejects old tickets. */
export async function mobileCacheWrite(native: Native, key: MobileCacheKey, ticket: string, payload: string): Promise<boolean> {
  const value = await request(native, { action: 'write', ...key, ticket, schemaVersion: 1, payload });
  if (typeof value.written !== 'boolean' || typeof value.stale !== 'boolean' || value.written === value.stale) throw invalid();
  return value.written;
}
/** Conditional removal cannot erase a newer replacement written after a corrupt
 * read. An authoritative deletion also invalidates pending in-memory reads. */
export async function mobileCacheRemove(native: Native, key: MobileCacheKey, expectedPayload?: string,
  options: { retainFaviconScope?: boolean } = {}): Promise<number> {
  if (options.retainFaviconScope && key.kind !== 'project-favicon') throw invalid();
  // Favicon absence owns one resource through its cache key serial and the
  // native remove's key-ticket invalidation. It must not retire sibling icons
  // or its own retained missing URL by changing the whole environment scope.
  if (expectedPayload === undefined && !options.retainFaviconScope) {
    environmentClears.set(key.environmentId, (environmentClears.get(key.environmentId) ?? 0) + 1);
    displayEnvironmentClears.set(key.environmentId, (displayEnvironmentClears.get(key.environmentId) ?? 0) + 1);
  }
  const value = await request(native, { action: 'remove', ...key, ...(expectedPayload === undefined ? {} : { expectedPayload }) });
  if (!natural(value.removed) || value.removed > 1) throw invalid();
  return value.removed;
}
/** Corrupt domain records are disposable. Cleanup is conditional and best effort;
 * a lost answer still propagates before any further caller-owned operation. */
export async function mobileCacheReadDecoded<T>(native: Native, key: MobileCacheKey, decode: (payload: string) => T | null,
  current: () => boolean): Promise<T | null> {
  const record = await mobileCacheRead(native, key);
  if (!current() || !record) return null;
  const decoded = decode(record.payload);
  if (decoded !== null) return decoded;
  try { await mobileCacheRemove(native, key, record.payload); }
  catch (error) { if (letGo(error)) throw error; }
  return null;
}
export async function mobileCacheClear(native: Native, scope: MobileCacheScope = {}, options: { retainDisplay?: boolean } = {}): Promise<number> {
  // Pending reads must lose ownership even if the clear fails or its reply is lost.
  if (scope.environmentId) environmentClears.set(scope.environmentId, (environmentClears.get(scope.environmentId) ?? 0) + 1);
  else { clearRevision++; environmentClears.clear(); }
  if (!options.retainDisplay) {
    if (scope.environmentId) displayEnvironmentClears.set(scope.environmentId, (displayEnvironmentClears.get(scope.environmentId) ?? 0) + 1);
    else { displayClearRevision++; displayEnvironmentClears.clear(); }
  }
  const value = await request(native, { action: 'clear', ...scope });
  if (!natural(value.removed)) throw invalid();
  return value.removed;
}
/** Clear one record kind across all environments, including in-flight writers. */
export async function mobileCacheClearKind(native: Native, cacheKind: MobileCacheKind): Promise<number> {
  if (!kind(cacheKind)) throw invalid();
  kindClearRevision++;
  kindClears.set(cacheKind, (kindClears.get(cacheKind) ?? 0) + 1);
  const value = await request(native, { action: 'clearKind', kind: cacheKind });
  if (!natural(value.removed)) throw invalid();
  return value.removed;
}
/** Counts serialized UTF-8 payload bytes, not SQLite pages or protected app data. */
export async function mobileCacheInspect(native: Native, environmentId?: string): Promise<MobileCacheSummary> {
  const value = await request(native, { action: 'inspect', ...(environmentId === undefined ? {} : { environmentId }) });
  if (!Array.isArray(value.rows) || !natural(value.recordCount) || !natural(value.payloadBytes)) throw invalid();
  const seen = new Set<string>();
  const rows = value.rows.map(raw => {
    const row = obj(raw), identity = JSON.stringify([row.environmentId, row.kind]);
    if (!str(row.environmentId) || !kind(row.kind) || !natural(row.recordCount) || row.recordCount === 0
      || !natural(row.payloadBytes) || seen.has(identity) || environmentId !== undefined && row.environmentId !== environmentId) throw invalid();
    seen.add(identity);
    return { environmentId: str(row.environmentId), kind: row.kind, recordCount: row.recordCount, payloadBytes: row.payloadBytes };
  });
  if (rows.reduce((sum, row) => sum + row.recordCount, 0) !== value.recordCount
    || rows.reduce((sum, row) => sum + row.payloadBytes, 0) !== value.payloadBytes) throw invalid();
  return { rows, recordCount: value.recordCount, payloadBytes: value.payloadBytes };
}
