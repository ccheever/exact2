import { admitsNetwork, admitsSecret, coversPath, grantError, hasGrant, rawGrantText, scopedGrantSet, unionGrantSets } from '../web/grant-admission.js';
export { admitsNetwork, admitsSecret, coversPath, grantError, hasGrant, rawGrantText, scopedGrantSet, unionGrantSets } from '../web/grant-admission.js';

// Captured while the entry graph loads. App source injection never replaces
// the page global, so separately loaded host modules retain browser authority.
const browserFetch = globalThis.fetch?.bind(globalThis);

export class FetchError extends Error {
  constructor(kind, message) { super(String(message)); this.name = 'FetchError'; this.kind = String(kind); }
}

export async function fetchWith(set, input, init = {}) {
  const value = typeof Request === 'function' && input instanceof Request ? input.url : new URL(String(input), globalThis.location?.href).href;
  const invalid = grantError(set);
  if (invalid) throw new FetchError('Refused', invalid);
  if (!admitsNetwork(set, value)) throw new FetchError('Refused', `outside the app's grants (${/^wss?:/i.test(value) ? 'net.websocket' : 'net.fetch'})`);
  try { return await browserFetch(input, { ...init, redirect: 'error' }); }
  catch (error) { throw new FetchError('Network', error?.message ?? error); }
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
      storage ??= import(new URL('storage-request.js', document.baseURI).href).then(m => m.createStorageRequests(appId, sourceSet));
      return { storage: await (await storage).run(req.storage, admitted) };
    }
    try {
      const response = await fetchWith(admitted, req.url, { method: req.method, headers: req.headers, body: ['GET', 'HEAD'].includes(req.method) ? undefined : req.raw });
      return { status: response.status, headers: [...response.headers], body: new Uint8Array(await readBody(response, req.maxResponseBytes)) };
    } catch (error) {
      return { failed: error?.kind === 'Refused' ? 2 : 1, message: String(error?.message ?? error) };
    }
  };
}

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
    get(name) { seen.read = true; return admitsSecret(admitted, name) ? store.get(name) ?? null : null; },
    set(name, value) { allowed(name); store.set(name, String(value)); },
    forget(name) { allowed(name); store.set(name, null); },
    keepKey(name, pair) { allowed(name); const handle = 'exact.key:' + crypto.randomUUID(); store.set(name, handle); return keys().then(service => service.put(handle, pair)); },
    key(name) { const handle = seen.get(name); return handle == null ? Promise.resolve(null) : keys().then(service => service.get(handle) ?? null); },
  };
  return seen;
}
