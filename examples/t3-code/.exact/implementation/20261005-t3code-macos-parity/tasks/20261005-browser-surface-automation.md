---
name: 20261005-browser-surface-automation
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
| framework issue | [X21](../issues/20261005-x21-two-way-websocket.md) | #126 | nonblocking: `previewAutomation.connect` and `subscribePreviewEvents` are streams on the Swift transport until #126 | — |
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

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |

## Next action

After part 1 merges: `prepare` (the stream seam on the Swift transport, the vendored Playwright script, WebKit muting),
then implement.
