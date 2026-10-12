// An app file used as a request body loads only when a fetch asks for it.
import { coversPath, FetchError, fetchWith, ownWithout } from './admission.js';
import { tsGrantSet } from './admission-data.js';
import { files } from './ts-fetch.js';

// `exactBodyFrom` (LLP 1108 D6 R2), with Hermes's checks and words
// (js/src/prelude.js): the app file at that path is the body, read from the
// page's store as a Blob (a picked file is its own File; nothing becomes a
// string), under `fs.read`, at most 64 MiB. A refusal is a FetchError before
// anything is sent. `files.appId` is the module's, set by ts-data.js.
export const methodOf = (input, init) => String(init.method ?? (typeof Request === 'function' && input instanceof Request ? input.method : 'GET')).toUpperCase();
export function bodyFromRefusal(input, init) {
  const path = init.exactBodyFrom, method = methodOf(input, init);
  if (typeof path !== 'string' || !path.startsWith('app:/')) return 'exactBodyFrom must be an app:/ path';
  if (new TextEncoder().encode(path).length > 4096) return 'exactBodyFrom: a path is at most 4096 bytes';
  if (/(^|\/)\.\.?(\/|$)|\0/.test(path.slice(5))) return 'exactBodyFrom: an app:/ path has no . or .. segment';
  if (method === 'GET' || method === 'HEAD') return `fetch: a ${method} request cannot have a body`;
  if (init.exactStream !== undefined && /^wss?:/i.test(typeof Request === 'function' && input instanceof Request ? input.url : String(input))) return 'exactBodyFrom: a WebSocket sends no body';
  if (init.body != null || typeof Request === 'function' && input instanceof Request && input.body != null) return 'fetch: a request has one body: body or exactBodyFrom';
  return null;
}
export function readBodyFile(path, grantSet) {
  // Refused before the storage adapters load, which an app without file grants does not ship.
  if (!coversPath(grantSet, 'fs.read', path)) return Promise.reject(Object.assign(new FetchError('Refused', `exactBodyFrom ${path}: denied: fs.read ${path}: no grant covers it; grant \`fs.read ${path}\``), { code: 'denied' }));
  return import(new URL('./storage-fs.js', import.meta.url).href)
    .then(m => m.requestBody(files.appId, grantSet, path))
    .catch(error => { throw Object.assign(new FetchError(error?.code === 'agent' ? 'Unsupported' : 'Refused', error?.message ?? error), { code: error?.code }); });
}
export async function fromFile(input, init, redirect, started) {
  const refusal = bodyFromRefusal(input, init);
  if (refusal) throw new TypeError(refusal);
  const path = init.exactBodyFrom, rest = ownWithout(init, ['exactBodyFrom', 'redirect']), ms = init.exactTimeout;
  if (ms !== undefined && (!Number.isInteger(ms) || ms < 1 || ms > 3600000)) throw new TypeError('exactTimeout must be an integer number of milliseconds from 1 to 3600000');
  // The read counts against the deadline and yields to the caller's abort,
  // as the exchange does: nothing is sent once either has ended it. (This
  // module's own `setTimeout` and `performance` are the app's refusals.)
  const signal = rest.signal !== undefined ? rest.signal : typeof Request === 'function' && input instanceof Request ? input.signal : null;
  const clock = globalThis.performance, remaining = ms === undefined ? null : ms - (clock.now() - started);
  if (remaining !== null && remaining <= 0) throw new FetchError('Timeout', `the request timed out after ${ms} ms`);
  const deadline = remaining === null ? null : AbortSignal.timeout(Math.max(1, Math.ceil(remaining)));
  // Already aborted: refused before anything is read or a rejection made that nobody would handle.
  if (signal?.aborted) throw signal.reason ?? new FetchError('Aborted', 'the fetch was aborted');
  let stop = () => {};
  const ended = new Promise((_, reject) => {
    const timedOut = () => reject(new FetchError('Timeout', `the request timed out after ${ms} ms`));
    const aborted = () => reject(signal.reason ?? new FetchError('Aborted', 'the fetch was aborted'));
    if (signal?.aborted) aborted(); else signal?.addEventListener?.('abort', aborted, { once: true });
    deadline?.addEventListener('abort', timedOut, { once: true });
    stop = () => { signal?.removeEventListener?.('abort', aborted); deadline?.removeEventListener('abort', timedOut); };
  });
  let body;
  const reading = readBodyFile(path, tsGrantSet);
  reading.catch(() => {}); // a read that loses the race is nobody's
  try { body = await Promise.race([reading, ended]); } finally { stop(); }
  // One deadline for the whole request: its instant goes to fetchWith, which
  // checks it by the clock just before sending, after its own grant work.
  const spent = clock.now() - started;
  if (ms !== undefined && spent >= ms) throw new FetchError('Timeout', `the request timed out after ${ms} ms`);
  const left = ms === undefined ? undefined : Math.max(1, Math.ceil(ms - spent));
  return fetchWith(tsGrantSet, input, { ...rest, body, ...(left === undefined ? {} : { exactTimeout: left }) }, ms === undefined ? null : { at: started + ms, ms }, redirect);
}
