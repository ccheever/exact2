// The attachment preview's media bodies (MIT reference, see LICENSE-T3:
// components/files/AttachmentFilePreview.tsx, BrowserDocumentFrame.tsx, AudioPreview.tsx).
// A rendered HTML page is a Contract frame (media-frame.ts), and a PDF the viewer page's; audio and video are Contract
// `video` nodes with `controls` (AVKit). The surface reads "Unable to load audio." / "Unable to load video."
// when the player reports an error, as the reference's onError does; Try again re-mints the URL, and the error state in
// between unmounts the failed player, so the next one is a new node.

export type MediaPreview = 'html' | 'audio' | 'video';
const MEDIA: readonly string[] = ['html', 'audio', 'video'];

/** Whether a filePreviewKind draws as a media body (not text, not an image, not "No preview"). */
export const isMediaPreview = (kind: string): kind is MediaPreview => MEDIA.includes(kind);

/** AttachmentFilePreview's onError messages (an image's `error` since exact2 #121); a page frame reports none. */
export function mediaErrorMessage(kind: string): string {
  return kind === 'audio' ? 'Unable to load audio.' : kind === 'video' ? 'Unable to load video.' : kind === 'image' ? 'Unable to load image.' : '';
}

/** The body for a loaded URL: text kinds and images are the attachment surface's own; the rest is media or "No preview". */
export function mediaBody(kind: string, rendered: boolean): MediaPreview | '' {
  if (kind === 'html') return rendered ? 'html' : '';
  return isMediaPreview(kind) ? kind : '';
}
