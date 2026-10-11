// client.ts's seam for "This machine" (20261005-local-primary-environment): the primary's status and
// switch (local-primary.ts) and the running threads' kept streams (keep-alive.ts), through one import.
// The Local environment switch, Network access and Tailscale Serve live in desktop-settings.json (decision
// U7, T3DesktopSettings.swift); t3-code.json keeps only the default endpoint (the reference's renderer
// localStorage `t3code:ui-state:v1`, server-exposure.ts).
import { adoptNetworkPrefs } from './server-exposure';
import { hasLegacyDesktopKeys } from './local-primary';
import type { Obj } from './domain';
export { refreshLocal } from './local-primary';
export { unknownLocalBackend, type LocalBackendStatus } from './local-backend';
export { adoptHandoff, keepAliveEvent, keptThread } from './keep-alive';
/** load(): the default endpoint; true when the saved file still has the keys desktop-settings.json took over (save once to drop them). */
export function adoptLocalPrefs(next: object, saved: Obj): boolean { adoptNetworkPrefs(next, saved); return hasLegacyDesktopKeys(saved); }
