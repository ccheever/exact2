// client.ts's seam for "This machine" (20261005-local-primary-environment): the primary's status and
// switch (local-primary.ts) and the running threads' kept streams (keep-alive.ts), through one import.
// 20261005-this-machine-network-access adds the exposure keys of t3-code.json (server-exposure.ts).
import { adoptLocalPrefs as adoptPrimaryPrefs } from './local-primary';
import { adoptNetworkPrefs } from './server-exposure';
import type { Obj } from './domain';
export { refreshLocal } from './local-primary';
export { unknownLocalBackend, type LocalBackendStatus } from './local-backend';
export { adoptHandoff, keepAliveEvent, keptThread } from './keep-alive';
/** load(): the Local environment switch, Network access, Tailscale Serve and the default endpoint. */
export function adoptLocalPrefs(next: object, saved: Obj): void { adoptPrimaryPrefs(next, saved); adoptNetworkPrefs(next, saved); }
