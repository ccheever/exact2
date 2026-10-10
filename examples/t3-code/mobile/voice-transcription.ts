// Pinned T3 Code365aa87982 packages/client-runtime/src/voice-input/transcription.ts; imports adapted only.
// Original SHA256 71245e9543a02ebd9dd3cc044df5fc82699426813beb979a73d402c42fc70b54. MIT: voice-LICENSE.
// @ref llp/1109.008-mobile-voice.decision.md#controller-reuse
/** Cancellation is cooperative: settle only after the underlying work has stopped. */
export type VoiceTranscriptionOptions = {
  readonly signal: AbortSignal;
};

/** Binds a recording to its selected implementation and resolved locale. */
export type PreparedVoiceTranscription = {
  readonly locale: string;
  readonly transcribe: (uri: string, options: VoiceTranscriptionOptions) => Promise<string>;
};

export type VoiceTranscriber = {
  readonly prepare: (options: VoiceTranscriptionOptions) => Promise<PreparedVoiceTranscription>;
};

export type VoiceTranscriptionErrorCode =
  | "unavailable"
  | "unsupported-locale"
  | "preparation-failed"
  | "transcription-failed"
  | "cancelled";

export class VoiceTranscriptionError extends Error {
  readonly code: VoiceTranscriptionErrorCode;

  constructor(code: VoiceTranscriptionErrorCode, message: string, options?: ErrorOptions) {
    super(message, options);
    this.name = "VoiceTranscriptionError";
    this.code = code;
  }
}

export function throwIfVoiceTranscriptionAborted(signal: AbortSignal): void {
  if (signal.aborted) {
    throw new VoiceTranscriptionError("cancelled", "Voice transcription was cancelled.");
  }
}
