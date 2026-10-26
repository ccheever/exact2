// ProviderAuthTerminal and the terminal branch of ProviderAuthenticationSection,
// T3 Code 1e2ecbd975 (MIT, see LICENSE-T3): apps/web/src/components/settings/
// ProviderAuthTerminal.tsx and ProviderAuthenticationSection.tsx:337-390. The terminal page
// (terminal-host/src/entry.ts in transcript mode) writes the transcript and sends the input over
// its own socket (`provider.auth.respond`, one at a time); the streams and the rest of the
// Account row are provider-setup.ts.
import { terminalFrameView, type TerminalFrameView } from '../terminal/terminal-frames';

/** The sign-in terminals' frames (terminal-frames.ts), one per provider instance. */
export const AUTH_TERMINALS = {};
export const authFrameId = (instanceId: string): string => `terminal-auth-${instanceId}`.replace(/[^A-Za-z0-9_-]/g, '_');

/** The sign-in terminal's frame: the page in transcript mode for the flow `<flowId>:<interactionId>`. */
export function authTerminalFrame(instanceId: string, flowId: string, interactionId: string, output: string, offset: number): TerminalFrameView {
  const flow = `${flowId}:${interactionId}`;
  return terminalFrameView(AUTH_TERMINALS, authFrameId(instanceId), { mode: 'transcript', identity: { authFlow: flow }, readOnly: false, canAddToChat: false,
    auth: { instanceId, flowId, interactionId } }, flowId ? { flow, output, offset } : undefined);
}

/** The transcript's delta write: the new suffix, or a reset after a gap or a rewind. */
export function terminalTranscriptUpdate(written: number, output: string, outputOffset = output.length) {
  const delta = outputOffset - written;
  return { written: outputOffset, reset: delta !== 0 && !(delta > 0 && delta <= output.length), data: delta === 0 ? '' : delta > 0 && delta <= output.length ? output.slice(-delta) : output };
}

/** The row's error for one of the terminal view's events ('' clears it, as a recovered terminal does). */
export function terminalEventError(type: string): string {
  if (type === 'auth-error') return 'The provider sign-in terminal is no longer available.';
  if (type === 'link-error') return 'Could not open the provider link.';
  if (type === 'error') return 'Could not load the sign-in terminal. Cancel and retry sign-in.';
  return '';
}
