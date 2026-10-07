// client.ts's seam for "This machine" (20261005-local-primary-environment): the primary's status and
// switch (local-primary.ts) and the running threads' kept streams (keep-alive.ts), through one import.
export { adoptLocalPrefs, refreshLocal } from './local-primary';
export { unknownLocalBackend, type LocalBackendStatus } from './local-backend';
export { adoptHandoff, keepAliveEvent, keptThread } from './keep-alive';
