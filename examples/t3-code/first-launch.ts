// The first-launch view (20261005-portable-app-download item 3): the first launch of a downloaded
// build unpacks the embedded server's runtime into `<T3 home>/runtime/versions/<version>`
// (T3LocalRuntime.swift; about 260 MB, a few seconds), and the window says so instead of showing an
// app that cannot connect yet. The reference has nothing to unpack (its Electron app runs the server
// from its own bundle), so this view is new: "Setting up T3 Code…", the stage, and a progress bar;
// on a failure (no disk space, a damaged archive, an interrupted unpack) the reason with Retry and
// Quit. It reads the embedded server's status (local-backend.ts, `install {phase, fraction, reason}`).
import { bridgeReply, type Native } from './protocol';
import { parseLocalBackendStatus, type LocalBackendStatus } from './local-backend';

export type FirstLaunchView = {
  /** The runtime is being unpacked, or its unpack failed. */
  show: boolean;
  failed: boolean;
  title: string;
  /** The stage line: it changes only at a stage change and every 25 % (it is the live region). */
  status: string;
  /** The bar's fill, 0–100 (every progress report, about every 2 %). */
  percent: number;
  error: string;
};

export const SETUP_TITLE = 'Setting up T3 Code…';
export const SETUP_FAILED_TITLE = 'T3 Code could not be set up';
export const CHECKING = 'Checking the server files…';
export const unpacking = (fraction: number) => `Unpacking the server files… ${Math.min(100, Math.floor(Math.max(0, fraction) * 4) * 25)}%`;

/** A failed runtime install is `failed` with the install's reason (a port failure has none and is fatal). */
export const installFailed = (status: LocalBackendStatus) => status.state === 'failed' && !!status.install?.reason;

export function firstLaunchView(status: LocalBackendStatus | undefined): FirstLaunchView {
  if (!status) return { show: false, failed: false, title: SETUP_TITLE, status: '', percent: 0, error: '' };
  const failed = installFailed(status);
  const show = failed || status.state === 'installing';
  const extracting = !failed && status.install?.phase === 'extract';
  const fraction = Math.min(1, Math.max(0, status.install?.fraction ?? 0));
  return {
    show, failed,
    title: failed ? SETUP_FAILED_TITLE : SETUP_TITLE,
    status: !show || failed ? '' : extracting ? unpacking(fraction) : CHECKING,
    percent: extracting ? Math.round(fraction * 100) : 0,
    error: failed ? status.install?.reason ?? '' : '',
  };
}

/**
 * The view's two buttons (`pageslocal:welcome-setup-retry`, `…-setup-quit`): Retry runs the install
 * again (the native side publishes `installing` at once, then `starting` or the next failure);
 * Quit quits the app. The status that comes back replaces the client's copy.
 */
export async function firstLaunchLocal(client: { localBackend: LocalBackendStatus }, native: Native, op: 'setup-retry' | 'setup-quit'): Promise<string> {
  if (op === 'setup-quit') {
    await bridgeReply(native, { op: 'localBackendQuit' });
    return '';
  }
  const reply = await bridgeReply(native, { op: 'localBackendRetry' });
  if (reply.ok) client.localBackend = parseLocalBackendStatus(reply.value);
  // A failure is already the status's (`failed` with the new reason), which the view shows.
  return '';
}
