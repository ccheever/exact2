// Codex `/feedback`, T3 Code 1e2ecbd975 (MIT, see LICENSE-T3):
// packages/client-runtime/src/state/threadFeedback.ts (parseCodexFeedbackCommand,
// codexFeedbackNotice, beginCodexFeedbackSubmission, codexFeedbackMessage, submitCodexFeedback).
// Changes: an atom command's result is the clone's `CommandResult` (success, failure, or a failure
// that was interrupted) in place of `AtomCommandResult`, so `isAtomCommandInterrupted` and
// `squashAtomCommandFailure` are `isCommandInterrupted` and `squashCommandFailure`; `MessageId`
// is a string. The functions are otherwise the reference's, line for line.

export type CommandResult<A> =
  | { readonly _tag: 'Success'; readonly value: A }
  | { readonly _tag: 'Failure'; readonly interrupted: boolean; readonly error: unknown };
export const commandSuccess = <A>(value: A): CommandResult<A> => ({ _tag: 'Success', value });
export const commandFailure = <A>(error: unknown): CommandResult<A> => ({ _tag: 'Failure', interrupted: false, error });
export const commandInterrupted = <A>(): CommandResult<A> => ({ _tag: 'Failure', interrupted: true, error: null });
export const isCommandInterrupted = <A>(result: CommandResult<A>): boolean => result._tag === 'Failure' && result.interrupted;
export const squashCommandFailure = <A>(result: CommandResult<A>): unknown => (result._tag === 'Failure' ? result.error : null);

export type ProviderUploadFeedbackResult = { readonly feedbackId: string };

type CodexFeedbackSubmissionDetails = {
  readonly id: string;
  readonly command: string;
  readonly createdAt: string;
};

export type CodexFeedbackSubmission = CodexFeedbackSubmissionDetails &
  (
    | { readonly status: 'uploading' | 'interrupted' }
    | { readonly status: 'sent'; readonly feedbackId: string }
    | { readonly status: 'failed'; readonly errorMessage: string }
  );

export function parseCodexFeedbackCommand(text: string): { readonly reason?: string } | null {
  const match = /^\/feedback(?:\s+([\s\S]*))?$/iu.exec(text.trim());
  if (!match) {
    return null;
  }
  const reason = match[1]?.trim();
  return reason ? { reason } : {};
}

export function codexFeedbackNotice(submission: CodexFeedbackSubmission) {
  switch (submission.status) {
    case 'interrupted':
      return null;
    case 'uploading':
      return { title: 'Sending feedback to OpenAI...', description: undefined };
    case 'sent':
      return {
        title: 'Feedback sent to OpenAI',
        description: `Thread ID: ${submission.feedbackId}`,
      };
    case 'failed':
      return { title: 'Could not send feedback to OpenAI', description: submission.errorMessage };
  }
}

export function beginCodexFeedbackSubmission(
  submissionsInFlight: Set<string>,
  threadKey: string,
): (() => void) | null {
  if (submissionsInFlight.has(threadKey)) return null;
  submissionsInFlight.add(threadKey);
  return () => submissionsInFlight.delete(threadKey);
}

/** A chat row that exists only on this client, such as a feedback exchange. */
export interface LocalChatMessage {
  readonly id: string;
  readonly role: 'user' | 'assistant';
  readonly text: string;
  readonly turnId: null;
  readonly streaming: boolean;
  readonly createdAt: string;
  readonly updatedAt: string;
}

export function codexFeedbackMessage(
  submission: CodexFeedbackSubmission,
  role: 'user' | 'assistant' = 'user',
): LocalChatMessage {
  const text =
    role === 'user'
      ? submission.command
      : submission.status === 'sent'
        ? `Feedback sent to OpenAI.\n\nThread ID: \`${submission.feedbackId}\``
        : submission.status === 'failed'
          ? `Could not send feedback to OpenAI.\n\n${submission.errorMessage}`
          : 'Sending feedback to OpenAI...';

  return {
    id: role === 'user' ? submission.id : `${submission.id}:feedback`,
    role,
    text,
    turnId: null,
    streaming: false,
    createdAt: submission.createdAt,
    updatedAt: submission.createdAt,
  };
}

export async function submitCodexFeedback(input: {
  readonly submission: CodexFeedbackSubmissionDetails;
  readonly clearDraft: () => void;
  readonly onUpdate: (submission: CodexFeedbackSubmission) => void;
  readonly upload: () => Promise<CommandResult<ProviderUploadFeedbackResult>>;
}): Promise<CommandResult<ProviderUploadFeedbackResult>> {
  input.onUpdate({ ...input.submission, status: 'uploading' });
  input.clearDraft();

  const result = await input.upload();
  if (result._tag === 'Success') {
    input.onUpdate({
      ...input.submission,
      status: 'sent',
      feedbackId: result.value.feedbackId,
    });
  } else if (isCommandInterrupted(result)) {
    input.onUpdate({ ...input.submission, status: 'interrupted' });
  } else {
    const error = squashCommandFailure(result);
    input.onUpdate({
      ...input.submission,
      status: 'failed',
      errorMessage: error instanceof Error ? error.message : 'An error occurred.',
    });
  }

  return result;
}
