---
name: 20261005-pr-links-previews-and-routing
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: daehyeon/t3-code
branch: null
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
| merged task PR | [20261005-environment-routes](20261005-environment-routes.md) | pending | Merged (saved-environment key; `gitHubRoutingConnectionKey` over several routes) | pending |
| merged task PR | [20261005-pr-conversation-and-refresh](20261005-pr-conversation-and-refresh.md) | pending | Merged | pending |
| merged task PR | [20261005-pr-handoffs-and-quick-actions](20261005-pr-handoffs-and-quick-actions.md) | pending | Merged (row menus, Check out menu and the hand-offs that "Act on" redirects; it already follows `20261005-pr-header-actions-and-stacks`, whose More menu also carries the radio) | pending |
| merged task PR | [20261005-fake-github-fixture](20261005-fake-github-fixture.md) | pending | Search cursors and a second-lane account served (also reached through `20261005-pr-conversation-and-refresh`) | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |

## Issue assessment at preparation

Checked sources and time: planning pass 2026-10-05 over `EXACT2-GAPS.md` and `../issues/` drafts (not reproduced, not searched upstream). Re-check at `prepare`.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X13](../issues/20261005-x13-hover-keys-during-pan.md) | Hover close/cancel during a pan | Documented clone limit | nonblocking (workaround: partial, r12) | Declare in matrix |
| [X17](../issues/20261005-x17-popover-position-try.md) | Hover card flips near window edges | AGENT-HANDOFF "flipped hover card overhang" | nonblocking (workaround: fixed placement) | Declare |
| [X19](../issues/20261005-x19-data-source-timers.md) | 350 ms open / 120 ms close hover delays; 10 s linked-thread poll | Delays held in Contract/`now` args today | nonblocking (workaround: `now` arguments, Contract tasks) | Reuse the details-card hover card's delay mechanism |
| [X34](../issues/20261005-x34-inline-span-frame.md) | Frame and hover of an inline link inside rendered Markdown, to anchor the card | `t3-anchor`/`t3-frame` hooks cover boxes ([X22](../issues/20261005-x22-reactive-layout-facts.md)), not inline runs | unknown | Spike at `prepare`; if the renderer cannot expose a link's frame, the hover card on inline links is held for a user decision (no matching workaround); autolinks still link |
| [X21](../issues/20261005-x21-two-way-websocket.md) | RPC to background environments | `T3Fleet.swift` transports | nonblocking | Reuse |
| [X9](../issues/20261005-x09-root-component-across-files.md) | `app.contract` cap | 1,327/1,500 | nonblocking | New files only |

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
| Routing | Two fixture backends A (focused) and B (background), the same fake account, separate `FAKE_GH_LOG`; GitHub sharing set A/B to off, read, read-write combinations | Open a PR; comment; Check out with "Act on" = B | Reads and writes land in the log of the allowed server only; with one side "read" a write stays on the origin; identity mismatch is refused before dispatch; Check out creates B's worktree | macOS | two logs, `git worktree list` |
| Paging | Fake gh with 130 PRs across 2 repositories | Scroll to the end; press "Load more pull requests" until the cap; fail one page | Request `cursors` equal the previous `nextCursors`; rows append in place; states and texts as listed | macOS | trace, shots |
| Visual and trace | Oracle on the same fixtures | Pairs at 1280×840 and 840×620, light and dark: count button, picker, card, list footer; `target/t3-ui-parity/trace-diff.mjs pr-links`, `pr-routing`, `pr-list` | Every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link; read multisets and write order equal | macOS | pair table, diff |
| Ported tests | `bun test` | Original names: "one server per repository", "where a pull request can be acted on", "merging the environments' own listings", "which environments a listing should ask", "who \"I\" am, per server", "the server a saved selection names", "linked pull request thread navigation", "does not probe another environment with %s routing permission", "keeps hover previews fresh after edits and turns", SSH-profile routing cases | Pass; Effect-runtime-only cases classified in the header | macOS | log |
| Keyboard focus, Escape, reduced motion | `prefer prefers-reduced-motion reduce` | Tab/Return through the count button, menu item, picker, "Load more pull requests"; Escape closes the picker and the hover card | Focus visible and returned to the trigger; nothing is linked on Escape; the card appears without fade when reduced | macOS | `tree --ax`, state |
| Gates | `git add -A` | Clone checks; `bun scripts/caps.mjs`; five repository checks | Green; every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `examples/t3-code/pages-prs.*`, `pages-pr-detail.*`, new `pages-pr-links.*`, `pages-pr-routing.ts`, `palette-linkpr.ts`, `settings-b-fleet.ts` (request helper), `r4-surfaces-prs.ts`, `AGENT-HANDOFF.md`.
Required environment: macOS 26.6.2, Xcode 27.0, Bun 1.4.2, two lane backends, fake gh, reference oracle. Attended and normal-launch rows use a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>` (see `20261005-embedded-server-runtime`).

## Progress

Planned. No branch.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

`prepare` after `20261005-environment-routes`, `20261005-pr-conversation-and-refresh` and `20261005-pr-handoffs-and-quick-actions` merge; run the inline-link frame spike first.
