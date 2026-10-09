---
name: 20261005-pr-links-previews-and-routing
plan: 20261005-t3code-macos-parity
implementation: done
verification: partial
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-pr-links-previews-and-routing
pr_url: https://github.com/ccheever/exact2/pull/311
verified_commit: null
---

# Pull request links, previews, cross-environment routing and list paging

## Outcome

Pull requests connect to threads and across servers as in the reference. The panel shows how
many threads link to it and can link or unlink it from a thread (also from an unsent draft);
`#N` and commit hashes in pull request text are links with a hover card; the Pull Requests page
reads every connected server, routes reads and writes through the server the person shares
GitHub access with, offers "Act on" for hand-offs, and loads further pages ("Load more pull
requests") from per-repository cursors.

## Scope and exclusions

Included: **C9** (linked-thread count, link/unlink menu item, thread picker, back arrow to the
thread's pull requests), the `c8d7d50a73` rule (a draft with no thread yet gets the picker, not
"Link to this thread"), **C10** (autolinks, hover card, click resolution), **C12** (routing,
"Act on", merged listings from several servers), and list paging with cursors.
Excluded: the row context menu, checks and stack popovers (`20261005-pr-handoffs-and-quick-actions`); Settings → Connections → GitHub
sharing UI (done: `connections.ts:97-105,451-453`, `connections.contract:156`); other hosts.
Reuse (done): `palette-linkpr.ts` (`parseChangeRequestUrl`, `findProjectForChangeRequest`,
`findProjectOnChangeRequestHost`, `linkMode`, `threadPullRequestKey`, `linkPullRequest` dispatching `thread.pull-request.link`),
`prCommandPayload` for unlink (`r4-surfaces-prs.ts:137`), background environments (`settings-b-fleet.ts`, `modules/apple/T3Fleet.swift`; a fleet request is `native.later({...request, fleet:<origin\nid>})`, `EnvironmentFleet.native`, `settings-b-fleet.ts:54-56`), list model (`pages-prs.ts`), `pageslocal` route.
Corrections: the clone reads one environment with `limit: 99` and no cursors (`pages-prs.ts:246,254`); the GitHub-sharing preference is stored but nothing reads it.

## Context and guidance

Parent specification: [spec](../../spec.md). Source behavior (T3 Code `1e2ecbd975`; `W/` = `apps/web/src/components/pullRequest/`):
`W/PullRequestThreadLinks.tsx:67-243`, `apps/web/src/hooks/usePullRequestLinking.ts`, `apps/web/src/components/CommandPalette.logic.ts:24-40` (`buildLinkedThreadActionItems`), `commandPaletteBus.ts`;
`W/PullRequestLinkPreview.tsx:32-151`, `W/pullRequestMarkdown.logic.ts:150-192` (`remarkPullRequestAutolinks`), `apps/web/src/components/ChatMarkdown.tsx:3043-3260`, `apps/web/src/lib/openPullRequestLink.ts`;
`packages/client-runtime/src/state/pullRequestRouting.ts:119-330`, `connection/githubRoutingPermissions.ts`, `W/pullRequestProjectAssignment.logic.ts`, `W/PullRequestDetailPanel.tsx:271-309,1550`;
`apps/web/src/routes/_chat.pull-requests.tsx:262-264,640-760,1087,1162-1225,1798-1870`, `W/pullRequestList.logic.ts:703-760,886-974`; contracts `pullRequest.ts:540-660,698-725`; server `PullRequestService.ts` (`routing`, `routingIdentity`, `linkedThreads`).
Clone paths `examples/t3-code/<file>`.
Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23).
Library revision: `20261005-platforms-v3`. Selected topics: state-and-data (late old replies; per-environment resources; failure retains previous value), components (list item state keyed by durable ids), layout-and-interaction (virtualized list; popover), design (loading/empty/error/disabled), accessibility, motion (card fade if the reference has one; reduced motion), testing-and-debugging. Unknown in the library: multi-environment transports, hover cards on inline text; the clone's runtime evidence on the pinned main is the basis.
Reference rules to keep: writes route only when both servers are "read-write", reads when neither is "off"; a routed call carries `host` and `expectedAccountId`; an SSH environment is trusted only while its saved profile still matches its stored connection key; the first reachable candidate wins (local servers first for reads; the origin first for writes when it is local); after any routed write the touched servers' caches are invalidated.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-clone-on-exact2-main](../20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| merged task PR | [20261005-environment-routes](20261005-environment-routes.md) | pending | Merged (saved-environment key; `gitHubRoutingConnectionKey` over several routes) | pending |
| merged task PR | [20261005-pr-conversation-and-refresh](20261005-pr-conversation-and-refresh.md) | pending | Merged | pending |
| merged task PR | [20261005-pr-handoffs-and-quick-actions](20261005-pr-handoffs-and-quick-actions.md) | pending | Merged (row menus, Check out menu and the hand-offs that "Act on" redirects; it already follows `20261005-pr-header-actions-and-stacks`, whose More menu also carries the radio) | pending |
| merged task PR | [20261007-real-github-lane](20261007-real-github-lane.md) | pending | Sandbox's bulk pull requests page the list (probe `R3`/`R4`); a second lane server per account | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| merged task PR | [20261008-fix-hover-cards](20261008-fix-hover-cards.md) (window hover layer, `hover-layer.contract`) | [#307](https://github.com/ccheever/exact2/pull/307) | Merged into `feat(example)/t3-code`; the card is drawn by its layer as kind `"pr-preview"` (coordinator 2026-10-08: no second hover helper) | merged (`3c8c11ef2`), merged here in `b06586859`; the three edits applied: PrLinkRun `inject hoverTipAt`, `hoverDelay("pr-preview") = 120`, `hoverOpenDelay("pr-preview") = 350`, the T3Window layer branch |
| framework issue | X17 popover side areas and flips | [#112](https://github.com/ccheever/exact2/issues/112) | The side the card opens on (PreviewCardPopup side top, align center, sideOffset 6) | the hover layer's own `hoverFlip` (#307) places it; this task adds no per-site flip (#112 is the core work) |
| framework issue | X34 frame of an inline run | [#272](https://github.com/ccheever/exact2/issues/272) | The card's anchor: an inline link inside rendered text | blocked rows: none; anchoring limited by #272: the card anchors at the link chip's own box (`frame(pr-link-<id>)` through `hoverTipAtFrame`); no inline-run frame or rectangle computation here (#272 is approved main-side work) |
| merged task PR | [20261008-fix-keyboard-focus](20261008-fix-keyboard-focus.md) | [#310](https://github.com/ccheever/exact2/pull/310) | The More and Check out menus' keyboard (`KeyMenu`) | merged (`f45eab04a`), merged here in `00af6adb3`: the Link item and each "Act on" row have a `KmItem` entry in menu order; an Act on row lights while focused |
| merged task PR | [20261005-pr-code-tab](20261005-pr-code-tab.md) | [#308](https://github.com/ccheever/exact2/pull/308) | Not needed: a commit autolink opens on the host, as the reference's (it is not a change request) | merged (`0e2901aec`), merged here in `b06586859`; its nine `Tip`s inside the diff list, file header, off-diff list and thread cards moved onto the hover layer (`LayerTip`, kind "tip"); a later Code-tab route for a commit link goes in `pages-pr-links.ts openLink` |

## Issue assessment at preparation

Checked sources and time: planning pass 2026-10-05 over `EXACT2-GAPS.md` and `../issues/` drafts (not reproduced, not searched upstream). Re-check at `prepare`.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X13](../../issues/closed/20261005-x13-hover-keys-during-pan.md) | Hover close/cancel during a pan | Documented clone limit | nonblocking (workaround: partial, r12) | Declare in matrix |
| [X17](../../issues/20261005-x17-popover-position-try.md) | Hover card flips near window edges | AGENT-HANDOFF "flipped hover card overhang" | nonblocking (workaround: fixed placement) | Declare |
| [X19](../../issues/20261005-x19-data-source-timers.md) | 350 ms open / 120 ms close hover delays; 10 s linked-thread poll | Delays held in Contract/`now` args today | nonblocking (workaround: `now` arguments, Contract tasks) | Reuse the details-card hover card's delay mechanism |
| [X34](../../issues/20261005-x34-inline-span-frame.md) | Frame and hover of an inline link inside rendered Markdown, to anchor the card | `t3-anchor`/`t3-frame` hooks cover boxes ([X22](../../issues/20261005-x22-reactive-layout-facts.md)), not inline runs | unknown | Spike at `prepare`; if the renderer cannot expose a link's frame, the hover card on inline links is held for a user decision (no matching workaround); autolinks still link 2026-10-07: #133 closed; main #178 makes the macOS agent hover inline runs (enter, leave, a point's hit test), so an inline link's hover can be built and driven; `frame()` of an inline run is still missing, so the card's anchor remains the open question (adopt-main-fixes-input). |
| [X21](../../issues/20261005-x21-two-way-websocket.md) | RPC to background environments | `T3Fleet.swift` transports | nonblocking | Reuse |
| [X9](../../issues/20261005-x09-root-component-across-files.md) | `app.contract` cap | 1,488/1,500 on the base (`44e939f1e`) and after this task (0 net root lines: +2 for `pr-select:`, −2 by folding `prClose` into `prSelect("")`; the rest in-place edits) | nonblocking | New files only; in-place edits of the root's existing lines |

## Implementation notes

- **Port** (names, tests, header changes): `pullRequestProjectAssignment.logic.ts` (`assignProjectsToEnvironments`, `resolvePickableEnvironments`), `pullRequestList.logic.ts` (`mergePullRequestLists` 703, `resolveQueryEnvironmentIds` 924, `resolveSelectedEnvironmentId` 953, `resolveProjectScope` 886, `findScopedProject` 901, `pullRequestEntryViewer` 143, `pullRequestEnvironmentSetKey` 761, snapshot read/write 827/841; only what `pages-prs.ts` lacks),
  `pullRequestRouting.ts` decision parts (`routingAllowed`, `matchesReference`, candidate order, `rejectedBeforeDispatch`, invalidation targets), `githubRoutingPermissions.ts` (`gitHubRoutingConnectionKey`, `routeConnectionKey`), `buildLinkedThreadActionItems`, `remarkPullRequestAutolinks` (`#N`, 40-hex; ignores links and code; word-character guards), `resolvePullRequestPreviewTarget`/`parseChangeRequestUrl` from `openPullRequestLink.ts` (merge with `palette-linkpr.ts`).
- **Linking.** `pullRequests.linkedThreads` (refresh every 10 s through `now`, and on refresh events); header count button `aria-label` "Linked from N threads" / "Linked threads" (shows "?" on error); click opens the palette with the PR URL as query and the linked threads as results ("Linked thread", "Archived thread"). Menu item "Link to thread" / "Link to this thread" / "Unlink from this thread". `linkPullRequest` takes the chosen thread id (today it uses the focused thread). Picker dialog: `aria-label` "Choose a thread", placeholder "Search threads or projects...", "No active threads found.", linked threads disabled with a check. Failure toasts "Could not link the pull request" / "Could not unlink the pull request". "Back to this thread's pull requests" arrow (tooltip "Back to pull requests") in the thread context.
- **Preview.** Card after 350 ms, closes after 120 ms; content repo + `#N` · state, title, author, "opened <relative>"; reads `pullRequests.detail`; on failure without a fallback it shows the URL as a tooltip. Click on a `#N` autolink asks `pullRequests.preview`, then opens the PR in the panel, else the system browser; ⌘/⌃/⇧/⌥-click keeps default behavior.
- **Routing.** A request for another server is `client.rpc(EnvironmentFleet.native(native, key), …)`; reads probe `pullRequests.routing{ref}` (2 s timeout, falls back to the origin); the call then carries `host` and `expectedAccountId`; "Act on" radio group (server label, machine kind) appears in the More and Check out menus only when more than one connected server holds the repository; the chosen server supplies project, workspace root and thread for hand-offs.
- **Paging.** Page size 99, up to 500; `cursors` from `nextCursors` per repository and server; "Load more pull requests"; "Updating pull requests"/"Loading more" with a spinner; when no cursor is left and the cap is reached: "Narrow your search to find more pull requests."; list error with content shown: "<error> Showing the last pull requests loaded." + Retry.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Linked threads | Two threads linked to PR 101 (one archived) | `bun scripts/agent.mjs macos --json "tap pr-row-101" tree`; press the count button | Label "Linked from 2 threads"; palette lists "Linked thread"/"Archived thread"; choosing one navigates; `pullRequests.linkedThreads` re-read every 10 s of `clock` | macOS 1280×840 | tree JSON, trace |
| Link / unlink | Sent thread; unsent draft | Menu item; in the draft the picker | `thread.pull-request.link`/`unlink` in the trace with the chosen thread id; draft opens the picker; failure toast on a refused dispatch | macOS | trace, shots |
| Autolinks | PR body with `#101`, a 40-hex hash, `` `#2` `` in code, `a#3` | Open the PR | Only the first two are links; code and word-attached forms are not | macOS | tree JSON |
| Hover card | PR text link | `(attended session)` hover 350 ms, leave | Card with the listed fields; closes after 120 ms; click opens the panel | macOS, real pointer; lane build with `T3_LOCAL_HOME=<lane>/t3-home`, `T3_LOCAL_PORT=<lane port 16xxx>` | recorded steps, shots |
| Routing | Two lane servers A (focused) and B (background) signed in to the same lane account (shared config dir), each with its own `T3_GITHUB_LANE` gh log; GitHub sharing set A/B to off, read, read-write combinations | Open a PR; comment; Check out with "Act on" = B | Reads and writes land in the log of the allowed server only; with one side "read" a write stays on the origin; identity mismatch is refused before dispatch; Check out creates B's worktree | macOS | two logs, `git worktree list` |
| Paging | Sandbox's 105 bulk open pull requests beside the seeded ones, and the second account's fork | Scroll to the end; press "Load more pull requests" until the cap; fail one page | Request `cursors` equal the previous `nextCursors`; rows append in place; states and texts as listed | macOS | trace, shots |
| Visual and trace | Oracle on the same fixtures | Pairs at 1280×840 and 840×620, light and dark: count button, picker, card, list footer; `target/t3-ui-parity/trace-diff.mjs pr-links`, `pr-routing`, `pr-list` | Every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link; read multisets and write order equal | macOS | pair table, diff |
| Ported tests | `bun test` | Original names: "one server per repository", "where a pull request can be acted on", "merging the environments' own listings", "which environments a listing should ask", "who \"I\" am, per server", "the server a saved selection names", "linked pull request thread navigation", "does not probe another environment with %s routing permission", "keeps hover previews fresh after edits and turns", SSH-profile routing cases | Pass; Effect-runtime-only cases classified in the header | macOS | log |
| Keyboard focus, Escape, reduced motion | `prefer prefers-reduced-motion reduce` | Tab/Return through the count button, menu item, picker, "Load more pull requests"; Escape closes the picker and the hover card | Focus visible and returned to the trigger; nothing is linked on Escape; the card appears without fade when reduced | macOS | `tree --ax`, state |
| Gates | `git add -A` | Clone checks; `bun scripts/caps.mjs`; five repository checks | Green; every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `examples/t3-code/pages-prs.*`, `pages-pr-detail.*`, new `pages-pr-links.*`, `pages-pr-routing.ts`, `palette-linkpr.ts`, `settings-b-fleet.ts` (request helper), `r4-surfaces-prs.ts`, `AGENT-HANDOFF.md`.
Required environment: macOS 26.6.2, Xcode 27.0, Bun 1.4.2, two lane backends, the real-GitHub lane (sandbox, shared lane config dirs, two accounts; `tools/github-lane`), reference oracle. Attended and normal-launch rows use a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>` (see `20261005-embedded-server-runtime`).

## Progress

2026-10-06: on hold (user decision: tasks that need a sign-in waited). 2026-10-07: the user lifted the hold.

2026-10-08: built and driven (draft PR). What is in it:
- **Routing (C12).** Every `pullRequests.*` request goes through `T3Client.rpc` → `pages-pr-environments.ts` → `pages-pr-routing.ts`
  `PullRequestRouter`, the reference's `createPullRequestRouter` ported with its decisions (`routingAllowed`, the strict local-first
  read, the 2 s `pullRequests.routing`/`routingIdentity` probes, candidate order, `rejectedBeforeDispatch`, routed-read memory and the
  invalidation fan-out after a write). The servers are the focused connection and every connected background environment; GitHub sharing
  is read from Settings › Connections under each server's current saved key (`connections.ts githubSharing`), so the stored preference
  is no longer UI only. A request names its server by `environmentId` or, when built from a reference, by the server holding its project.
- **Merged listings and paging.** `pages-pr-paging.ts`: one server per repository (`assignProjectsToEnvironments`), each server's
  `pullRequests.list` folded (`mergePullRequestLists`), page size 99 up to 500, per-server `cursors` from the last `nextCursors`, the
  slice appended under the rows shown, a refresh after a continuation reads every shown row again; the footer's "Load more pull
  requests", "Loading more" / "Updating pull requests" with a spinner, "Narrow your search to find more pull requests." and the
  "… Showing the last pull requests loaded." banner with Retry.
- **Act on.** `pages-pr-acton.ts`: the More and Check out menus' radio group on the page when more than one server holds the repository;
  a hand-off on another server checks out there and moves the window to that server's draft.
- **Links (C9).** The page's linked-thread count (`pullRequests.linkedThreads`, re-read when the 10 s tick moves and on announcements),
  its palette of linked threads (archived ones too), the More menu's Link / Link to this thread / Unlink from this thread item, the
  picker ("Choose a thread"; an unsent draft gets it, c8d7d50a73), refusal toasts, the back arrow beside a thread with several pull
  requests. `linkPullRequest` takes the chosen thread.
- **Autolinks and link clicks (C10).** `#N` and 40-hex commits in the panel's text become links (`remarkPullRequestAutolinks` on the
  source: not in code, links or word-attached), each pull request link a chip; a click with no modifier opens the pull request in the
  panel (a `#N` asks `pullRequests.preview` first; the page's selection by `pr-select:`), otherwise the browser. The hover card's data
  and view are built (`chatlocal:pr-preview`, `prDetail.preview`, `PrLinkPreviewCard`); its placement on #307's layer is below.
- Root: 0 net lines (see X9). Fork-ported logic and tests: `pages-pr-routing.*`, `pages-pr-links-logic.*`.

2026-10-08 (resumed after #307): the base merged in twice (`b06586859`: #308, #312, #323, #326, #307; `00af6adb3`: #310) and
once more for the records (`abc0afaf5`: #328). What changed:
- **Hover card.** `PrLinkRun` injects `hoverTipAt` and sends `hoverTipAtFrame` with the link chip's own box (`frame(pr-link-<id>)`);
  the window's hover layer draws `PrLinkPreviewCard` for kind "pr-preview" (350 ms open, 120 ms close) and places it with its
  `hoverFlip`. A commit link sends kind "tip" with its URL. No inline-run frame, no rectangle computation, no per-site flip
  (#272 and #112 are the framework's).
- **#308's tips.** The Code tab's nine `Tip`s inside its scroll areas and cards became `LayerTip` (hover-layer.contract, kind
  "tip"), so the layer draws them above the clip.
- **#310's keyboard.** The More menu's Link item and the "Act on" rows in More and Check out have `KmItem` entries in menu
  order; an Act on row lights while it has the focus.
- **List fix (base bug the retry found).** `pages-prs.ts` joined an in-flight list read across answers. The runner lets an
  answer go when it asks the resource again (a data revision, a typed search) and rejects that answer's reads, so the
  answer that joined it failed and the list kept its old rows: a second search never landed and "Loading more" stayed. A
  read is now joined only by the answer that started it (`pages-prs-live.test.ts`, a new test that fails on the old join).

### Acceptance results

Evidence: `https://raw.githubusercontent.com/ccheever/exact2/ee9a40ac4f1022bcfa8d21f43d812bd8125ab60a/pr-links-previews-and-routing/<file>` (01–12 from attempt 1, 13–16 and `retry-drive-record.txt` from attempts 2 and 3).

| Criterion | Result | Proof | Blocker |
| --- | --- | --- | --- |
| Linked threads | pass (live + unit) | 02 (count 2), 06 (palette: Linked / Archived thread), "Old work" opened from it (record), 10 s tick: `liveNow` 83000 → 93000 under `clock +10000` (record), `pages-pr-links.test.ts` "on the page the count names the threads and reads again only when the 10 s tick moves" | — |
| Link / unlink | pass (live + unit) | 07 (picker), 08 (count 3 after choosing "Write the changelog"; `pullRequests.linkedThreads` read back with three threads), unit: link, unlink beside a linked thread, the draft's picker, the refusal toast | — |
| Autolinks | pass (live + unit) | 05 (before/after: `#115`, `c46352e` links; `` `#2` ``, `a#3` text), 09 (`#115` clicked → #115 selected), 14 (the commit link is SF Mono: its run is 60.59 pt wide; SF Mono 14 pt measures 60.55, SF Pro 55.69), `pages-pr-links-logic.test.ts` "pull request autolinks" | — |
| Hover card | pass (live, agent pointer) | 13 (light and dark: repository, `#115` · Open, the title, the author, "opened 22h ago", above the link); record: drawn on the first hover after the read; leaving, the card is there at +100 ms and gone at +140 ms; a second hover: nothing at +300 ms, the card at +370 ms; 14 (the commit link's URL tip, kind "tip"); unit tests for the read and the URL tooltip on failure | blocked rows: none; anchoring limited by #272 (the chip's own box). The real-pointer rows (onto the card, ⌘-click) are in STATUS "Next real-input batch" |
| Routing | pass (unit/integration), partly live | `pages-pr-routing.test.ts` (93, ported), `pages-pr-environments.test.ts` (remote origin read through B with `host`/`expectedAccountId`; sharing off either side probes nothing; one side "read" keeps a write on A; identity mismatch refused; a background row's project goes to its server); live: sharing set to Read and act on both (12), both lane servers are loopback so the origin answers first as in the reference; Check out with Act on = B created B's worktree only (11, `git worktree list` in the record) | — |
| Paging | pass (live + unit) | 15: "Load more pull requests" with the search empty: 99 rows, then 128 (A's continuation from `nextCursors`), the footer gone when no server has more; 01 (the base said "Narrow your search"); `pages-pr-paging.test.ts`; the list's join fix and its test | — |
| Back arrow | pass (live) | 16: "Write the changelog" holds #132 and #115; #115 opened from the thread's details shows the back arrow; pressing it shows the thread's pull requests (#132, #115) | — |
| Visual and trace | not run (oracle) | before/after pairs 01–05; dark and 840×620 shots taken (count, More, picker, Check out with Act on), 13 dark | user decision 2026-10-06 (no oracle or trace tools) |
| Ported tests | pass | routing 93 and links 43 (fork-ported, original names; Effect-runtime-only cases classified in the test headers) | — |
| Keyboard focus, Escape, reduced motion | pass (live, agent keys) | Return on the count opened the palette; Escape closed it and the picker (nothing linked: count stayed 2); focus returns to the trigger; the card has no fade under reduced motion (`still`); #310's menu keyboard covers the Link item and the Act on rows (`KmItem` entries; Return, ↓, ↓ on Check out ended on the first Act on row, which is also the selected one, so the focus was not told apart from the selection) | the Act on focus row: STATUS "Next real-input batch", steps in section C |
| Gates | pass (final head) | below Attempts | — |

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (agent live, the one session) | `a3d15d561` + records merge `732f0e3f3` | every after step but two passed on real GitHub with two lane servers; `load-more` ran with the search still applied and found no button; `thread-surface` could not find `pr-row-115` (the search had not settled) | `live-drive-record.txt`, images 01–12 | the retry |
| 2 (agent live, the retry) | `00af6adb3` (after #307, #308, #310) | hover card, its timing, the commit tip and the monospace face passed; Load more stayed "Loading more" (99 → 99) and searches after the first never landed; the back arrow step looked for the composer badge, which a thread without a provider does not show | `retry-drive-record.txt`, 13, 14 | the list's join (fixed in `d4ca27743`) |
| 3 (the failed rows again, after the fix) | `abc0afaf5` | a probe on A alone: `#132`, `#168`, `#117` land in about 1.2 s each; Load more 99 → 128; the back arrow shown and pressed; the Code tab's tips not hovered: `LayerTip` had no test id, so the drive's filter found none (#168's off-diff list, 2 conversations, holds the path and thread author/age tips); `LayerTip` now gives its trigger the tip id as its test id (records commit), and the tips are a batch row | `retry-drive-record.txt`, 15, 16 | — |

Gates on the final head (Attempt 3): `bun test examples/t3-code --timeout 60000` 3462 pass, 0 fail (1 skip); strict tsc clean; contract build OK (5512 slots); `bun scripts/caps.mjs` within caps; `cargo test -p t3-code-macos --lib` 13 passed, 0 failed; the five checks green (`cargo build --all-targets --keep-going` 0; `cargo test --lib --bins --tests --no-fail-fast` 3521 passed, 0 failed, 34 ignored; clippy `-D warnings` 0; `cargo fmt --check` 0; caps 0; `bun scripts/boot.mjs` 0).

## Real-input batch steps

Rows in STATUS "Next real-input batch". Agent mode can run B and C (the triggers and rows have test ids, and `tree` reads
`focused`); a real pointer and real keys confirm them on hardware. Hold the real-input lock
`target/t3-ui-parity/lanes/.realinput-lock` (owner note `pr-links-previews-and-routing: real input`).

Setup for all three: `bun target/pr-links/lanes.mjs start a` (127.0.0.1:16360; the base checkout's shared lane dir, primary
account), and for C also `start b` (127.0.0.1:16361). An agent-mode session of this branch's bundle
(`target/pr-links/drive-retry.mjs` pairs A, and B with `pair-b`, and sets GitHub sharing to Read and act). For a real pointer:
`cliclick` moves on the window's screen place from `orca computer list-windows`, as fix-hover-cards' real-pointer session did;
set the app frontmost through System Events first and clear the agent's pointer with an agent contact. Cleanup: close the
session, `lanes.mjs stop a` (and `b`), release the lock.

**A. The PR-link hover card and clicks** (needs `bun target/pr-links/comment.mjs`, the autolink fixture comment on #132;
`comment.mjs delete` after).
1. Pull Requests › #132 › Summary › Comments. Onto `#115`: the card after ~0.35 s. Onto the card: it stays (waits on #327, as
   #307's rows). Away: gone within ~0.2 s.
2. A plain click on `#115` selects #115. ⌘-click on `https://github.com/daehyeonmun2021/playground/pull/116`: the panel does not
   move and the default browser opens the URL (read back, then that tab closed). A click on `c46352e` opens its commit page.

**B. The Code tab's tips** (`LayerTip`: the trigger's test id is the tip id; the bubble's is `<tip id>-bubble`). For each
one: hover the trigger (`tap <tip id> hover`, or the real pointer onto it), then read `hover-layer` and the bubble's text; the
bubble is drawn above the scroll area's clip.
1. #168 › Code. Open "Conversations not on the diff loaded so far" (2): `pull-request-code-orphan-tip-<path>` (a group's
   path) and, in a thread card, `pull-request-thread-author-tip-<comment id>` and `pull-request-thread-age-tip-<comment id>`
   (the same tips are on the inline threads in the diff).
2. #168 › Code › the scope menu › one commit: `pull-request-code-scope-note-tip` ("A comment is anchored to the
   whole change…").
3. #115 › Code: tick a file's Viewed box; push a commit that changes it with the second account's lane gh; Refresh: the header
   reads "Changed", and `pull-request-code-changed-tip-<path>` sits on it.
4. #168 › Code, then stop lane A and press Refresh: the viewed read should fail, keep the boxes and show the warning with
   `pull-request-code-viewed-error-tip` on it (start the lane again after). If the panel shows the connection error
   instead, this tip needs a fixture that fails only the viewed read.
5. `pull-request-code-viewed-short-tip` (the host reports ticks for fewer files than the change has) and
   `pull-request-code-withheld-tip` (a slice the host truncated, or a patch it did not inline): check #168 after "Load more
   files" ×3 (310 files and a binary `assets/logo.png`) first; if neither shows, they need a fixture the sandbox lacks.
6. `pull-request-code-viewed-here-tip` needs a host that keeps no viewed record (`viewedFiles: "environment"`, not GitHub); no
   lane has one.

**C. Act on by keyboard, the focus and the selection told apart** (A and B paired; A, the focused server, is the selected
Act on row by default).
1. #132 › Return on Check out: the focus goes to "In a separate worktree". ↓ "In this repository", ↓ server A's row (the
   selected one), ↓ server B's row: B has the focus (`tree pr-act-on-<B id>` reads `focused: true`; B shows the accent, A
   keeps the selected tint).
2. Space on B: B becomes the selected row (`aria-checked` moves to B). Escape: the menu closes and the focus is back on
   Check out.
3. The same from More: Return on More, ↓ past the Link item, Refresh and the three hand-offs to the Act on rows.

## Next action

2026-10-09 (records sync, `t3-code-records-337`): merged into `feat(example)/t3-code` as #311 (`96c4c38f2`); the record moved to `tasks/closed/`. Its three real-input rows stay in `STATUS.md` "Next real-input batch".

None for this task: the coordinator checks conflicts and moves the PR to ready. Three rows wait in STATUS "Next real-input
batch" (sections A, B and C above): the hover card and clicks by real input, the Code tab's tips, and the Act on focus.
