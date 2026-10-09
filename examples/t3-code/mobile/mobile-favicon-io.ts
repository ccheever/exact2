// @ref llp/1109.009-mobile-settings.decision.md#offline-cache-storage
import { mobileCacheClear, mobileCacheClearKind, mobileCacheList, mobileCacheRead, mobileCacheRemove,
  mobileCacheTicket, mobileCacheWrite } from './mobile-client-cache';
import { mobileFaviconDataUrl, type MobileFaviconIO } from './mobile-favicon-cache';
import { obj } from './shared/domain';
import { letGo, letGoAware } from './shared/let-go';
import { bridgeReply, ClientError, type Native } from './shared/protocol';

// Plain serial only. Neither the module nor the cache retains this answer's IO.
let requestSerial = 0;
const cancelled = () => new ClientError('Project icon request was cancelled.', 'Cancelled');

/** Await all IO inside use. Returning closes the adapter, drops its native handle,
 * and makes an accidentally escaped adapter refuse subsequent work. ExactReply
 * has no answer-cancellation hook: a let-go download ends at its bounded native
 * timeout or module teardown, without trying another call through the lost answer. */
export async function withMobileFaviconIO<T>(nativeInput: Native, use: (io: MobileFaviconIO) => Promise<T>): Promise<T> {
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
    remove: (key, payload) => mobileCacheRemove(handle(), key, payload),
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
  try { return await use(io); }
  finally { native = null; }
}
