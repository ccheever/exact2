// ProviderAuthTerminal and the terminal branch of ProviderAuthenticationSection,
// T3 Code 1e2ecbd975 (MIT, see LICENSE-T3): apps/web/src/components/settings/
// ProviderAuthTerminal.tsx and ProviderAuthenticationSection.tsx:337-390. The terminal
// view (T3TerminalView auth mode) writes the transcript and sends the input itself
// (`provider.auth.respond`, T3TerminalSessions.providerAuthCall); the streams and the
// rest of the Account row are provider-setup.ts.

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
