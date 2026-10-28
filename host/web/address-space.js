// The local network on the web (LLP 1133.036 D6): the address space Local
// Network Access gives a destination as its URL writes it and the page
// (https://wicg.github.io/local-network-access/), each space's permission as
// the browser answers it, and the rule a tool's, a wake's or a package
// event's fetch or WebSocket open passes: the JS target's (host/web-js
// admission.js) and the page module's (http-body.js `request`).
import { isLocalNetwork } from './grant-admission.js';

const RANK = { public: 0, local: 1, loopback: 2 };
/** The space of `host` as written: `loopback`, `local` or `public`; a name the browser resolves is `public` here. */
export const spaceOf = host => isLocalNetwork(host, true) ? (isLocalNetwork(host, false) ? 'local' : 'loopback') : 'public';

// Each space's permission as the browser answers it (D8): `unknown` where it
// answers neither name. `local-network-access` is Chrome's name before 145.
const states = { local: 'unknown', loopback: 'unknown' };
const NAMES = { local: ['local-network', 'local-network-access'], loopback: ['loopback-network', 'local-network-access'] };
async function watch(space) {
  for (const name of NAMES[space]) {
    try {
      const status = await navigator.permissions.query({ name });
      states[space] = status.state;
      status.onchange = () => { states[space] = status.state; };
      return;
    } catch { /* a name this browser does not know */ }
  }
}
if (typeof navigator !== 'undefined' && navigator.permissions?.query) { watch('local'); watch('loopback'); }

/** The page's own space. */
const pageSpace = () => spaceOf(globalThis.location?.hostname ?? '');

/** Under the agent: the substitute's state for `space` (`local` or `loopback`), which stands for the browser's (D19). */
export function setSpaceState(space, state) { if (space in states) states[space] = state; }

/**
 * D6: why a fetch of `url` or a WebSocket open asked by `origin` is refused
 * before it is sent, or null. Only a tool's, a wake's or the device's reach
 * to a space less public than the page's is checked, against that space's
 * permission, or `state` where the runner's substitute answers it.
 */
export function reachRefusal(url, origin, state = null) {
  if (!origin || origin === 'app') return null;
  let host;
  try { host = new URL(url).hostname; } catch { return null; }
  const space = spaceOf(host.includes(':') && !host.startsWith('[') ? `[${host}]` : host);
  if (RANK[space] <= RANK[pageSpace()]) return null;
  state ??= states[space];
  return state === 'granted' ? null : state === 'denied' ? 'local network access is denied' : 'person-required';
}

/**
 * D6's redirect gate: a tool's, a wake's or the device's fetch that would
 * follow redirects goes out with `redirect: "error"` while the browser
 * answers a less public space's permission and does not answer `granted`, so
 * the browser never requests a hop that could prompt.
 */
export function redirectFor(origin, redirect, state = null) {
  if (!origin || origin === 'app' || redirect !== 'follow') return redirect;
  const page = RANK[pageSpace()];
  const held = Object.entries(states).some(([space, answered]) => RANK[space] > page && ![null, 'unknown', 'granted'].includes(state ?? answered));
  return held ? 'error' : redirect;
}
