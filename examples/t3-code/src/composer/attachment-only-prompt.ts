// The prompt an attachment-only message is sent with, and its round trip through a queued edit. Adapted from T3 Code
// 2359643cff (MIT; see LICENSE-T3): packages/shared/src/composerContextReferences.ts. No imports, so the timeline's
// minimap and the prompt links can read it too.
/** ATTACHMENT_ONLY_BOOTSTRAP_PROMPT: the text an attachment-only message sends. */
export const ATTACHMENT_ONLY_PROMPT = '[User attached one or more files without additional text. Respond using the conversation context and the attached files.]';
/** isAttachmentOnlyBootstrapPrompt: Claude's ultrathink effort sends the prompt as `Ultrathink:\n<prompt>`. */
export function isAttachmentOnlyPrompt(text: string): boolean {
  return text.replace(/^Ultrathink:\s*/i, '') === ATTACHMENT_ONLY_PROMPT;
}
/** stripAttachmentOnlyBootstrapPrompt: an attachment-only message edits as an empty field, keeping "Ultrathink:". */
export function stripAttachmentOnlyPrompt(text: string): string {
  return isAttachmentOnlyPrompt(text) ? text.slice(0, -ATTACHMENT_ONLY_PROMPT.length).trimEnd() : text;
}
/** restoreAttachmentOnlyBootstrapPrompt: an edit left empty (or "Ultrathink:" alone) with attachments sends the prompt. */
export function restoreAttachmentOnlyPrompt(text: string, hasAttachments: boolean): string {
  if (!hasAttachments) return text;
  const trimmed = text.trim();
  if (trimmed.length === 0) return ATTACHMENT_ONLY_PROMPT;
  return /^Ultrathink:\s*$/i.test(trimmed) ? `Ultrathink:\n${ATTACHMENT_ONLY_PROMPT}` : text;
}
