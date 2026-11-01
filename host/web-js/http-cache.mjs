// The render server's HTTP cache (LLP 1153.001 D7): one shared cache in front
// of every render's outbound fetch, as RFC 9111 defines one. A render is
// anonymous (LLP 1048.002), so an answer one render was given is an answer
// any render may be given, for as long as the origin's headers allow.
//
// It stores a GET's final answer only when the origin gave it an explicit
// lifetime (`s-maxage`, `max-age` or `Expires`; no heuristic freshness) and
// nothing forbids a shared cache to keep it (`no-store`, `private`,
// `no-cache`, `Vary: *`, or a request with `Authorization` the response did
// not mark shareable, RFC 9111 §3.5). A request whose fetch `cache` mode or
// `Cache-Control` asks past the cache goes past it, as a browser's would.
// A stale answer within `stale-while-revalidate` (RFC 5861) is given while one
// refetch replaces it. A redirected answer is not kept: the runtime checks the
// URL a redirect led to against the app's grants (admission.js).

const SHAREABLE = new Set([200, 203, 204, 404, 405, 410, 414, 501]);
/** On each response the cached fetch gives: how many ms it stays fresh for any reader (0: not shareable, or stale). */
export const FRESH = Symbol.for('exact.fresh');
const fresh = (response, ms) => Object.defineProperty(response, FRESH, { value: Math.max(0, ms), configurable: true });
const directives = value => {
  const out = new Map();
  for (const part of (value ?? '').toLowerCase().split(',')) {
    const [name, arg] = part.split('=').map(s => s?.trim());
    if (name) out.set(name, arg?.replace(/^"|"$/g, '') ?? true);
  }
  return out;
};
const seconds = v => { const n = Number(v); return Number.isFinite(n) && n >= 0 ? n : null; };

/** A fetch with a shared cache before `outbound`. `now()` is ms since the epoch. */
export function cachingFetch(outbound, { entries = 256, bytes = 64 << 20, entryBytes = 8 << 20, now = Date.now } = {}) {
  // URL → { status, statusText, headers, body, stored, initialAge, lifetime, stale, vary }; insertion order is recency.
  const cache = new Map(), refreshing = new Map(), storing = new Set();
  let held = 0;
  const drop = url => { const e = cache.get(url); if (e) { held -= e.body.byteLength; cache.delete(url); } };
  const age = e => e.initialAge + (now() - e.stored) / 1000;
  // A constructed Response has no URL; the answer keeps the one it was fetched from, as a browser's cache gives it.
  const answer = (url, e) => {
    const headers = new Headers(e.headers);
    headers.set('age', String(Math.floor(age(e))));
    return fresh(Object.defineProperty(new Response(e.status === 204 ? null : e.body, { status: e.status, statusText: e.statusText, headers }), 'url', { value: url }), (e.lifetime - age(e)) * 1000);
  };

  // Stores `response` once its body has been read, beside the caller's own read.
  function keep(url, request, response, sent) {
    fresh(response, 0);
    if (response.redirected || !SHAREABLE.has(response.status)) return;
    const cc = directives(response.headers.get('cache-control'));
    if (cc.has('no-store') || cc.has('private') || cc.has('no-cache')) return;
    if (request.headers.has('authorization') && !cc.has('public') && !cc.has('s-maxage') && !cc.has('must-revalidate')) return;
    const varyNames = (response.headers.get('vary') ?? '').toLowerCase().split(',').map(s => s.trim()).filter(Boolean);
    if (varyNames.includes('*')) return;
    const date = Date.parse(response.headers.get('date') ?? ''), received = now();
    let lifetime = seconds(cc.get('s-maxage')) ?? seconds(cc.get('max-age'));
    if (lifetime === null && response.headers.has('expires')) {
      const expires = Date.parse(response.headers.get('expires'));
      lifetime = Number.isNaN(expires) ? 0 : Math.max(0, (expires - (Number.isNaN(date) ? received : date)) / 1000);
    }
    if (!lifetime) return;
    // RFC 9111 §4.2.3's initial age.
    const apparent = Number.isNaN(date) ? 0 : Math.max(0, (received - date) / 1000);
    const initialAge = Math.max(apparent, (seconds(response.headers.get('age')) ?? 0) + (received - sent) / 1000);
    const vary = varyNames.map(name => [name, request.headers.get(name)]);
    const length = Number(response.headers.get('content-length'));
    if (length > entryBytes) return;
    fresh(response, (lifetime - initialAge) * 1000);
    const stored = response.clone().arrayBuffer().then(body => {
      if (body.byteLength > entryBytes) return;
      drop(url);
      cache.set(url, { status: response.status, statusText: response.statusText, headers: [...response.headers], body, stored: received, initialAge, lifetime, stale: seconds(cc.get('stale-while-revalidate')) ?? 0, vary });
      held += body.byteLength;
      for (const [old] of cache) { if (cache.size <= entries && held <= bytes) break; drop(old); }
    }, () => {}).finally(() => storing.delete(stored));
    storing.add(stored);
  }

  async function go(url, request, input, init) {
    const sent = now();
    const response = await outbound(input, init);
    keep(url, request, response, sent);
    return response;
  }

  function fetch(input, init) {
    // Only a GET is looked at; anything else goes out untouched (a Request made of it would take its body).
    const method = init?.method ?? (input instanceof Request ? input.method : 'GET');
    const around = () => outbound(input, init).then(r => fresh(r, 0));
    if (String(method).toUpperCase() !== 'GET') return around();
    let request;
    try { request = new Request(input, init); } catch { return around(); }
    const asked = directives(request.headers.get('cache-control'));
    if (request.cache === 'no-store' || asked.has('no-store')) return around();
    const url = request.url, e = cache.get(url);
    const past = request.cache === 'reload' || request.cache === 'no-cache' || asked.has('no-cache') || asked.get('max-age') === '0';
    if (e && !past && e.vary.every(([name, value]) => request.headers.get(name) === value)) {
      const a = age(e);
      if (a < e.lifetime + e.stale) {
        if (request.signal.aborted) return Promise.reject(request.signal.reason);
        cache.delete(url); cache.set(url, e);
        if (a >= e.lifetime && !refreshing.has(url)) {
          // Its own request: the caller's signal ends the caller's wait, not the cache's refetch.
          refreshing.set(url, go(url, request, url, { headers: request.headers, redirect: request.redirect }).catch(() => {}).finally(() => refreshing.delete(url)));
        }
        return Promise.resolve(answer(url, e));
      }
      drop(url);
    }
    return go(url, request, input, init);
  }
  /** Settles once every answer being read into the cache is stored. */
  fetch.stored = async () => { while (storing.size || refreshing.size) await Promise.allSettled([...storing, ...refreshing.values()]); };
  return fetch;
}
