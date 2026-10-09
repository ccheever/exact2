import { admitsNetwork, admitsSecret, coversPath, createGrantSet, grantError, hasGrant, rawGrantText, scopedGrantSet, unionGrantSets } from '../web/grant-admission.js';
import { faultMessage, takeFault } from '../web/faults.js';
export { admitsNetwork, admitsSecret, coversPath, createGrantSet, grantError, hasGrant, rawGrantText, sameGrantDeclaration, scopedGrantSet, unionGrantSets } from '../web/grant-admission.js';

// Captured while the entry graph loads. App source injection never replaces
// the page global, so separately loaded host modules retain browser authority.
const browserFetch = globalThis.fetch?.bind(globalThis);

export class FetchError extends Error {
  constructor(kind, message) { super(String(message)); this.name = 'FetchError'; this.kind = String(kind); }
}

const refusal = capability => `outside the app's grants (${capability})`;
const assetPath = value => typeof value === 'string' && /^\/assets\/(?:[A-Za-z0-9_-]+\/)*[A-Za-z0-9_-]+\.[A-Za-z0-9]+$/.test(value);
const noHeaders = headers => headers == null || [...new Headers(headers)].length === 0;
const hostAsset = (input, init) => assetPath(input) && String(init.method ?? 'GET').toUpperCase() === 'GET'
  && init.body == null && noHeaders(init.headers);

// `exactTimeout` (ms): the whole exchange ends by then, as on native, and the
// fetch rejects with a FetchError of kind "Timeout".
function deadlineOf(init) {
  const ms = init.exactTimeout;
  if (ms === undefined) return null;
  if (!Number.isInteger(ms) || ms < 1 || ms > 3600000) throw new TypeError('exactTimeout must be an integer number of milliseconds from 1 to 3600000');
  return { ms, signal: AbortSignal.timeout(ms) };
}

// `exactIndependentHttp.maxResponseBytes`: the largest body the fetch takes,
// checked with Hermes's words (js/src/prelude.js); without it, the ordered
// lane's 64 MiB, as on native. Browsers have no lanes, so only the bound applies.
function ceilingOf(init) {
  const independent = init.exactIndependentHttp;
  if (independent === undefined) return 64 * 1024 * 1024;
  const n = independent?.maxResponseBytes;
  if (!Number.isInteger(n) || n <= 0 || n > 67108864) throw new TypeError('exactIndependentHttp.maxResponseBytes must be an integer from 1 to 67108864');
  return n;
}

// Read a response's clone to its end, refused the moment it passes `limit`
// (LLP 1109 D3: a response over its size limit is `refused` on every host).
// A failed read lets both branches go at once: a branch's cancel settles only
// when the other's does, so awaiting one would wait on a body that never ends.
async function within(response, limit) {
  const reader = response.clone().body?.getReader();
  if (!reader) return;
  let size = 0;
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) return;
      if ((size += value.length) > limit) throw new FetchError('Refused', 'HTTP response exceeds limit');
    }
  } catch (error) { reader.cancel().catch(() => {}); response.body?.cancel().catch(() => {}); throw error; }
}

// `expires`, when a caller already spent part of the deadline (an
// `exactBodyFrom` read, ts-fetch.js): its instant on the page's clock and the
// deadline as the caller gave it, checked just before the request goes out.
export async function fetchWith(set, input, init = {}, expires = null) {
  let value, asset, deadline, signal;
  init ??= {};
  deadline = deadlineOf(init);
  const until = expires ?? (deadline ? { at: performance.now() + deadline.ms, ms: deadline.ms } : null);
  const ceiling = ceilingOf(init);
  try {
    asset = hostAsset(input, init);
    value = typeof Request === 'function' && input instanceof Request ? input.url
      : asset ? new URL(String(input), globalThis.location?.href).href : new URL(String(input)).href;
  }
  catch (error) { throw new FetchError('Network', error?.message ?? error); }
  const invalid = grantError(set);
  if (invalid) throw new FetchError('Refused', invalid);
  if (!asset && !admitsNetwork(set, value, 'fetch')) throw new FetchError('Refused', refusal('net.fetch'));
  // @ref LLP 1103 D1, D2 — a driver fault: the refused connection's failure, never sent.
  if (!asset && takeFault(value)) throw new FetchError('Network', faultMessage(value));
  try {
    const { exactTimeout: _, exactIndependentHttp: __, ...rest } = init;
    // The caller's signal, from `init` or the input `Request` (`null`
    // clears the Request's, `undefined` keeps it, as `fetch` has them).
    const own = rest.signal !== undefined ? rest.signal : typeof Request === 'function' && input instanceof Request ? input.signal : undefined;
    // The deadline reaches the request only until its body is read: a
    // response that arrived in time stays readable after the deadline.
    let relay;
    if (deadline) {
      const ended = new AbortController();
      relay = () => ended.abort(deadline.signal.reason);
      deadline.signal.addEventListener('abort', relay, { once: true });
      signal = own ? AbortSignal.any([own, ended.signal]) : ended.signal;
    } else signal = rest.signal;
    // By the clock, just before it goes out: grant work and a body read that
    // held the event loop past the deadline fire no timer first.
    if (until && performance.now() >= until.at) throw new FetchError('Timeout', `the request timed out after ${until.ms} ms`);
    const response = await browserFetch(typeof Request === 'function' && input instanceof Request ? input : value, { ...rest, ...(signal ? { signal } : {}), redirect: 'follow' });
    // A redirect that left the grants names where it led (podcast F5), as
    // the native executor does; the browser followed it to this last hop.
    if (response.url && (asset
      ? new URL(response.url).origin !== globalThis.location?.origin
      : !admitsNetwork(set, response.url, 'fetch'))) throw new FetchError('Refused', `${refusal('net.fetch')}: redirected to ${new URL(response.url).origin}`);
    // Its clone is read whole before it answers, as the native executor
    // collects a body: within the deadline, so a stalled body is this fetch's
    // Timeout, and a body the network cuts off is its Network failure
    // (`failure(x)`'s `offline`, LLP 1109 D3); one over its ceiling is
    // Refused, and the response is let go. The response keeps its URL,
    // type and null body, and its own read is served from what the clone took.
    await within(response, ceiling);
    if (deadline) deadline.signal.removeEventListener('abort', relay);
    return response;
  }
  catch (error) {
    if (error instanceof FetchError) throw error;
    if (deadline && signal?.aborted && signal.reason === deadline.signal.reason) throw new FetchError('Timeout', `the request timed out after ${deadline.ms} ms`);
    // The caller's own abort is the native executor's `Aborted`, not a lost connection.
    if (error?.name === 'AbortError') throw new FetchError('Aborted', error?.message ?? error);
    throw new FetchError('Network', error?.message ?? error);
  }
}

export function fetchHostAsset(input, init = {}) {
  const url = new URL(input, globalThis.document?.baseURI ?? globalThis.location?.href);
  if (url.origin !== globalThis.location?.origin) return Promise.reject(new Error('host asset left the app origin'));
  return browserFetch(url, { ...init, redirect: 'error' });
}

export function createRequestExecutor(appId, sourceSet, readBody = response => response.arrayBuffer()) {
  let storage = null;
  return async req => {
    const admitted = scopedGrantSet(sourceSet, req.scope);
    const why = grantError(admitted);
    if (why) return { failed: 2, message: why };
    if (req.storage != null) {
      storage ??= import(new URL('./storage-request.js', import.meta.url).href).then(m => m.createStorageRequests(appId, sourceSet));
      return { storage: await (await storage).run(req.storage, admitted) };
    }
    try {
      // Its ceiling bounds the read inside `fetchWith` too, so an oversized body that never ends is refused, not awaited.
      const response = await fetchWith(admitted, req.url, { method: req.method, headers: req.headers, body: ['GET', 'HEAD'].includes(req.method) ? undefined : req.raw,
        ...(req.maxResponseBytes != null ? { exactIndependentHttp: { maxResponseBytes: req.maxResponseBytes } } : {}) });
      return { status: response.status, headers: [...response.headers], body: new Uint8Array(await readBody(response, req.maxResponseBytes)) };
    } catch (error) {
      return { failed: error?.kind === 'Refused' ? 2 : 1, message: String(error?.message ?? error) };
    }
  };
}

let appSet = null;
export function setAppGrantSet(...sets) {
  appSet = sets.length === 1 ? createGrantSet(sets[0]) : unionGrantSets(...sets);
  return rawGrantText(appSet);
}
export const appGrantSet = () => appSet;

export function createSecretFacade(store, admitted, keys) {
  const refused = name => {
    const parse = grantError(admitted);
    return `secret ${name} is not granted${parse ? ': ' + parse : ''}`;
  };
  const allowed = name => {
    if (!admitsSecret(admitted, name)) throw Object.assign(new Error(refused(name)), { refuse: true });
  };
  const seen = {
    read: false,
    get(name) {
      if (String(name).startsWith('exact.kept.')) return null;
      seen.read = true;
      return admitsSecret(admitted, name) ? store.get(name) ?? null : null;
    },
    set(name, value) { allowed(name); store.set(name, String(value)); },
    forget(name) { allowed(name); store.set(name, null); },
    keepKey(name, pair) { allowed(name); const handle = 'exact.key:' + crypto.randomUUID(); store.set(name, handle); return keys().then(service => service.put(handle, pair)); },
    key(name) { const handle = seen.get(name); return handle == null ? Promise.resolve(null) : keys().then(service => service.get(handle) ?? null); },
  };
  return seen;
}
