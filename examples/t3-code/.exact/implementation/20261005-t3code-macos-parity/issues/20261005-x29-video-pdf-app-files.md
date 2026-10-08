---
name: 20261005-x29-video-pdf-app-files
plan: 20261005-t3code-macos-parity
status: closed-upstream
kind: framework-gap
blocks: [20261005-media-actions]
upstream_url: https://github.com/ccheever/exact2/issues/115
reproduced_on: null
rest_upstream_url: https://github.com/ccheever/exact2/issues/273
---

# X29: `video` and `audio` from app-written local files, and a PDF viewer element

## Summary

T3 Code plays video and audio and shows PDFs through ordinary browser elements (`<video>`, `<audio>`, an `<iframe>` with Chromium's PDF viewer) whose sources are local files, blob URLs or signed URLs. Exact2's `video` element accepts only http(s) or bundled-asset sources, `image` accepts `app:/` files, and there is no PDF viewer element (`EXACT2-GAPS.md` X29). The clone covers every surface with app-module native views (AVKit, PDFKit). The result looks right, but each surface needs a native hook that the agent cannot drive. The requested support is `app:/` sources for `video` and `audio`, and a PDF element.

## Why this issue arose

### The T3 Code behavior
All paths at `1e2ecbd975` under `apps/web/src/`:
- **Composer shelf.** A dropped, pasted or picked video file shows a thumbnail tile: `<video muted playsInline preload="metadata">` cut to its first frame (`components/chat/ChatComposer.tsx:335-358`, `lib/videoFirstFrame.ts`, `attachVideoThumbnail`). Opening the tile plays the file in a dialog (`components/chat/ExpandedImageDialog.tsx:47` `ExpandedVideo`, a `<video controls>` that autoplays by default, `:17`). The file is a client-side `File`; the browser plays it through a blob URL.
- **Timeline.** A sent video is a 4:3 tile with the first frame, muted and without controls, whose whole face is the "Play <name>" button. It opens the media dialog (`components/media/MediaVideoPlayer.tsx:167-215`). States: loading ("Loading video", `:196`), failed ("Video preview unavailable", `:196`), ready, and the `controls` player when it is not a tile (`:174`). The source is the signed asset URL.
- **Attachment and Files previews.** Video: `<video controls playsInline src>` with the error "Unable to load video." (`components/files/AttachmentFilePreview.tsx:250-258`). Audio: `<audio>` with "Unable to load audio." (`components/files/AudioPreview.tsx:9`, `AttachmentFilePreview.tsx:247`). PDF: an `<iframe>` of the signed URL plus `#toolbar=0&view=FitH`, so the page shows alone, fitted to the panel width, with scrolling, pinch zoom, selection and find working inside it (`components/files/BrowserDocumentFrame.tsx:7,26-32`; Files panel `FilePreviewPanel.tsx:228,944`). Files with no preview show "No preview for this file".
- Reference tests: `lib/videoFirstFrame.test.ts`; the media components have no unit test.

### What exact2 does today
- `EXACT2-GAPS.md` (written from framework source at `c1522fdac`, checked against `main` `d2cb661eb`), summary row X29: "`video` from `app:/` files; a PDF viewer element | Composer video preview, PDF attachments | framework feature | AVPlayerView, PDFView natively". Detail: "`video` takes only http(s) or bundled assets (`Mac/NodeViewMac.swift:402-407`); `image` takes `app:/`."
- The bundled library (`20261005-platforms-v3`) does not cover `video`, `audio` or PDF: unknown. It says real media is not virtual-clock playback (testing-and-debugging).
- Whether `audio` accepts any source, and how `video` treats a `file:` URL, was not checked: to confirm at `issue-open`.
- Observed in the clone (mc-orch tree, 2026-10-05): sent-video tiles use the Contract `video url` element with the signed http URL (`r6-media-video.contract`, `R6VideoTile`); that source type works.

### Where the clone hits it
- **Composer shelf video.** Staged files live in the app's data folder, not at an http URL. `T3ComposerVideo.swift` plays them in an `AVPlayerView` through hook `t3-video` (typed links written beside the staged copy by `T3ComposerAttach.swift`); the shelf thumbnail is a frame cut by `AVAssetImageGenerator` (`T3ComposerAttach.swift:66`).
- **PDF and audio previews.** `R6MediaPreview.swift` (hook `t3-media`, `data-media-kind` pdf/html/audio/video) draws a `PDFView` scaled to the width and an `AVPlayerView` for audio and video (`r6-media.contract:19`; the Files panel uses the same hook for HTML, `r4-surfaces-files.contract:197`).
- **Difference a user sees.** Controls are AppKit's, not Chromium's; the PDF viewer has PDFKit's selection, find and zoom, not Chromium's. These stay even after support lands, because an exact2 element would also be backed by AVKit and PDFKit.
- **Difference in verification.** Native views are outside the agent: `EXACT2-GAPS.md` X8 says "The agent cannot click, drag or scroll inside native views (terminal, browser, device screen, PDF/video)", so checks inside them need a real-input session. The reference's media actions (context menu, Open original; `20261005-media-actions`) have to attach to a native view instead of an element.

## Why it must be resolved

The goal is a clone with the reference's behavior and a build whose results an agent can prove. Today three hooks and three Swift files (`T3ComposerVideo.swift`, the PDF/audio/video parts of `R6MediaPreview.swift`, the frame cutter in `T3ComposerAttach.swift`) stand in for three elements. They carry their own tests (`macos/tests/r6-media`), and the media states inside them (loading, failed, retry) can only be checked in a real-input session. `20261005-media-actions` waits for neither (nonblocking, workaround: the native views), but its rows for the video slot and the PDF/audio previews are attended, and they would become agent rows. The cost of keeping the workaround is native code, hand-checked rows and no agent coverage of a user-visible surface.

## Requested support

On the macOS host first, the web way:
- **A.** `video` and `audio` accept `app:/` sources (as `image` does) and the file URL of a file the app wrote (web: `URL.createObjectURL(file)` / `blob:`), with `poster`, `controls`, `muted`, `preload="metadata"`, `autoplay`, `playsinline`, and the `error` and `loadedmetadata` events.
- **B.** A PDF element with the reference's behavior: web `<object type="application/pdf" data>` / `<iframe src="x.pdf#toolbar=0&view=FitH">`; on macOS backed by PDFKit with fit-to-width, scrolling, selection and find. Props: `src`, `fit="width"`, `toolbar=false`.
Trade-off: A and B can ship separately; A alone removes the composer and timeline hooks, B alone removes the PDF hook. Other hosts: iOS has AVKit and PDFKit; web has native elements.

## How to reproduce

To confirm on the pinned `main` at `issue-open`. Minimal app: bundle no video; the data module writes `clip.mp4` to the app data folder; the Contract renders `video src="app:/clip.mp4" controls`. Reference (web): plays after `URL.createObjectURL(file)`. Expected on exact2: refused or empty (EXACT2-GAPS: only http(s) and bundled assets). Second case: `object type="application/pdf"` has no matching element. Clone scenario: composer Attach a `.mov`, open the shelf tile, and see the `t3-video` hook play it.

## Acceptance for the fix

- Agent: `tree` lists the `video` node, `state` shows its playing and error facts, a screenshot shows the first frame; a missing file raises the error event ("Unable to load video.").
- A PDF page renders at the panel width without a toolbar; `layout` reports its size; selection and find work (AppKit test).
- Conformance case against Chrome for `video` attributes where they apply; real media is not virtual-clock playback, so playback rows state their real-time wait.

## App adoption after resolution

Remove `T3ComposerVideo.swift`, hook `t3-video`, the PDF and audio branches of `R6MediaPreview.swift` and the `t3-media` kinds for them; use `video`, `audio` and the PDF element in `r6-media.contract`, `r4-surfaces-files.contract` and the composer's expanded preview. Update `20261005-media-actions` rows to agent rows. `issue-close` verifies: composer shelf video plays; a PDF attachment renders fit-to-width; audio and video previews show "Unable to load …" on a missing file; the removed hooks are gone from `app.json`.

## Status and next action

Draft; not reproduced on the pinned `main`; not searched upstream; not published.
Next: `issue-open` (reproduce, search for duplicates, prepare the report for the user's approval; publication only after approval).

## Merged upstream; partly fixed (2026-10-07, adopt-main-fixes-r4)

[#115](https://github.com/ccheever/exact2/issues/115) was closed by main #205 (`e3b0be7ba`), in the feature
branch since main `463acda68` ([20261007-adopt-main-fixes-r4](../tasks/closed/20261007-adopt-main-fixes-r4.md)):
a bundled (`assets/`) PDF in an `iframe` is shown by WebKit's PDF view on macOS and iOS instead of its bytes
as HTML text, and the web serves `.pdf` as `application/pdf`. #205's own open points are not on main: an
`iframe` of an `app:/` file, and a PDF element with a fit-to-width mode and `load`/`error` (proposal B).
`video` and `audio` from `app:/` were already on main (`4e2c79acb`).

Adoption: none. The clone's PDF is the server's signed asset URL over http, which an `iframe` could load
before #205 too (#205 changed only bundled files). `R6MediaPreview.swift`'s `PDFView` stays because WebKit's
PDF view does not take Chromium's `#toolbar=0&view=FitH` and draws its own white surround (#205's capture),
while T3 Code shows the page alone on Chromium's #282828 surface fitted to the panel width
(`BrowserDocumentFrame.tsx`); doing that without a native view needs the PDF element #205 left open.

## Rest filed upstream (2026-10-08)

Upstream (the rest): https://github.com/ccheever/exact2/issues/273 (#273, [Design] Show a PDF: an `iframe` of an `app:/` file, and a PDF element fitted to its width (rest of #115)). Reproduced on main `0365ad1a4` (relevant files unchanged on main `e200397ec`) before filing: a PDF the data module wrote to `app:/data/media/doc.pdf`: the web's `iframe` becomes `about:blank`, macOS's stays blank, and both still fire `load`; a bundled `assets/doc.pdf` shows on both; `object`/`embed` are not tags. One "Decision needed" comment (`rules/DEFERRED.md:427-431`). Searched open and closed issues and PRs: no duplicate.

## Decided upstream (2026-10-08): the PDF view is a declared difference

[Charlie on #273](https://github.com/ccheever/exact2/issues/273#issuecomment-6055587160): "Add scoped app:/ iframe loading first; defer a separate PDF element. … Keep fitted PDF UI in a
module until a separate consumer-driven design is selected."
- **Declared difference (permanent):** `PDFView` in `R6MediaPreview.swift` (the page alone, fitted to the width, on
  #282828) stays. The clone's PDF is the server's signed `http` URL, so an `app:/` iframe does not apply.
- [#327](https://github.com/ccheever/exact2/pull/327) audit (open on main, 2026-10-08): approved scoped `app:/` iframe; the fitted PDF surface stays deferred.
