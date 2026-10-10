---
name: 20261005-browser-surface-capture
plan: 20261005-t3code-macos-parity
implementation: done
verification: verified
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-browser-surface-capture
pr_url: https://github.com/ccheever/exact2/pull/349
verified_commit: 70e2ddd4b6232d3a96d5134298a4a6376d3d7580
---

# Browser surface part 3: Annotate, screenshots, recording and picture in picture

## Outcome

The Browser tab's Annotate, Capture screenshot (Shift-click records) and Float preview over chat buttons, drawn disabled
and marked "part 3" by [part 1](20261005-browser-surface.md), work as in the reference: annotation with select,
marquee, draw and erase tools whose payload lands in the composer; screenshots and recordings saved as artifacts with
their toasts; the separate preview window and the floating mini player's browser source. The More menu's Open/Close
separate preview window row works; downloads that a page starts go to the artifact directory.

Split from [20261005-browser-surface](20261005-browser-surface.md) at its `prepare` (planned split, part 3).

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
- **Fix round of realinput-1010c** (the coordinator's real-input session of 2026-10-10, row A, bundle `d1ae77d7` built
  from `a63794adb`): a person's Escape while annotating is the page's (the panel toggle declares Escape only while the
  shown tab is not annotating, `r4-surfaces.contract`); the floating player's handle grows its hover box to the pill's
  box and the pill's buttons hear no hover of their own (X62), their tooltips follow the pointer's place in the box
  (`pointermove`), close on a press and end at their buttons near the canvas's right edge; a player gesture keeps one
  serial, so the canvas moves the frame of the gesture's start by the pan's total (MiniPlayerShell); the header's Toggle
  right panel drops its hover on press (`chat.contract`); a draft's sidebar row reads its chips by their labels
  (`sidebar-view.ts`, `replaceContextReferences`). Float reading "Close floating preview" after the panel is reopened,
  and the unfocused editor of a Draw after an Erase, are the reference's own states (checked over CDP) and stay.
- **Review of 2026-10-10 (second)** (two should-fix findings): a press on a pill button never drags the floating player,
  as the reference's buttons stop the pointer-down (MiniPlayerShell). The handle keeps the hover and `pointermove`; a box
  behind the pill (`browser-mini-grab`, the handle's whole box) takes the `pan`, and the pill lets the pointer through
  (`pointer-events="none"`) except on its buttons, so the dot and the pill's padding still drag. The difference that was
  declared in `EXACT2-GAPS.md` on a design rule (LLP 1057.001 rule 3: a `pan` takes a nested button's contact past the
  slop) is gone. The claim that every Bun row of the realinput-1010c round fails on the previous code is corrected: the
  canvas row passes there (the canvas did not change); it documents the mechanism, and the fix is the contract's `pan`.

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
| merged task PR | [20261005-browser-surface](20261005-browser-surface.md) (part 1) | [#337](https://github.com/ccheever/exact2/pull/337) | Merged into `feat(example)/t3-code` | merged as `dce6d78df` (2026-10-09) |
| framework issue | [X8](../../issues/closed/20261005-x08-agent-pointer-native-views.md) | #107 (fixed) | Pointer input into the page for annotate drags; real drags stay attended | agent clicks reach the page (drive 2: `tap browser-page clicks 1 at …` selected `#save`); real drags are in the real-input batch |
| merged task PR | [20261005-browser-surface-automation](20261005-browser-surface-automation.md) (part 5) | [#346](https://github.com/ccheever/exact2/pull/346) | Merged into `feat(example)/t3-code` | merged as `5f0ae7dca`; this branch merged it at `a0527085c` |
| merged task PR | [20261005-browser-surface-navigation](20261005-browser-surface-navigation.md) (part 2) | [#352](https://github.com/ccheever/exact2/pull/352) (re-land of [#348](https://github.com/ccheever/exact2/pull/348), reverted by [#351](https://github.com/ccheever/exact2/pull/351)) | Part 2 back on `feat(example)/t3-code` | merged as `ab220bfdb` (2026-10-10); this branch merged it at `8038df70e` (#352's code superseded the #348 code it carried; part 3 kept), and the base again at `63bdd73b4` (#367–#370, #372) |
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

**The final head.** Code head **`b6ca2cbc4`**: the second review round's fix (`42e43621a`: the player's drag is a box
behind the pill, the canvas row's claim corrected) and the base merged after it (`b6ca2cbc4`: X72 renumbered and moved
to main, records and `formatUsd`'s comment); the commit after it changes only this record. Before it: the realinput-1010c
fixes (`85ea13fbe`), the base merged at `871f4ffe1` (#384, main `bc357d03c`; part 3's icon buttons say `title=""`, LLP
1115's AppKit help tags), and `b691e6388` (main's `now()` is `performanceNow()` in the player's gesture serial;
`usage-pooled.test.ts` lists the handle as a pointer taker). The second review round's drives: before-c (the bundle of
after2, the code of `85ea13fbe`, kept and run with `EXACT_MAC_BIN`), after3 and its retry after4 (this round's bundle),
and the reference over CDP, the same steps (rows "Review of 2026-10-10 (second)"). The fix round's drives (agent mode, window images, the same steps): before-a
and before-b drove the bundle realinput-1010c drove (`a63794adb`, kept and run with `EXACT_MAC_BIN`), after1 and after2
this round's builds before the #384 merge; the reference took the same steps over CDP (rows "realinput-1010c"). The
review of 2026-10-10 (findings 1–6) was fixed in `adc7f4806` and `a63794adb`, with drive 10 beside its reference drive;
drives 7–9 ran earlier code (`8038df70e`, `81ceb373c`, `63bdd73b4`); their rows stand for what later rounds did not change.

**Window images.** Drives 7–9 take `screenshot <png> window`: the app renders its own window into the image, so it shows
the window's real content while the screen is locked (the plain form, which the earlier drives used, is blank then). The
images below are those renders. What they cannot show is what needs a person's pointer or keys (a real hover, the focus
ring, drags, the clipboard, Finder, the save panel); those stay open acceptance rows with their steps.

| Criterion | Action | Expected | Result |
| --- | --- | --- | --- |
| **realinput-1010c, row 1**: a real Escape while annotating closed the right panel and floated the page; the pick ended by the panel closing, no chip | Annotate preview; Escape in the page | The page's overlay cancels the pick and the panel stays (the reference's pick focuses the page; PickPreload's Escape) | **pass in agent mode at `85ea13fbe`'s code** (after1): while annotating the tree's `panel-toggle-right` declares no key shortcut (before: `Escape`); after the key: `annotate cancelled in the page`, `right-panel` and "Annotate preview" in the tree, no player; before: `annotate cancel` (the panel's), the page floated. Reference: the panel stays, "Annotate preview" ([image](https://raw.githubusercontent.com/ccheever/exact2/feedab8ed6e4686cb27af670a0bcc9ea77182976/browser-surface-capture/c1010-escape.png), [drive record](https://raw.githubusercontent.com/ccheever/exact2/13ee8fd60b1ae23b319a2a9aa7bc1077719a68ef/browser-surface-capture/c1010-drive-record.txt), [reference record](https://raw.githubusercontent.com/ccheever/exact2/35da06e55fde2da70209cdc040ac501955341676/browser-surface-capture/c1010-reference-record.txt)). Bun: "Escape while the shown tab annotates is the page's". By real keys: step 2 below |
| **realinput-1010c, row 1 (state)**: after that, reopening the panel left Float reading "Close floating preview" while the page sat in the panel | Toggle right panel on a Browser tab (closePreviewPanel floats it), then Toggle right panel again | The reference's state | **no change: the reference does the same**: its tree reads `button "Close floating preview" [pressed]` with the page in the panel (PreviewView `pictureInPicture = miniPlayerTabId === tabId`; ChatView `toggleRightPanel` opens the panel with `toggleVisibility`, which keeps the store's player), and its first click closes the hidden player ([reference record](https://raw.githubusercontent.com/ccheever/exact2/35da06e55fde2da70209cdc040ac501955341676/browser-surface-capture/c1010-reference-record.txt)). With row 1 fixed, Escape no longer leads there. Asked in the PR ("Decision needed") |
| **realinput-1010c, row 2**: the pill showed only on the 12-pt dot and hid on its buttons | Float; the pointer on the handle dot, then on Pop into separate window, then off the player | The pill stays while the pointer is on it (the reference's `group-hover`), with the button's tooltip | **pass in agent mode at the final code** (after2): on Pop into separate window the pill stays and "Pop into separate window" shows, ending at the button inside the window; before: the pill hid. Reference: the pill stays with that tooltip ([image](https://raw.githubusercontent.com/ccheever/exact2/36ee4d966ef4b014737fee0970f46b1e28b3dfd5/browser-surface-capture/c1010-pill.png)). Bun: "the handle's hover box covers the pill while it shows…". A real pointer: step 3 |
| **realinput-1010c, row 3**: the handle drag did not track the pointer ((-200, -100) moved (-100, -188); a later drag sent it back; a small drag did nothing) | Drag the handle (-200, -100) in four moves, the clock moving between them as a person's does | MiniPlayerShell: the frame at the gesture's start plus the pointer's total | **pass** (after2): (1028, 495) → (928, 395), moved (-100, -100) (x stops at the readable chat lane); before: (928, 307), (-100, -188). Reference from the same place: (-100, -100) ([image](https://raw.githubusercontent.com/ccheever/exact2/73dcbaf2be0209751500c6699fed678d45290788/browser-surface-capture/c1010-drag-resize.png), frames in the [drive record](https://raw.githubusercontent.com/ccheever/exact2/13ee8fd60b1ae23b319a2a9aa7bc1077719a68ef/browser-surface-capture/c1010-drive-record.txt)). Bun: "one gesture keeps one serial" fails before; it is a line check of the contract's `pan`, where the fix is, and the drive above is the behavior. "a resize's pans under one serial land where one pan of their total does" **passes on the previous code too** (run on an export of `a63794adb`'s example, review of 2026-10-10, second): the canvas did not change, so it documents why one serial per gesture matters, not the fix. A real pointer: step 3 |
| **realinput-1010c, row 4**: the resize kept the aspect ratio but overshot (an 80-pt NW drag took 240 × 333 to 416 × 577) | The north-west grip (-80, -80) in four moves | resizePreviewMiniPlayer's corner projection | **pass** (after2): 240 × 333 → **305 × 423**, the bottom-right corner held; before: → 403 × 559. Reference: 240 × 333 → 305 × 423 ([image](https://raw.githubusercontent.com/ccheever/exact2/73dcbaf2be0209751500c6699fed678d45290788/browser-surface-capture/c1010-drag-resize.png), [reference record](https://raw.githubusercontent.com/ccheever/exact2/35da06e55fde2da70209cdc040ac501955341676/browser-surface-capture/c1010-reference-record.txt)) |
| **realinput-1010c, row 5**: after Erase, the next Draw's editor appears without focus | Annotate; Draw; a stroke; Erase on it; Draw; a stroke | The reference's state | **no change: the reference does the same**: over CDP its overlay's `activeElement` is the comment after the first stroke, none (the page's body) after the erase and after the second stroke, and a key then goes to the page; PickPreload's `updateStatus` focuses the comment only the first time it shows (`editorWasShown`), as the port does ([image](https://raw.githubusercontent.com/ccheever/exact2/64fcd7cd2e7e727caba48cbfbc602e12e2ff98c8/browser-surface-capture/c1010-erase-draw-reference.png), [reference record](https://raw.githubusercontent.com/ccheever/exact2/35da06e55fde2da70209cdc040ac501955341676/browser-surface-capture/c1010-reference-record.txt)). Step 2 now clicks the comment before typing |
| **realinput-1010c, row 6**: the "Toggle right panel (⌥⌘B)" and "Pop into separate window" tooltips stayed after the pointer left | Hover and press the header's Toggle right panel, the pointer elsewhere, Float; the pill's buttons | No tooltip without the pointer on its trigger (Base UI closes one on press and on leave) | **pass** (after2): no header tooltip after Float; before: "Toggle right panel (⌥⌘B)" at the header in every later image (it left the header under the pointer as the panel opened, so no leave came). Reference: none ([image](https://raw.githubusercontent.com/ccheever/exact2/b10b88a71c69341428b785bb6a17c3519a6841d4/browser-surface-capture/c1010-header-tooltip.png)). The pill's tooltips follow the pointer's place in the hover box and close on a press. Bun: "the header's Toggle right panel drops its hover on press" |
| **realinput-1010c, row 7**: a draft's sidebar row showed `[101](t3-context://v1/preview-annotation/…)` | A chip "101" in a new thread's draft; open another thread | SidebarDraftRow: `replaceComposerContextReferences(prompt, label)` | **pass** (after1): the row reads "101" and the tree has no `t3-context` text (before: 2 nodes, the row shows the link). Reference: the row reads its chips' labels ("Tighten the button spacing Sen…") ([image](https://raw.githubusercontent.com/ccheever/exact2/c0bb33cf941c7d56ab7b817097cb3848239b599c/browser-surface-capture/c1010-sidebar-draft.png)). Bun (`sidebar.test.ts`): "a draft row reads its context chips by their labels" |
| **Review of 2026-10-10 (second), finding 1**: the PR and row 3 said every Bun row of the realinput-1010c round fails on the previous code | HEAD's `browser-capture.test.ts` and `sidebar.test.ts` on an export of `a63794adb`'s example | Each claim as true | **corrected**: 71 pass, 5 fail there. Five of the six new rows fail; "a resize's pans under one serial land where one pan of their total does" passes, since `chat-canvas-view.ts`, `previewMiniPlayerStore.ts` and `browser-capture.ts` did not change. It documents why one serial matters; the fix is the contract's `pan` (the line row "one gesture keeps one serial") and the drive. The test file's comment, row 3 above and the PR say so |
| **Review of 2026-10-10 (second), finding 2**: a drag that started on a pill button moved the player once past the slop (the handle's `pan` held the buttons, LLP 1057.001 rule 3); declared in `EXACT2-GAPS.md` without a framework issue | Float; rest on the dot; press on "Open preview in right panel" and drag (-200, -100) in four moves; then a drag (-100, -40) from the pill's bottom padding; the dot with the pill hidden (+50, +50) | MiniPlayerShell: the buttons stop the pointer-down, so a press there never drags; the pill's padding and the dot drag (the pointer-down reaches the handle's group) | **pass** (after3, after4): the button drag leaves the player at (1028, 495), floating, the button focused; before-c: moved (-100, -100), and a second press on Pop into separate window moved it (+50, +50) again. The padding drag: (1028, 495) → (928, 455), (-100, -40). The dot (after4, the pill hidden): (928, 455) → (978, 495) (y stops at the canvas's bottom). The pill and "Pop into separate window" still show on the button, and Open in right panel still restores. Reference from the same place: unmoved, still floating; the padding drag (-100, -40) ([button drag](https://raw.githubusercontent.com/ccheever/exact2/7ae8b6507aa6973bb4ba52c03a37f3cb695a4a29/browser-surface-capture/r1010b-button-drag.png), [padding drag](https://raw.githubusercontent.com/ccheever/exact2/d88d63786672aa9d73c8096e7e119580803193bd/browser-surface-capture/r1010b-padding-drag.png), [drive record](https://raw.githubusercontent.com/ccheever/exact2/e5c7e67d297274105115de844efcad5a6b3fa34c/browser-surface-capture/r1010b-drive-record.txt)). The declared difference is removed. Bun: "a press on a pill button never drags the player" (fails on `b691e6388`). A real pointer: step 3 d |
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
| **Open: Annotate's real drags and Escape** | Real-input step 2 | A real marquee over empty space gives a region; Draw makes a stroke; Erase removes it; a real Escape cancels in the page and keeps the panel | realinput-1010c: Region and Draw passed, Escape failed (row 1 above, fixed); **re-check by real keys** |
| **Open: the player by a real pointer** | Real-input step 3 | The pill stays shown from the handle onto it, no flicker; drag and resize follow the pointer; a press on a pill button never drags; no tooltip stays | realinput-1010c failed the pill, the drag, the resize and two tooltips (rows 2–4 and 6 above, fixed); **re-check by a real pointer** |
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
rule, `toastAct`'s keep rule, the artifact error toasts; five new in the realinput-1010c round: the panel toggle's Escape rule while annotating, the pill's hover box, one serial per gesture, a resize's pans under one serial (305 × 423 from 240 × 333; the one row of the round that passes on the previous code, since the canvas did not change), the header toggle's press; one new in the second review round of 2026-10-10: a press on a pill button never drags the player), `sidebar.test.ts` +1 (a draft row's chip labels), part 5's `browser-automation.test.ts` Auto-show row (read through part 4's `resolveBrowserOpenDefaults` since the merge of #354). AppKit `macos/tests/browser-capture`
34: recordingCompositor.test.ts 7 (8 methods, the it.each's two rows), Manager.test.ts `fitPictureInPictureContentSize` 2
and `recordingFileExtension` 1; clone: Annotate 9, recording 8, capture 6 (the screenshot row checks the page's own pixels
since the review of 2026-10-10). `macos/tests/browser` 29 (part 1 and part 2's
rows; two part 1 rows expect downloads). Substitutions are in each file's header.

## Checks

At code head `b6ca2cbc4` (the record commit after it changes only this file), once: `bun test examples/t3-code --timeout
60000` **4,355 pass, 1 skip, 0 fail** (exit 0; the first run of the check script exited 1 because the path filter also
matched two scratch exports of older revisions under this worktree's `target/`, made for the claim check of finding 1;
with them removed the suite ran alone); strict `tsc` (README command) exit 0; `contract build` of `app.contract` exit 0
(**1,341 lines**, unchanged); `git add -A && bun scripts/caps.mjs` "All budgets within cap"; the five checks: `cargo build
--all-targets --keep-going` 0, `cargo test --lib --bins --tests --no-fail-fast` 0, `cargo clippy --all-targets
--keep-going -- -D warnings` 0, `cargo fmt --all -- --check` 0, caps 0, `bun scripts/boot.mjs` 0. No Rust or Swift
changed, so `cargo test -p t3-code-macos --lib` and the AppKit binaries were not rerun (last results at `a63794adb`).

Previous round: At code head `b691e6388` (the record commit after it changes only this file), once: `bun test examples/t3-code --timeout
60000` **4,354 pass, 1 skip, 0 fail** (exit 0; an earlier run on `871f4ffe1` failed one row, `usage-pooled.test.ts`'s
pointer-taker list, and `contract build` refused part 3's one `now()` call after main's rename; `b691e6388` fixes both); strict `tsc` (README command) exit 0;
`contract build` of `app.contract` exit 0 (**1,341 lines**; this round adds none, the base merges +26); `git add -A && bun
scripts/caps.mjs` "All budgets within cap"; the five checks: `cargo build --all-targets --keep-going` 0, `cargo test --lib
--bins --tests --no-fail-fast` 0, `cargo clippy --all-targets --keep-going -- -D warnings` 0, `cargo fmt --all -- --check`
0, caps 0, `bun scripts/boot.mjs` 0. No Rust or Swift changed this round, so `cargo test -p t3-code-macos --lib` and the
AppKit binaries were not rerun (their last results, at `a63794adb`, follow).

Previous round: At code head `a63794adb` (the record commit after it changes only this file), once: `bun test examples/t3-code --timeout
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

2026-10-10, fix round of realinput-1010c (the coordinator's real-input session, row A: seven findings). Merged the base
first (`800c24a8b`, `b5dceb1c8`: part 4 #354, #371 #375 #376 #378 #379 #382 #383; conflicts kept both sides: part 4's
profiles and defaults beside part 3's capture view, the More menu's keyed rows with part 3's window row, the planner's
Auto-show read through part 4's defaults). Checked each finding against the reference over CDP: rows 1, 2, 3, 4, 6 and 7
differ and are fixed in `85ea13fbe`; Float's label after a reopened panel and the unfocused editor after Erase are the
reference's own states (no change). Drives before-a/before-b (the realinput bundle) and after1/after2 (this round)
reproduced and then cleared rows 1–4, 6 and 7 in agent mode; after2 stopped on the driver's clock ("the clock cannot go
backwards", 48028.0 → 48027.99999999999) after rows 2–4 and 6, so rows 1 and 7 are after1's (its build differs only in
the pill's tooltip alignment). Merged the base again (`871f4ffe1`, #384): part 3's icon buttons say `title=""`.
`app.contract`: 1,315 → 1,341 lines from the base merges (this round adds none there).

2026-10-10, review round (second; an independent review of #349: two should-fix findings). Ran HEAD's
`browser-capture.test.ts` and `sidebar.test.ts` on an export of `a63794adb`'s example: five of the round's six new rows
fail there and the canvas row passes, so the claim is corrected (the test file's comment, row 3, the PR). The pill's
declared drag difference is gone by structure: the drag is a box behind the pill (`42e43621a`, a new Bun row that fails
on `b691e6388`). Merged the base (`b6ca2cbc4`, records and a comment). Drives before-c (the after2 bundle), after3 and
after4 (the retry, adding the dot drag with the pill hidden) and the reference over CDP took the same steps.
`app.contract`: 1,341 lines (unchanged).

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
| Reference drive, realinput-1010c (CDP) | T3 Code 0.0.45, `A/ref-app.sh browser-surface-capture 16700`, the audit lane's data | Escape in the page keeps the panel; Float's label after a reopen; the pill on its buttons; drag (-200, -100) → (-100, -100); NW grip → 305 × 423; Erase then Draw unfocused; no stuck tooltip; the draft row's labels | [record](https://raw.githubusercontent.com/ccheever/exact2/35da06e55fde2da70209cdc040ac501955341676/browser-surface-capture/c1010-reference-record.txt), [driver](https://raw.githubusercontent.com/ccheever/exact2/b74ca0e0f97fa1c9c228ad34dc9049e581023338/browser-surface-capture/c1010-ref-drive.mjs.txt) | none |
| before-a, before-b (agent mode) | the bundle realinput-1010c drove (`a63794adb`), `EXACT_MAC_BIN` | Every op answered (83 and 36 lines): rows 1–4, 6, 7 reproduced (the panel floats on Escape, the pill hides, (-100, -188), 403 × 559, the stuck header tooltip, the link text in the draft row) | [record](https://raw.githubusercontent.com/ccheever/exact2/13ee8fd60b1ae23b319a2a9aa7bc1077719a68ef/browser-surface-capture/c1010-drive-record.txt), [script](https://raw.githubusercontent.com/ccheever/exact2/9114163e1f7a2f69c6b99db90106c19cb47cd121/browser-surface-capture/c1010-drive.sh.txt) | — |
| after1 (agent mode) | `85ea13fbe` without the pill tooltip alignment, bundle rebuilt | Every op answered (81 lines): rows 1–4, 7 pass; row 6's header tooltip showed because the agent's pointer still rested on the toggle (the drive then moves it first) | same record | the drive's step, fixed |
| Reference drive, second review (CDP) | T3 Code 0.0.45, `A/ref-app.sh browser-surface-capture 16700`, one CDP session at 1280 × 840 | A drag from "Open preview in right panel" (-200, -100): unmoved, still floating; a drag from the pill's padding (-100, -40): moved (-100, -40) | [record](https://raw.githubusercontent.com/ccheever/exact2/e5c7e67d297274105115de844efcad5a6b3fa34c/browser-surface-capture/r1010b-drive-record.txt), [driver](https://raw.githubusercontent.com/ccheever/exact2/9bbd80059e82f02a073fda04b19912abb23c932c/browser-surface-capture/r1010b-ref-drive.mjs.txt) | — |
| before-c (agent mode) | the bundle of after2 (the code of `85ea13fbe`), kept and run with `EXACT_MAC_BIN` | Every op answered (75 lines, exit 0): the button drag moved the player (-100, -100), a press on Pop into separate window then moved it (+50, +50) | [record](https://raw.githubusercontent.com/ccheever/exact2/e5c7e67d297274105115de844efcad5a6b3fa34c/browser-surface-capture/r1010b-drive-record.txt), [script](https://raw.githubusercontent.com/ccheever/exact2/7e51929707010a9a0d8281fb7dc1c0fd98c46e65/browser-surface-capture/r1010b-drive.sh.txt) | — |
| after3 (agent mode) | `42e43621a` + the base merge `b6ca2cbc4`, bundle rebuilt | Every op answered (75 lines, exit 0): neither button press drags, the padding drag moves (-100, -40), the pill's tooltip and restore work; the dot step met Pop into separate window (the focused button keeps the pill shown, as the reference's focus-within) | same record | the drive's step |
| after4 (agent mode, the retry) | the same bundle | Every op answered (89 lines, exit 0): after3's results, and the dot with the pill hidden drags (928, 455) → (978, 495) | same record, [compose](https://raw.githubusercontent.com/ccheever/exact2/da5265cfd24a6145dc3486c43f1a03675c4dadb2/browser-surface-capture/r1010b-compose.py.txt) | none |
| after2 (agent mode, the retry) | the final code before the #384 merge, bundle rebuilt | Ops 1–67 of 83: rows 2, 3, 4, 6 pass; stopped at op 68 on the driver's clock (a rounding error); rows 1 and 7 stand on after1 | same record, [compose](https://raw.githubusercontent.com/ccheever/exact2/8304a391aaeae7a5bae2d687142496e50fd3271b/browser-surface-capture/c1010-compose.py.txt) | none for this task |

## Real-input batch steps

For the coordinator's batch, on #349's final head. They need an unlocked screen and a normal launch; everything else
already passed in agent mode (drive 9, and this round's after1/after2). Rewritten after realinput-1010c (row A) for every
row it failed: step 0 adds the header tooltip's setup, step 2 the page's Escape and the unfocused editor, step 3 the
pill, the drag, the resize and the tooltips, step 7 the draft's sidebar row. Conventions: "page (x, y)" is a point in the Browser tab's page, i.e. the page's
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
   Available: Codex …" toast (and "Nightly needs the beta mobile app", if shown) with its × (it covers the page's top
   right). Rest the pointer on the chat header's "Toggle right panel" until its tooltip shows (row 6's setup), click it, then "Browser"
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
2. Annotate's real drags, Erase and Escape (same thread, the Browser tab still open). Click "Annotate preview", click the
   overlay toolbar's "Region" (at the page's top, y 10–52), drag page (60, 340) → (320, 430) (empty space), type-text `201`,
   Return. Read back: `annotate picked attach: elements 0, regions 1, strokes 0`. Click "Annotate preview", "Draw", drag
   page (80, 470) → (300, 530): the editor shows with its caret in the comment. "Erase", click page (190, 500) (the
   stroke's middle: the stroke and the editor go). "Draw", drag page (80, 560) → (300, 600): the editor shows **without**
   the focus, as the reference's does (PickPreload focuses the comment only the first time it shows; realinput-1010c
   row 5, checked over CDP), so click the comment field first, then type-text `202`, Return. Read back: `elements 0,
   regions 0, strokes 1`.
   Escape (realinput-1010c row 1, fixed): click "Annotate preview" (read back: it reads "Cancel annotation"), then press
   Escape (`orca computer key escape`). Read back: `t3.browser: annotate cancelled in the page` (not `annotate cancel`)
   and `annotate applied N cancelled`; the AX tree still has the "Right panel" region and the Browser tab's page, no
   "Floating browser preview" region; the button reads "Annotate preview"; no chip was added.
3. The floating player by a real pointer (realinput-1010c rows 2, 3, 4, 6, fixed). Geometry, from the player's AX frame
   (region "Floating browser preview"; at 1280 × 840 it opens at window (1028, 495), 240 × 333): the handle dot is 14 pt in
   from its right and top edges; the pill's buttons are 24 pt from its top, Close floating preview 23 pt, Pop into
   separate window 49 pt and Open in right panel 75 pt in from its right edge.
   a. Move the pointer onto "Float preview over chat" and click it. Read back: no "Right panel" region; the player at the
      chat's bottom right; **no "Toggle right panel (⌥⌘B)" tooltip** anywhere (step 0 left the header's toggle hovered
      when it pressed it; window image).
   b. Rest the pointer on the handle dot: the pill shows (window image). Move it left in 4-pt steps along y = top + 24 to
      Pop into separate window, resting 1 s: the pill stays in three window images 300 ms apart, and "Pop into separate
      window" shows above the button, inside the window (it ends at the button's right edge near the window's edge).
      Move the pointer off the player (onto the chat): the pill and the tooltip go (window image).
   c. Click "Pop preview into separate window": a "Preview · Capture fixture" window opens (`pip open`); move off and
      back, click it again ("Close popped-out preview"; `pip close`); move off the player: no tooltip stays.
   d. Drag from the handle dot by (-200, -100) in one drag (`orca computer drag`): the player's top-left goes to (928, 395)
      (moved (-100, -100): x stops at the readable chat lane, as the reference's does; the realinput bundle gave
      (-100, -188)). Drag the dot by (+60, +40): top-left (988, 435). Drag it by (+10, +6): top-left (998, 441). It never
      jumps back to the corner.
      A press on a pill button never drags (review of 2026-10-10, second): rest on the dot, then press on "Open preview in
      right panel" and drag (-200, -100) in one drag: the player stays where it was and stays floating (no `restore` in
      the log), as the reference's (its buttons stop the pointer-down). Rest on the dot again, press in the pill's bottom
      padding (28 pt in from the pill's left edge, 2 pt above its bottom) and drag (-100, -40): the player moves with it.
   e. Press 6 pt outside the player's top-left corner (the north-west grip; the corner itself may meet the west edge's
      grip) and drag (-80, -80): the size goes from 240 × 333 to **305 × 423** (the reference's corner projection), the
      bottom-right corner held unless the layout lifts it above the composer (the realinput bundle gave 416 × 577).
   f. Click "New thread" in the sidebar's `work` row: the player is gone; click the step 1 thread: it is back where it
      was left. Click "Open preview in right panel": the panel opens on the Browser tab and the player closes. Click
      "Toggle right panel" with the Browser tab shown: the player floats it again. Then click "Toggle right panel" once
      more: the panel opens on the tab and Float reads "Close floating preview" while the page sits in the panel, as the
      reference's does (realinput-1010c row 1's state, checked over CDP; its first click closes the hidden player).
      Click the pill's "Close": the player closes.
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
7. A draft's sidebar row (realinput-1010c row 7, fixed). With chips left in the `work` new thread's draft (step 2's
   `201`, `202`), click the "Timeline verification" thread in the sidebar. Read back: the `work` draft row's second
   line reads the chips' labels (e.g. `201 202`), never `[…](t3-context://…)`; the reference's row reads its chips'
   labels ("Tighten the button spacing Sen…").

After: `osascript -e 'quit app id "com.exact.t3code.macos.lanecapture"'` (an Apple Event quit stops its server), kill the
fixture pid in `<run>/pids.txt`, `defaults delete com.exact.t3code.macos.lanecapture`, remove `lane/apps/`, release the lock.

## Next action

PR [#349](https://github.com/ccheever/exact2/pull/349) stays a **draft**. The realinput-1010c findings are fixed (rows
1–4, 6, 7) or kept as the reference's own behavior (rows 1b and 5, "Decision needed" in the PR); the second review's two
findings are fixed at code head `b6ca2cbc4` (the canvas row's claim corrected; a press on a pill button never drags, so
the declared difference is gone).
Left: the real-input batch above (steps 1–7, rewritten for every row realinput-1010c failed), then the coordinator's
review and the ready flip. Follow-up outside this task: the floating device player (`r6-device.contract`) has the same
per-pan serial and the same sibling handle and pill hover boxes (not changed here).

## Delivery

Merged on 2026-10-10 as `70e2ddd4b` (#349, squash) after two independent reviews and the realinput-1010c fix round. The real-input rows passed in `realinput-1010d` on the fix round's bundle (code head `b6ca2cbc4`): steps 1, 4 and 5; a real Escape cancels Annotate in the page; the pill holds under hover; a press on a pill button never drags, a press on its padding does; drags follow the pointer's total; the NW resize; thread switch, restore and close; tooltips hide; step 7's labels ([fix round](https://raw.githubusercontent.com/ccheever/exact2/10c4009a2b19eb891713cd0fed56905057929b93/realinput-1010d/D4-349-fix-round.png), [pill](https://raw.githubusercontent.com/ccheever/exact2/4f2df998e1042f628051a603eff6e80fba76b23e/realinput-1010d/D4-349-pill.png), [app log](https://raw.githubusercontent.com/ccheever/exact2/8246315c3bb0972bd8bde40ea194d8992f690408/realinput-1010d/D4-349-app-log-t3browser.txt)). Step 6's focus passed in `realinput-1010c`. Reduced motion was not re-run in 1010d: it passed earlier, and the toolbar dot's code is unchanged. Two new findings moved to [realinput-1010d-followups](../20261010-realinput-1010d-followups.md): RD-1 (the pill does not show when the pointer arrives on the dot and rests, until it moves again) and RD-2 (where Float puts the player after a re-float).
