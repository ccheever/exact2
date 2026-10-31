// The attachment image size limit, adapted from T3 Code 9c161462c9 (MIT; see LICENSE-T3):
// packages/client-runtime/src/imageAttachmentSize.ts. An attached image is fitted to this edge as it is
// staged (composer-attach-stage.ts, by compressImage); a SnapShot capture is fitted natively as it is saved
// (T3ImageFit.swift).

/**
 * Longest edge, in pixels, a PNG or JPEG attachment keeps. Anthropic rejects an image over
 * 2000px on a side once a request holds more than 20 images, and Claude Code forwards images
 * sent into a running turn unchanged, so one large paste breaks every later request in that
 * thread.
 */
export const IMAGE_ATTACHMENT_MAX_EDGE = 2000;
