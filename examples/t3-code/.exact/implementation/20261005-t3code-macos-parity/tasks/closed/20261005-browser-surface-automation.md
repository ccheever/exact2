---
name: 20261005-browser-surface-automation
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-browser-surface-automation
pr_url: https://github.com/ccheever/exact2/pull/346
verified_commit: 5f0ae7dca
---

# Browser surface part 5: the previewAutomation host, links from chat and terminal, and Mute

## Outcome

Agents drive Browser tabs through the 14 `preview_*` tools: the desktop clone is a `previewAutomation` host served by
injected-script automation. Tabs an agent opens appear through the server's preview events. Links in chat, pull
requests, checks and the terminal open in a Browser tab when "Open links in" is "T3 Code". The tab menu's Mute / Unmute
item and the audible indicator work where WebKit offers the mechanism.

Split from [20261005-browser-surface](closed/20261005-browser-surface.md) at its `prepare` (planned split, part 5). It starts
after part 1 merges into `feat(example)/t3-code`.

## Scope and exclusions

From the parent's scope (its numbering):

1. **Mute.** The audible and muted indicator on the tab and the Mute / Unmute item in the tab menu (the `toggle-mute`
   slot `right-panel-tabs.ts` keeps; `RightPanelTabs.tsx:214-240` `tabMuteMenuItem`). WebKit: `WKWebView` media
   playback state and muting (private `_setPageMuted:` is not public; confirmed at `prepare`, else a declared
   difference in X1's table).
12. **Agent automation.** `previewAutomation.connect` (a request stream), `.respond`, `.focusHost` (`rpc.ts:419-421`),
    serving `preview_open`, `_navigate`, `_status`, `_snapshot`, `_click`, `_type`, `_press`, `_scroll`, `_evaluate`,
    `_wait_for`, `_resize`, `_set_appearance`, `_recording_start`, `_recording_stop`
    (`apps/server/src/mcp/toolkits/preview/tools.ts:53-234`); the host wait budget (`previewAutomationHostBudget`), open
    readiness, the request consumer, target resolution, the agent cursor overlay; `subscribePreviewEvents` (agent-opened
    and agent-navigated tabs: `browser-state.ts` `applyServerEvent` is ported and tested already) and the preview RPCs
    part 1 does not call (`preview.navigate`, `.resize`, `.refresh`). Part 1 calls `preview.open`, `.close`, `.list`
    and `.reportStatus`. WebKit, in place of CDP: `evaluateJavaScript` and user scripts for evaluate, wait-for and page
    state; console and network capture by injected script (no subresource status); synthetic DOM events or `NSEvent`
    input instead of trusted CDP input; the ARIA snapshot from a vendored Playwright script.
13. **Links.** "Open links in" ("Your default browser" or "T3 Code"; `IntegrationsSettings.tsx:562-565`): chat Markdown
    links, pull request and check links (`useOpenLink.ts`), terminal links (`openTerminalLinkInPreview.ts`) and script
    `previewUrl`s. ⌘ or Ctrl-click always uses the system browser; only http(s) URLs open in the app; a failed in-app
    open falls back to the system browser (`browserLinkTarget.ts:25-34`). Settings rows "Open links in".

Excluded: parts 2, 3 and 4.

## Context and guidance

Parent specification: [spec](../spec.md); engine decisions and declared differences: the parent record and
`EXACT2-GAPS.md` "Browser surface: declared differences (X1 path B)". Reference: T3 Code `1e2ecbd975`
(`apps/web/src/components/preview/previewAutomation*.ts`, `PreviewAutomationHosts.tsx`, `AgentBrowserCursor.tsx`,
`apps/desktop/src/preview/{Manager,PlaywrightInjectedRuntime}.ts`, `apps/server/src/mcp/toolkits/preview/*`,
`apps/web/src/browser/{browserLinkTarget,useOpenLink}.ts`). The preview RPCs and the automation stream go into the
Swift transport behind one seam, which #126's runner-owned streams replace (issues README, "Overlaps to watch").

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-browser-surface](closed/20261005-browser-surface.md) (part 1) | [#337](https://github.com/ccheever/exact2/pull/337) | Merged into `feat(example)/t3-code` | merged as `dce6d78df` (2026-10-09) |
| framework issue | [X21](../../issues/closed/20261005-x21-two-way-websocket.md) | #126 | nonblocking: `previewAutomation.connect` and `subscribePreviewEvents` are streams on the Swift transport until #126 | — |
| scheduling preference | `20261005-terminal-integrations` (link routing hook), `20261005-right-panel-tab-menu` (Mute slot) | none | Not prerequisites | — |

## Acceptance and reproduction

Rows from the parent's table: "Agent tools" (with the Browser disabled, the no-host error text equals
`previewAutomation.ts:725-727`), "Links", "Mute", and the "Open and tabs" relaunch case for agent-opened tabs. Tests to
port (`bun:test`, original names; counts from the parent): `browserLinkTarget` (6),
`components/preview/openTerminalLinkInPreview` (7), `openPreviewSession`'s link entry point,
`previewAutomationHostBudget` (10), `previewAutomationOpenReadiness` (13), `previewAutomationRequestConsumer` (11),
`previewAutomationTarget` (4), `agentBrowserCursorLogic` (3), `packages/contracts` `preview.test.ts` (23),
`RightPanelTabs.test.tsx`'s three `tabMuteMenuItem` tests (`:253-283`). The server-side MCP tests stay with the server.
Standard gates as the parent's.

## Progress

2026-10-09: written at part 1's `prepare` (planned split). Planned; starts after part 1 merges.

2026-10-09 (`prepare` and implementation, draft PR [#346](https://github.com/ccheever/exact2/pull/346)):

- **Stream seam.** The previewAutomation streams ride the Swift transport's `subscribe` op: `preview-automation` and
  `preview-events` keys, drained by `client.ts` and by the fleet drain for background environments. #126 replaces this
  later, as the issues README plans.
- **Vendored Playwright.** The injected script is playwright-core 1.60.0's `source3`, Apache-2.0, recorded in
  `VENDOR.json`.
- **WebKit mute.** There is no public page mute (`_setPageMuted:` is SPI), so the clone mutes media elements; declared.
- **Built.**
  - The host (`browser-automation*.ts`, `T3BrowserAutomation*.swift`): all 14 tools, per environment as
    `PreviewAutomationHosts`, the budget carried in the plan because the data module reads no clock.
  - Links (`browser-links.ts`): chat, PR and check, terminal, script, the setting row.
  - Mute and the audible indicator.
  - EXACT2-GAPS "Part 5" declared differences.
- **Checks** after merging `d564a5c02`: the five checks pass (cargo test 3,521 passed); Bun 7,184 pass; AppKit
  `browser-automation` 17/17.
- **Live.** The two agent-mode sessions failed before the tool run (lane setup, then Enter not sending). The app's host
  registration was seen in the server trace. One more session is needed (Next action).

2026-10-09, session 3 (the coordinator's go-ahead): one agent-mode pass. Base runs came from evidence-base at
`d564a5c02`, under its `.build-lock`: base-1 and base-2, the second after base-1 stopped at a menu-dismiss op. The branch
run was branch-1, at `d6b8b05ec`.

- **End to end, passed.** The lane agent called 22 `preview_*` tools through the lane server's MCP endpoint, and the
  app's host answered each one.
  - All but the three expected errors returned ok: freeform resize (part 2; wired since by part 2, #348) and the two
    recording calls (part 3).
  - In the server trace: `previewAutomation.respond` 34; broker invoke 31 ok + 3 failed; `preview.open` 2.
- **"Open links in", Mute, chat link and ⌘-click, passed in agent mode.** Before/after images were taken.
- **No-host text captured.** The base build has no host, so the server answered the agent with the
  `previewAutomation.ts:725-727` text.
- **Settings request loop: pre-existing.** On the base, getConfig, getSettings and device.list each ran at 26.4/s on
  Settings › Integrations; on the branch, 13.0/s each. It is in STATUS "Found, not in scope".
- **Found and fixed.** A muted tab lost its indicator once another tab was shown: WebKit pauses muted media in a page
  that is out of the window. Fixed in `b6b3e417e` with an AppKit case that failed before the fix; the pause is declared.
  Not re-driven live, because of the session limit.

2026-10-09, part 2's viewport (browser-surface-navigation, [#348](https://github.com/ccheever/exact2/pull/348), after
#346 merged as `5f0ae7dca`):

- **Resize.** The plan resolves a preset to its size and orientation (`resolvePreviewViewport`; an unknown preset or a
  freeform size without both sides is the execution error), and `T3BrowserViewport` renders every fixed size: a page
  the panel shows is sized by part 2's stage, a page no stage holds is sized by the host at its zoom.
- **The agent default.** A tab `preview_open` creates on Fill is resized to 1280×800 before the data module adopts it.
- **Rollback.** A size not rendered in time goes back to the tab's previous one (declared: without the latest-setting
  and epoch checks).
- **Appearance.** `preview_set_appearance` uses part 2's `setColorScheme`, and the host's status reads the page's own
  appearance, so the More menu and the agent agree.
- **Tests.** Bun `resize resolves a preset to its size…`; AppKit `browser-automation` 20/20 (three new: a fixed viewport
  where no stage holds the page, the rollback, the new tab's default).

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| Unit and AppKit | `45b857d3c`, then `ac610d7f2` (with `d564a5c02` merged) | Bun 7,184 pass / 0 fail (part-5 files 120); AppKit `browser-automation` 17/17 (evaluate, snapshot, native and DOM click, type, press, scroll, wait_for, status, navigate readiness, open create/reuse, host errors, colour scheme, mute/audible); part 1's `browser` 23 with one title race in the first run, 3/3 reruns clean | [AppKit](https://raw.githubusercontent.com/ccheever/exact2/1d166de14fac5ad79b791d748d118104f479b38e/browser-surface-automation/appkit-browser-automation.txt), [Bun names](https://raw.githubusercontent.com/ccheever/exact2/1d166de14fac5ad79b791d748d118104f479b38e/browser-surface-automation/bun-part5-tests.txt), [snapshot PNG](https://raw.githubusercontent.com/ccheever/exact2/1d166de14fac5ad79b791d748d118104f479b38e/browser-surface-automation/appkit-automation-snapshot.png) | — |
| Five checks | `ac610d7f2` | build, test (3,521 passed, 0 failed, 34 ignored), clippy, fmt, caps, boot: all exit 0; `cargo test -p t3-code-macos --lib` 13; contract build OK (5,736 slots); strict tsc clean | [checks](https://raw.githubusercontent.com/ccheever/exact2/1d166de14fac5ad79b791d748d118104f479b38e/browser-surface-automation/checks.txt) | — |
| Live session 1 (agent mode) | `7fa518900` | Failed at lane setup: no default model selection and no ACP Registry cache ("No valid cached ACP Registry index"); screenshots without `window` were blank; stopped at op 28/42 (no Browser tab to open the menu on) | [record](https://raw.githubusercontent.com/ccheever/exact2/1d166de14fac5ad79b791d748d118104f479b38e/browser-surface-automation/lane-sessions.txt) | lane fixed: registry cache and default model seeded |
| Live session 2 (agent mode, the retry) | same build | `type composer … key Enter` did not send the prompt; stopped at op 14/43. The server trace shows the app's host: `subscribePreviewEvents`, `previewAutomation.connect`, `PreviewAutomationBroker.connect`/`acquireConnection` at +1.49 s, `focusHost` at +1.55 s, `preview.list`, disconnect at quit | [record](https://raw.githubusercontent.com/ccheever/exact2/1d166de14fac5ad79b791d748d118104f479b38e/browser-surface-automation/lane-sessions.txt), [trace](https://raw.githubusercontent.com/ccheever/exact2/1d166de14fac5ad79b791d748d118104f479b38e/browser-surface-automation/lane-session-2-server-trace.txt), [screenshot](https://raw.githubusercontent.com/ccheever/exact2/1d166de14fac5ad79b791d748d118104f479b38e/browser-surface-automation/session-2-unsent.png) | one more session (coordinator go-ahead); the script now taps `send-message` |
| Live session 3, base-1 (agent mode, evidence-base `d564a5c02`) | base build | The no-host answer to `preview_status` (the `previewAutomation.ts:725-727` text); a hand-opened tab's menu without Mute. Stopped at op 23: the `chat-header` tap did not close the tab menu, so `settings-integrations` was not there | [record](https://raw.githubusercontent.com/ccheever/exact2/056511fac8d7d0b99ec4df0d5906f358cc39be2e/browser-surface-automation/live-session-3.txt), [no-host image](https://raw.githubusercontent.com/ccheever/exact2/056511fac8d7d0b99ec4df0d5906f358cc39be2e/browser-surface-automation/07-no-host-after.png), [tab menu pair](https://raw.githubusercontent.com/ccheever/exact2/056511fac8d7d0b99ec4df0d5906f358cc39be2e/browser-surface-automation/03-tab-menu-before-after.png) | rerun as base-2, closing the menu with its Close item |
| Live session 3, base-2 | base build | Settings › Integrations: the inert "Open links in" row; the request loop at 26.4/s per method. Stopped at op 36: a multi-word chat link has no single text to tap (the base's link click was not taken; its links always go to the system browser) | [record](https://raw.githubusercontent.com/ccheever/exact2/056511fac8d7d0b99ec4df0d5906f358cc39be2e/browser-surface-automation/live-session-3.txt), [loop](https://raw.githubusercontent.com/ccheever/exact2/056511fac8d7d0b99ec4df0d5906f358cc39be2e/browser-surface-automation/settings-loop.txt), [pair](https://raw.githubusercontent.com/ccheever/exact2/056511fac8d7d0b99ec4df0d5906f358cc39be2e/browser-surface-automation/01-open-links-in-before-after.png) | — |
| Live session 3, branch-1 | `d6b8b05ec` | All 54 ops. The agent's 22 `preview_*` calls answered by the app's host (3 expected errors); the tab opened in the panel. Mute: speaker → speaker-off. "Open links in" → T3 Code. A chat link opened tab_2 in the app; ⌘-click went to the system browser (one URL in the open log). The loop at 13.0/s per method. Found: the muted indicator was gone once tab_2 was shown | [record](https://raw.githubusercontent.com/ccheever/exact2/056511fac8d7d0b99ec4df0d5906f358cc39be2e/browser-surface-automation/live-session-3.txt), [results](https://raw.githubusercontent.com/ccheever/exact2/056511fac8d7d0b99ec4df0d5906f358cc39be2e/browser-surface-automation/e2e-results-branch-1.txt), [spans](https://raw.githubusercontent.com/ccheever/exact2/056511fac8d7d0b99ec4df0d5906f358cc39be2e/browser-surface-automation/branch-preview-spans.txt), [images](https://raw.githubusercontent.com/ccheever/exact2/056511fac8d7d0b99ec4df0d5906f358cc39be2e/browser-surface-automation/05-agent-e2e-after.png) | the muted-indicator fix |
| Muted indicator behind another tab | `b6b3e417e` | AppKit: WebKit pauses a muted element out of the window (the case failed before: audible false, element paused, page hidden); after the fix the tab stays audible-and-muted, plays again when shown, and an element the page paused stops counting. 17/17 | [AppKit](https://raw.githubusercontent.com/ccheever/exact2/056511fac8d7d0b99ec4df0d5906f358cc39be2e/browser-surface-automation/appkit-browser-automation-v2.txt) | not re-driven live (session limit); real-input batch step 4 |
| Five checks after the fix | `b6b3e417e` | build, test (3,521 passed, 0 failed, 34 ignored), clippy, fmt, caps, boot: all exit 0; `cargo test -p t3-code-macos --lib` 13; Bun 7,184 pass / 0 fail; strict tsc clean; contract build OK (5,736 slots) | #346 body "Checks" | — |

## Real-input batch steps

Run these with the part-5 lane (`target/bsa-lane` in the part-5 worktree: the seeded home, the fixture server on 16751,
"Lane browser agent"). Build with `target/bsa-build.sh`; `target/bsa-lane/drive.sh <run> branch` is the agent-mode
version of the same steps ([copy](https://raw.githubusercontent.com/ccheever/exact2/056511fac8d7d0b99ec4df0d5906f358cc39be2e/browser-surface-automation/lane-drive-v2.sh.txt)).
1. **Focus give-back.** Send `e2e http://127.0.0.1:16751/form`. While the agent clicks and types in the page, click
   the composer and type `abc`. Read back: `abc` is in the composer and not in the page's `#name` field.
2. **Real link clicks.** In Settings › Integrations, set "Open links in" to T3 Code. In the agent's reply, click
   "LanePageB": a Browser tab opens at `/b`. ⌘-click it: the system browser opens it (`T3_REMOTE_OPEN_LOG` has the
   URL), and no new tab opens.
3. **Agent cursor.** During the run, watch the page: the cursor glides to each click target and pings on the click.
4. **Muted behind another tab.** After the run, choose Mute tab on the agent's tab (the tone plays), then open
   LanePageB in the app. The first tab's chip still shows the speaker-off icon. Show the first tab again: the tone's
   time advances again.

## Next action

The rows left are the real-input batch (steps 1-4; STATUS lists them) and the parts that wait on part 3 (recording, the
floating preview). Part 2 (browser-surface-navigation, #348) wired freeform and preset resize and the 1280×800 agent
default into this host after #346 merged (Progress, "part 2's viewport"). The settings request loop is
pre-existing and recorded in STATUS "Found, not in scope" for the coordinator to schedule. The PR stays a draft until
the coordinator flips it.

## Delivery

Merged on 2026-10-10 as `5f0ae7dca` (#346, squash). Rows that need real input are in `examples/t3-code/STATUS.md` "Next real-input batch".
