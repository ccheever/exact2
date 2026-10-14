---
name: 20261011-browser-recording-and-downloads
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-browser-recording-and-downloads
pr_url: https://github.com/ccheever/exact2/pull/445
verified_commit: 1f813789b
---

# The agent's recording tools, agent-driven downloads, and the page mute check

## Outcome

The user decided on 2026-10-11 that every item below is built now, matching the reference T3 Code. They are the
follow-ups the Browser surface's part 3 ([capture](20261005-browser-surface-capture.md)) and part 5
([automation](20261005-browser-surface-automation.md)) left declared in `EXACT2-GAPS.md` ("Browser surface:
declared differences (X1 path B)", Part 3 "Downloads", Part 5 "Recording (part 3)" and "Mute").

| Id | Clone (base `6db5cadea`) | Reference (check it first) |
| --- | --- | --- |
| BR-1 | `preview_recording_start` answers an execution error once the tab is ready ("recording arrives with browser-surface part 3"), and `preview_recording_stop` answers that nothing records, though part 3's recorder works from the Browser row's Capture button | The `previewAutomation` host's `recordingStart` starts the same recording the Capture button does and answers `{tabId, recording: true, startedAt}`; `recordingStop` (the server asks `transferToEnvironment: true`) stops it, saves the artifact, uploads the encoded file once as a pending attachment and answers the artifact with its `uploadedAttachmentId`; the server claims the upload and gives the agent the file's path in its environment. No active recording: `PreviewAutomationRecordingNotActiveError` |
| BR-2 | A download from a page asks with a save panel whoever started it (agent-mode runs, which show nothing modal, save every download into the artifact directory); a page is never marked agent-driven | A page an agent's control action (click, type, press, scroll, evaluate; not snapshot or wait for) drove last saves its downloads into the artifact directory as `browser-download-<id>-<name>` with no dialog; a person's input on the page, or a URL-bar navigation, hands it back, and a download then asks with the Save dialog. Nothing is shown or returned about the download either way |
| BR-3 | Mute silences the document's `<audio>` and `<video>` elements only (declared: `_setPageMuted:` is SPI) | `webContents.setAudioMuted`: the whole page, Web Audio and subframes too |

## Steps

1. Reference first: `PreviewAutomationHosts.tsx` (the `recordingStart` and `recordingStop` cases),
   `browserRecording.ts`, `browserRecordingUpload.ts`, the server's `mcp/toolkits/preview/handlers.ts`
   (`claimPreviewRecording`), `Manager.ts` (`agentDrivenWebContents`, `installDownloadHandler`, `handleHumanInput`,
   `navigate`, `READ_ONLY_CONTROL_ACTIONS`).
2. BR-1: the automation host answers both recording operations with part 3's recording lifecycle (one recording per
   tab, shared with the Capture button) and uploads the stopped recording to the request's environment as the
   reference does.
3. BR-2: mark a page agent-driven on the agent's non-read-only control actions; clear it on a person's input on the page
   and on a navigation; the download decision reads it.
4. BR-3: check every public WebKit header of the SDK for a page mute; record the result; keep the declared difference
   if only SPI has it.
5. Tests (Bun, AppKit), one agent-mode drive with the lane's fake ACP agent calling the preview tools through the lane
   server's MCP endpoint, before/after evidence, the checks once on the final head.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| BR-1 | Bun tests of the host's recording path; AppKit test of the module's hand-off; the agent drive: `preview_recording_start`, a click, `preview_recording_stop` through the lane server's MCP endpoint, the returned path's file read back | before / after tool results (text) and the app with its recording state (images) |
| BR-2 | AppKit tests: a download after the agent's click goes to the artifact directory with no panel; after a person's click or a navigation it asks; the agent drive: `preview_click` on a download link with the save panel enabled | before / after text records |
| BR-3 | The SDK's public headers searched; the SPI's presence at run time | text record; `EXACT2-GAPS.md` row |
| BR-2 under a real pointer | real-input steps (the save panel after a person's click) | — |

## Reference

- BR-1: `apps/web/src/components/preview/PreviewAutomationHosts.tsx:732-781` (`recordingStart`, `recordingStop`);
  `apps/web/src/browser/browserRecording.ts` (`startBrowserRecording`, `stopBrowserRecording`,
  `stopBrowserRecordingForUpload`, `readActiveBrowserRecordingTargets`), `browserRecordingScope.ts`
  (`resolveBrowserRecordingStopTarget`), `browserRecordingUpload.ts`; `apps/web/src/components/preview/previewAutomationErrors.ts`
  (`PreviewAutomationRecordingNotActiveError`); `packages/contracts/src/previewAutomation.ts`
  (`PreviewAutomationRecordingStatus`, `PreviewAutomationRecordingArtifact`, `PREVIEW_RECORDING_STOP_TIMEOUT_MS`, the
  recording transfer errors); `apps/server/src/mcp/toolkits/preview/handlers.ts` (`claimPreviewRecording`);
  `packages/client-runtime/src/state/attachments.ts` (`runAttachmentUploadCycle`).
- BR-2: `apps/desktop/src/preview/Manager.ts` `READ_ONLY_CONTROL_ACTIONS` (:523), `agentDrivenWebContents` (:694),
  `withControlSession` (:1500-1537), `handleHumanInput` (:1955-1962), `navigate` (:2541-2543), `installDownloadHandler`
  (:3568-3583).
- BR-3: `apps/desktop/src/preview/Manager.ts` `setAudioMuted`; `apps/web/src/components/RightPanelTabs.tsx`
  `tabMuteMenuItem`.

## Cause and fix

**BR-1.** The previewAutomation host (T3BrowserAutomation, `T3BrowserAutomationRecording.swift`) waits for the tab as for
every operation (`requireReadyTab`; a stop does not wait), lists the request in its status
(`presentation.browserAutomation.recordings`, with the request's host deadline) and answers `previewAutomation.respond`
with the outcome the data module sends back (`browserAutomation` with `answer`), or with an execution error once the
deadline passes with none. The data module (`browser-automation-recording.ts`) runs the reference's two cases on part
3's lifecycle, the client's one `BrowserRecordings`, so the agent's recording is the Capture button's: `recordingStart`
answers `{tabId, recording: true, startedAt}` (an already running one its own start; the same server tab under another
page is the conflict, answered as the operation's failure, as `PreviewAutomationOperationError.fromCause` does);
`recordingStop` resolves the tab (`resolveBrowserRecordingStopTarget`), stops it, saves the artifact and, as the server
asks (`transferToEnvironment`), uploads the file once (`stopBrowserRecordingForUpload`, `uploadBrowserRecording`), then
answers the artifact with its `uploadedAttachmentId`, which the server claims; nothing recording is
`PreviewAutomationRecordingNotActiveError`. The upload is native (`browserRecordingUpload`: `attachments.createUploadUrl`
on the request's connection, then the saved file's bytes through the transport's `uploadAttachment`), so the file never
crosses the data module; it keeps the request's deadline (the module's clock) and reports whether a failure came past it
(the deadline error) or not (the transfer error). The contracts' three transfer errors now carry the contracts' messages.

Two things the live drives found, each fixed before the next drive:
- *The lifecycle cannot run detached from the shell's answers* (drive after-1: the start timed out at the host deadline,
  no `record arm`). A native call belongs to the answer whose continuation makes it: a let-go answer's are refused, and a
  given answer's wait for the next settle. The panel now projects a serial for each handed-over request
  (`shell.panel.automationRecording`); `app.contract`'s `automationRecording` task sends
  `shelllocal:surface-browser-automation-recording` on a queued mutation (`automationRecordingServed`), and that send's
  answer runs every listed request to its end, as the Capture button's send runs its recording.
- *A module request with a top-level `fleet` goes to the fleet* (drive after-2: the recording was saved, the stop
  failed): `T3Module.later` routes any request naming one, even an empty one. The upload's connection is now
  `connection: {generation, fleet}`, guarded by a Bun row.

Review of 2026-10-12 (two should-fix findings, fixed in `1c4ae3191`):
- *One queued send held a later request behind an earlier one.* A queued send waits for the reply before it, and each
  send's answer waited for every running request, so thread B's `preview_recording_start` listed during thread A's
  `preview_recording_stop` (a large upload's budget is about 118.5 s) could not start before A's send replied, and failed
  at its own 13.5 s host deadline; the reference's `handleRequest` runs each request on its own. The window now has four
  queued sends (`automationRecordingServed0`-`3`); the panel projects `{serial, slot}`, `slot` naming a send whose answer
  runs nothing (`automationRecordingView`), the task sends on it, and each send's answer waits only for the requests it
  started. Only a fifth request in flight at once waits, behind one of the four (declared in `EXACT2-GAPS.md` part 5
  "Recording (part 3)": a Contract's mutations are declared, and a mutation's sends replace or queue).
- *The agent's send counted as the person's use of the panel.* `shelllocal:surface-browser-automation-recording` went
  through `surfaceLocal`, which created the viewed thread's panel state, bumped its `userRevision` and installed the
  terminal and browser cleanups, so an agent request served before the panel was restored made `restoreRightPanel`
  skip the saved panel and `syncRightPanels` delete it. The op now returns at the top of `surfaceLocal`, before the
  panel is read.

**BR-2.** `T3BrowserDownloads.agentDriven` (Manager.ts `agentDrivenWebContents`) is set by the host's control session for
every action but the read-only ones (`READ_ONLY_CONTROL_ACTIONS`: snapshot, wait for) and cleared by a navigation
(`T3BrowserSession.navigate`, the URL bar's and the agent's, as Manager.ts `navigate`) and by a person's press or key on
the page: a local `NSEvent` monitor in `T3BrowserSessions` (`humanInput`) hit-tests a mouse press, or reads the key
window's first responder for a key, and hands back the page it lands on. The agent's native input goes straight to the
page's view (`T3BrowserAutomationInput`), never through the application's queue, so the monitor hears only the person
(the reference's preload reports trusted `pointerdown`/`keydown` and skips the agent's expected ones). A download from an
agent-driven page goes to the artifact directory as `browser-download-<id>-<name>` with no dialog; any other asks with the
save sheet (`askWhere`); an agent-mode run, which shows nothing modal, still saves every download. Nothing is shown or
returned about a download, as in the reference; the tab's report and the host log name who started it (`by`).

**BR-3.** Confirmed: no public WebKit API mutes a page. Every public header of the macOS 27.0 SDK's WebKit (221 files)
has only the camera and microphone capture states and `WKWebExtensionTab.setMuted(_:for:)`, a method of a protocol the app
implements when an extension asks it to mute its own tab; `WKWebView` answers `_setPageMuted:` and `_mediaMutedState` as
SPI only. The declared difference stays (`EXACT2-GAPS.md` Part 5 "Mute", re-dated).

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| BR-1 | pass, agent mode, end to end through the lane server's MCP endpoint (the lane's fake ACP agent, the thread's MCP credential, no real provider): before (feature tip), `preview_recording_start` and `_stop` answer errors and nothing records; after, the start answers `{tabId: "tab_1", recording: true, startedAt}`, the Capture button shows Stop recording (red, with the dot) while the agent records, the stop answers the artifact (`video/mp4;codecs=avc1`, 303,982 bytes) at a path in the server's attachments that the agent reads back as an MP4 of that size, and a second stop is the not-active error; reference, the same results (its recording shows on the floating preview's dot: the agent's tab floats there, part 3's declared presentation difference). Bun 16 rows, AppKit 7 rows (`BrowserRecordingHandOffTests`). Review of 2026-10-12: a start listed during another thread's stop now runs on a free send and answers before that stop's upload ends (before: after it), and the agent's send leaves the viewed thread's unrestored panel alone (before: `userRevision` 1, the saved panel deleted, restore skipped); the live drive after-4 repeats after-3's results on the four sends | [recording](https://raw.githubusercontent.com/ccheever/exact2/dc2df06fcd0e3dbc0bb859ebb79743f2b270389a/browser-recording-and-downloads/br1-recording.png), [chrome row](https://raw.githubusercontent.com/ccheever/exact2/6f5f16654d8b9627a4de242fd2bd97aff23f520a/browser-recording-and-downloads/br1-recording-chrome.png), [results](https://raw.githubusercontent.com/ccheever/exact2/0922b3af79a83f5757a811110c3351576897f24c/browser-recording-and-downloads/br1-done.png), [tool results](https://raw.githubusercontent.com/ccheever/exact2/64eafb1f6a42328c5d1d99f8303fc1eea377d696/browser-recording-and-downloads/br1-e2e-record.txt), [review fixes before/after](https://raw.githubusercontent.com/ccheever/exact2/b6fd4c148a2bbbef0e5cebddbde98dd64ae4c845/browser-recording-and-downloads/review-fixes-record.txt) ([scenario](https://raw.githubusercontent.com/ccheever/exact2/23802748fb476759d132f2244134d38b37b456a4/browser-recording-and-downloads/review-fixes-scenario.ts.txt)), [drive after-4](https://raw.githubusercontent.com/ccheever/exact2/eab5959d9cf3c27004c0dde0ef80aaa353ea47c7/browser-recording-and-downloads/review-fixes-drive.txt) |
| BR-2 | pass (AppKit, the page's save dialogs on, a person's input through `NSApplication.sendEvent`): the agent's click on a download link saves `browser-download-<id>-export.csv` in the artifact directory with no sheet; a person's click asks with the save sheet before and after the agent drove the page; a snapshot and a wait leave the page the person's; a navigation hands it back. Before (the same test file against the feature tip's modules): the agent's download asks with the sheet (6 assertions fail). Two mutations (no hand-back on a person's press; reading marks the page) each fail it. Live: the agent's download in the after drive logs `by=agent`; the reference saved its agent's download without a dialog | [AppKit before/after](https://raw.githubusercontent.com/ccheever/exact2/dd1c08bc4e7cf386edefbcf7c6c83c1b03338eec/browser-recording-and-downloads/br2-appkit-record.txt) |
| BR-3 | confirmed SPI-only; declared difference kept | [headers and SPI](https://raw.githubusercontent.com/ccheever/exact2/1e3c700a2dff252e37923cfb78f687a381b35a39/browser-recording-and-downloads/br3-record.txt) |
| BR-2 under a real pointer | open: real-input step 1 | below |

Drives (agent mode, 1280×840): before = the evidence-base bundle (`6db5cadea`) on lane `browser-recording-and-downloads-before`; after = this branch's bundle of `266c552a4` on lane `browser-recording-and-downloads-3` (the app code of the final head but for `f02e62e82`, which removes an unused Swift error case and updates a test); reference = T3 Code (Electron) `1e2ecbd975` on the lane's reference home over CDP. Lane scripts: [fake ACP agent](https://raw.githubusercontent.com/ccheever/exact2/573f0cc9404dcd980de60f82c8387fbbc92175f6/browser-recording-and-downloads/lane-fake-acp.mjs.txt), [fixture](https://raw.githubusercontent.com/ccheever/exact2/dee1157de63bb85d21b82c71882963703d8bbc1b/browser-recording-and-downloads/lane-fixture.mjs.txt), [lane setup](https://raw.githubusercontent.com/ccheever/exact2/e4f7867823ffa443f80d1b6175dd9336bbd66f71/browser-recording-and-downloads/lane-setup.sh.txt), [drive](https://raw.githubusercontent.com/ccheever/exact2/8c3c60e05d89686e6d886307aba09d017d902d27/browser-recording-and-downloads/drive.sh.txt). After the review fixes, after-4 = this branch's bundle of `d51b8d9fc` on lane `browser-recording-and-downloads-4`, the same drive: start, the clicks, the stop's 281,551-byte MP4 read back, the second stop's error, all as in after-3; the three requests ran on send 0 in turn (the panel's projection ends at `{serial: 3, slot: 0}`). In agent mode the recording's `startedAt` reads the agent's virtual clock (`2026-01-01T00:00:00.001Z`); a person's app reads the wall clock (`composerNow`).

## Real-input batch steps

Launch this branch's bundle as a lane copy (its own bundle id, the lane home outside any git checkout) on a home seeded
with `lane-setup.sh` (the "Lane browser agent" fake ACP agent as the only provider), with the lane fixture
(`FIXTURE_PORT=16443 bun lane-fixture.mjs`) running. Never send a message to a real provider.

1. **BR-2, a person's download after the agent's.** In a new thread send `rec http://127.0.0.1:16443/rec`. When the reply
   lists the results, `<home>/userdata/browser-artifacts` holds `browser-download-…-export.csv` (the agent's click) and no
   dialog showed. Click "Export CSV" in the Browser tab with the real pointer. Expected (reference): a save sheet asks,
   named `export.csv`; Cancel; no new file in the artifact directory. Then type a letter into the page's Name field and
   click "Export CSV" again: the sheet asks again.
2. **BR-1, a person's Stop during the agent's recording.** Send `rec http://127.0.0.1:16443/rec` again; while the reply is
   pending (the agent holds the recording about 12 s), click the red Capture button (Stop recording). Expected
   (reference's shared lifecycle): "Recording saved" with Reveal in Finder and Copy path; the agent's
   `preview_recording_stop` then answers the not-active error in its reply.

## Tests

- `browser-automation-recording.test.ts` (16): the handed-over requests read defensively; start answers the status and
  the Capture button's recordings show it; stop answers the saved, uploaded artifact once; a local stop; not active (none,
  or another tab named); a conflict and a missing page; a failed transfer removes its pending upload; the 50 MiB and
  deadline errors with the contracts' messages; the panel's serial; a request listed while another send's request runs
  starts at once on a free send and answers first; with all four sends running, a new one waits behind one of them; the
  send runs each request once through its own native; the upload's request shape (no top-level `fleet`); the contract's
  four queued sends and task; the send is not the person's use of the panel (no panel state, no `userRevision`).
- `browser-automation.test.ts` (+1): a recording stop lists the thread's sessions first and names its current tab.
- AppKit `macos/tests/browser-automation`: `BrowserRecordingHandOffTests` (7: the start waits for the tab and answers the
  data module's outcome once; no page times out as the overlay wait; the stop is handed over at once and a host error
  passes through; no answer fails at the host deadline; the upload mints a pending file attachment and sends the saved
  bytes on the request's connection or fleet; a file outside the artifact directory and a spent budget mint nothing; a
  rejected transfer names its pending attachment and remove deletes it) and `BrowserDownloadTests` (3, above); the part-5
  error row now reads the hand-off.

## Checks

On `ee1e9f1fd` (the review fixes `1c4ae3191` merged with `origin/feat(example)/t3-code` at `e91bdc8af`; this record's
later commit changes only the record). The review fixes change no Rust or Swift, so the AppKit binaries and
`cargo test -p t3-code-macos --lib` were not re-run (last run on `f02e62e82`: AppKit `browser-automation`, `browser`,
`browser-capture` 36, 29 and 34 tests, 0 failures; `t3-code-macos` 21 passed).

| Check | Exit | Note |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 0 | 4685 pass, 1 skip, 0 fail (314 files) |
| strict `tsc` (README command) | 0 | |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | `app.contract` 1,418 lines; 28,866,586 bytes |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap |
| `cargo build --all-targets --keep-going` | 0 | |
| `cargo test --lib --bins --tests --no-fail-fast` | 0 | 3,756 passed, 0 failed, 36 ignored (95 suites) |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `bun scripts/boot.mjs` | 0 | |

The first run of these on `d51b8d9fc` (the fixes merged with the feature branch at `fe0e39c91`) had `cargo test` exit 101
on main's `apps/color-gallery` button (`button_migration`, the failure adopt-main-fixes-r9 recorded); the feature branch
then moved to `e91bdc8af`, whose round 10 fixes that sweep, and the run above is on the merge of it.

## Not done / not verified

- Real-input steps 1 and 2: ran in `realinput-1011c` and passed ("Real-input results" below).

## Real-input results

`realinput-1011c` (2026-10-12 21:43 to 10-13 20:17 KST, five sittings; the attended session for the verify-1011c groups g1–g5 and the rows the 2026-10-11/12 merges brought) ran on the bundle of `1f813789b`, the T3 tip after #497, which holds this task: one lane copy per batch beside the reference T3 Code `1e2ecbd975`, real keys and pointer, light and dark from each lane app's own Appearance setting, never System Settings. Each row got one pass; failures were recorded, not retried. Session notes: [00-session-notes.txt](https://raw.githubusercontent.com/ccheever/exact2/a94ddd5cdbf485f1f57dc92c965c46c4bb17c8be/realinput-1011c/00-session-notes.txt).

| Step | Session row | Result | Evidence |
| --- | --- | --- | --- |
| 1, BR-2 | BRD-1 (B10) | Pass: the person's save sheet named `export.csv`, Cancel, no new artifact; it asks again after an edit | [BRD-1-save-sheet.png](https://raw.githubusercontent.com/ccheever/exact2/4a2b813363885c6c400668468930b952d053a1e1/realinput-1011c/b10-browser-agent/BRD-1-save-sheet.png) |
| 2, BR-1 | BRD-2 (B10) | Pass, after one redo in a new thread (the first attempt met the fake agent's one run per thread, a fixture fault): "Recording saved" with Reveal in Finder and Copy path | [BRD-2-saved-toast.png](https://raw.githubusercontent.com/ccheever/exact2/4a2b813363885c6c400668468930b952d053a1e1/realinput-1011c/b10-browser-agent/BRD-2-saved-toast.png) |

G4-08 (the Browser tab's menu, page mute declared SPI-only) passed on the same bundle.

## Delivery

Merged on 2026-10-12 as `443380976` (#445, squash) into `feat(example)/t3-code`.

## Next action

None. Closed after `realinput-1011c`, which passed both steps.
