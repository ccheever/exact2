---
name: 20261005-pr-links-previews-and-routing
plan: 20261005-t3code-macos-parity
implementation: built
verification: partial
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-pr-links-previews-and-routing
pr_url: null
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

Parent specification: [spec](../spec.md). Source behavior (T3 Code `1e2ecbd975`; `W/` = `apps/web/src/components/pullRequest/`):
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
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| merged task PR | [20261005-environment-routes](closed/20261005-environment-routes.md) | pending | Merged (saved-environment key; `gitHubRoutingConnectionKey` over several routes) | pending |
| merged task PR | [20261005-pr-conversation-and-refresh](closed/20261005-pr-conversation-and-refresh.md) | pending | Merged | pending |
| merged task PR | [20261005-pr-handoffs-and-quick-actions](closed/20261005-pr-handoffs-and-quick-actions.md) | pending | Merged (row menus, Check out menu and the hand-offs that "Act on" redirects; it already follows `20261005-pr-header-actions-and-stacks`, whose More menu also carries the radio) | pending |
| merged task PR | [20261007-real-github-lane](closed/20261007-real-github-lane.md) | pending | Sandbox's bulk pull requests page the list (probe `R3`/`R4`); a second lane server per account | pending |
| merged task PR | [20261005-hot-file-split](closed/20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| in-flight PR | fix-hover-cards (window hover layer, `hover-layer.contract`) | [#307](https://github.com/ccheever/exact2/pull/307) | Merged into `feat(example)/t3-code`; the card is drawn by its layer as kind `"pr-preview"` (coordinator 2026-10-08: no second hover helper) | not merged at this PR's draft; three edits ready (`target/pr-links/hover-patch.py`: PrLinkRun `inject hoverTipAt`, `hoverDelay("pr-preview") = 120`, T3Window layer branch) |
| framework issue | X17 popover side areas and flips | [#112](https://github.com/ccheever/exact2/issues/112) | The side the card opens on (PreviewCardPopup side top, align center, sideOffset 6) | the hover layer's own flip (#307) places it until `position-try` lands |
| task (in flight) | [20261005-pr-code-tab](20261005-pr-code-tab.md) | pending | Not needed: a commit autolink opens on the host, as the reference's (it is not a change request) | the hook for a later Code-tab route is `pages-pr-links.ts openLink` (commit links have no target there) |

## Issue assessment at preparation

Checked sources and time: planning pass 2026-10-05 over `EXACT2-GAPS.md` and `../issues/` drafts (not reproduced, not searched upstream). Re-check at `prepare`.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X13](../issues/closed/20261005-x13-hover-keys-during-pan.md) | Hover close/cancel during a pan | Documented clone limit | nonblocking (workaround: partial, r12) | Declare in matrix |
| [X17](../issues/20261005-x17-popover-position-try.md) | Hover card flips near window edges | AGENT-HANDOFF "flipped hover card overhang" | nonblocking (workaround: fixed placement) | Declare |
| [X19](../issues/20261005-x19-data-source-timers.md) | 350 ms open / 120 ms close hover delays; 10 s linked-thread poll | Delays held in Contract/`now` args today | nonblocking (workaround: `now` arguments, Contract tasks) | Reuse the details-card hover card's delay mechanism |
| [X34](../issues/20261005-x34-inline-span-frame.md) | Frame and hover of an inline link inside rendered Markdown, to anchor the card | `t3-anchor`/`t3-frame` hooks cover boxes ([X22](../issues/20261005-x22-reactive-layout-facts.md)), not inline runs | unknown | Spike at `prepare`; if the renderer cannot expose a link's frame, the hover card on inline links is held for a user decision (no matching workaround); autolinks still link 2026-10-07: #133 closed; main #178 makes the macOS agent hover inline runs (enter, leave, a point's hit test), so an inline link's hover can be built and driven; `frame()` of an inline run is still missing, so the card's anchor remains the open question (adopt-main-fixes-input). |
| [X21](../issues/20261005-x21-two-way-websocket.md) | RPC to background environments | `T3Fleet.swift` transports | nonblocking | Reuse |
| [X9](../issues/20261005-x09-root-component-across-files.md) | `app.contract` cap | 1,478/1,500 on the base and after this task (0 net root lines: +2 for `pr-select:`, −2 by folding `prClose` into `prSelect("")`) | nonblocking | New files only; in-place edits of the root's existing lines |

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
  and view are built (`chatlocal:pr-preview`, `prDetail.preview`, `PrLinkPreviewCard`); its placement waits on #307.
- Root: 0 net lines (see X9). Fork-ported logic and tests: `pages-pr-routing.*`, `pages-pr-links-logic.*`.

### Acceptance results

Evidence: `https://raw.githubusercontent.com/ccheever/exact2/0d0e299947b680711e74cb4ca22cddeff8597849/pr-links-previews-and-routing/<file>`.

| Criterion | Result | Proof | Blocker |
| --- | --- | --- | --- |
| Linked threads | pass (live + unit) | 02 (count 2), 06 (palette: Linked / Archived thread), "Old work" opened from it (record), 10 s tick: `liveNow` 83000 → 93000 under `clock +10000` (record), `pages-pr-links.test.ts` "on the page the count names the threads and reads again only when the 10 s tick moves" | — |
| Link / unlink | pass (live + unit) | 07 (picker), 08 (count 3 after choosing "Write the changelog"; `pullRequests.linkedThreads` read back with three threads), unit: link, unlink beside a linked thread, the draft's picker, the refusal toast | — |
| Autolinks | pass (live + unit) | 05 (before/after: `#115`, `c46352e` links; `` `#2` ``, `a#3` text), 09 (`#115` clicked → #115 selected), `pages-pr-links-logic.test.ts` "pull request autolinks" | the commit link should be monospace: the after shot shows it proportional; check in the retry (not a pixel loop) |
| Hover card | not run live | data and view built; unit "pointing at a link makes the panel read its card; leaving it lets go", "the hovered link's card reads the detail once, and a failure leaves the URL tooltip" | waiting on fix-hover-cards #307 (three edits ready); real pointer in the real-input batch |
| Routing | pass (unit/integration), partly live | `pages-pr-routing.test.ts` (93, ported), `pages-pr-environments.test.ts` (remote origin read through B with `host`/`expectedAccountId`; sharing off either side probes nothing; one side "read" keeps a write on A; identity mismatch refused; a background row's project goes to its server); live: sharing set to Read and act on both (12), both lane servers are loopback so the origin answers first as in the reference; Check out with Act on = B created B's worktree only (11, `git worktree list` in the record) | — |
| Paging | pass (unit), partly live | 01 (Load more offered at 99 rows with `nextCursors`; the base said "Narrow your search"), `pages-pr-paging.test.ts` (cursors equal the last `nextCursors`, rows appended, spinner first, growth to 500 then "Narrow your search…", a failed page keeps the rows with the banner, a refresh after a continuation reads 198 rows) | the live press of Load more ran with a search still applied (drive step error); it runs in the retry |
| Visual and trace | not run (oracle) | before/after pairs 01–05; dark and 840×620 shots taken (count, More, picker, Check out with Act on) | user decision 2026-10-06 (no oracle or trace tools) |
| Ported tests | pass | routing 93 and links 43 (fork-ported, original names; Effect-runtime-only cases classified in the test headers) | — |
| Keyboard focus, Escape, reduced motion | pass (live, agent keys) | Return on the count opened the palette; Escape closed it and the picker (nothing linked: count stayed 2); focus returns to the trigger (`paletteClose` focuses `pull-request-linked-threads` / `pull-request-more`); the card has no fade under reduced motion (`still`) | the card's reduced-motion and the menus' arrow keys: #307 and fix-keyboard-focus |
| Gates | pass | `bun test examples/t3-code` 3303/0; strict tsc clean; contract build OK (3959 slots); `cargo test -p t3-code-macos --lib` 13/0; five checks green (cargo test 3521 passed, 0 failed, 34 ignored); caps and boot OK | — |

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (agent live, the one session) | `a3d15d561` + records merge `732f0e3f3` | every after step but two passed on real GitHub with two lane servers; `load-more` ran with the search still applied and found no button; `thread-surface` could not find `pr-row-115` (the search had not settled) | `live-drive-record.txt`, images 01–12 | the retry: hover card after #307, Load more, the back arrow |

## Real-input batch steps

1. Lane: `bun target/pr-links/lanes.mjs start a` (127.0.0.1:16360; `T3_GITHUB_LANE_SHARED` = the base checkout's lane dir, primary
   account). The autolink fixture comment on #132 stays posted (`target/pr-links/comment.json`; delete with `comment.mjs delete` after).
2. App: copy this branch's `T3 Code (Exact).app` to `T3 Code (Lane PRL).app`, bundle id `com.exact.t3code.laneprl`, re-sign ad hoc; launch by
   path with `CFFIXED_USER_HOME=<worktree>/target/pr-links/realinput/home`, `T3_LOCAL_HOME=<…>/realinput/t3-home`, `T3_LOCAL_PORT=16362`,
   `T3CODE_TELEMETRY_ENABLED=false`; pair to A with `lanes.mjs pair a`'s URL (put in with `set-value`).
3. Hover card (after #307 and the three edits): Pull Requests › #132 › Summary › Comments. Move the pointer onto `#115`; read back after ~0.5 s with
   `screencapture -x -o -l <window>`: the card above the link (repository, `#115` · Open, the title, author, "opened …"). Onto the card: it stays.
   Away: gone within ~0.2 s. Escape over it closes it.
4. Click: a plain click on `#115` selects #115. ⌘-click on `https://github.com/daehyeonmun2021/playground/pull/116`: the panel does not move and Chrome
   opens the URL (front tab read back, then closed). The commit `c46352e`: Chrome opens its commit page.
5. Cleanup: quit the copy, `lanes.mjs stop a`, delete the copy's Keychain item (`com.exact.t3code.macos.access-token`, account `<origin>\n<environment
   id>`) and its preferences, release the lock.

## Next action

Waiting on fix-hover-cards (#307). Once it merges into `feat(example)/t3-code`: merge it here, apply `target/pr-links/hover-patch.py` (three edits), rebuild,
and run the retry session: the hover card (agent hover, then the real-input steps above), "Load more pull requests" pressed with the search cleared, and the
back arrow beside "Write the changelog" (#132 and #115 linked). Then the coordinator moves the PR to ready.
