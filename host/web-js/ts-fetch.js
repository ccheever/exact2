// App-module fetch binding, injected by the bundler, never installed as a
// page-wide global. Host modules keep the browser function at every load time.
import { fetchWith } from './admission.js';
import { tsGrantSet } from './admission-data.js';
export const fetch = (input, options) => fetchWith(tsGrantSet, input, options);

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
