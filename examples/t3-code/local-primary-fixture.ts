// Tests: the primary environment's state (local-primary.ts) as a scenario needs it. The singleton
// is the app's one primary; each test file that sets it resets it after each test.
import { parseLocalBackendStatus, unknownLocalBackend } from './local-backend';
import { primary } from './local-primary';

/** This Mac's embedded server, ready at `origin` as environment `environmentId`. */
export function primaryAt(origin: string, environmentId: string, label = 'This Mac', enabled = true): void {
  primary.update(parseLocalBackendStatus({ state: 'ready', enabled, httpBaseUrl: origin, wsBaseUrl: origin.replace(/^http/, 'ws'), bearerReady: true, environmentId, label }), enabled);
}
/** A development build without its lane variables: no primary (the hosted rules apply). */
export function noPrimary(): void {
  primary.update(parseLocalBackendStatus({ state: 'refused', refused: 'Development build: set T3_LOCAL_HOME and T3_LOCAL_PORT to start the local server.' }), true);
}
/** The Local environment switched off. */
export function primaryOff(): void { primary.update(parseLocalBackendStatus({ state: 'stopped', enabled: false }), false); }
export function resetPrimary(): void { primary.update(unknownLocalBackend(), true); }
