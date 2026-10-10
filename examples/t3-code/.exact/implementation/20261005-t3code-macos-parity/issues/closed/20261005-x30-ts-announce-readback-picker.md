---
name: 20261005-x30-ts-announce-readback-picker
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: [20261005-composer-fidelity, 20261005-media-actions, 20261005-pr-conversation-and-refresh, 20261005-settings-scoped-controls-and-theme-editor]
upstream_url: https://github.com/ccheever/exact2/issues/116
reproduced_on: 4c893fef6
---

# X30: Data-module topic announce and resource invalidation; pixel readback; any-type file pick with bytes and image transcode

Moved to main `issues/20261009-any-type-file-input-ruling.md` (2026-10-09); tracked there.

## Summary

T3 Code's client code does three things that browser APIs make trivial: it re-renders from code after an async step, reads decoded pixels (image accent, video first frame, theme inspection), and handles any file the user picks, drops or saves, with bytes in script and a decode/re-encode pipeline for images. Exact2 gives TypeScript data modules `native.available/call/watch/later` only (`js/src/prelude.js:820-842`), refuses canvas readback (LLP 1056:387), and requires a literal `accept` on a file input. The clone uses native Swift for each case. Main tracks only the any-type file input; the Inspect lookup is X68, main `issues/20261009-elements-from-point-read.md` ([#321](https://github.com/ccheever/exact2/issues/321)).

## Why it arose

### The T3 Code behavior
All paths at `1e2ecbd975` under `apps/web/src/` unless noted.

**1. Re-render or re-read from code.** The pull request link dialog shows "Resolving pull request..." while its 450 ms wait runs (`components/PullRequestThreadDialog.tsx:54`); a stream event updates a query (`packages/client-runtime/src/state/runtime.ts:574-576`); a refresh drops a stale answer on purpose (`packages/client-runtime/src/state/usage.ts:111-112`, `registry.refresh(query)`).

**2. Pixel readback.**
- Image chip accent: the loaded thumbnail is drawn at 16×16 on a canvas and averaged with each pixel weighted by alpha (`components/contextChipParts.tsx:143-168`).
- Video first frame for the composer tile (`lib/videoFirstFrame.ts`).
- Theme editor **Inspect app colors**: the user hovers and clicks any element and the editor selects the palette role that paints it and shows "N uses" while it spotlights every element that uses the role (`components/settings/ThemeEditorPanel.tsx:1180-1215`, `themeInspector.ts`). It reads computed paint (`getComputedStyle`) of the element under the pointer, not pixels.

**3. Any file in, any file out, images transcoded.**
- Attach files: a hidden `<input type="file" multiple>` with no `accept` (`components/chat/ChatComposer.tsx:7473-7487`), plus drop and paste, each `File` read as bytes.
- Images are decoded and re-encoded: HEIC/HEIF become JPEG at quality 0.92; an image over the wire cap is re-encoded down a quality ladder, then scaled, longest edge 2048; sources over 50 MiB are refused (`lib/imageCompression.ts:25-60,404-520`).
- Save: an `<a download>` anchor saves media, attachments and exported themes; theme import picks a JSON file (`components/settings/ThemeImportDialog.tsx:471`).

### Where the clone hits it
`20261005-composer-fidelity` (attachment bytes, the image accent and shelf rows), `20261005-media-actions` (Save image/video, Copy image and their pickers), `20261005-pr-conversation-and-refresh` (stream events waking a read), and `20261005-settings-scoped-controls-and-theme-editor` (the Inspect row, plan decision U18).

## Clone workaround
- **Announce.** `wakeShell` calls `native.later({op: 'r10Wake', topic: 't3.notify'})`; the Swift side allows only `t3.notify` and announces it (`modules/apple/R10Connect.swift`, `r10-connect-timing.ts`), used around the 450 ms PR link wait (`r9-connect-checkout.ts`). Stream events wake reads by bumping `client.revision` (`client.ts`). A state a script sets mid-command is drawn only after a native round trip. This part is not in #116 or any main issue, so `r10Wake` stays.
- **Pixels.** `T3ImageAccent.swift` re-fetches the image bytes natively and averages 16×16 pixels (`op: imageAccent`); `T3ComposerImageChip.swift` does the same for draft images; `T3ComposerAttach.swift` cuts a video frame with `AVAssetImageGenerator`. The theme editor has no Inspect button (U18).
- **Files.** `T3ComposerAttach.swift` opens an `NSOpenPanel` of any files, converts GIF/HEIC/HEIF/JPEG/PNG/WebP to PNG drafts (limit 10 MiB), stages others up to 50 MiB, and reads them back at send; `uploadAttachment` sends bytes (`T3Transport.swift`); `openText` and `saveText` do the JSON import and export (`settings-appearance-import.ts`, `T3Module.swift`).
- #116's decision (2026-10-08): `T3ImageAccent.swift`, the averaging in `T3ComposerImageChip.swift`, and the frame cutter and HEIC/PNG transcode in `T3ComposerAttach.swift` stay native, a permanent declared difference. An any-type `input type="file"` would replace only the `NSOpenPanel` in `T3ComposerAttach.swift`.

## Evidence and history
- Filed 2026-10-06 as [#116](https://github.com/ccheever/exact2/issues/116), reproduced on exact2 `4c893fef6` before filing.
- 2026-10-08: the theme editor's Inspect lookup filed separately as X68, [#321](https://github.com/ccheever/exact2/issues/321).
