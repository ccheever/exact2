// App files as a request's body or a reply's: loaded only when a fetch asks for one.
import { coversPath, FetchError, fetchWith, ownWithout, within } from './admission.js';
import { tsGrantSet } from './admission-data.js';
import { files } from './ts-fetch.js';

// `exactBodyFrom` (LLP 1108 D6 R2), with Hermes's checks and words
// (js/src/prelude.js): the app file at that path is the body, read from the
// page's store as a Blob (a picked file is its own File; nothing becomes a
// string), under `fs.read`, at most 64 MiB. A refusal is a FetchError before
// anything is sent. `files.appId` is the module's, set by ts-data.js.
export const methodOf = (input, init) => String(init.method ?? (typeof Request === 'function' && input instanceof Request ? input.method : 'GET')).toUpperCase();
const pathRefusal = (name, path) => {
  if (typeof path !== 'string' || !path.startsWith('app:/')) return `${name} must be an app:/ path`;
  if (new TextEncoder().encode(path).length > 4096) return `${name}: a path is at most 4096 bytes`;
  if (/(^|\/)\.\.?(\/|$)|\0/.test(path.slice(5))) return `${name}: an app:/ path has no . or .. segment`;
  return null;
};
export function bodyFromRefusal(input, init) {
  const path = init.exactBodyFrom, method = methodOf(input, init), refused = pathRefusal('exactBodyFrom', path);
  if (refused) return refused;
  if (method === 'GET' || method === 'HEAD') return `fetch: a ${method} request cannot have a body`;
  if (init.exactStream !== undefined && /^wss?:/i.test(typeof Request === 'function' && input instanceof Request ? input.url : String(input))) return 'exactBodyFrom: a WebSocket sends no body';
  if (init.body != null || typeof Request === 'function' && input instanceof Request && input.body != null) return 'fetch: a request has one body: body or exactBodyFrom';
  return null;
}
// `exactBodyTo` (2026-10-26), with Hermes's checks and words: a 2xx reply's
// body becomes the app file at that path (storage-fs.js `bodyTarget`), under
// `fs.write`, at most `exactIndependentHttp.maxResponseBytes` or 1 GiB.
function bodyToRefusal(input, init) {
  const refused = pathRefusal('exactBodyTo', init.exactBodyTo);
  if (refused) return refused;
  if (methodOf(input, init) === 'HEAD') return 'exactBodyTo: a HEAD reply has no body to write';
  const independent = init.exactIndependentHttp, n = independent?.maxResponseBytes;
  if (independent !== undefined && (!Number.isInteger(n) || n < 1 || n > 1073741824)) return 'exactIndependentHttp.maxResponseBytes must be an integer from 1 to 1073741824';
  return null;
}
const storageFs = () => import(new URL('./storage-fs.js', import.meta.url).href);
const storageFailure = error => Object.assign(new FetchError(error?.code === 'agent' ? 'Unsupported' : 'Refused', error?.message ?? error), { code: error?.code });
export function readBodyFile(path, grantSet) {
  // Refused before the storage adapters load, which an app without file grants does not ship.
  if (!coversPath(grantSet, 'fs.read', path)) return Promise.reject(Object.assign(new FetchError('Refused', `exactBodyFrom ${path}: denied: fs.read ${path}: no grant covers it; grant \`fs.read ${path}\``), { code: 'denied' }));
  return storageFs().then(m => m.requestBody(files.appId, grantSet, path)).catch(error => { throw storageFailure(error); });
}
function openBodyTarget(path, grantSet) {
  if (!coversPath(grantSet, 'fs.write', path)) return Promise.reject(Object.assign(new FetchError('Refused', `exactBodyTo ${path}: denied: fs.write ${path}: no grant covers it; grant \`fs.write ${path}\``), { code: 'denied' }));
  return storageFs().then(m => m.bodyTarget(files.appId, grantSet, path)).catch(error => { throw storageFailure(error); });
}
export async function fromFile(input, init, redirect, started) {
  const from = init.exactBodyFrom !== undefined, to = init.exactBodyTo !== undefined;
  const refusal = (from ? bodyFromRefusal(input, init) : null) ?? (to ? bodyToRefusal(input, init) : null);
  if (refusal) throw new TypeError(refusal);
  const ms = init.exactTimeout, rest = ownWithout(init, ['exactBodyFrom', 'exactBodyTo', 'redirect', ...(to ? ['exactIndependentHttp'] : [])]);
  if (ms !== undefined && (!Number.isInteger(ms) || ms < 1 || ms > 3600000)) throw new TypeError('exactTimeout must be an integer number of milliseconds from 1 to 3600000');
  // The file work counts against the deadline and yields to the caller's
  // abort, as the exchange does: nothing is sent once either has ended it.
  // (This module's own `setTimeout` and `performance` are the app's refusals.)
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
  // Work that loses the race is nobody's; a body target that loses it is let go.
  const raced = work => { work.catch(() => {}); return Promise.race([work, ended]); };
  let body, target, opening;
  try {
    if (from) body = await raced(readBodyFile(init.exactBodyFrom, tsGrantSet));
    if (to) target = await raced(opening = openBodyTarget(init.exactBodyTo, tsGrantSet));
  } catch (error) { opening?.then(t => t.close(), () => {}); throw error; }
  finally { stop(); }
  try {
    // One deadline for the whole request: its instant goes to fetchWith, which
    // checks it by the clock just before sending, after its own grant work.
    const spent = clock.now() - started;
    if (ms !== undefined && spent >= ms) throw new FetchError('Timeout', `the request timed out after ${ms} ms`);
    const left = ms === undefined ? undefined : Math.max(1, Math.ceil(ms - spent));
    const sent = { ...rest, ...(from ? { body } : {}), ...(left === undefined ? {} : { exactTimeout: left }) }, expires = ms === undefined ? null : { at: started + ms, ms };
    if (!to) return await fetchWith(tsGrantSet, input, sent, expires, redirect);
    // A 2xx body goes to the file, counted as it arrives, within the deadline;
    // any other reply is read as usual, at most 1 MiB, and the file is untouched.
    const ceiling = init.exactIndependentHttp?.maxResponseBytes ?? 1073741824;
    let written;
    const response = await fetchWith(tsGrantSet, input, sent, expires, redirect, async reply => {
      if (!reply.ok) return within(reply, Math.min(ceiling, 1048576));
      try { written = await target.put(reply, ceiling); }
      catch (error) { throw error?.kind === 'Refused' || error?.kind === 'Unavailable' ? new FetchError('Refused', error.message) : error; }
    });
    if (written === undefined) return response;
    // The body is the file's now: this response has none, and says what was written.
    const done = new Response(null, { status: response.status, statusText: response.statusText, headers: response.headers });
    return Object.defineProperties(done, { url: { value: response.url }, exactBodyTo: { value: { path: init.exactBodyTo, bytes: written } } });
  } finally { target?.close(); }
}
