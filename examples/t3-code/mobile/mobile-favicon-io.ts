// @ref llp/1109.009-mobile-settings.decision.md#offline-cache-storage
import { mobileCacheClear, mobileCacheClearKind, mobileCacheList, mobileCacheRead, mobileCacheRemove,
  mobileCacheTicket, mobileCacheWrite, mobileCacheReadRevision, type MobileCacheKey } from './mobile-client-cache';
import { decodeMobileFaviconEntry, mobileFaviconResourceKey, mobileFaviconDataUrl, type MobileFaviconIO } from './mobile-favicon-cache';
import { obj } from './shared/domain';
import { mobileCacheCatalogIdentity } from './mobile-client-cache-catalog';
import { letGo, letGoAware } from './shared/let-go';
import { bridgeReply, ClientError, type Native } from './shared/protocol';

// Plain serial only. Neither the module nor the cache retains this answer's IO.
let requestSerial = 0;
const cancelled = () => new ClientError('Project icon request was cancelled.', 'Cancelled');

/** Await all IO inside use. Returning closes the adapter, drops its native handle,
 * and makes an accidentally escaped adapter refuse subsequent work. ExactReply
 * has no answer-cancellation hook: a let-go download ends at its bounded native
 * timeout or module teardown, without trying another call through the lost answer. */
export async function withMobileFaviconIO<T>(nativeInput: Native, use: (io: MobileFaviconIO) => Promise<T>,
  catalog?: MobileFaviconCatalog): Promise<T> {
  let native: Native | null = letGoAware(nativeInput);
  const handle = () => {
    if (!native) throw new ClientError('This operation was superseded.', 'superseded');
    return native;
  };
  const image = async (fields: object) => {
    const result = await bridgeReply(handle(), { op: 'mobileFaviconImage', ...fields });
    if (!result.ok) throw new ClientError(result.error!.message, result.error!.kind);
    return obj(result.value);
  };
  const io: MobileFaviconIO = {
    list: (after, limit) => mobileCacheList(handle(), 'project-favicon', after, limit),
    read: key => mobileCacheRead(handle(), key),
    ticket: key => mobileCacheTicket(handle(), key),
    write: (key, ticket, payload) => mobileCacheWrite(handle(), key, ticket, payload),
    remove: (key, payload) => mobileCacheRemove(handle(), key, payload, { retainFaviconScope: payload === undefined }),
    clear: environmentId => environmentId === undefined
      ? mobileCacheClearKind(handle(), 'project-favicon')
      : mobileCacheClear(handle(), { kind: 'project-favicon', environmentId }),
    async load(url, signal) {
      handle();
      if (signal.aborted) throw cancelled();
      const requestId = `favicon-${++requestSerial}`;
      let cancellation: Promise<void> | undefined, cancelError: unknown;
      const abort = () => {
        // The cancellation belongs to this same live invocation and is awaited
        // below. letGoAware refuses it if another call already lost the answer.
        cancellation ??= image({ action: 'cancel', requestId }).then(() => {}, error => { cancelError = error; });
      };
      signal.addEventListener('abort', abort, { once: true });
      try {
        const value = await image({ action: 'load', requestId, url });
        if (signal.aborted) throw cancelled();
        if (!mobileFaviconDataUrl(value.dataUrl)) throw new ClientError('The project icon returned invalid image data.', 'Favicon');
        return value.dataUrl;
      } finally {
        signal.removeEventListener('abort', abort);
        await cancellation;
        if (letGo(cancelError)) throw cancelError;
      }
    },
  };
  try { return await use(catalog ? mobileFaviconCatalogIO(io, catalog) : io); }
  finally { native = null; }
}

/** Invocation-only catalog access. Empty identity means forgotten/ambiguous or
 * pending a replacement clear. Disabled saved identities may still hydrate. */
export interface MobileFaviconCatalog {
  identity(environmentId: string): string;
  current(): boolean;
}
export const mobileFaviconCatalogRevision = (environmentId: string, catalogIdentity: string,
  revision = mobileCacheReadRevision(environmentId, 'project-favicon')) => JSON.stringify([revision, catalogIdentity]);

/** Raw schema1 entry plus saved-home provenance. Other cache kinds and the
 * source entry decoder remain unchanged. Exact payload bytes own eviction. */
export function mobileFaviconCatalogIO(io: MobileFaviconIO, catalog: MobileFaviconCatalog): MobileFaviconIO {
  const assert = () => { if (!catalog.current()) throw new ClientError('The favicon owner changed.', 'superseded'); };
  const revision = (environmentId: string) => mobileFaviconCatalogRevision(environmentId, catalog.identity(environmentId),
    io.scopeRevision?.(environmentId) ?? mobileCacheReadRevision(environmentId, 'project-favicon'));
  const identityFor = (key: MobileCacheKey) => {
    const identity = catalog.identity(key.environmentId);
    try {
      const parts = JSON.parse(identity);
      if (!Array.isArray(parts) || parts.length !== 2 || parts[0] !== key.environmentId
        || mobileCacheCatalogIdentity([{ environmentId: parts[0], origin: parts[1] }], key.environmentId) !== identity)
        throw new Error();
    } catch { throw new ClientError('The saved favicon environment changed.', 'superseded'); }
    return identity;
  };
  const encoded = (key: MobileCacheKey, payload: string, identity: string) => {
    const row = obj(JSON.parse(payload)), entry = decodeMobileFaviconEntry(row);
    if (!entry || key.kind !== 'project-favicon' || row.environmentId !== key.environmentId
      || mobileFaviconResourceKey(entry) !== key.key) throw new ClientError('Invalid cached favicon.', 'Cache');
    return JSON.stringify({ ...row, catalogIdentity: identity });
  };
  return {
    scopeRevision: revision,
    async list(after, limit) { assert(); const rows = await io.list(after, limit); assert(); return rows; },
    async read(key) {
      assert(); const captured = revision(key.environmentId), identity = catalog.identity(key.environmentId);
      const record = await io.read(key); assert();
      if (!record || revision(key.environmentId) !== captured) return null;
      let matches = false;
      try { matches = !!identity && obj(JSON.parse(record.payload)).catalogIdentity === identity; } catch { /* Conditional corrupt-row cleanup below. */ }
      if (matches) return record;
      await io.remove(key, record.payload); assert();
      return null;
    },
    async ticket(key) { assert(); identityFor(key); const captured = revision(key.environmentId);
      const ticket = await io.ticket(key); assert();
      if (revision(key.environmentId) !== captured) throw new ClientError('The favicon cache changed.', 'superseded');
      return ticket;
    },
    async write(key, ticket, payload) {
      assert(); const identity = identityFor(key), captured = revision(key.environmentId);
      const written = await io.write(key, ticket, encoded(key, payload, identity)); assert();
      return written && revision(key.environmentId) === captured;
    },
    async remove(key, payload) {
      assert();
      // Hydration retained original augmented bytes. A new resolve retained raw
      // bytes, so add the same provenance used by write before comparing them.
      const expected = payload === undefined || typeof obj(JSON.parse(payload)).catalogIdentity === 'string'
        ? payload : encoded(key, payload, identityFor(key));
      const result = await io.remove(key, expected); assert(); return result;
    },
    async clear(environmentId) { assert(); const result = await io.clear(environmentId); assert(); return result; },
    async load(url, signal) { assert(); const result = await io.load(url, signal); assert(); return result; },
  };
}
