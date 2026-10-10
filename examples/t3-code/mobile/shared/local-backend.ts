// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/local-backend.ts at 38352ceaf4cd35a40b7b24ce992db87c2357a99b.
// The embedded T3 server's state for TypeScript (20261005-embedded-server-runtime). The native
// module runs one local backend per app (T3LocalBackend.swift); this reads its status
// (`localBackendStatus`) and watches `t3.local`, so `20261005-local-primary-environment` can show
// "This machine" from it. No UI here. The bearer stays native (memory only); TypeScript sees only
// whether it is ready.
import { obj, str, num } from './domain';
import { bridgeReply, type Native } from './protocol';

export type LocalBackendState = 'refused' | 'runtime-missing' | 'installing' | 'starting' | 'ready' | 'restarting' | 'stopped' | 'failed';
const STATES: readonly LocalBackendState[] = ['refused', 'runtime-missing', 'installing', 'starting', 'ready', 'restarting', 'stopped', 'failed'];

export interface LocalBackendStatus {
  state: LocalBackendState;
  port: number | null;
  httpBaseUrl: string;
  wsBaseUrl: string;
  bearerReady: boolean;
  restartAttempt: number;
  /** Milliseconds until a scheduled restart, when one is scheduled. */
  nextRestartMs: number | null;
  /** The last exit of a server run: `code=<n>` or `signal=<n>`. */
  lastExit: string;
  /** While installing: the phase and its fraction; when missing or failed: the reason. */
  install: { phase: string; fraction: number; reason: string } | null;
  /** Why a development build starts nothing (item 8 of the ticket). */
  refused: string;
  /** Port selection and other start failures. */
  failure: string;
  version: string;
  pid: number | null;
}

export const unknownLocalBackend = (): LocalBackendStatus => ({
  state: 'stopped', port: null, httpBaseUrl: '', wsBaseUrl: '', bearerReady: false, restartAttempt: 0,
  nextRestartMs: null, lastExit: '', install: null, refused: '', failure: '', version: '', pid: null,
});

const numberOrNull = (value: unknown): number | null => (typeof value === 'number' && Number.isFinite(value) ? value : null);

/** The native status object, as typed fields (unknown values fall back to their empty form). */
export function parseLocalBackendStatus(value: unknown): LocalBackendStatus {
  const raw = obj(value), install = raw.install === undefined || raw.install === null ? null : obj(raw.install);
  const state = STATES.includes(str(raw.state) as LocalBackendState) ? (str(raw.state) as LocalBackendState) : 'stopped';
  return {
    state,
    port: numberOrNull(raw.port),
    httpBaseUrl: str(raw.httpBaseUrl),
    wsBaseUrl: str(raw.wsBaseUrl),
    bearerReady: raw.bearerReady === true,
    restartAttempt: num(raw.restartAttempt) || 0,
    nextRestartMs: numberOrNull(raw.nextRestartMs),
    lastExit: str(raw.lastExit),
    install: install ? { phase: str(install.phase), fraction: num(install.fraction) || 0, reason: str(install.reason) } : null,
    refused: str(raw.refused),
    failure: str(raw.failure),
    version: str(raw.version),
    pid: numberOrNull(raw.pid),
  };
}

/** Reads the status and asks to be read again when `t3.local` changes. */
export async function readLocalBackend(native: Native): Promise<LocalBackendStatus> {
  native.watch('t3.local');
  const reply = await bridgeReply(native, { op: 'localBackendStatus' });
  return reply.ok ? parseLocalBackendStatus(reply.value) : unknownLocalBackend();
}
