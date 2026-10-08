---
name: 20261005-browser-surface-capture
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: null
pr_url: null
verified_commit: null
---

# Browser surface part 3: Annotate, screenshots, recording and picture in picture

## Outcome

The Browser tab's Annotate, Capture screenshot (Shift-click records) and Float preview over chat buttons, drawn disabled
and marked "part 3" by [part 1](20261005-browser-surface.md), work as in the reference: annotation with select,
marquee, draw and erase tools whose payload lands in the composer; screenshots and recordings saved as artifacts with
their toasts; the separate preview window and the floating mini player's browser source. The More menu's Open/Close
separate preview window row works; downloads that a page starts go to the artifact directory.

Split from [20261005-browser-surface](20261005-browser-surface.md) at its `prepare` (planned split, part 3). It starts
after part 1 merges into `feat(example)/t3-code`.

## Scope and exclusions

From the parent's scope (its numbering):

2. **Chrome row, part 3's buttons.** Annotate ("Annotate preview" / "Cancel annotation (Esc)", tooltip "Annotate elements,
   regions, and drawings"; disabled with "Page didn't load — pick unavailable until the page renders" on a failed load),
   Capture ("Capture screenshot", tooltip "Screenshot · Shift-click to record"; "Stop recording" with a pulsing dot),
   Float preview over chat ("Close floating preview" while floating) (`PreviewChromeRow.tsx:121-320`).
3. **More menu, part 3's row.** Open or Close separate preview window.
7. **Annotate.** Tools select, marquee, draw, erase (`PickPreload.ts:119`, hints such as "Draw freehand (D)" at `:949`),
   Escape cancels (`:1332-1341`), elements with component names where the page exposes them, regions, strokes and style
   changes, a cropped screenshot; the result is a `PreviewAnnotationPayload` added to the composer
   (`composerDraftStore.ts:676` `addPreviewAnnotation`) as a context chip (kind `preview-annotation`;
   `r4-timeline-chips.ts` renders sent chips). WebKit: a user script in the module's content world (part 1's favicon
   script shows the mechanism) and `takeSnapshot` for the crop.
8. **Screenshot and recording.** Screenshot width at most 1,280 px (`WKWebView.takeSnapshot`); recording at 30 or 60 fps
   (setting), optional key-press and mouse-press overlays with password fields excluded (`RecordingInput.ts`,
   `RecordingCursor.ts`), the compositor, one recording at a time (`BrowserRecordingConflictError`), the encoded file
   uploaded once as an attachment (`browserRecordingUpload.ts`), toasts "Screenshot saved" (Copy image, Copy path) and
   "Recording saved" (Reveal, Copy path) (`PreviewView.tsx:340-575`), artifacts pruned after N days
   (`settings.ts:1187-1190`); downloads go to the artifact directory (`Manager.ts:3570-3584`; part 1 refuses them,
   `T3BrowserSession.swift` `decidePolicyFor navigationResponse`). Frames: `takeSnapshot` polling or ScreenCaptureKit
   with the Screen Recording prompt (X1 path B; chosen at `prepare`).
9. **Picture in picture.** The separate preview window (480×320, minimum 240×160, about 12 frames per second, JPEG
   quality 80; `Manager.ts:144-157`) and the floating mini player's browser side ("Float preview over chat", the
   "Auto-show floating preview" setting, "Reconnecting preview…"; `ThreadPreviewMiniPlayer.tsx:103-198`;
   `previewMiniPlayerStore.ts`'s browser source, which the clone leaves out today). One `WKWebView` can sit in one
   place: the player borrows the tab's page as the panel's `t3-browser` view does (`T3BrowserView.swift`).
10. **Settings rows, part 3's.** Browser recording frame rate, key presses, mouse presses, Auto-show floating preview.

Excluded: navigation aids, zoom and the device toolbar (part 2); profiles (part 4); automation, links, Mute (part 5).

## Context and guidance

Parent specification: [spec](../spec.md); engine decisions and declared differences: the parent record and
`EXACT2-GAPS.md` "Browser surface: declared differences (X1 path B)". Reference: T3 Code `1e2ecbd975`
(`apps/desktop/src/preview/{PickPreload,PickedElementPayload,AnnotationKeyboard,RecordingInput,RecordingCursor}.ts`,
`apps/web/src/browser/{browserRecording,browserRecordingScope,recordingCompositor,browserRecordingUpload}.ts`,
`apps/web/src/components/preview/ThreadPreviewMiniPlayer.tsx`). Part 1's seams: the disabled buttons and menu row in
`browser-surface.contract` (`BrowserChrome`, `BrowserMore`), `T3BrowserSession.swift`.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-browser-surface](20261005-browser-surface.md) (part 1) | its draft PR | Merged into `feat(example)/t3-code` | pending |
| framework issue | [X8](../issues/closed/20261005-x08-agent-pointer-native-views.md) | #107 (fixed) | Pointer input into the page for annotate drags; real drags stay attended | — |
| scheduling preference | `20261005-floating-device-player` (player layout) | none | Not a prerequisite | — |

## Acceptance and reproduction

Rows from the parent's table: "Annotate", "Screenshot", "Recording", "Separate window and player", and "States" for
these surfaces (reduced motion stops the recording dot's pulse). Tests to port (`bun:test`, original names; counts from
the parent): `browserRecording` (28), `browserRecordingScope` (5), `recordingCompositor` (7), `apps/desktop`
`PickedElementPayload` (11), `AnnotationKeyboard` (2), `RecordingInput` (5); the mini player's browser source in
`previewMiniPlayerStore.test.ts`. Standard gates as the parent's.

## Progress

2026-10-09: written at part 1's `prepare` (planned split). Planned; starts after part 1 merges.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |

## Next action

After part 1 merges: `prepare` (frame source: `takeSnapshot` polling or ScreenCaptureKit; the overlay script's world),
then implement.
