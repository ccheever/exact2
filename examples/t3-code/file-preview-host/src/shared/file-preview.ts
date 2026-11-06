// What the viewers read about a file (MIT reference, see LICENSE-T3: packages/shared/src/filePreview.ts
// decodeFilePreviewText and FILE_TEXT_PREVIEW_MAX_BYTES, packages/client-runtime/src/state/attachments.ts
// formatAttachmentSize; T3 Code 7c3b3b3605ef). The classifiers are the app's own (src/panel/file-preview-kind.ts).
export { filePreviewKind, type OfficePreviewKind } from "../../../src/panel/file-preview-kind";

export const FILE_TEXT_PREVIEW_MAX_BYTES = 1024 * 1024;

export function decodeFilePreviewText(bytes: Uint8Array, truncated = false) {
  const bounded = bytes.subarray(0, FILE_TEXT_PREVIEW_MAX_BYTES);
  if (bounded.some((byte) => byte === 0))
    throw new Error("This file contains binary data and cannot be shown as text.");
  const decoder = new TextDecoder("utf-8", { fatal: true });
  try {
    return {
      text: decoder.decode(bounded, { stream: truncated || bytes.length > bounded.length }),
      truncated: truncated || bytes.length > bounded.length,
    };
  } catch {
    throw new Error("This file is not UTF-8 text. Open it in another app to view its contents.");
  }
}

export function formatAttachmentSize(sizeBytes: number): string {
  return sizeBytes >= 1024 * 1024
    ? `${(sizeBytes / (1024 * 1024)).toFixed(1)} MB`
    : `${Math.max(1, Math.ceil(sizeBytes / 1024))} KB`;
}
