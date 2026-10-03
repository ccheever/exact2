// App-module fetch binding, injected by the existing bundler, never installed
// as a page-wide global. Host asset/module fetches keep their own authority.
import { installGrants } from '../web/http-body.js';
let authority = { error: 'the source has not installed its grants', permits: () => false };
export function install(data, spec) { return authority = installGrants(data, 'typescript', spec); }
export async function fetch(input, options = {}) {
  const url = typeof Request === 'function' && input instanceof Request ? input.url : new URL(String(input), globalThis.location?.href).href;
  const request = typeof Request === 'function' && input instanceof Request ? input : null, method = options.method ?? request?.method ?? 'GET';
  const asset = method === 'GET' && !options.body && !request?.body && [...new Headers(options.headers ?? request?.headers)].length === 0
    && /^\/assets\/(?:[A-Za-z0-9_-]+\/)*[A-Za-z0-9_-]+\.[A-Za-z0-9]+$/.test(String(input));
  if (authority.error || !asset && !authority.permits(url)) throw Object.assign(new Error(`refused by grant: ${url}${authority.error ? ': ' + authority.error : ''}`), { kind: 'Refused' });
  return globalThis.fetch(input, { ...options, redirect: 'error' });
}

// The usual browser global spellings share this app-local view. Computed
// access, aliases and destructuring therefore get the same scoped fetch.
const methods = new WeakMap();
export const appGlobal = new Proxy(globalThis, { get(target, name) {
  if (name === 'fetch') return fetch;
  if (name === 'globalThis' || name === 'self' || name === 'window') return appGlobal;
  const value = Reflect.get(target, name, target);
  if (typeof value !== 'function') return value;
  if (!methods.has(value)) methods.set(value, new Proxy(value, { apply(fn, receiver, args) { return Reflect.apply(fn, receiver === appGlobal ? target : receiver, args); } }));
  return methods.get(value);
} });
