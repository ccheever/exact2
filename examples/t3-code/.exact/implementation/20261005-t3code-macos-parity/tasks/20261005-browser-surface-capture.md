---
name: 20261005-browser-surface-capture
plan: 20261005-t3code-macos-parity
implementation: done
verification: partial
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-browser-surface-capture
pr_url: https://github.com/ccheever/exact2/pull/349
verified_commit: null
---

# Browser surface part 3: Annotate, screenshots, recording and picture in picture

## Outcome

The Browser tab's Annotate, Capture screenshot (Shift-click records) and Float preview over chat buttons, drawn disabled
and marked "part 3" by [part 1](closed/20261005-browser-surface.md), work as in the reference: annotation with select,
marquee, draw and erase tools whose payload lands in the composer; screenshots and recordings saved as artifacts with
their toasts; the separate preview window and the floating mini player's browser source. The More menu's Open/Close
separate preview window row works; downloads that a page starts go to the artifact directory.

Split from [20261005-browser-surface](closed/20261005-browser-surface.md) at its `prepare` (planned split, part 3).

Built (2026-10-09), on part 1's seams (X1 path B, the `WKWebView` in the clone's module):
- **Chrome row** (`browser-capture.contract` `BrowserCaptureButtons`): Annotate ("Annotate preview" / "Cancel
  annotation", tooltip "Annotate elements, regions, and drawings", "Cancel annotation (Esc)" while on, disabled with "Page
  didn’t load — pick unavailable until the page renders" on a failed load; secondary with the primary glyph while on),
  Capture ("Capture screenshot", "Screenshot · Shift-click to record"; "Stop recording" with the destructive glyph and the
  pulsing dot, still under reduced motion), Float preview ("Float preview over chat" / "Close floating preview").
- **More menu**: "Open separate preview window" / "Close separate preview window", enabled once the page exists.
- **Annotate**: `assets/browser-annotate.js` ports PickPreload's overlay (the four tools and their keys V/R/D/E, hover
  outline, element labels, marquee up to 20 elements else a region, freehand strokes, erase, the editor with its
  comment, the style panel and drag handle, Escape, Enter attaches, ⌘/Ctrl+Enter sends) into the module's own content
  world in a closed shadow root; `T3BrowserAnnotate.swift` reads React's fiber in the page's world for component names,
  crops the targets with `takeSnapshot` into a draft image; `browser-annotation.ts` validates the payload
  (PickedElementPayload), keeps it beside the drafts, puts the `preview-annotation` chip at the composer's caret (the
  native editor draws it with the reference's amber family) and the crop on the shelf, and sends the
  `preview-annotation` record (the timeline's chip already renders it); a dropped crop toasts "Could not capture the
  picked element"; ⌘Return presses Send at once through the window's task (`annotationSend`), which reads a serial the
  panel projects whatever it shows (`shell.panel.annotationSend`), a foreground send with the "auto" dispatch mode (ChatView
  `onSendAnnotation`; `markAnnotationSend`, `client-ops-composer.ts`); when Send cannot go (a command in flight, Send
  unavailable, a pending question, the provider unavailable) the info toast "Annotation attached to draft" says so and the
  annotation stays in the draft (onSend's `notifyDirectAnnotationAttached`); a pick ends when the panel stops showing its
  tab (Float, the panel closed, another tab or thread: PreviewView's unmount, `cancelPickElement`; `cancelHiddenPicks`).
- **Screenshot**: `browser-screenshot-<site>-<id>.png` at the page's own pixel size (captureScreenshot saves capturePage's
  image as it is; MAX_SCREENSHOT_WIDTH is the automation snapshot's) in `<T3 home>/userdata/browser-artifacts`,
  the stacked "Screenshot saved" toast with Copy path, Reveal in Finder and Copy image (each copy reads "Copied!" for 2 s;
  the toast stays, `app.contract` `toastAct`'s keep rule), the reference's error titles; a failed Reveal says nothing
  (`void bridge.revealArtifact`) and a recording's failed Copy path keeps only Reveal.
- **Recording**: `browser-recording.ts` ports browserRecording.ts's lifecycle (one recording per tab,
  BrowserRecordingConflictError, the grant queue, a stop that waits for its start, a shared stop, one upload per
  recording) over the module's seams; `T3BrowserRecorder.swift` arms the in-page cursor and input overlay
  (`assets/browser-recording.js`: RecordingCursor and RecordingInput, password fields excluded), polls `takeSnapshot` at
  30 or 60 fps, composites the key badges and press rings (RecordingDecorations) and encodes H.264 MP4; the stop saves
  `browser-recording-<id>.mp4` with "Recording saved" (Reveal in Finder, Copy path). `uploadBrowserRecording` is ported
  with an injected uploader for part 5's automation host, whose `preview_recording_*` (merged with #346) still answer
  that nothing records: wiring them needs the module's automation answer to wait on the data module's recording and
  upload, a follow-up declared in `EXACT2-GAPS.md` (Part 5, "Recording (part 3)").
- **Separate window**: `T3BrowserPictureInPicture` (480×320, at least 240×160, floating panel on all spaces, about 12
  fps, the aspect ratio kept by `fitPictureInPictureContentSize`).
- **Floating player**: `previewMiniPlayerStore` has the browser source again; `BrowserMiniPlayer` floats the tab's page
  over the chat through the same `t3-browser` view (the page is borrowed, as the panel borrows it), with "Reconnecting
  preview…", the recording dot, Open in right panel, Pop into separate window, Close and the eight resize grips; Float
  closes the right panel (handlePictureInPicture); closing the panel on a Browser tab floats it (closePreviewPanel). Its
  store key is the device player's (`deviceThreadId`, activeThreadRef: a draft's own id too).
- **Held pages**: a page a recording or the separate window needs while no view shows it moves into an offscreen host
  window (`T3BrowserParking`), as acquireBrowserSurfaceActivity keeps a hidden surface painting.
- **Downloads**: `.download` for a response the page cannot show, an attachment and `<a download>` (WKDownload): an agent
  run saves `browser-download-<id>-<name>` in the artifact directory; a person's download asks with a save panel (part
  5 does not mark its pages agent-driven yet, a follow-up).
- **Settings keys**: `browserRecordingFrameRate` (30/60, default 30), `browserRecordingShowKeyPresses`,
  `browserRecordingShowMousePresses` (off) and `browserAutoShowFloatingPreview` (on), the reference's names and defaults,
  are client settings (`settings-core.ts`) the recording reads (`browserRecordingSettings`); part 5's planner reads
  `browserAutoShowFloatingPreview` (`browser-automation.ts`, wired at the merge of #346, with a Bun row). Their rows in Settings › Integrations › Browser are part 4's (coordinator, 2026-10-09): this branch built
  them first (the drives used them) and then left them to part 4.

Decisions at `prepare` (2026-10-09):
- **Frame source: `takeSnapshot` polling**, not ScreenCaptureKit. Measured in this worktree: pipelined snapshots reach
  about 60 fps (1280-pt page, 2× pixels); they need no Screen Recording permission (ScreenCaptureKit prompts, which a lane
  run must not grant); they hold the page's own pixels only. A page in no window is throttled to about 1 fps (5 distinct
  frames in 4 s), one in an offscreen alpha-0 window is not (60 of 60), hence the parking window.
- **Overlay content world: the module's own (`.defaultClient`)**, as part 1's favicon script: the page cannot see or call
  the overlay or its handler; the DOM is shared, so the overlay draws; component names need the page's world, so one
  read-only lookup of React's fiber runs there per submit (marked elements, marks removed).

Declared differences: `EXACT2-GAPS.md` "Browser surface: declared differences (X1 path B)", the "Part 3" paragraph.

## Scope and exclusions

From the parent's scope (its numbering): 2 (part 3's buttons), 3 (the separate window row), 7 (Annotate), 8 (screenshot
and recording, downloads, artifacts pruned by the server's Storage rule), 9 (picture in picture), 10 (the settings keys part 3 reads; their rows
are part 4's, coordinator 2026-10-09). Excluded: navigation aids, zoom and the device toolbar (part 2); profiles (part 4); automation, links, Mute (part 5:
the recording upload's caller, agent-driven downloads and the auto-show trigger are its).

## Context and guidance

Reference T3 Code `1e2ecbd975`: `apps/desktop/src/preview/{PickPreload,PickedElementPayload,AnnotationKeyboard,RecordingInput,RecordingCursor,Manager}.ts`,
`apps/web/src/browser/{browserRecording,browserRecordingScope,recordingCompositor,browserRecordingUpload,annotationTheme}.ts`,
`apps/web/src/components/preview/{PreviewView,PreviewChromeRow,PreviewMoreMenu,ThreadPreviewMiniPlayer}.tsx`,
`apps/web/src/previewMiniPlayerStore.ts`, `apps/web/src/lib/{composerContextRecords,elementContext,previewAnnotation}.ts`,
`apps/web/src/components/settings/IntegrationsSettings.tsx`, `apps/server/src/storageCleanup.ts`.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-browser-surface](closed/20261005-browser-surface.md) (part 1) | [#337](https://github.com/ccheever/exact2/pull/337) | Merged into `feat(example)/t3-code` | merged as `dce6d78df` (2026-10-09) |
| framework issue | [X8](../issues/closed/20261005-x08-agent-pointer-native-views.md) | #107 (fixed) | Pointer input into the page for annotate drags; real drags stay attended | agent clicks reach the page (drive 2: `tap browser-page clicks 1 at …` selected `#save`); real drags are in the real-input batch |
| merged task PR | [20261005-browser-surface-automation](closed/20261005-browser-surface-automation.md) (part 5) | [#346](https://github.com/ccheever/exact2/pull/346) | Merged into `feat(example)/t3-code` | merged as `5f0ae7dca`; this branch merged it at `a0527085c` |
| merged task PR | [20261005-browser-surface-navigation](closed/20261005-browser-surface-navigation.md) (part 2) | [#352](https://github.com/ccheever/exact2/pull/352) (re-land of [#348](https://github.com/ccheever/exact2/pull/348), reverted by [#351](https://github.com/ccheever/exact2/pull/351)) | Part 2 back on `feat(example)/t3-code` | merged as `ab220bfdb` (2026-10-10); this branch merged it at `8038df70e` (#352's code superseded the #348 code it carried; part 3 kept), and the base again at `63bdd73b4` (#367–#370, #372) |
| scheduling preference | `20261005-floating-device-player` (player layout) | none | Not a prerequisite | — |

## Acceptance and reproduction

Lane (agent mode, 1280×840): `T3_LOCAL_HOME=<lane>/t3-home-<run>` (a fresh copy of a home seeded by the runtime's
`t3 project add`), `T3_LOCAL_PORT=16720`, `T3_LOCAL_RUNTIME_DIR` (the staged runtime, unpacked), isolated `CODEX_HOME`,
`CLAUDE_CONFIG_DIR`, `XDG_*`, the lane `PATH`, `T3CODE_TELEMETRY_ENABLED=false`; fixture on 127.0.0.1:16721
([fixture](https://raw.githubusercontent.com/ccheever/exact2/f03062d925a92c942289bd76d6ffa49bb1c0634c/browser-surface-capture/fixture.mjs.txt));
closed port 16749. The project opens as a new thread's draft (`t3 app <project> --base-dir <home>`). Script of drives 7–9:
[drive7.sh](https://raw.githubusercontent.com/ccheever/exact2/d07ce43b888f822e9d570ccaf7952cee0dcd2f74/browser-surface-capture/h81ceb373c-drive7.sh.txt)
(`drive7.sh <run> <app worktree> [before]`); record of drives 7–9 (every op's answer, each tree's facts, the host's
`t3.browser:` lines, the artifacts): [drive record](https://raw.githubusercontent.com/ccheever/exact2/daaecd1208063683ae151eb3cbe77da98f002ad2/browser-surface-capture/h63bdd73b4-drive-record-7-9.txt).

**The final head.** Code head **`a63794adb`**; the commit after it changes only this record. The review of 2026-10-10
(findings 1–6, below) was fixed in `adc7f4806` and `a63794adb`, with the base merged at `2a6395e93` (#373, #374). Drive 10
ran a bundle built from `a63794adb` and the reference drive took the same steps (rows "Review of 2026-10-10"). Drives 7–9
ran earlier code (`8038df70e`, `81ceb373c`, `63bdd73b4`); their rows stand for what this round did not change.

**Window images.** Drives 7–9 take `screenshot <png> window`: the app renders its own window into the image, so it shows
the window's real content while the screen is locked (the plain form, which the earlier drives used, is blank then). The
images below are those renders. What they cannot show is what needs a person's pointer or keys (a real hover, the focus
ring, drags, the clipboard, Finder, the save panel); those stay open acceptance rows with their steps.

| Criterion | Action | Expected | Result |
| --- | --- | --- | --- |
| **Review of 2026-10-10, finding 1**: the ⌘Return send waited for a Browser tab and fired later | Annotate; Float preview (or close the panel); pick in the floating page; ⌘Return | The reference's PreviewView unmounts with the panel and cancels the pick (`cancelPickElement`), and a pick that settled with ⌘Return sends at once (onSendAnnotation) | **pass at `a63794adb`**. Drive 10: after Annotate the tree has "Cancel annotation"; after Float it has `browser-mini-player`, no `right-panel`, and the host logs `annotate cancel` and `annotate applied 3 cancelled`; the floating page has no overlay. The reference does the same: no overlay in the floating page, and a click and ⌘Return there add nothing ([image](https://raw.githubusercontent.com/ccheever/exact2/66535614bf573ec4e293d8de3d0f58094cf279e4/browser-surface-capture/r1010-annotate-then-float.png), [record](https://raw.githubusercontent.com/ccheever/exact2/3e4e26606df956e7ed18615f2748655bcadc043e/browser-surface-capture/r1010-drive-record.txt)). The send serial is the panel's (`shell.panel.annotationSend`), not the Browser tab view's, and the task no longer waits on `commandPending`. Bun: "Float or a closed panel ends the tab's pick, once" and "a pick that settled with ⌘↩ as the panel hid still sends at once" (the first fails without `cancelHiddenPicks`) |
| **Review of 2026-10-10, finding 2**: onSend's guard for a direct annotation | A second Annotate; a pick; ⌘Return with no provider (both lanes, equal states) | The info toast "Annotation attached to draft", "Sending is unavailable right now. Finish the current action, then send."; the annotation stays in the draft; a pending question is never answered by it | **pass at `a63794adb`**. Drive 10: `annotate applied 2 chip+image send`, then state shows the toast and its description, `canSend` false, `commandPending` false, and `composerText` holds both chips. Reference: the same toast, both chips in the composer ([image](https://raw.githubusercontent.com/ccheever/exact2/9f9b99e71a622b93ba509968c4e12a2766700298/browser-surface-capture/r1010-command-return.png)). Bun: the held toast spends the mark; a pending question's send toasts and commits nothing; an unavailable provider toasts instead of the send error (both fail without the guard); the window's action is checked line by line |
| **Review of 2026-10-10, finding 3**: a person's screenshot was scaled to 1,280 px | Maximize panel; Capture | The page's own pixels (Manager.ts captureScreenshot saves capturePage's size) | **pass at `a63794adb`**. Drive 10 saved 2046 × 1496; the reference saved 2046 × 1496 in the same state ([image](https://raw.githubusercontent.com/ccheever/exact2/016cf72fd69335856b6d259cde711a0f862ad864/browser-surface-capture/r1010-maximized-capture.png)). AppKit: the screenshot row checks the window's backing scale, and a 2,000-px image keeps its width (the scaled code gave 1,280). The `EXACT2-GAPS.md` "Screenshots" entry is removed |
| **Review of 2026-10-10, finding 4**: Contract-level rows | `bun test examples/t3-code/browser-capture.test.ts` | Rows that fail without Float's `rightPanelAt` rule, `toastAct`'s keep rule and the `annotationSend` task | **pass**: "Float preview closes the window's right panel", "a screenshot's and a recording's buttons keep their toast (toastAct)", "the window sends at once whatever the panel shows, or asks for 'Annotation attached to draft'" (line checks of `app.contract`, as `audit-wave-followups.test.ts`) |
| **Review of 2026-10-10, finding 5**: reference column on every image | Drive 10 and the reference drive, the same steps; before = the base `950e8e2e5` | Before, after and reference for Capture, Annotate, Float and ⌘Return | **done**: [Capture](https://raw.githubusercontent.com/ccheever/exact2/5be9e97a9b319ec2a40298e58921777d4775aa6a/browser-surface-capture/r1010-capture.png), [Annotate](https://raw.githubusercontent.com/ccheever/exact2/f89d214044a3c8ad93e79fdedb74d926e224ddb5/browser-surface-capture/r1010-annotate.png), [Float](https://raw.githubusercontent.com/ccheever/exact2/bb336a22d937fa0406a9b0b4e485ad0bd6e55238/browser-surface-capture/r1010-float.png), [⌘Return](https://raw.githubusercontent.com/ccheever/exact2/9f9b99e71a622b93ba509968c4e12a2766700298/browser-surface-capture/r1010-command-return.png). The reference's images are its renderer at 2× with the preview's webview captured and placed under the app's layers ([driver](https://raw.githubusercontent.com/ccheever/exact2/ab9330c0f876b34621ca5151a7c76e8bbabdceb6/browser-surface-capture/r1010-ref-drive.mjs.txt), [composite](https://raw.githubusercontent.com/ccheever/exact2/913d3dff7a7605795127dba15b483f2ffa003715/browser-surface-capture/r1010-ref-composite.py.txt)). The ⌘Return image compares equal provider states (none), so it shows the guard; drive 9's send image (a provider present) shows the provider error and not the user message: its chips rest on drive 9's record (op 79) |
| **Review of 2026-10-10, finding 6**: the reference's artifact toasts | A failing Reveal; a failing Copy path on a recording and on a screenshot | Reveal says nothing; a recording's error toast keeps only Reveal; a screenshot's keeps its three buttons | **pass** (Bun: "a failed Reveal says nothing; a recording's failed Copy path keeps only Reveal, a screenshot's keeps its buttons", fails without the fix) |
| Final head: a new thread's annotation chip (review, 2026-10-09) | A new thread's draft; Browser tab, fixture page; Annotate; click `#save`; a comment; Return | The chip at the composer's caret, the crop on the shelf | **pass at `63bdd73b4`** (drive 9): `annotate picked attach: elements 1, regions 0, strokes 0, screenshot ok`, `annotate applied 1 chip+image`; `composerText` = `[Tighten the button spacing](t3-context://v1/preview-annotation/preview-annotation_annotation_2) ` ([image](https://raw.githubusercontent.com/ccheever/exact2/5cd21d685fecf11ec7ba673a6c8a407f0f52d561/browser-surface-capture/h63bdd73b4-annotate.png)) |
| Final head: Float preview | Float preview; hover the handle; the pill's Open in right panel | The player floats the page and the right panel closes (handlePictureInPicture); the pill shows and stays; Open in right panel restores the tab | **pass at `63bdd73b4`** (drive 9): after Float the tree has `browser-mini-player` and no `right-panel`; with the handle hovered the epoch stays 58 for 2 s; Open in right panel brings back `right-panel` and `browser-url` and the player closes ([image](https://raw.githubusercontent.com/ccheever/exact2/6c6ad0205640123bb3bb839a9739f075cb74094b/browser-surface-capture/h63bdd73b4-float.png): drive 7's open launcher, the fix, the pill). Drive 7 failed the panel half (finding below) |
| Final head: ⌘Return send | A second Annotate on the same draft; click `#save`; a comment; ⌘Return in the editor | The message sends in the foreground with both annotations (ChatView `onSend(undefined, "auto", "foreground", …)`); the overlay closes; the thread opens | **pass at `63bdd73b4`** (drive 9): `annotate picked send`, `annotate applied 2 chip+image send`, `capture.sendSerial` 1, the composer empties, the header names the new thread ("Image: preview-annotation-annotation_2.png") and its transcript holds the chips "Tighten the button spacing" and "Send this one too"; Annotate reads "Annotate preview" again. The lane's Claude CLI is not signed in (isolated `CLAUDE_CONFIG_DIR`): the turn ends "Not logged in", no provider turn ran ([image](https://raw.githubusercontent.com/ccheever/exact2/b1a8882f8871ddb89340cca380fefdfa14960f57/browser-surface-capture/h63bdd73b4-command-return.png): it shows the provider error, not the user message; the chips are the drive record's op 79). Drive 7 failed it (finding below). With no provider on either side, drive 10 and the reference show onSend's guard instead (finding 2 above) |
| Part 2 (#352's re-land) beside part 3 | ⌘= and ⌘0 in the page; More › Show device toolbar, then close it; Capture | Zoom 110% and back; the device toolbar shows and hides; Annotate, Capture and Float stay in the row | **pass at `63bdd73b4`** (drive 9): zoom 110% → 100%, `browser-device-toolbar` shown and closed, the screenshot saved; the three buttons present throughout |
| Visual results, agent mode | Window images of drive 9; before = the base build `950e8e2e5` (`t3-code-evidence-base`, never edited), the same steps | The chrome row and the More menu as the reference's; the toasts, the recording state, the overlay, the chip and the player | **established for agent-mode states** at `63bdd73b4`: [chrome row and More menu](https://raw.githubusercontent.com/ccheever/exact2/a8d58fc38f35474afc3c567efab4896e7ca9293b/browser-surface-capture/h63bdd73b4-chrome-and-more.png) (before: the three buttons disabled and "Part 3" on Open separate preview window; after: enabled, the row without its mark; reference: the same rows plus Open DevTools, absent here by X1 path B: Safari's Web Inspector, `EXACT2-GAPS.md`), [Capture](https://raw.githubusercontent.com/ccheever/exact2/b5dcf5428add9c87aa7ce4c15b8faaf007437a97/browser-surface-capture/h63bdd73b4-capture.png) ("Screenshot saved" with Copy path, Reveal in Finder, Copy image; the destructive glyph and dot while recording; "Recording saved"), [Annotate](https://raw.githubusercontent.com/ccheever/exact2/5cd21d685fecf11ec7ba673a6c8a407f0f52d561/browser-surface-capture/h63bdd73b4-annotate.png) (the element label, the editor, the amber chip, the crop), the float and send images above. Real-pointer looks (hover by a pointer, focus rings) are in the real-input rows |
| Ported tests | `bun test examples/t3-code`; the AppKit binaries | The ported tests pass | pass at `48bce98c6`: see "Tests" and "Checks" |
| Annotate | Fixture page; Annotate; select, marquee, draw, erase; Esc; Enter, ⌘Return | Payload with elements, regions, strokes, style changes and a crop; the chip in the composer; Esc cancels; ⌘Return sends | AppKit (real clicks, keys and drags into a page): select + Return gives the element (tag, selector, preview), the comment and a PNG crop of the target plus 20 px; a fake React fiber gives "SubmitButton", its source and owner stack; ⌘Return "send"; Escape cancels and removes the overlay; marquee over empty space a region, drags two strokes, erase removes one each; a style change records 1 → 0.5 and is undone at teardown; a navigation settles nothing; the page cannot see the overlay ([select](https://raw.githubusercontent.com/ccheever/exact2/2fb539b683765a32d2a95e96fcd9f92a3d38fd8a/browser-surface-capture/annotate-select.png), [style panel](https://raw.githubusercontent.com/ccheever/exact2/7a7f4ff605315b360c04907f73f1f530948e4f82/browser-surface-capture/annotate-style-panel.png), [region and drawing](https://raw.githubusercontent.com/ccheever/exact2/934b46af1a358a97a7d93a88c4b454367614d537/browser-surface-capture/annotate-region-draw.png)). Bun: the chip at the caret once per pick, the crop on the shelf, the dropped-crop toast, ⌘Return's send (in the foreground, "auto"), a new thread's draft. Live: drive 9 above. Real drags and Escape by real keys: open row below |
| Screenshot | Open tab; Capture | The page's own pixels (captureScreenshot; this row read "≤ 1,280 px" until the review of 2026-10-10 matched the reference); "Screenshot saved" with Copy image, Copy path (and Reveal); clipboard | Live (drive 9): `t3.browser: screenshot browser-screenshot-127-0-0-1-….png` in the lane's `userdata/browser-artifacts` and the toast (image above); drives 1–2: 1,078 px wide ([the saved file](https://raw.githubusercontent.com/ccheever/exact2/68c014d7efdf8a70e6ccff58a5120459ffd8c7c9/browser-surface-capture/live-screenshot-artifact.png)), "Copy path" and "Reveal in Finder" pressed by label (`artifact copy-path`, `artifact reveal`; an agent run writes a private pasteboard and opens no Finder). AppKit: a page saves at the window's backing scale (900 pt → 1,800 px on a 2× display), an image over 1,280 px keeps its width; actions refuse files outside the directory. Drive 10: the maximized page 2046 × 1496, as the reference. Bun: the toast's three buttons, "Copied!" for 2 s, the error titles. The person's clipboard and Finder: open row below |
| Recording | Frame rate 60, keys and mouse on (set in Settings); Shift-click Capture; interact; Stop | Pulsing dot; file saved; "Recording saved" with Reveal and Copy path; overlays show keys and presses, never password input; conflicts | Live (drive 9, defaults): Shift-click → `record capture 30fps 1078x1496`, `browser-capture-dot` in the tree, the stop → `record saved browser-recording-….mp4` and "Recording saved" (image above). Drive 2 (60 fps, keys and mouse on): 267 frames in 4.5 s ([frames](https://raw.githubusercontent.com/ccheever/exact2/4e28229bbe6648ac44cfbf7a77a50f7976eb25da/browser-surface-capture/live-recording-frames.png), [the file](https://raw.githubusercontent.com/ccheever/exact2/910fdc5008af528cfda8626788d04d38aaff693a/browser-surface-capture/live-recording.mp4)). AppKit: 30 fps vs 60 fps frame counts, a playable MP4 of the snapshot's size, key labels and a nil label in a password field, the cursor re-installed after a navigation ([decorations](https://raw.githubusercontent.com/ccheever/exact2/fcb11b165a57b6220e032d8f3b08aa4766b345ed/browser-surface-capture/decorated-last-frame.png)). Bun: the lifecycle's 36 rows. The upload's oversize and deadline errors are ported; its caller is part 5 |
| Separate window and player | More › Open separate preview window; Float preview; drag; Open in right panel; close the panel | 480×320, min 240×160, about 12 fps; the player borrows the page; "Reconnecting preview…"; closing the panel floats the tab | Live: the More row opened and closed the window (drive 2); drive 4: Pop into separate window from the pill opened and closed it, closing the panel on the Browser tab floated it, Close closed the player; drive 9: Float and restore above. AppKit: the window's title "Preview · <title>", floating level, all spaces, minimum size, no minimize/zoom, ≥ 3 frames, the page's aspect ratio, frames continue with the panel hidden, closing releases the hold; the two `fitPictureInPictureContentSize` rows. Bun: float/unfloat, the panel hides, the player shows unless the panel shows the tab, sized by its page; closing the panel floats it. Drag, resize and a thread switch: open row below |
| Downloads | A page's download links | To the artifact directory (agent run); a save panel (a person) | AppKit: `browser-download-<id>-<name>` in the directory, content intact; a redirected download ends its load. Live (drive 4): the fixture's attachment and `<a download>` links saved `browser-download-…-0-report.bin` and `browser-download-…-1-export.csv` in the lane's artifact directory. A person's save panel: open row below |
| States | Each surface | Focus ring on every chrome button; Escape cancels one thing; no pulse with reduced motion | The dot's pulse and the player's dot are `none` under reduced motion (Contract `still`); the buttons are `button`s with AppKit's ring; a failed page's Annotate tip live (drive 4, closed port 16749: "Page didn’t load — pick unavailable until the page renders"). Real focus, Escape and reduced motion: open row below |
| Standard gates | Clone checks, caps, the five checks | Green | pass at `48bce98c6`: see "Checks" |
| **Open: Annotate's real drags and Escape** | Real-input step 2 | A real marquee over empty space gives a region; Draw makes a stroke; Erase removes it; a real Escape cancels | **not passed** (real input; AppKit rows only) |
| **Open: the player by a real pointer** | Real-input step 3 | The pill stays shown from the handle onto it, no flicker; drag and resize; a thread switch | **not passed** (real input) |
| **Open: clipboard, Reveal and the save panel** | Real-input steps 4 and 5 | Copy image and Copy path reach the person's clipboard (restored after); Reveal in Finder opens Finder at the file; a person's download asks with a save panel | **not passed** (real input; agent runs write a private pasteboard, record Reveal and save downloads without asking) |
| **Open: focus and reduced motion** | Real-input step 6 | Tab and Shift-Tab reach Annotate, Capture and Float preview with the focus ring; no pulse with reduced motion | **not passed** (real input) |
| **Open: a person's ⌘Return and chip** | Real-input step 1 | The chip as seen at the caret; a real ⌘Return in the editor sends in the foreground | **not passed by real keys** (agent mode passed at `63bdd73b4`) |

**Finding of drive 2 (fixed, re-driven in drive 3).** The drive's thread was a new thread's draft: `client.threadId` is
empty while the Browser tab names the id `addBrowserSurface` allocated for the draft. Annotate's chip went to that id's
stored draft rather than the open composer, and Float preview did nothing (no thread key for the player). Both key by the
open thread (`applyAnnotation` compares with `activeRef`; the player's store key is the device player's `deviceThreadId`,
activeThreadRef, since the #352 merge), with a Bun row for the draft case.

**Finding of drive 3 (fixed, re-driven in drive 4).** Hovering the floating player's handle started a render storm, and
the pill's Open in right panel did nothing: the handle and the pill were sibling hover boxes, so between the host's two
hover events neither was hovered and the pill flipped every frame. The pill is the handle's child, as the reference's
`group` (`browser-capture.contract`); a shape row fails before the fix. The floating device player (`r6-device.contract`,
another task's) has the same two sibling boxes (a follow-up, not made here).

**Finding of drive 5 (fixed, re-driven in drive 6).** ⌘Return in Annotate's editor sent the composer without the new
annotation: the window's shortcuts come before the page and the send button declares `Meta+Enter`. While Annotate is on in
a Browser tab of the open thread, the send button leaves `Meta+Enter` out (`browser-annotation.ts`
`sendChordsWhileAnnotating`, `composer-presentation.ts`, beside #360's and #372's comment drafts), so the key reaches the
page; a Bun row fails without it.

**Findings of drive 7 (fixed in `81ceb373c`, re-driven in drives 8 and 9).** The window images showed two differences:
- **⌘Return started the thread out of view.** The window's key monitor (`T3ComposerIntent`) saw the page's ⌘Return, and in
  a new thread's draft ⌘↩ resolves to `composer.sendBackground` (#366's subscribed keybindings make it resolve every
  time; drive 6 passed on timing). The reference's annotation send is `onSend(undefined, "auto", "foreground", …)`:
  `applyAnnotation` marks the send and the client's send takes the mark (`markAnnotationSend` / `takeAnnotationSend`,
  once, within 10 s of the window's clock), sending in the foreground with the "auto" dispatch mode. Two Bun rows fail
  without it (a draft with a ⌘↩ gesture; a running thread that would queue).
- **Float left the right panel open on its launcher.** handlePictureInPicture closes the right panel
  (`rightPanelStore.close`). The data module hid its surfaces, but the window's own open state kept the panel on "Open a
  surface" (drives 3–6 read only the tree's `browser-url`). `chatLocal` clears it on Float (`rightPanelAt`, #372's
  per-thread state, at the second merge).

**Finding of the second merge (fixed in `48bce98c6`).** #369 resolves the colour scheme once at the root and
`app-color-scheme.test.ts` allows no other read of `viewport.prefersColorScheme`; part 3's provide read it. It now
provides the window's resolved `scheme` (the full suite failed that one row before the fix).

Tests (`bun:test`; reference names unless marked clone): `browser-recording.test.ts` 36 (browserRecording.test.ts 31:
27 tests and the it.each's 4 rows; browserRecordingScope.test.ts 5), `browser-annotation.test.ts` 25
(PickedElementPayload.test.ts 20: 11 tests, the it.each's 9 rows; clone 5), `browser-annotate-page.test.ts` 6
(AnnotationKeyboard.test.ts 2; clone 4), `browser-recording-input.test.ts` 8 (RecordingInput.test.ts 5, its it.each's
rows), `previewMiniPlayerStore.test.ts` 6 (re-ported with browser sources), `browser-capture.test.ts` 30 (clone;
PreviewView.test.tsx's annotation rows followed; three new at `81ceb373c`: the draft's foreground send, "auto" on a
running thread, the mark's 10 s; nine new in the review round of 2026-10-10: the hidden tab's pick ends, the panel's send
serial, the window's task and guard, the held toast, a pending question, an unavailable provider, Float's `rightPanelAt`
rule, `toastAct`'s keep rule, the artifact error toasts), part 5's `browser-automation.test.ts` Auto-show row. AppKit `macos/tests/browser-capture`
34: recordingCompositor.test.ts 7 (8 methods, the it.each's two rows), Manager.test.ts `fitPictureInPictureContentSize` 2
and `recordingFileExtension` 1; clone: Annotate 9, recording 8, capture 6 (the screenshot row checks the page's own pixels
since the review of 2026-10-10). `macos/tests/browser` 29 (part 1 and part 2's
rows; two part 1 rows expect downloads). Substitutions are in each file's header.

## Checks

At code head `a63794adb` (the record commit after it changes only this file), once: `bun test examples/t3-code --timeout
60000` **4,087 pass, 1 skip, 0 fail** (exit 0); strict `tsc` (README command) exit 0; `contract build` of `app.contract` exit
0 (**1,315 lines**: 1,292 before this round, +4 for the guard, the rest the base's #373 and #374); `cargo test -p
t3-code-macos --lib` 13 pass; AppKit (README recipe): `browser-capture` 34, `browser` 29 and part 5's `browser-automation`
25 pass; `git add -A && bun scripts/caps.mjs` "All budgets within cap"; the five checks: `cargo build --all-targets
--keep-going` 0, `cargo test --lib --bins --tests --no-fail-fast` 0, `cargo clippy --all-targets --keep-going -- -D
warnings` 0, `cargo fmt --all -- --check` 0, caps 0, `bun scripts/boot.mjs` 0.

## Progress

2026-10-09: written at part 1's `prepare` (planned split). Built the same day: the frame-source and content-world
decisions above; the overlay and the recorder were written in parallel by two forks with fixed interfaces, then wired
into the session (`T3BrowserSession.swift`, `T3BrowserSessions.swift`, `T3BrowserView.swift`), the data module and the
views. Four live agent-mode drives (the screen was locked): drive 2 found the draft-thread bug, drive 3 confirmed its fix
after merging part 5 (#346) and found the pill's hover flip, drive 4 confirmed that fix and ran the remaining ops. At the
merge, part 5's planner was given the Auto-show floating preview setting (`browser-automation.ts`, with a Bun row).
Review (Charlie, 2026-10-09, on #349): keep draft, integrate the parent without losing parts 2 and 5, re-drive a new
thread's chip, Float preview and ⌘Return on the final candidate, keep the real-input rows as explicit acceptance work, and
claim no visual result from locked-screen captures. Merged `46436acc3` (part 2; the base then reverted it); drive 5 found
the ⌘Return finding; drive 6 passed the three on `76532d13d`.

2026-10-10: part 2 re-landed (#352, `ab220bfdb`). Merged the base (`8038df70e`): #352's code superseded the #348 code
this branch carried, part 3 kept; the floating player's store key became the device player's `deviceThreadId` (#355).
Drive 7 (`8038df70e`, the first with window images) found two differences, fixed in `81ceb373c` with Bun rows; drive 8
passed them. The base moved (#367–#370, #372: the right panel's open state per thread); merged it (`63bdd73b4`, Float now
clears `rightPanelAt`) and drove the final code once more (drive 9: every row passed). The full suite then found part 3's
system-scheme read (#369), fixed in `48bce98c6`. The real-input rows stay open with the steps below.

2026-10-10, review fix round (an independent review of #349: six should-fix findings). Fixed in `adc7f4806`: the
annotation send's serial is the panel's and the task no longer waits on `commandPending` (finding 1); onSend's guard,
"Annotation attached to draft", at the window and in the client's send (finding 2); a person's screenshot keeps the page's
pixels, and the `EXACT2-GAPS.md` entry is gone (finding 3); Contract-level rows (finding 4); a silent failed Reveal and the
recording's Reveal-only error toast (finding 6). Merged the base (`2a6395e93`, #373 #374; one `provide` conflict in
`app-window.contract`, both sides kept). The reference drive (finding 5) then showed that Annotate followed by Float ends
the pick in the reference (PreviewView unmounts with the panel and cancels it): ported in `a63794adb`. Drive 10 and the
reference drive took the same steps with no provider on either side; every image now has its reference column.
`app.contract`: 1,292 → 1,296 lines here (the guard), 1,315 after the base merge.
`app.contract`: 1,230 → 1,237 lines (the annotation send task and its state, the canvas re-ask key, the toast keep rule),
then 1,292 after the base merges (+1 here: Float clears the panel).

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| Drive 1 (agent mode) | branch at `fa62589a3` + the app.contract provide fix (bundle `d1ae77d7…`) | Ops 1–38 of 112: Settings › Integrations (the rows this branch had then), the Browser tab, a screenshot (artifact saved), Copy path, Reveal; stopped at op 39 on a driver word (`tap … at` is `tap … clicks 1 at`) | [drive record](https://raw.githubusercontent.com/ccheever/exact2/ba5e74bc3022370db26c6932383a5e1257f9e6aa/browser-surface-capture/drive-record.txt) | the drive script |
| Drive 2 (agent mode, the retry) | same bundle | Ops 1–81 of 118: screenshot and its toast buttons, Annotate (picked with a crop), recording at 60 fps with overlays (saved, "Recording saved"), the separate window open and closed; the chip and Float preview failed on a new thread's draft; stopped at op 82 (no floating player) | [drive record](https://raw.githubusercontent.com/ccheever/exact2/ba5e74bc3022370db26c6932383a5e1257f9e6aa/browser-surface-capture/drive-record.txt), [recording frames](https://raw.githubusercontent.com/ccheever/exact2/4e28229bbe6648ac44cfbf7a77a50f7976eb25da/browser-surface-capture/live-recording-frames.png), [screenshot artifact](https://raw.githubusercontent.com/ccheever/exact2/68c014d7efdf8a70e6ccff58a5120459ffd8c7c9/browser-surface-capture/live-screenshot-artifact.png) | fixed after the drive (draft-thread key); re-check in the real-input batch |
| Drive 3 (agent mode) | after merging #346 (`a0527085c`), bundle rebuilt | Ops 1–40 of 81: the screenshot, Annotate on a new thread's draft (`annotate applied 1 chip+image`, the chip in the composer), Float preview (the player shows, the panel hides); stopped at op 41: the pill's Open in right panel did nothing, and the hover flipped every frame (finding of drive 3) | [drive record](https://raw.githubusercontent.com/ccheever/exact2/ac612296529cc97d375ddbd2cad063f8f7b367a6/browser-surface-capture/drive-record-3-4.txt) | fixed after the drive |
| Drive 4 (agent mode) | the pill fix (`8a6020496`), bundle rebuilt | All 69 ops: the hover holds (epoch 30 for 2 s), restore, two downloads saved, the failed page's Annotate tip, closing the panel floats the tab, the separate window from the pill opens and closes, Close | [drive record](https://raw.githubusercontent.com/ccheever/exact2/ac612296529cc97d375ddbd2cad063f8f7b367a6/browser-surface-capture/drive-record-3-4.txt), [drive script](https://raw.githubusercontent.com/ccheever/exact2/fdd9795b50fa3d9c749bd661674ffbde18b7bc92/browser-surface-capture/drive4.sh.txt) | none (real-input rows below) |
| Drive 5 (agent mode) | the #348 merge (`6ba2f006c`), bundle rebuilt | All 60 ops: part 2's zoom and device toolbar beside part 3's buttons, a screenshot, a new thread's chip, Float, the steady hover, restore; ⌘Return in a second Annotate sent the composer instead (finding of drive 5) | [drive record](https://raw.githubusercontent.com/ccheever/exact2/1846bd8dff5b1091a90aa06bcd7b0140211b630b/browser-surface-capture/drive-record-5-6.txt) | fixed after the drive |
| Drive 6 (agent mode) | the ⌘Return fix (`76532d13d`), bundle rebuilt: that day's candidate | All 60 ops pass, including ⌘Return's send with both annotations | [drive record](https://raw.githubusercontent.com/ccheever/exact2/1846bd8dff5b1091a90aa06bcd7b0140211b630b/browser-surface-capture/drive-record-5-6.txt), [drive script](https://raw.githubusercontent.com/ccheever/exact2/2d41f99155f92df790d0432aef75ab85cd4c24dc/browser-surface-capture/drive6.sh.txt) | none; the open acceptance rows are real input and visual results |
| Drive 7 (agent mode) | the #352 merge (`8038df70e`), bundle rebuilt | Every op answered (83 result lines, agent exit 0); window images for the first time. The chip, Float's player and the hover passed; Float left the right panel open on its launcher, and ⌘Return started the thread in the background (findings of drive 7) | [drive record](https://raw.githubusercontent.com/ccheever/exact2/daaecd1208063683ae151eb3cbe77da98f002ad2/browser-surface-capture/h63bdd73b4-drive-record-7-9.txt) | fixed in `81ceb373c` |
| Before (agent mode) | the base build `950e8e2e5` in `t3-code-evidence-base` (never edited, not rebuilt) | The same steps to the More menu: Annotate, Capture and Float disabled, the window row marked "Part 3" | same record, `drive7-before` | none |
| Drive 8 (agent mode, the retry) | `81ceb373c`, bundle rebuilt | Every op answered (83 lines, exit 0): Float closes the panel, ⌘Return sends in the foreground (the thread opens with both chips) | same record | none |
| Drive 9 (agent mode) | the second base merge (`63bdd73b4`), bundle rebuilt: the final code but for `48bce98c6`'s one Contract line | Every op answered (83 lines, exit 0): the three final rows, part 2 beside part 3, screenshot, recording, every window image in the PR | same record; images in "Acceptance" | none; the open rows are real input |
| Reference drive (CDP) | T3 Code 0.0.45 (`1e2ecbd975`), `A/ref-app.sh browser-surface-capture 16730`, `ref-drive.mjs` over the renderer and the preview's webview | Capture, record, stop; Annotate, a pick, Return; Float and the pill; a second Annotate with ⌘Return ("Annotation attached to draft"); Annotate then Float (the pick ends); the maximized capture (2046 × 1496) | [record](https://raw.githubusercontent.com/ccheever/exact2/3e4e26606df956e7ed18615f2748655bcadc043e/browser-surface-capture/r1010-drive-record.txt), [driver](https://raw.githubusercontent.com/ccheever/exact2/ab9330c0f876b34621ca5151a7c76e8bbabdceb6/browser-surface-capture/r1010-ref-drive.mjs.txt) | none |
| Drive 10 (agent mode) | the review fixes (`a63794adb`), bundle rebuilt; no provider in the lane (both CLIs pinned to a missing path, as the reference lane) | Every op answered (86 lines, agent exit 0): the same steps as the reference drive; findings 1–3 pass live | [record](https://raw.githubusercontent.com/ccheever/exact2/3e4e26606df956e7ed18615f2748655bcadc043e/browser-surface-capture/r1010-drive-record.txt), [script](https://raw.githubusercontent.com/ccheever/exact2/55125a5a1ac1ff094ce9902416d8228784c20e19/browser-surface-capture/r1010-drive10.sh.txt), [compose](https://raw.githubusercontent.com/ccheever/exact2/69702e6b82070de05ad75701f761242a4afc223a/browser-surface-capture/r1010-compose.py.txt) | none; the open rows are real input |

## Real-input batch steps

For the coordinator's batch, on #349's final head. They need an unlocked screen and a normal launch; everything else
already passed in agent mode (drive 9). Conventions: "page (x, y)" is a point in the Browser tab's page, i.e. the page's
web area frame (`orca computer get-app-state --app com.exact.t3code.macos.lanecapture --json`) origin plus (x, y); the
fixture's `#save` button spans page x 48–228, y 200–240. Pointer by `orca computer click` / `drag` on the copy only (or
`cliclick m:x,y` to rest it); text by `orca computer type-text` with **digits only** (the Mac may be on Korean 2-Set; never
switch it, never paste); a read-back must answer before the next action. Logs: `<run>/app.log` lines `t3.browser: …`.

0. Setup. Check no system prompt covers the screen. Take the lock: `mkdir <ROOT>/target/t3-ui-parity/lanes/.realinput-lock`
   and an `owner` file naming `browser-surface-capture (#349)`. In this worktree, build
   (`EXACT_APP_DIR=$PWD/examples/t3-code bun host/apple/build.mjs t3-code-macos --bundle`), then run
   `target/capture-lane/real-lane-app.sh real1`
   ([script](https://raw.githubusercontent.com/ccheever/exact2/6e461ef4b70cb9accf865440e6f9eb9888974205/browser-surface-capture/h81ceb373c-real-lane-app.sh.txt)):
   it copies the bundle as "T3 Code (Lane Capture)" (`com.exact.t3code.macos.lanecapture`), starts the fixture on 16721
   and the app on 16720 with the lane's isolated home, `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `XDG_*` and `PATH`, and opens the
   `work` project as a new thread's draft. Read back: the window's title row "work / New thread". Close the "Update
   Available: Codex …" toast with its × (it covers the page's top right). Click "Toggle right panel", then "Browser"
   under "Open a surface"; click the "Search or enter URL" field, type-text `127.0.0.1:16721`, press Return. Read back:
   `fixture.log` has `GET /` and the page shows "Capture fixture".
1. Chip and ⌘Return by real keys. Click "Annotate preview" (read back: `annotate start`; the button reads "Cancel
   annotation"). Click page (138, 220) (`#save`): the element label `button#save` and the editor appear. Type-text `101`,
   press Return. Read back: `annotate picked attach: elements 1, regions 0, strokes 0, screenshot ok`, `annotate applied 1
   chip+image`; the composer shows the amber chip "101" at its caret and the crop on the shelf (window image). Click
   "Annotate preview", click page (138, 220), type-text `102`, press ⌘Return (`hotkey cmd+return`). Read back: `annotate
   picked send …`, `annotate applied 2 chip+image send`; the title row names the new thread, its user message shows the
   chips "101" and "102", the composer is empty and the button reads "Annotate preview" again (the turn ends "Not logged
   in": the lane's Claude CLI has no account).
2. Annotate's real drags and Escape (same thread, the Browser tab still open). Click "Annotate preview", click the
   overlay toolbar's "Region" (at the page's top, y 10–52), drag page (60, 340) → (320, 430) (empty space), type-text `201`,
   Return. Read back: `annotate picked attach: elements 0, regions 1, strokes 0`. Click "Annotate preview", "Draw", drag
   page (80, 470) → (300, 530) (the editor shows), then "Erase", click page (190, 500) (the stroke's middle: the stroke and
   the editor go), "Draw", drag page (80, 560) → (300, 600), type-text `202` in the editor, Return. Read back: `elements 0,
   regions 0, strokes 1`.
   Click "Annotate preview", then press Escape. Read back: `annotate cancelled in the page`; the button reads "Annotate
   preview"; no chip was added.
3. The floating player by a real pointer. Click "Float preview over chat". Read back: no "Right panel" region in the AX
   tree; the player shows the fixture page at the chat's bottom right. Rest the pointer on the player's top edge (the
   handle), then move it in 10-pt steps onto the pill at the player's top right; take three window images 300 ms apart
   with the pointer resting on the pill: the pill (Open in right panel, Pop into separate window, Close) shows in all
   three. Click "Pop preview into separate window": a "Preview · Capture fixture" window opens (`pip open`); click it
   again ("Close popped-out preview"; `pip close`). Drag the handle 200 pt left and 100 pt up: the player moves and stays
   there. Drag its north-west grip 80 pt outward: it grows and keeps the page's aspect ratio. Click "New thread" in the
   sidebar's `work` row: the player is gone; click the step 1 thread: the player is back where it was left. Click "Open
   preview in right panel": the panel opens on the Browser tab and the player closes. Click "Toggle right panel" with the
   Browser tab shown: the player floats it again. Click the pill's "Close": the player closes.
4. Clipboard and Finder. Save the person's clipboard first (`osascript -e 'clipboard info'` names its types; keep its text
   with `pbpaste`). Open the right panel on the Browser tab, click "Capture screenshot". Read back: `screenshot
   browser-screenshot-127-0-0-1-….png`, the "Screenshot saved" toast. Click "Copy path": it reads "Copied!" for 2 s;
   `pbpaste` prints `…/t3-home-real/userdata/browser-artifacts/browser-screenshot-127-0-0-1-….png`; `artifact copy-path`.
   Click "Copy image": "Copied!"; `osascript -e 'clipboard info'` lists `«class PNGf»`; `artifact copy-image`. Click
   "Reveal in Finder": a Finder window shows `browser-artifacts` with the file selected (`osascript -e 'tell application
   "Finder" to get name of selection'`); close that Finder window. Restore the clipboard.
5. A person's download. Click the page's "Downloads" link (page ≈ (190, 300)), then `export.csv` at page (142, 210): a
   save panel sheet asks (name `export.csv`); click Cancel. Read back: no `t3.browser: download` line, no new file in
   the artifact directory or the lane home's Downloads. The same for `report.bin` at page (142, 140).
6. Focus and reduced motion. Click the URL field, then press Tab once per control: Annotate preview, Capture screenshot
   and Float preview over chat each take the focus (AX focused element) with AppKit's ring (a window image about 0.9 s
   after the key); Shift-Tab walks back. Reduced motion without touching System Settings, in agent mode on the same
   build: `bun scripts/agent.mjs macos --size 1280x840 "clock +15000 real" … (steps 0's panel and page as in drive7.sh)
   'prefer {"prefers-reduced-motion":"reduce"}' "tap browser-capture modifiers Shift" "clock +500 real"
   "screenshot <dir>/dot.png over 1200 every 100 window" "tap browser-capture"`: the recording dot is the same in every
   frame (no pulse); without the `prefer` it pulses.

After: `osascript -e 'quit app id "com.exact.t3code.macos.lanecapture"'` (an Apple Event quit stops its server), kill the
fixture pid in `<run>/pids.txt`, `defaults delete com.exact.t3code.macos.lanecapture`, remove `lane/apps/`, release the lock.

## Next action

PR [#349](https://github.com/ccheever/exact2/pull/349) stays a **draft**. The review of 2026-10-10 (findings 1–6) is fixed
at code head `a63794adb` and driven against the reference (drive 10). Left: the real-input batch above (steps 1–6), then
the coordinator's review and the ready flip.
