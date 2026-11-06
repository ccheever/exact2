// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/r6-media-preview.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Lane r6-media: the attachment preview's media bodies (MIT reference, see LICENSE-T3:
// components/files/AttachmentFilePreview.tsx, BrowserDocumentFrame.tsx, AudioPreview.tsx).
// A PDF and a rendered HTML page draw in the app module's `t3-media` hook (R6MediaPreview.swift:
// PDFKit; a WebKit view whose document is the attachment's bytes under a sandbox CSP with no
// network); audio and video are Contract `video` nodes with `controls` (AVKit). The surface reads
// "Unable to load audio." / "Unable to load video." when the player reports an error, as the
// reference's onError does; Try again re-mints the URL, and the error state in between unmounts
// the failed player, so the next one is a new node.

export type MediaPreview = 'pdf' | 'html' | 'audio' | 'video';
const MEDIA: readonly string[] = ['pdf', 'html', 'audio', 'video'];

/** Whether a filePreviewKind draws as a media body (not text, not an image, not "No preview"). */
export const isMediaPreview = (kind: string): kind is MediaPreview => MEDIA.includes(kind);

/** AttachmentFilePreview's onError messages (an image's `error` since exact2 #121); a PDF or page frame reports none. */
export function mediaErrorMessage(kind: string): string {
  return kind === 'audio' ? 'Unable to load audio.' : kind === 'video' ? 'Unable to load video.' : kind === 'image' ? 'Unable to load image.' : '';
}

/** The body for a loaded URL: text kinds and images are r5-panels' own; the rest is media or "No preview". */
export function mediaBody(kind: string, rendered: boolean): MediaPreview | '' {
  if (kind === 'html') return rendered ? 'html' : '';
  return isMediaPreview(kind) ? kind : '';
}
