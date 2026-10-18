// The attachment image size limit, adapted from T3 Code 9c161462c9 (MIT; see LICENSE-T3):
// packages/client-runtime/src/imageAttachmentSize.ts. Draft images are decoded and encoded
// natively, so the requests that stage them (Attach files, a paste, a sidebar row drop, a SnapShot
// capture) carry this edge and T3ImageFit.swift scales a PNG, JPEG or HEIC source over it.

/**
 * Longest edge, in pixels, a PNG or JPEG attachment keeps. Anthropic rejects an image over
 * 2000px on a side once a request holds more than 20 images, and Claude Code forwards images
 * sent into a running turn unchanged, so one large paste breaks every later request in that
 * thread.
 */
export const IMAGE_ATTACHMENT_MAX_EDGE = 2000;
