// SSH password prompts (MIT reference, see LICENSE-T3: packages/ssh/src/auth.ts,
// apps/web/src/components/desktop/SshPasswordPromptDialog.tsx,
// apps/desktop/src/ssh/DesktopSshPasswordPrompts.ts, at 1e2ecbd975).
// T3SshAuth.swift holds the queue and runs `ssh` again with the askpass helper; the
// password lives only in the native secure field and the child's environment, never
// here: the dialog's Continue asks the module to read the field it drew
// (`t3-ssh-password`, X35 / #134 workaround). This file has the pure parts, ported
// for their tests and for the dialog's view, and the dialog's data source.
import { obj, str, num } from './domain';
import { bridgeReply, type Native } from './protocol';

/** isSshAuthFailure: the messages ssh prints when the server refuses every method it offered. */
export function isSshAuthFailure(error: unknown): boolean {
  const normalized = (error instanceof Error ? error.message : String(error)).toLowerCase();
  return /permission denied \((?:publickey|password|keyboard-interactive|hostbased|gssapi-with-mic)[^)]*\)/u.test(normalized)
    || /authentication failed/u.test(normalized)
    || /too many authentication failures/u.test(normalized);
}

/** ASKPASS_POSIX_SCRIPT: ssh runs it through SSH_ASKPASS; it prints the secret from the environment, never a dialog. */
export const ASKPASS_POSIX_SCRIPT = `#!/bin/sh
# Invoked by ssh via SSH_ASKPASS when T3 Code re-runs ssh with a cached password
# from the renderer's in-app prompt. We never expose a native dialog here - if
# T3_SSH_AUTH_SECRET is missing, that's a caller bug and we fail loudly.
if [ "\${T3_SSH_AUTH_SECRET+x}" = "x" ]; then
  printf "%s\\n" "$T3_SSH_AUTH_SECRET"
  exit 0
fi
printf 'T3 Code ssh-askpass invoked without T3_SSH_AUTH_SECRET.\\n' >&2
exit 1
`;

/** buildSshChildEnvironment without the file writes: the helper's path is `<askpassDirectory>/ssh-askpass.sh`. */
export function buildSshChildEnvironment(input: { interactiveAuth?: boolean; baseEnv?: Record<string, string | undefined>; askpassDirectory: string; authSecret?: string | null; hostDisplay?: string }): Record<string, string | undefined> {
  const baseEnv = { ...input.baseEnv };
  if (!input.interactiveAuth) return baseEnv;
  return {
    ...baseEnv,
    SSH_ASKPASS: `${input.askpassDirectory.replace(/\/+$/u, '')}/ssh-askpass.sh`,
    SSH_ASKPASS_REQUIRE: 'force',
    ...(input.authSecret === undefined ? {} : { T3_SSH_AUTH_SECRET: input.authSecret ?? '' }),
    ...(baseEnv.DISPLAY || input.hostDisplay ? {} : { DISPLAY: 't3code' }),
  };
}

/** describeSshTarget: `user@destination` when the request names a user. */
export function describeSshTarget(request: { username: string | null; destination: string }): string {
  return request.username ? `${request.username}@${request.destination}` : request.destination;
}
export function formatRemainingSeconds(seconds: number): string {
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
}
export const EXPIRED_MESSAGE = 'This SSH password prompt expired. Try connecting again.';
export const KEYS_HINT = 'Use SSH keys to avoid repeated password prompts on new SSH sessions.';
/** getPromptErrorMessage. */
export function promptErrorMessage(error: unknown): string {
  const message = error instanceof Error ? error.message : typeof error === 'string' && error ? error : 'SSH password prompt failed.';
  return message.includes('expired') || message.includes('no longer pending') ? EXPIRED_MESSAGE : message;
}

export type SshPromptView = {
  open: boolean; requestId: string; target: string; prompt: string; remaining: string; expired: boolean;
  error: string; hint: string; queued: number;
};
const CLOSED: SshPromptView = { open: false, requestId: '', target: '', prompt: '', remaining: '', expired: false, error: '', hint: KEYS_HINT, queued: 0 };

/** The dialog for one queued request: its countdown against the window's wall time, and the error that replaced the hint. */
export function promptView(request: Record<string, unknown> | null, now: number, queued = 0): SshPromptView {
  if (!request || !str(request.requestId)) return CLOSED;
  const expiresAt = num(request.expiresAt);
  // A window clock read before the dialog opened is stale: the countdown never shows more than the request's lifetime.
  const lifetime = num(request.timeoutMs) > 0 ? num(request.timeoutMs) : Infinity;
  const remainingMs = expiresAt > 0 ? Math.min(lifetime, Math.max(0, expiresAt - now)) : null;
  const expired = request.expired === true || (remainingMs !== null && remainingMs <= 0);
  const error = expired ? EXPIRED_MESSAGE : str(request.error);
  return {
    open: true, requestId: str(request.requestId), target: describeSshTarget({ username: str(request.username) || null, destination: str(request.destination) }),
    prompt: str(request.prompt), remaining: remainingMs === null ? '' : expired ? 'Expired' : formatRemainingSeconds(Math.ceil(remainingMs / 1000)),
    expired, error, hint: error || KEYS_HINT, queued,
  };
}

/** The `sshPrompt` source: the queue's first request, asked again whenever the module's queue changes. */
export async function sshPromptSource(native: Native | null | undefined, now: number): Promise<SshPromptView> {
  if (!native?.available) return CLOSED;
  native.watch('t3.ssh-prompt');
  try {
    const reply = await bridgeReply(native, { op: 'sshPromptState' });
    if (!reply.ok) return CLOSED;
    const value = obj(reply.value);
    return promptView(value.request ? obj(value.request) : null, now, num(value.queued));
  } catch { return CLOSED; }
}

/**
 * The `sshPromptAnswer` source: `submit` has the module read the dialog's secure field for this
 * request (an expired one keeps the dialog with the expiry message), `cancel` answers none, and
 * `dismiss` removes an expired request. The module drops the request from the queue on success.
 */
export async function sshPromptAnswer(native: Native | null | undefined, requestId: string, answer: string): Promise<{ ok: boolean; error: string }> {
  if (!native?.available || !requestId) return { ok: false, error: '' };
  try {
    const reply = await bridgeReply(native, { op: 'sshPromptResolve', requestId, answer: answer === 'submit' || answer === 'dismiss' ? answer : 'cancel' });
    return reply.ok ? { ok: true, error: '' } : { ok: false, error: promptErrorMessage(reply.error?.message ?? '') };
  } catch (error) { return { ok: false, error: promptErrorMessage(error) }; }
}
