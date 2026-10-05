---
name: 20261005-x30-ts-announce-readback-picker
plan: 20261005-t3code-macos-parity
status: draft
kind: framework-gap
blocks: [20261005-composer-fidelity, 20261005-media-actions, 20261005-pr-conversation-and-refresh, 20261005-settings-scoped-controls-and-theme-editor]
upstream_url: null
reproduced_on: null
---

# X30: Data-module topic announce and resource invalidation; pixel readback; any-type file pick with bytes and image transcode

## Summary

T3 Code's client code does three things that browser APIs make trivial: it re-renders from code after an async step, reads decoded pixels (image accent, video first frame, theme inspection), and handles any file the user picks, drops or saves, with bytes in script and a decode/re-encode pipeline for images. Exact2 gives TypeScript data modules `native.available/call/watch/later` only, refuses canvas readback, and requires a literal `accept` on a file input (`EXACT2-GAPS.md` X30). The clone uses native Swift for each case. The requested support is a TS-callable announce or invalidate, a pixel-read path, and a picker that returns bytes, plus image decode/encode.

## Why this issue arose

### The T3 Code behavior
All paths at `1e2ecbd975` under `apps/web/src/` unless noted.

**1. Re-render or re-read from code.** State changes made by script show up at once because the reference holds state in React and atoms. Examples: the pull request link dialog shows "Resolving pull request..." while its 450 ms wait runs (`components/PullRequestThreadDialog.tsx:54`); a stream event updates a query (`packages/client-runtime/src/state/runtime.ts:574-576`); a refresh drops a stale answer on purpose ("Invalidate even on failure so reconnects cannot reuse the old summary", `packages/client-runtime/src/state/usage.ts:111-112`, `registry.refresh(query)`).

**2. Pixel readback.**
- Image chip accent: the loaded thumbnail is drawn at 16×16 on a canvas and averaged with each pixel weighted by alpha; a fully transparent or unreadable image keeps the default tone (`components/contextChipParts.tsx:143-168`).
- Video first frame for the composer tile (`lib/videoFirstFrame.ts`; tests `lib/videoFirstFrame.test.ts`).
- Theme editor **Inspect app colors**: the button toggles an inspect mode ("Select an element · Esc to cancel"); the user hovers and clicks any element and the editor selects the palette role that paints it and shows "N uses" while it spotlights every element that uses the role (`components/settings/ThemeEditorPanel.tsx:1180-1215`, `themeInspector.ts`: `inspectThemeRoleAtElement`, `highlightThemeRoleUsage`, `withThemeTokenProbeSession`). It does not read pixels: it reads computed paint (`getComputedStyle`) of the element under the pointer before and after setting a probe colour on a role (`themeInspector.ts:259-345,405-453`). Tests `themeInspector.test.ts` ("changedThemePaintKinds", "themeRoleFromUtilityClass") cover the pure parts.

**3. Any file in, any file out, images transcoded.**
- Attach files: a hidden `<input type="file" multiple>` with no `accept` (`components/chat/ChatComposer.tsx:7473-7487`), plus drop and paste; each `File` is classified (`components/chat/composerAttachmentFiles.ts`) and read as bytes.
- Images are decoded and re-encoded: HEIC/HEIF become JPEG at quality 0.92; an image over the wire cap is re-encoded down a quality ladder 0.92/0.85/0.78/0.68, then scaled 0.75 and 0.55, longest edge 2048; sources over 50 MiB are refused as "too-large", undecodable ones as "unreadable" (`lib/imageCompression.ts:25-60,404-520`; tests `lib/imageCompression.test.ts`).
- Save: an `<a download>` anchor saves media, attachments and exported themes (`components/media/mediaContent.ts:34`, `components/ChatView.tsx:3648`, `components/files/AttachmentFilePreview.tsx:205`, `components/settings/ThemeSettings.tsx:84`); theme import picks a JSON file (`components/settings/ThemeImportDialog.tsx:471`).

### What exact2 does today
- `EXACT2-GAPS.md` (written from framework source at `c1522fdac`, checked against `main` `d2cb661eb`), summary row X30: "TS can announce a topic / invalidate a resource; pixel readback; any-type file picker with bytes and image transcode | Wake reads, image accent colour, attachments | framework feature | `R10Connect`, `T3ImageAccent`, `T3ComposerAttach`". Detail: "TS `native` has only available/call/watch/later (`js/src/prelude.js:820-842`); Canvas readback is refused (LLP 1056:387); a file `input` needs a literal `accept`."
- The bundled library (`20261005-platforms-v3`) does not cover these: unknown (capabilities: "Unlisted APIs … Unknown in this library"). It documents `refresh result` and `mutation … refreshes` as Contract-side invalidation (state-and-data), not as a call from a data module.
- Observed in the clone (mc-orch tree, 2026-10-05): a data module can only watch topics the native module announces (`native.watch('t3.events')`, `client.ts:302`; `t3.notify`, `shell.ts:242`).

### Where the clone hits it
- **Announce.** `wakeShell` calls `native.later({op: 'r10Wake', topic: 't3.notify'})`; the Swift side allows only `t3.notify` and announces it (`modules/apple/R10Connect.swift:41-48`, `r10-connect-timing.ts:16-17`). It is used around the 450 ms PR link wait (`r9-connect-checkout.ts:149,155`). Stream events wake reads by bumping `client.revision` (`client.ts:143`). Difference: a state a script sets mid-command is drawn only after a native round trip; any new case needs a Swift change.
- **Pixels.** `T3ImageAccent.swift` re-fetches the image bytes natively and averages 16×16 pixels (`op: imageAccent`); `T3ComposerImageChip.swift:32` does the same for draft images; `T3ComposerAttach.swift:66` cuts a video frame with `AVAssetImageGenerator`. Differences: a second download for the accent; http(s) sources only. The theme editor has no Inspect button; the clone's editor lists the roles and swatches only (`settings-appearance-editor.contract`).
- **Files.** `T3ComposerAttach.swift` opens an `NSOpenPanel` of any files, converts GIF/HEIC/HEIF/JPEG/PNG/WebP to PNG drafts (limit 10 MiB), stages others up to 50 MiB, and reads them back at send; `uploadAttachment` sends bytes (`T3Transport.swift:470`); `openText` and `saveText` do the JSON import and export (`settings-appearance-import.ts:252`, `T3Module.swift`). Difference to confirm: the reference shrinks an oversized image to fit with the quality ladder; the clone's handling above 10 MiB (shrink or refuse) was not verified.

## Why it must be resolved

The parity goal needs these behaviors to match and the client logic to stay ported. Each case needs Swift beside the ported logic: three ops (`r10Wake`, `imageAccent`, the attach/save family) and the image pipeline in AppKit instead of the reference's `imageCompression.ts`, so the ported TypeScript has no tests that reach it. Waiting rows:
- `20261005-composer-fidelity`: attachment bytes in queue edit; the image accent and shelf rows.
- `20261005-media-actions`: "Save image/video", "Copy image" and their file pickers.
- `20261005-pr-conversation-and-refresh`: stream events waking a read.
- `20261005-settings-scoped-controls-and-theme-editor`: the Inspect row, blocked until plan decision U18 is made (waive Inspect, or this issue is resolved for it).
Keeping the workaround costs one more Swift op per new case and lets image-handling behavior drift from the reference.

## Requested support

On the macOS host first, the web way where one exists:
- **Announce / invalidate (A):** a call from a data module that re-asks a named resource or announces a topic the module watches. The web analogue is calling `setState` after an `await`, or `queryClient.invalidateQueries`. **(B):** allow the module's `t3.notify` style topics to be announced from TypeScript without a native op.
- **Pixel readback (A):** decode an image and read its pixels in script: `createImageBitmap(blob)` and `OffscreenCanvas.getContext('2d').getImageData(...)`, and a frame of a video through `drawImage`. Not the window's pixels. **(B) for Inspect:** the web way is `document.elementFromPoint(x, y)` plus `getComputedStyle(element)`. Exact2's closest form is a call from the app that returns the node under a point and the resolved paint rows with their source (the agent's `layout` already reports "resolved style source" and "authored/inherited/initial or dynamic provenance", library testing-and-debugging; whether an app can call it is to confirm). With that, Inspect maps a node's `background-color` expression back to the palette role.
- **Files:** `<input type="file" multiple>` with no `accept`, `File.arrayBuffer()`, drop and paste (`DataTransfer.files`, `ClipboardEvent.clipboardData.files`), and a save path (`showSaveFilePicker`, or `<a download>`), returning bytes to TypeScript. **Transcode:** `createImageBitmap` plus `OffscreenCanvas.convertToBlob({type: 'image/jpeg', quality})`, including HEIC decode, or a native codec call that exposes the same.
Trade-off: A forms are the minimum for the clone; B for Inspect is a larger feature that would also serve other inspectors.

## How to reproduce

To confirm on the pinned `main` at `issue-open`.
1. Announce: a data-module command sets a module-level flag, awaits a promise, and expects the Contract to show it. Reference (web): `setState` draws it. Expected on exact2: no re-read until the command ends or the native module announces a topic.
2. Pixels: a data module draws a 16×16 canvas and calls `getImageData`. Expected: refused (LLP 1056:387 per `EXACT2-GAPS.md`).
3. Files: a Contract `input type="file"` without a literal `accept`, then read bytes in the data module. Expected: refused or no bytes (per `EXACT2-GAPS.md`).
4. Clone scenario: drop an image chip, open Attach files with a HEIC and a 12 MiB PNG.

## Acceptance for the fix

- A data-module command shows an intermediate state while it waits (agent `clock` and `screenshot` prove it).
- A script reads pixels of a decoded image and returns the alpha-weighted average; a fully transparent image returns none.
- A picked file of any type returns its bytes to TypeScript; a HEIC decodes; an oversized PNG re-encodes to the byte budget (AppKit test and agent drive with the isolated attach folder).
- For Inspect: a call returns the role of a pointed node, and the editor's "N uses" count equals the number of nodes painted by that role in a fixture.

## App adoption after resolution

Remove `r10Wake` and `wakeShell` (`r10-connect-timing.ts`), `T3ImageAccent.swift` and `T3ComposerImageChip.swift` averaging, the frame cutter in `T3ComposerAttach.swift`, and port `lib/imageCompression.ts` with its tests; implement Inspect in `20261005-settings-scoped-controls-and-theme-editor` (or close that row by decision U18). Reopen the waiting rows in `20261005-composer-fidelity`, `20261005-media-actions` and `20261005-pr-conversation-and-refresh`. `issue-close` verifies: the PR link dialog wait draws without a native wake; an HEIC attaches as JPEG; an oversized image shrinks to fit; Inspect selects the right role.

## Status and next action

Draft; not reproduced on the pinned `main`; not searched upstream; not published.
Next: `issue-open` (reproduce, search for duplicates, prepare the report for the user's approval; publication only after approval).
