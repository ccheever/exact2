---
name: 20261011-browser-recording-and-downloads
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: unverified
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-browser-recording-and-downloads
pr_url: null
verified_commit: null
---

# The agent's recording tools, agent-driven downloads, and the page mute check

## Outcome

The user decided on 2026-10-11 that every item below is built now, matching the reference T3 Code. They are the
follow-ups the Browser surface's part 3 ([capture](closed/20261005-browser-surface-capture.md)) and part 5
([automation](closed/20261005-browser-surface-automation.md)) left declared in `EXACT2-GAPS.md` ("Browser surface:
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

## Next action

Build BR-1 and BR-2, record BR-3, then the drive and the checks.
