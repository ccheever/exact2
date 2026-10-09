---
name: 20261009-timeline-work-rows
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-timeline-work-rows
pr_url: null
verified_commit: null
---

# Timeline: fork rule, notification sources, work group icon and height, inspector details, subagent card, muted icons

## Outcome

The thread timeline's work rows and message actions match the reference:
- "Fork from this response" shows only where the reference shows it.
- Background notification rows decode their source and use the right icon and card.
- A collapsed work group uses the reference's icon choice.
- An expanded work group grows with its open rows.
- File search, web search and file change rows show the reference inspector.
- The subagent card shows the provider icon and the reference's elapsed format.
- Tool image icons use the muted tone.

Found by the 2026-10-09 desktop audit ([review](../reviews/20261009-desktop-audit.md)). Reference: T3 Code `1e2ecbd975`
as an Electron production build. Clone: `c603c22d6`, a development build.

## Findings

Evidence paths are under the repository root. They stay local and are not committed.

Fixture: TH-2, TH-5 and TH-6 need the "Timeline extras" thread. The auditor seeded it into both lane homes with
`target/t3-audit/lanes/thread/tools/extras.py` (local; backups `statev2.sqlite.pre-extras`). Reuse it or rebuild the
same projection rows in the task's lane.

| Id | Reference | Clone | Steps | Evidence |
| --- | --- | --- | --- | --- |
| TH-1 | `AssistantForkButton` returns null when the item has no `runId` or `canForkProjectedAssistantItem` is false (it also checks provider capabilities). The fixture message (`runId` null) shows only Copy and the time. | `timeline-presentation.ts:373` sets `canFork` from kind, completed, streaming and local only. The Fork button shows; pressing it shows the error banner "Only a completed response can be forked." (`client-ops-threads.ts:44-48`). | Open "Timeline verification". Hover the assistant message's meta row. Clone: tap `fork-["verify-timeline","fixture-markdown"]`. | `target/t3-audit/evidence/thread/TH-1-ref.png`, `TH-1-clone.png` |
| TH-2 | `OrchestrationV2NotificationSource` decodes `{kind:'background_command'}` to `command` and `{kind:'background_task', work:'subagent'}` to `subagent`. A command notification shows the terminal icon. A subagent notification is drawn as that subagent's card. | `timeline-worklog.ts:418-421` and `timeline-inspect.ts:37` compare `source.kind` with `command`/`subagent` on the raw wire JSON; nothing maps `background_command`/`background_task`. "Background build finished" shows the zap icon. A subagent notification would miss the subagent branch (read from source, not driven). | Seed extras. Open "Timeline extras". Look at "Background build finished". | `target/t3-audit/evidence/thread/TH-2-ref.png`, `TH-2-clone.png` |
| TH-3 | The group row uses the primary tool source icon, else the last entry's `toolIcon`, and `toolSurface` before the summary glyph. The fixture group shows the themed logo square. | `timeline-rows.ts:238-240` uses `summaryIcon(summaryKind)` or `entryIcon(single)` only. The fixture group shows the hammer. | Open "Timeline verification". Compare the icon left of "Ran 2 commands, read 1 file, and performed 8 other actions". | `target/t3-audit/evidence/thread/TH-3-ref.png`, `TH-3-clone.png` |
| TH-4 | The list's max-height is `calc(min(18rem, 50dvh) + expandedContentHeight)`: each open row adds its detail height. After opening `printf "verified output"`, all 11 rows stay visible. | `timeline.contract:430` uses a fixed `max-height="18rem"`. After one open row, "Icon failed-icon" is cut and the last two rows need scrolling inside the region. With several open rows about four rows show. | Open "Timeline verification". Expand the group, then expand `printf "verified output"` and more rows. | `target/t3-audit/evidence/thread/TH-4-ref.png`, `TH-4-clone.png`, `TH-4-clone-2.png` |
| TH-5 | `V2ItemInspector`: file_search shows "parse", then a list item "project/fixture.txt:1" with the preview below. web_search shows the pattern, then a link "Tables" with an external-link icon (`https://example.test/tables`) and the snippet. file_change shows "project/fixture.txt" with green "+1" and red "-0". | file_search: a mono block "fixture.txt:1 / Timeline verification fixture" (no `project/`). web_search: a mono block with the URL as plain text, not a link. file_change: "project/fixture.txt" only, no stats. | Seed extras. Open "Timeline extras". Expand "Searched code 1 time, changed 1 file, and performed 1 other action". Open "Searched parse", "markdown table syntax" and "project/fixture.txt". | `target/t3-audit/evidence/thread/TH-5-ref.png`, `TH-5-clone.png` |
| TH-6 | `SubagentAvatar` uses `ProviderInstanceIcon` for the driver (the OpenAI mark for codex) with the status dot. Elapsed uses `formatElapsedSeconds` ("0s", "12s", "1m 05s", "1h 02m") and ticks each second while live. With no child thread the row is a plain div, not a button. | `timeline-events.ts:23-34` uses icon `bot` and `formatDuration` ("1ms", "2.5s", "1m 5s"), and leaves elapsed empty while the subagent runs. The row is a disabled button "Open Subagent". | Seed extras. Open "Timeline extras". Look at "Subagent / Found two flaky tests.". | `target/t3-audit/evidence/thread/TH-6-ref.png`, `TH-6-clone.png` |
| TH-10 | `ToolActivityIconView` with `muted` applies `opacity-70 light:brightness-60` to image icons, so the fixture's #1478e6 favicon reads as slate blue. | The website, themed and failed-icon squares show full #1478e6. | Open "Timeline verification". Expand the group. Compare "Icon website", "Icon themed", "Icon failed-icon". | `target/t3-audit/evidence/thread/TH-10-ref.png`, `TH-10-clone.png` |

## Scope and exclusions

Included: the seven findings above.

Excluded:
- TH-7 (the Mermaid preview's black box): one root cause with PG-1 and PA-11, in [app-color-scheme](20261009-app-color-scheme.md).
- TH-8 (Custom snooze date): [shell-sidebar-palette-keys](20261009-shell-sidebar-palette-keys.md).
- TH-9 (file links): [markdown-links-and-files-preview](20261009-markdown-links-and-files-preview.md).
- Rows the audit did not compare (pending approval and question panels, attachments, the checkpoint card, the error and
  woke banners): the server did not surface the seeded rows.
- Framework code. Layout-measuring hooks are a permanent declared difference ([#127](https://github.com/ccheever/exact2/issues/127), X22).

## Context and guidance

Reference (`target/t3-ref/src-1e2ecbd975`):
- TH-1: `apps/web/src/components/chat/MessagesTimeline.tsx:2554-2559`; `packages/client-runtime/src/state/threadWorkflows.ts:165-185`.
- TH-2: `packages/contracts/src/orchestrationV2.ts:1006-1036`; `apps/web/src/components/chat/V2LifecycleRow.tsx:351`.
- TH-3: `apps/web/src/components/chat/MessagesTimeline.logic.ts:1541-1583` (`WorkGroupToggleTimelineRow`).
- TH-4: `apps/web/src/components/chat/MessagesTimeline.tsx:3317-3364`.
- TH-5: `apps/web/src/components/chat/V2ItemInspector.tsx` (file_search, web_search, file_change branches).
- TH-6: `SubagentAvatar`, `ProviderInstanceIcon`, `apps/web/src/components/chat/AgentElapsed.tsx` (`formatElapsedSeconds`).
- TH-10: `apps/web/src/components/chat/MessagesTimeline.tsx:4594, 4654`.

Clone (`examples/t3-code`): `timeline-presentation.ts:373`, `client-ops-threads.ts:44-48`, `timeline-worklog.ts:418-421`,
`timeline-inspect.ts:37`, `timeline-rows.ts:238-240`, `timeline.contract:430`, `timeline-events.ts:23-34`.

Notes:
- TH-2: decode the wire shapes once, where the source is read, so the icon and the subagent card both use the decoded
  kind. Port the reference's decoder and its tests by name.
- TH-4: Contract has action-only reads `frame(id)` and `measure("id")` (`docs/contract-grammar.md:543-545`). The clone
  already uses `frame()` (`app-window.contract:488`). Grow the cap by the open rows' detail heights. If that cannot be
  done without a measuring hook, add a declared row to `EXACT2-GAPS.md` and say so in the PR.
- TH-6: the elapsed time ticks on the timeline's clock (`timelineNow`). Data modules have no timers (#124).
- TH-10: opacity 0.7. Use `brightness(0.6)` in light mode only if `contract vocab filter` admits it; otherwise declare it.

## Acceptance

Before/after evidence: one side-by-side image per scenario (base build | branch build, same state,
`screenshot <abs.png> window`).

| Id | How to verify | Before/after pair | Input |
| --- | --- | --- | --- |
| TH-1 | A message with no `runId` shows Copy and the time only. A message with a `runId` and a capable provider still shows Fork. Unit test on `canFork`. | `th1-no-fork.png` | agent |
| TH-2 | "Background build finished" shows the terminal icon. A `background_task`/`subagent` notification draws the subagent card. Unit test on the decoder with the reference's shapes. | `th2-notification-icon.png` | agent |
| TH-3 | The fixture group shows the themed logo square. Unit test on the icon choice order. | `th3-group-icon.png` | agent |
| TH-4 | With `printf "verified output"` open, all 11 rows show; with three rows open, every row stays reachable as in the reference. | `th4-one-open.png`, `th4-three-open.png` | agent |
| TH-5 | file_search shows the "project/fixture.txt:1" item and preview. web_search shows a "Tables" link with the external icon. file_change shows "+1" "-0". | `th5-inspector.png` | agent |
| TH-6 | The card shows the OpenAI mark and "0s"; a live subagent ticks each second; with no child thread the row is not a button. Unit test on `formatElapsedSeconds`. | `th6-subagent-card.png` | agent |
| TH-10 | The three image icons are muted as in the reference. | `th10-muted-icons.png` | agent |

## Acceptance results

Agent mode on the branch bundle (lane `timeline-work-rows`, base port 16820, 1280x840), the same steps as the before
build (the feature tip, `t3-code-evidence-base`) and the reference (Electron production build, CDP). Each image is
before | after | reference.

| Id | Result | Evidence |
| --- | --- | --- |
| TH-1 | pass (agent mode): the fixture answer (`runId` null) shows Copy and the time only. A completed answer of a run keeps Fork; a provider session without fork capabilities hides it; an inherited row keeps the fallback (unit tests). | [th1-no-fork.png](https://raw.githubusercontent.com/ccheever/exact2/acc8233a40a44852af64ba657203944d43052bcb/timeline-work-rows/th1-no-fork.png) |
| TH-2 | pass (agent mode): "Background build finished" (`background_command`) shows the terminal icon; "Subagent finished" (`background_task`, `work: subagent`) is drawn as the Review Docs card (Finished, 6:07 PM, opens its thread). Decoder tests ported by name. | [th2-notification-icon.png](https://raw.githubusercontent.com/ccheever/exact2/9529958296835a9e67fa21557f4e179b0b89fbf7/timeline-work-rows/th2-notification-icon.png) |
| TH-3 | pass (agent mode): the fixture group shows the themed logo square. Unit test on the order (primary source's icon, last entry's icon, surface, summary glyph). | [th3-group-icon.png](https://raw.githubusercontent.com/ccheever/exact2/2549635dcdf78c23bf195ae1e75582b317094ecb/timeline-work-rows/th3-group-icon.png) |
| TH-4 | pass (agent mode): with `printf "verified output"` open all 11 rows show; with three rows open (printf, exit 2, Independent dynamic output) the list grows by their details and every row shows without scrolling inside the region, as in the reference. | [th4-one-open.png](https://raw.githubusercontent.com/ccheever/exact2/2c77c66415ca9e78cdbe7da066b52747225bf39d/timeline-work-rows/th4-one-open.png), [th4-three-open.png](https://raw.githubusercontent.com/ccheever/exact2/81334916ef41a63af7edd9f43844dc70cfe0b6a6/timeline-work-rows/th4-three-open.png) |
| TH-5 | pass (agent mode): file_search "project/fixture.txt:1" over its preview; web_search "Tables" as a link with the external-link mark over "Pipes and dashes."; file_change "project/fixture.txt" with green "+1" and red "-0". | [th5-inspector.png](https://raw.githubusercontent.com/ccheever/exact2/bff96c9ef0dea2ef713fee83c2107747811018aa/timeline-work-rows/th5-inspector.png) |
| TH-6 | pass (agent mode): the OpenAI mark with the status dot and "0s"; Watch Build (no child thread) is a plain row, not a button. A live agent ticks from its start on the timeline clock (unit tests; the server marks the seeded running subagent stopped when it starts, so the fixture has no live agent; its elapsed is the time since the seed, 1h 14m after vs 37s in the reference lane). | [th6-subagent-card.png](https://raw.githubusercontent.com/ccheever/exact2/6aa3f015dd4d768ed55fbe7936ff5a4f080f0987/timeline-work-rows/th6-subagent-card.png) |
| TH-10 | pass (agent mode): Icon website, Icon themed and Icon failed-icon (and the group's icon) are muted: opacity 0.7 in Contract, brightness 0.6 in the light appearance in the image hook (`T3ToolActivityIcon.swift`). | [th10-muted-icons.png](https://raw.githubusercontent.com/ccheever/exact2/438b3ee93ae5a751fa8f9ee6a757ed00f4329c9b/timeline-work-rows/th10-muted-icons.png) |

Tests: `timeline-work-rows.test.ts` (new): the reference's "decodes notification sources stored before specific kinds
existed", "decodes a notification source kind from a newer server as generic background work" and "allows native,
portable, and capability-unknown exact-run forks", plus the rows above (`canFork`, the group icon order, the inspector
parts and Open diff, `formatElapsedSeconds`, `deriveSubagentElapsedMs`, the subagent card). `chat.test.ts`: its answer
rows carry `canFork`. Checks: see the PR.

No new declared difference: TH-4 hears the list's size from the element resize event (`resize=`), no measuring hook;
TH-10's brightness is drawn by the app's own image hook, which draws the image.

## Progress

2026-10-09: built (one commit) and merged `feat(example)/t3-code` (`6e2040c58`); the first agent stopped at a usage
limit before the live drive. 2026-10-10: the bundle rebuilt, one live drive plus one retry (the three-open state at the
top of the timeline), images composed and uploaded, the draft PR opened.

## Real-input batch steps

None: every row is agent mode. (Pressing a web result opens the system browser, as the reference's `target=_blank`
link does; not pressed in the lane.)

## Next action

None: review and merge.
