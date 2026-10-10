---
name: 20261005-x29-video-pdf-app-files
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: [20261005-media-actions]
upstream_url: https://github.com/ccheever/exact2/issues/115
reproduced_on: null
rest_upstream_url: https://github.com/ccheever/exact2/issues/273
---

# X29: `video` and `audio` from app-written local files, and a PDF viewer element

Moved to main `issues/20261009-app-file-iframe-loading.md` (2026-10-09); tracked there.

## Summary

T3 Code plays video and audio and shows PDFs through ordinary browser elements (`<video>`, `<audio>`, an `<iframe>` with Chromium's PDF viewer) whose sources are local files, blob URLs or signed URLs. Exact2's `video` element took only http(s) or bundled-asset sources (`Mac/NodeViewMac.swift:402-407`), `image` took `app:/` files, and there was no PDF viewer element. The clone covers every surface with app-module native views (AVKit, PDFKit), which the agent cannot drive.

## Why it arose

### The T3 Code behavior
All paths at `1e2ecbd975` under `apps/web/src/`:
- **Composer shelf.** A dropped, pasted or picked video file shows a thumbnail tile: `<video muted playsInline preload="metadata">` cut to its first frame (`components/chat/ChatComposer.tsx:335-358`, `lib/videoFirstFrame.ts`). Opening the tile plays the file in a dialog (`components/chat/ExpandedImageDialog.tsx:47` `ExpandedVideo`). The file is a client-side `File`; the browser plays it through a blob URL.
- **Timeline.** A sent video is a 4:3 tile with the first frame whose whole face is the "Play <name>" button (`components/media/MediaVideoPlayer.tsx:167-215`). The source is the signed asset URL.
- **Attachment and Files previews.** Video: `<video controls playsInline src>` (`components/files/AttachmentFilePreview.tsx:250-258`). Audio: `<audio>` (`components/files/AudioPreview.tsx:9`). PDF: an `<iframe>` of the signed URL plus `#toolbar=0&view=FitH`, so the page shows alone, fitted to the panel width (`components/files/BrowserDocumentFrame.tsx:7,26-32`; Files panel `FilePreviewPanel.tsx:228,944`).

### Where the clone hits it
`20261005-media-actions` (nonblocking: its video slot and PDF/audio preview rows are attended, because the media actions attach to a native view instead of an element). Sent-video tiles already use the Contract `video url` element with the signed http URL (`r6-media-video.contract`, `R6VideoTile`).

## Clone workaround
- **Composer shelf video.** Staged files live in the app's data folder. `T3ComposerVideo.swift` plays them in an `AVPlayerView` through hook `t3-video` (typed links written beside the staged copy by `T3ComposerAttach.swift`); the shelf thumbnail is a frame cut by `AVAssetImageGenerator` (`T3ComposerAttach.swift`).
- **PDF and audio previews.** `R6MediaPreview.swift` (hook `t3-media`, `data-media-kind` pdf/html/audio/video) draws a `PDFView` scaled to the width and an `AVPlayerView` for audio and video (`r6-media.contract`; the Files panel uses the same hook for HTML, `r4-surfaces-files.contract`).
- Controls are AppKit's, not Chromium's; the PDF viewer has PDFKit's selection, find and zoom. Native views are outside the agent (X8), so checks inside them need a real-input session.
- #273's decision (2026-10-08): `PDFView` in `R6MediaPreview.swift` (the page alone, fitted to the width, on #282828) is a permanent declared difference. The clone's PDF is the server's signed `http` URL, so the `app:/` iframe main tracks does not apply to it.

## Evidence and history
- Filed 2026-10-06 as [#115](https://github.com/ccheever/exact2/issues/115).
- 2026-10-07 ([20261007-adopt-main-fixes-r4](../../tasks/closed/20261007-adopt-main-fixes-r4.md), main `463acda68`): main #205 (`e3b0be7ba`) closed #115: a bundled (`assets/`) PDF in an `iframe` is shown by WebKit's PDF view on macOS and iOS, and the web serves `.pdf` as `application/pdf`. `video` and `audio` from `app:/` were already on main (`4e2c79acb`). Adoption: none. `PDFView` stays because WebKit's PDF view does not take Chromium's `#toolbar=0&view=FitH` and draws its own white surround (#205's capture), while T3 Code shows the page alone on Chromium's #282828 surface fitted to the panel width.
- 2026-10-08: the rest filed as [#273](https://github.com/ccheever/exact2/issues/273), reproduced on main `0365ad1a4`: a PDF the data module wrote to `app:/data/media/doc.pdf`: the web's `iframe` becomes `about:blank`, macOS's stays blank, and both still fire `load`; a bundled `assets/doc.pdf` shows on both; `object`/`embed` are not tags.
