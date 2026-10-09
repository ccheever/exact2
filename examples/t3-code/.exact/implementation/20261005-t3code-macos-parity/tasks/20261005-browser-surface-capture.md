---
name: 20261005-browser-surface-capture
plan: 20261005-t3code-macos-parity
implementation: done
verification: partial
delivery: open
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
  picked element"; ⌘Return presses Send through the window's task (`annotationSend`, 6 root lines).
- **Screenshot**: `browser-screenshot-<site>-<id>.png` (at most 1,280 px wide) in `<T3 home>/userdata/browser-artifacts`,
  the stacked "Screenshot saved" toast with Copy path, Reveal in Finder and Copy image (each copy reads "Copied!" for 2 s;
  the toast stays, `app.contract` `toastAct`'s keep rule), the reference's error titles.
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
  preview…", the recording dot, Open in right panel, Pop into separate window, Close and the eight resize grips; closing
  the panel on a Browser tab floats it (closePreviewPanel).
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

Declared differences: `EXACT2-GAPS.md` "Browser surface: declared differences (X1 path B)", section "Part 3 (capture)".

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
| scheduling preference | `20261005-floating-device-player` (player layout) | none | Not a prerequisite | — |

## Acceptance and reproduction

Lane (agent mode, 1280×840): `T3_LOCAL_HOME=<lane>/t3-home` (seeded by the runtime's `t3 project add`),
`T3_LOCAL_PORT=16720`, `T3_LOCAL_RUNTIME_DIR` (the staged runtime, unpacked), isolated `HOME`, `CODEX_HOME`,
`CLAUDE_CONFIG_DIR`, `XDG_*`, `T3CODE_TELEMETRY_ENABLED=false`; fixture on 127.0.0.1:16721 ([fixture](https://raw.githubusercontent.com/ccheever/exact2/f03062d925a92c942289bd76d6ffa49bb1c0634c/browser-surface-capture/fixture.mjs.txt));
closed port 16749; [drive script](https://raw.githubusercontent.com/ccheever/exact2/2ea24336269464d6b24e5cbcc116e8ff12763568/browser-surface-capture/drive.sh.txt). The screen was locked during the drives: window
captures are blank, so the live rows are read from trees, state and host logs ([drive record](https://raw.githubusercontent.com/ccheever/exact2/ba5e74bc3022370db26c6932383a5e1257f9e6aa/browser-surface-capture/drive-record.txt)); page-level images come from the app's own artifacts and the AppKit tests.

| Criterion | Action | Expected | Result |
| --- | --- | --- | --- |
| Ported tests | `bun test examples/t3-code`; the AppKit binaries | The ported tests pass | pass: see "Tests" |
| Annotate | Fixture page; Annotate; select, marquee, draw, erase; Esc; Enter, ⌘Return | Payload with elements, regions, strokes, style changes and a crop; the chip in the composer; Esc cancels; ⌘Return sends | AppKit (real clicks, keys and drags into a page): select + Return gives the element (tag, selector, preview), the comment and a PNG crop of the target plus 20 px; a fake React fiber gives "SubmitButton", its source and owner stack; ⌘Return "send"; Escape cancels and removes the overlay; marquee over empty space a region, drags two strokes, erase removes one each; a style change records 1 → 0.5 and is undone at teardown; a navigation settles nothing; the page cannot see the overlay ([select](https://raw.githubusercontent.com/ccheever/exact2/2fb539b683765a32d2a95e96fcd9f92a3d38fd8a/browser-surface-capture/annotate-select.png), [style panel](https://raw.githubusercontent.com/ccheever/exact2/7a7f4ff605315b360c04907f73f1f530948e4f82/browser-surface-capture/annotate-style-panel.png), [region and drawing](https://raw.githubusercontent.com/ccheever/exact2/934b46af1a358a97a7d93a88c4b454367614d537/browser-surface-capture/annotate-region-draw.png)). Bun: the chip at the caret once per pick, the crop on the shelf, the dropped-crop toast, ⌘Return's send, a new thread's draft. Live (drive 2): Annotate on the lane page, a click on `#save`, a comment and Return: `t3.browser: annotate picked attach: elements 1, regions 0, strokes 0, screenshot ok`; the chip did **not** reach the composer: the drive's thread was a new thread's draft (finding below). Drive 3, after the fix, on a new thread's draft: `annotate applied 1 chip+image`, and the composer holds `[Tighten the button spacing](t3-context://v1/preview-annotation/…)`. Real drags and Escape by real keys: real-input batch |
| Screenshot | Open tab; Capture | Image ≤ 1,280 px wide; "Screenshot saved" with Copy image, Copy path (and Reveal); clipboard | Live (drives 1 and 2): `t3.browser: screenshot browser-screenshot-127-0-0-1-….png` in the lane's `userdata/browser-artifacts`, 1,078 px wide ([the saved file](https://raw.githubusercontent.com/ccheever/exact2/68c014d7efdf8a70e6ccff58a5120459ffd8c7c9/browser-surface-capture/live-screenshot-artifact.png)); "Copy path" and "Reveal in Finder" pressed by label (`artifact copy-path`, `artifact reveal`; an agent run writes a private pasteboard and opens no Finder). AppKit: an 1,800-px page saves ≤ 1,280 px; actions refuse files outside the directory. Bun: the toast's three buttons, "Copied!" for 2 s, the error titles. The clipboard read with a real Copy image: real-input batch |
| Recording | Frame rate 60, keys and mouse on (set in Settings); Shift-click Capture; interact; Stop | Pulsing dot; file saved; "Recording saved" with Reveal and Copy path; overlays show keys and presses, never password input; conflicts | Live (drive 2): the drive set keys, mouse and 60 fps through Integrations rows this branch had then (since left to part 4; `record arm keys=true mouse=true`, `record capture 60fps 1078x1496`); while recording the button reads "Stop recording" with `browser-capture-dot`; the stop saved `browser-recording-….mp4` (267 frames in 4.5 s, about 59 fps) and the tree shows "Recording saved" ([frames](https://raw.githubusercontent.com/ccheever/exact2/4e28229bbe6648ac44cfbf7a77a50f7976eb25da/browser-surface-capture/live-recording-frames.png): the drawn cursor and a press ring; [the file](https://raw.githubusercontent.com/ccheever/exact2/910fdc5008af528cfda8626788d04d38aaff693a/browser-surface-capture/live-recording.mp4)). AppKit: 30 fps vs 60 fps frame counts, a playable MP4 of the snapshot's size, key labels and a nil label in a password field, the cursor re-installed after a navigation ([decorations](https://raw.githubusercontent.com/ccheever/exact2/fcb11b165a57b6220e032d8f3b08aa4766b345ed/browser-surface-capture/decorated-last-frame.png)). Bun: the lifecycle's 36 rows (conflicts, one stop, startup waits). The upload's oversize and deadline errors are ported; its caller is part 5 |
| Separate window and player | More › Open separate preview window; Float preview; drag; Open in right panel; close the panel | 480×320, min 240×160, about 12 fps; the player borrows the page; "Reconnecting preview…"; closing the panel floats the tab | Live: the More row opened the window (`capture.separateWindow: true`) and closed it (drive 2). Float preview did nothing in drive 2 (finding below); in drive 3 it floated the page (`browser-mini-player`, panel hidden), and in drive 4: the pill's Open in right panel restored the tab to the panel, its Pop into separate window opened and closed the window (`browserMini.separateWindow` true, then false), closing the panel on the Browser tab floated it, and Close closed the player. The pill's hover holds (drive 4: no render while the pointer rests; drive 3 found it flipping, finding below). AppKit: the window's title "Preview · <title>", floating level, all spaces, minimum size, no minimize/zoom, ≥ 3 frames, the page's aspect ratio, frames continue with the panel hidden, closing releases the hold; the two `fitPictureInPictureContentSize` rows. Bun: float/unfloat, the panel hides, the player shows unless the panel shows the tab, sized by its page; closing the panel floats it. A drag and a thread switch: real-input batch |
| Downloads | A page's download links | To the artifact directory | AppKit: `browser-download-<id>-<name>` in the directory, content intact; a redirected download ends its load. Live (drive 4): the fixture's attachment and `<a download>` links saved `browser-download-…-0-report.bin` and `browser-download-…-1-export.csv` in the lane's artifact directory (`t3.browser: download …`) |
| States | Each surface | Focus ring on every chrome button; Escape cancels one thing; no pulse with reduced motion | The dot's pulse and the player's dot are `none` under reduced motion (Contract `still`); the buttons are `button`s with AppKit's ring; a failed page's Annotate tip live (drive 4, closed port 16749: "Page didn’t load — pick unavailable until the page renders"); real keyboard focus and Escape by real keys: real-input batch |
| Standard gates | Clone checks, caps, the five checks | Green | see "Checks" |

**Finding of drive 2 (fixed, re-driven in drive 3).** The drive's thread was a new thread's draft: `client.threadId` is
empty while the Browser tab names the id `addBrowserSurface` allocated for the draft. Annotate's chip went to that id's
stored draft rather than the open composer (`composerText` stayed empty), and Float preview did nothing (no thread key
for the player). Both now key by the open thread (`browser-capture.ts` `playerThreadKey`; `applyAnnotation` compares
with `activeRef`), with a Bun row for the draft case; the module logs `annotate applied <serial> <outcome>`. Drive 3
passed both on a new thread's draft.

**Finding of drive 3 (fixed, re-driven in drive 4).** Hovering the floating player's handle started a render storm
(epochs 38 → 106 in 0.6 s, then 109 → 387 in 2.5 s; 3 in 2.5 s before the hover), and the pill's Open in right panel
did nothing. The handle and the pill were sibling hover boxes: the host hands a hover over as two events (the handle
out, then the pill in), so between them neither was hovered, the pill hid and refused the pointer, the resting
pointer's next hit-test found the handle again, and it showed the pill: a flip every frame, and a tap that landed on a
hidden pill. The pill is now the handle's child, as the reference's `group` (`browser-capture.contract`), so it is one
hover; a shape row fails before the fix. Drive 4: no render while the pointer rests, and restore, close and the
separate window from the pill work. The floating device player (`r6-device.contract`, another task's) has the same two
sibling boxes; the same change would fix it there (a follow-up, not made here).

Tests (`bun:test`; reference names unless marked clone): `browser-recording.test.ts` 36 (browserRecording.test.ts 31:
27 tests and the it.each's 4 rows; browserRecordingScope.test.ts 5), `browser-annotation.test.ts` 25
(PickedElementPayload.test.ts 20: 11 tests, the it.each's 9 rows; clone 5), `browser-annotate-page.test.ts` 6
(AnnotationKeyboard.test.ts 2; clone 4), `browser-recording-input.test.ts` 8 (RecordingInput.test.ts 5, its it.each's
rows), `previewMiniPlayerStore.test.ts` 6 (re-ported with browser sources), `browser-capture.test.ts` 15 (clone;
PreviewView.test.tsx's annotation rows followed). AppKit `macos/tests/browser-capture` 34: recordingCompositor.test.ts 7
(8 methods, the it.each's two rows), Manager.test.ts `fitPictureInPictureContentSize` 2 and `recordingFileExtension` 1;
clone: Annotate 9, recording 8, capture 6. `macos/tests/browser` 23 (two part 1 rows now expect downloads).
Substitutions are in each file's header.

## Checks

At the PR's head (after merging `origin/feat(example)/t3-code` at `5f0ae7dca`, #346, and the pill fix): `bun test
examples/t3-code --timeout 60000` 3,739 pass, 0 fail; strict `tsc` clean; `contract build` of `app.contract` OK (1,237
lines); `cargo test -p t3-code-macos --lib` 13 pass; AppKit after the merge: `browser-capture` 34, `browser` 23 and part
5's `browser-automation` 17 pass (before the merge the README's recipe over every directory passed but `mermaid`, which
needs `T3_SERVER`, a running T3 server's origin, unset here; no Swift changed after the merge); `git add -A && bun
scripts/caps.mjs` within caps; the five checks (`cargo build --all-targets --keep-going`, `cargo test --lib --bins
--tests --no-fail-fast`, `cargo clippy --all-targets --keep-going -- -D warnings`, `cargo fmt --all -- --check`, caps,
`bun scripts/boot.mjs`) pass.

## Progress

2026-10-09: written at part 1's `prepare` (planned split). Built the same day: the frame-source and content-world
decisions above; the overlay and the recorder were written in parallel by two forks with fixed interfaces, then wired
into the session (`T3BrowserSession.swift`, `T3BrowserSessions.swift`, `T3BrowserView.swift`), the data module and the
views. Four live agent-mode drives (the screen was locked): drive 2 found the draft-thread bug, drive 3 confirmed its fix
after merging part 5 (#346) and found the pill's hover flip, drive 4 confirmed that fix and ran the remaining ops. At the
merge, part 5's planner was given the Auto-show floating preview setting (`browser-automation.ts`, with a Bun row).
`app.contract`: 1,230 → 1,237 lines (the annotation send task and its state, the canvas re-ask key, the toast keep rule).

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| Drive 1 (agent mode) | branch at `fa62589a3` + the app.contract provide fix (bundle `d1ae77d7…`) | Ops 1–38 of 112: Settings › Integrations (the rows this branch had then), the Browser tab, a screenshot (artifact saved), Copy path, Reveal; stopped at op 39 on a driver word (`tap … at` is `tap … clicks 1 at`) | [drive record](https://raw.githubusercontent.com/ccheever/exact2/ba5e74bc3022370db26c6932383a5e1257f9e6aa/browser-surface-capture/drive-record.txt) | the drive script |
| Drive 2 (agent mode, the retry) | same bundle | Ops 1–81 of 118: screenshot and its toast buttons, Annotate (picked with a crop), recording at 60 fps with overlays (saved, "Recording saved"), the separate window open and closed; the chip and Float preview failed on a new thread's draft; stopped at op 82 (no floating player) | [drive record](https://raw.githubusercontent.com/ccheever/exact2/ba5e74bc3022370db26c6932383a5e1257f9e6aa/browser-surface-capture/drive-record.txt), [recording frames](https://raw.githubusercontent.com/ccheever/exact2/4e28229bbe6648ac44cfbf7a77a50f7976eb25da/browser-surface-capture/live-recording-frames.png), [screenshot artifact](https://raw.githubusercontent.com/ccheever/exact2/68c014d7efdf8a70e6ccff58a5120459ffd8c7c9/browser-surface-capture/live-screenshot-artifact.png) | fixed after the drive (draft-thread key); re-check in the real-input batch |
| Drive 3 (agent mode) | after merging #346 (`a0527085c`), bundle rebuilt | Ops 1–40 of 81: the screenshot, Annotate on a new thread's draft (`annotate applied 1 chip+image`, the chip in the composer), Float preview (the player shows, the panel hides); stopped at op 41: the pill's Open in right panel did nothing, and the hover flipped every frame (finding of drive 3) | [drive record](https://raw.githubusercontent.com/ccheever/exact2/ac612296529cc97d375ddbd2cad063f8f7b367a6/browser-surface-capture/drive-record-3-4.txt) | fixed after the drive |
| Drive 4 (agent mode) | the pill fix (`8a6020496`), bundle rebuilt | All 69 ops: the hover holds (epoch 30 for 2 s), restore, two downloads saved, the failed page's Annotate tip, closing the panel floats the tab, the separate window from the pill opens and closes, Close | [drive record](https://raw.githubusercontent.com/ccheever/exact2/ac612296529cc97d375ddbd2cad063f8f7b367a6/browser-surface-capture/drive-record-3-4.txt), [drive script](https://raw.githubusercontent.com/ccheever/exact2/fdd9795b50fa3d9c749bd661674ffbde18b7bc92/browser-surface-capture/drive4.sh.txt) | none (real-input rows below) |

## Real-input batch steps

Needs an unlocked screen, the real-input lock and a normal launch of a lane copy (part 1's recipe, "Real-input batch
steps", with the lane on 16720/16721). Each step reads its effect back (host `t3.browser:` lines, the artifact directory,
the AX focus):
1. A new thread's draft: Browser tab, the fixture page, Annotate, click `#save`, type a comment, Return: the amber chip at
   the composer's caret and the crop on the shelf, as seen (agent mode passed in drive 3); again with ⌘Return: the
   message sends with the annotation.
2. Annotate's real drags: marquee over empty space (a region), Draw (a stroke), Erase; Escape by a real key cancels.
3. Float preview: the player floats the page over the chat; a real pointer moving from the handle onto the pill keeps it
   shown, with no flicker; drag and resize it; Open in right panel; close the panel on a Browser tab (it floats); Pop into
   separate window from the pill (agent mode passed restore, close, the panel's float and the window in drive 4).
4. Capture: Copy image and Copy path by real clicks (the person's clipboard; restore it after), Reveal in Finder opens
   Finder at the file (close it).
5. A page's download by a real click: the save panel asks (cancel it); an agent run saves to the artifact directory
   (drive 4 passed).
6. Tab and Shift-Tab reach Annotate, Capture, Float preview with AppKit's ring; reduced motion on: no pulse.
7. Before/after images of the chrome row, the More menu, the toasts and the player (the screen was
   locked for the agent drives).

## Next action

PR [#349](https://github.com/ccheever/exact2/pull/349) ready for review (checks green, the branch clean against
`feat(example)/t3-code`); merge it again with the base once part 2 (#348) lands. Then the real-input batch above (STATUS
"Next real-input batch").
